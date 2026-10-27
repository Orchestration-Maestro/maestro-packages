//! The pipeline's phases: recon through the runner, every model phase
//! through the bridge, each step ledgered with a sequential evidence id.

use std::fmt::Write;

use crate::evidence::{Ledger, LlmCall};
use crate::methodology;
use crate::pipeline::{
    Deepen, Hypothesis, Judge, ModelCtx, Plan, RunState, Verification, default_summary_for,
    extract_json, shallow_finding, truncate,
};
use crate::report::{Finding, Refuted, Threat};

/// Output ceiling for every model call. The bridge serves local models, so
/// the ceiling only bounds one request's window, not a budget: thinking
/// models get room to reason and still answer.
const MAX_TOKENS: u32 = 32768;

impl crate::pipeline::Engine {
    /// Phase 1: deterministic recon through the runner, raw and ledgered.
    pub(crate) fn phase_recon(&self, target: &str, ledger: &mut Ledger) -> Result<String, String> {
        let mut recon = String::new();
        for (command, timeout) in methodology::recon_commands(target) {
            let result = self.runner.exec(&command, timeout)?;
            let id = ledger.record_exec("recon", &command, &result.output, result.exit_code);
            let _ = write!(
                recon,
                "\n=== {command} [{id}] exit={} ===\n{}\n",
                result.exit_code,
                truncate(&result.output, 8000)
            );
        }
        Ok(recon)
    }

    /// Phase 2: raw recon into the structured target model.
    pub(crate) fn phase_target_model(
        &self,
        ctx: &ModelCtx,
        target: &str,
        recon: &str,
        ledger: &mut Ledger,
    ) -> Result<String, String> {
        let prompt = methodology::target_model_prompt(target, truncate(recon, 20000));
        let reply = self
            .llm
            .chat(&ctx.model, &ctx.system, &prompt, MAX_TOKENS)?;
        let doc = reply.content;
        let id = ledger.record_llm(
            "target-model",
            "structured target model from recon",
            &LlmCall {
                prompt,
                output: doc.clone(),
                model: ctx.model.clone(),
                tokens_out: reply.tokens_out,
            },
        );
        Ok(format!("{doc}\n[evidence {id}]"))
    }

    /// Phase 3: the breadth-first hypothesis sweep.
    pub(crate) fn phase_hypotheses(
        &self,
        ctx: &ModelCtx,
        target_model: &str,
        ledger: &mut Ledger,
    ) -> Vec<Hypothesis> {
        let prompt = methodology::hypothesis_prompt(
            truncate(target_model, 12000),
            self.config.max_hypotheses,
        );
        let reply = match self.llm.chat(&ctx.model, &ctx.system, &prompt, MAX_TOKENS) {
            Ok(reply) => reply,
            Err(error) => {
                ledger.record_note("hypotheses", &format!("failed: {error}"));
                return Vec::new();
            }
        };
        let raw = reply.content;
        let value: serde_json::Value =
            serde_json::from_str(&extract_json(&raw)).unwrap_or_default();
        let arr = value
            .get("hypotheses")
            .and_then(|h| h.as_array())
            .cloned()
            .unwrap_or_default();
        let hypotheses: Vec<Hypothesis> = arr
            .into_iter()
            .filter_map(|h| serde_json::from_value::<Hypothesis>(h).ok())
            .filter(|h| !h.command.trim().is_empty())
            .take(self.config.max_hypotheses)
            .collect();
        ledger.record_llm(
            "hypotheses",
            &format!("{} hypotheses selected", hypotheses.len()),
            &LlmCall {
                prompt,
                output: raw,
                model: ctx.model.clone(),
                tokens_out: reply.tokens_out,
            },
        );
        hypotheses
    }

