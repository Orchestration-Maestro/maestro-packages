//! The unified security methodology: one fixed pipeline, fused into a
//! single Maestro method.
//!
//! Fusions: a fixed phase shape, a breadth-first hypothesis sweep, a
//! Plan-Act-Observe-Reflect verification loop, strict evidence levels,
//! counterevidence discipline, a verification gate, Maestro's evidence
//! knowledge (every claim cites the command that produced it;
//! looked-for-not-proven is named, not guessed; observed mechanics are
//! separated from behaviour), trust-boundary definition and execution
//! safety, standard hardening fix guidance, deletion-first severity
//! judgement.

/// System prompt for every phase call. The model is a runtime value; this
/// prompt is the only constant.
pub const SYSTEM_PROMPT: &str = "You are the unified security engine of the \
maestro-security extension. One pipeline, fixed order, no skipping, no \
reordering.

PHASE 1 RECON: raw data only, collected by the pipeline itself. No \
interpretation.
PHASE 2 TARGET MODEL: structured model of the target built only from recon \
evidence: services (port/protocol/version), tech stack, endpoints, \
identities, data flows. Every node cites the recon line that proved it. A \
node without evidence does not exist.
PHASE 3 HYPOTHESES: breadth-first sweep across vulnerability classes: \
authentication, injection, SSRF, XSS, CSRF, IDOR, path traversal, \
deserialization, cryptography, configuration, supply chain, denial of \
service, information disclosure. Each hypothesis names its class, its target \
node from phase 2, one concrete test command, the output that would prove \
it, and the output that would refute it. Many cheap tests before any deep \
dive. When the target is a host or container environment, the sweep also \
covers the environment itself: image and container configuration, exposed \
services and their credentials, installed tooling, agent frameworks and \
skill code, and local data stores.
PHASE 4 VERIFY: Plan-Act-Observe-Reflect per hypothesis, at most three \
attempts. Evidence levels are strict:
  CONFIRMED: direct observation of the vulnerable behaviour, reproducible \
  from the recorded command and output.
  LIKELY: strong indirect evidence, not directly reproducible.
  POSSIBLE: circumstantial only.
  REFUTED: tested, behaviour absent.
Counterevidence discipline: for every CONFIRMED result, actively try once to \
refute it (re-run, alternate input, check false-positive conditions). If the \
refutation succeeds, downgrade the level.
PHASE 5 DEEPEN: CONFIRMED findings only. One escalation step, one concrete \
impact statement (which data or action is affected), one minimal fix drawn \
from standard hardening patterns.
PHASE 6 REPORT: the exact JSON schema the pipeline supplies. Every finding \
carries id, title, class, severity, evidence level, evidence (command plus \
output excerpt plus evidence id), impact, fix, and target. Refuted \
hypotheses are listed separately to prove coverage.

EVIDENCE DISCIPLINE (every phase):
- Findings are numbered F-001, F-002, ... in report order.
- Distinguish artifact identity (what the file, binary or service is) from \
behaviour (what it does).
- Every claim cites the command that produced it, the exact output line, and \
the evidence id the pipeline assigned. A claim without a command is a guess, \
not a finding.
- Looked-for-not-proven is named in the report's unknown list with what is \
missing, never filled with a guess.
- Separate behaviour (what the target does) from mechanics (how it does it \
internally). Observed mechanics that should not be imitated go to the \
report's runtime_mechanics list.
- When a tool fails, diagnose the blocker (wrong path, missing dependency, \
permission) before retrying. Never change the target to make a test pass.

EXECUTION SAFETY (every command you generate):
- Recon and verification are non-destructive: read-only probes, no data \
modification, no service restarts.
- Commands that write, exfiltrate or modify are labelled destructive and \
proposed, never assumed to have run.
- A hung probe is REFUTED-by-timeout, not a finding.

STYLE: terse. Facts over adjectives. One line per fact. No filler.";

/// Deterministic recon commands for a target: (command, timeout seconds).
pub fn recon_commands(target: &str) -> Vec<(String, u32)> {
    let mut cmds = vec![
        (format!("nmap -Pn -sV --top-ports 100 {target}"), 600),
        (format!("curl -sS -m 15 -D - -o /dev/null {target}"), 120),
    ];
    if target.starts_with("http://") || target.starts_with("https://") {
        cmds.push((
            format!("httpx -u {target} -tech-detect -status-code -title"),
            120,
        ));
    }
    cmds
}

/// Phase 2: turn raw recon into the target model document.
pub fn target_model_prompt(target: &str, recon: &str) -> String {
    let schema = "{\"services\":[{\"port\":0,\"protocol\":\"\",\"version\":\"\",\
\"evidence\":\"recon line\"}],\"tech\":[{\"name\":\"\",\"evidence\":\"recon line\"}],\
\"endpoints\":[{\"url\":\"\",\"evidence\":\"recon line\"}],\
\"identities\":[{\"name\":\"\",\"kind\":\"\",\"evidence\":\"recon line\"}],\
\"notes\":\"\"}";
    format!(
        "Target: {target}\n\nRAW RECON (pipeline-collected):\n{recon}\n\n\
Build the phase-2 TARGET MODEL now as JSON: {schema}.\n\
Every evidence field must quote a recon line verbatim. Omit any node you \
cannot evidence."
    )
}

/// Phase 3: the breadth-first hypothesis sweep.
pub fn hypothesis_prompt(target_model: &str, max: usize) -> String {
    let schema = "{\"hypotheses\":[{\"class\":\"\",\"claim\":\"\",\
\"command\":\"one shell command\",\"expected_if_true\":\"\",\
\"refuted_if\":\"\"}]}";
    format!(
        "TARGET MODEL (phase 2):\n{target_model}\n\n\
Run the phase-3 HYPOTHESES sweep now. Return JSON: {schema}.\n\
At most {max} hypotheses. Prefer breadth: distinct classes over repeated \
attempts on one class. Every command must be non-destructive and runnable \
in the analysis sandbox shell with nmap, httpx, nuclei, sqlmap, gobuster, ffuf, nikto, \
curl, python3, jq available."
    )
}

/// Phase 4, per hypothesis: the verification plan.
pub fn verify_prompt(hypothesis: &str, target_model: &str) -> String {
    let schema = "{\"plan\":\"one sentence\",\"command\":\"one shell command\",\
\"expected_if_true\":\"\",\"refuted_if\":\"\",\
\"refutation_command\":\"a second command that would disprove a positive \
result\"}";
    format!(
        "TARGET MODEL (phase 2):\n{target_model}\n\nHYPOTHESIS (phase 3):\n\
{hypothesis}\n\nPhase 4 VERIFY, attempt 1. Return JSON: {schema}."
    )
}

/// Phase 4, per hypothesis: the judge call after an execution.
pub fn judge_prompt(hypothesis: &str, command: &str, output: &str) -> String {
    let schema = "{\"evidence_level\":\"CONFIRMED|LIKELY|POSSIBLE|REFUTED\",\
\"reasoning\":\"one or two sentences\",\
\"unknown\":[\"looked for but not proven; what is missing\"],\
\"counterevidence\":\"what would disprove this, and whether it was tried\"}";
    format!(
        "HYPOTHESIS (phase 3):\n{hypothesis}\n\nCOMMAND RUN BY PIPELINE:\n\
{command}\n\nRAW OUTPUT (evidence):\n{output}\n\nJudge strictly. Return \
JSON: {schema}."
    )
}

/// Phase 5: deepen one confirmed finding.
pub fn deepen_prompt(finding: &str) -> String {
    let schema = "{\"escalation\":\"one step further this enables\",\
\"impact\":\"concrete data or action affected\",\
\"fix\":\"minimal standard hardening fix\",\
\"severity\":\"critical|high|medium|low|info\"}";
    format!(
        "CONFIRMED FINDING (phase 4):\n{finding}\n\nPhase 5 DEEPEN. Return \
JSON: {schema}."
    )
}

/// Threat phase (maestro.cyber): the threat model from the target model.
pub fn threat_prompt(target_model: &str) -> String {
    let schema = "{\"threats\":[{\"asset\":\"\",\"actor\":\"\",\
\"path\":\"\",\"impact\":\"\",\"likelihood\":\"low|medium|high\",\
\"hardening\":\"standard named control\"}]}";
    format!(
        "TARGET MODEL (phase 2):\n{target_model}\n\n\
Build the THREAT MODEL now. Return JSON: {schema}.\n\
Rank by impact times likelihood. Every hardening is a standard, named \
control (for example: enforce TLS 1.2, disable the feature, least-privilege \
account, parameterised queries), not a bespoke invention."
    )
}

/// Phase 6: the final report assembly. `report_data` is the JSON document
/// `{target, target_model, findings, refuted}` built by the pipeline.
pub fn report_prompt(report_data: &str) -> String {
    let schema = "{\"summary\":\"three sentences max\",\
\"unknown\":[\"looked for, not proven\"],\
\"runtime_mechanics\":[\"observed mechanics, not to be imitated\"],\
\"findings\":[{\"id\":\"F-001\",\"title\":\"\",\"class\":\"\",\
\"severity\":\"critical|high|medium|low|info\",\
\"evidence_level\":\"CONFIRMED|LIKELY\",\
\"evidence\":{\"command\":\"\",\"output\":\"\",\"evidence_id\":\"\"},\
\"impact\":\"\",\"fix\":\"\",\"target\":\"\"}],\
\"refuted\":[{\"class\":\"\",\"hypothesis\":\"\",\"evidence_id\":\"\"}]}";
    format!(
        "FINAL REPORT DATA:\n{report_data}\n\n\
Phase 6 REPORT. Return the final report JSON exactly: {schema}.\n\
Severity follows deletion-first judgement: if the fix is a one-line config \
change, it is at most medium unless the impact is critical data. Every \
evidence_id must come from the pipeline-assigned ids above."
    )
}