    /// Phase 4: verify every hypothesis, then split findings from refuted.
    pub(crate) fn phase_verify(&self, ctx: &ModelCtx, state: &mut RunState) {
        let target_model = state.target_model.clone();
        let hypotheses = std::mem::take(&mut state.hypotheses);
        for hypothesis in &hypotheses {
            if let Some(verification) =
                self.verify_one(ctx, &target_model, hypothesis, &mut state.ledger)
            {
                state.verifications.push(verification);
            }
        }
        let verifications = std::mem::take(&mut state.verifications);
        for verification in &verifications {
            if verification.level == "REFUTED" || verification.level == "POSSIBLE" {
                state.refuted.push(Refuted {
                    class: verification.hypothesis.class.clone(),
                    hypothesis: verification.hypothesis.claim.clone(),
                    evidence_id: verification.evidence_id.clone(),
                });
            } else {
                let f_id = state.next_f_id();
                state
                    .findings
                    .push(shallow_finding(&state.target, verification, f_id));
            }
        }
        state.verifications = verifications;
    }

    /// One hypothesis through plan, act, observe, reflect, counterevidence.
    pub(crate) fn verify_one(
        &self,
        ctx: &ModelCtx,
        target_model: &str,
        hyp: &Hypothesis,
        ledger: &mut Ledger,
    ) -> Option<Verification> {
        let hyp_json = serde_json::to_string_pretty(hyp).unwrap_or_default();
        let plan_raw = self
            .llm
            .chat(
                &ctx.model,
                &ctx.system,
                &methodology::verify_prompt(&hyp_json, truncate(target_model, 8000)),
                MAX_TOKENS,
            )
            .ok()?
            .content;
        let plan: Plan = serde_json::from_str(&extract_json(&plan_raw)).ok()?;
        let result = self.runner.exec(&plan.command, 300).ok()?;
        let id = ledger.record_exec("verify", &plan.command, &result.output, result.exit_code);
        let judge_raw = self
            .llm
            .chat(
                &ctx.model,
                &ctx.system,
                &methodology::judge_prompt(
                    &hyp_json,
                    &plan.command,
                    truncate(&result.output, 8000),
                ),
                MAX_TOKENS,
            )
            .ok()?
            .content;
        let judge: Judge = serde_json::from_str(&extract_json(&judge_raw)).ok()?;
        let executed = self.counterevidence(ctx, &plan, &judge, ledger);
        let counterevidence = if executed.is_empty() {
            judge.counterevidence
        } else if judge.counterevidence.is_empty() {
            executed
        } else {
            format!("{}\n{executed}", judge.counterevidence)
        };
        Some(Verification {
            hypothesis: hyp.clone(),
            command: plan.command,
            output: truncate(&result.output, 8000).to_string(),
            evidence_id: id,
            level: judge.evidence_level,
            reasoning: judge.reasoning,
            counterevidence,
            unknown: judge.unknown,
        })
    }

    /// Counterevidence discipline: a CONFIRMED result gets one active
    /// refutation try.
    pub(crate) fn counterevidence(
        &self,
        ctx: &ModelCtx,
        plan: &Plan,
        judge: &Judge,
        ledger: &mut Ledger,
    ) -> String {
        if judge.evidence_level != "CONFIRMED" || plan.refutation_command.trim().is_empty() {
            return String::new();
        }
        match self.runner.exec(&plan.refutation_command, 120) {
            Ok(result) => {
                let id = ledger.record_exec(
                    "counterevidence",
                    &plan.refutation_command,
                    &result.output,
                    result.exit_code,
                );
                format!(
                    "[{id}] exit={}\n{}\n[model: {}]",
                    result.exit_code,
                    truncate(&result.output, 4000),
                    ctx.model
                )
            }
            Err(error) => format!("refutation command failed: {error}"),
        }
    }

    /// Phase 5: deepen each verified finding in place.
    pub(crate) fn phase_deepen(&self, ctx: &ModelCtx, state: &mut RunState) {
        let verifications = std::mem::take(&mut state.verifications);
        for verification in &verifications {
            if verification.level == "REFUTED" || verification.level == "POSSIBLE" {
                continue;
            }
            let Some(finding) = state
                .findings
                .iter_mut()
                .find(|f| f.evidence.id == verification.evidence_id)
            else {
                continue;
            };
            if finding.escalation.is_empty()
                && let Some(deep) = self.deepen(ctx, verification, &mut state.ledger)
            {
                apply_deepen(finding, deep);
            }
        }
        state.verifications = verifications;
    }

    /// Phase 5: one deepening call per verified finding.
    pub(crate) fn deepen(
        &self,
        ctx: &ModelCtx,
        v: &Verification,
        ledger: &mut Ledger,
    ) -> Option<Deepen> {
        let payload = serde_json::json!({
            "hypothesis": v.hypothesis.claim,
            "class": v.hypothesis.class,
            "command": v.command,
            "output": v.output,
            "evidence_id": v.evidence_id,
            "judgement": v.reasoning,
            "counterevidence": v.counterevidence,
        })
        .to_string();
        let reply = self
            .llm
            .chat(
                &ctx.model,
                &ctx.system,
                &methodology::deepen_prompt(&payload),
                MAX_TOKENS,
            )
            .ok()?;
        let raw = reply.content;
        let deep: Deepen = serde_json::from_str(&extract_json(&raw)).ok()?;
        ledger.record_llm(
            "deepen",
            &v.hypothesis.claim,
            &LlmCall {
                prompt: payload,
                output: raw,
                model: ctx.model.clone(),
                tokens_out: reply.tokens_out,
            },
        );
        Some(deep)
    }

    /// Threat phase: the threat model from the target model (maestro.cyber).
    pub(crate) fn phase_threat(
        &self,
        ctx: &ModelCtx,
        target_model: &str,
        ledger: &mut Ledger,
    ) -> Vec<Threat> {
        let prompt = methodology::threat_prompt(truncate(target_model, 12000));
        let reply = match self.llm.chat(&ctx.model, &ctx.system, &prompt, MAX_TOKENS) {
            Ok(reply) => reply,
            Err(error) => {
                ledger.record_note("threat", &format!("failed: {error}"));
                return Vec::new();
            }
        };
        let raw = reply.content;
        let value: serde_json::Value =
            serde_json::from_str(&extract_json(&raw)).unwrap_or_default();
        let arr = value
            .get("threats")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();
        let threats: Vec<Threat> = arr
            .into_iter()
            .filter_map(|t| serde_json::from_value(t).ok())
            .collect();
        ledger.record_llm(
            "threat",
            &format!("{} threats modelled", threats.len()),
            &LlmCall {
                prompt,
                output: raw,
                model: ctx.model.clone(),
                tokens_out: reply.tokens_out,
            },
        );
        threats
    }

    /// Phase 6: the three-sentence summary plus runtime mechanics.
    pub(crate) fn phase_summary(
        &self,
        ctx: &ModelCtx,
        report_data: &str,
        ledger: &mut Ledger,
    ) -> (String, Vec<String>) {
        let prompt = methodology::report_prompt(truncate(report_data, 20000));
        let Ok(reply) = self.llm.chat(&ctx.model, &ctx.system, &prompt, MAX_TOKENS) else {
            return (default_summary_for(report_data), Vec::new());
        };
        let raw = reply.content;
        ledger.record_llm(
            "report",
            "final summary",
            &LlmCall {
                prompt,
                output: raw.clone(),
                model: ctx.model.clone(),
                tokens_out: reply.tokens_out,
            },
        );
        let value: serde_json::Value =
            serde_json::from_str(&extract_json(&raw)).unwrap_or_default();
        let summary = value
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| default_summary_for(report_data), str::to_string);
        let mechanics: Vec<String> = value
            .get("runtime_mechanics")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        (summary, mechanics)
    }
}

/// Copy the non-empty deepen fields onto a confirmed finding.
fn apply_deepen(finding: &mut Finding, deep: Deepen) {
    finding.severity = deep.severity;
    finding.escalation = deep.escalation;
    finding.impact = deep.impact;
    finding.fix = deep.fix;
    if !deep.likelihood.is_empty() {
        finding.likelihood = deep.likelihood;
    }
    if !deep.trace.is_empty() {
        finding.trace = deep.trace;
    }
    if !deep.conditions.is_empty() {
        finding.conditions = deep.conditions;
    }
    if deep.execution.is_some() {
        finding.execution = deep.execution;
    }
    if deep.mitre.is_some() {
        finding.mitre = deep.mitre;
    }
    if let Some(confidence) = deep.confidence {
        finding.confidence = confidence.clamp(0.0, 1.0);
    }
}
