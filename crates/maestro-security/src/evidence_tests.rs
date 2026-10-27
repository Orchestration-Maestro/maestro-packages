//! Baseline retained evidence and report behaviours.
use super::{Ledger, LedgerDoc, LlmCall};

/// Mixed execution, model and note evidence for persistence checks.
fn mixed_ledger() -> Ledger {
    let mut ledger = Ledger::default();
    ledger.record_exec("recon", "probe target", &"full output\n".repeat(2000), 7);
    ledger.record_llm(
        "verify",
        "verification",
        &LlmCall {
            prompt: "check evidence".into(),
            output: "candidate refuted".into(),
            model: "resolved".into(),
            tokens_out: 42,
        },
    );
    ledger.record_note("report", "No finding confirmed");
    ledger.record_llm(
        "report",
        "summarize",
        &LlmCall {
            prompt: "summarize evidence".into(),
            output: "summary".into(),
            model: "resolved".into(),
            tokens_out: 8,
        },
    );
    ledger
}

/// Baseline: mixed entries share sequential ids and account only for model spend.
#[test]
fn baseline_sequence_phases_and_spend() {
    let empty = Ledger::new();
    assert!(empty.phases().is_empty());
    assert_eq!(empty.spend().llm_calls, 0);
    assert_eq!(empty.spend().tokens_out, 0);
    let ledger = mixed_ledger();
    assert_eq!(ledger.spend().llm_calls, 2);
    assert_eq!(ledger.spend().tokens_out, 50);
    let phases = ledger.phases();
    assert_eq!(
        phases
            .iter()
            .map(|p| p.evidence_id.as_str())
            .collect::<Vec<_>>(),
        ["ev-0001", "ev-0002", "ev-0003", "ev-0004"]
    );
    assert_eq!(
        phases.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["recon", "verify", "report", "report"]
    );
    assert_eq!(phases[1].note, "verification");
    assert_eq!(phases[2].note, "No finding confirmed");
}

/// Baseline: retained ledgers keep run identity and untruncated command output.
#[test]
fn baseline_exec_retention() {
    crate::test_support::isolated(|_| {
        mixed_ledger()
            .save("run-one", "target", "security", "resolved")
            .unwrap();
        let retained: LedgerDoc = serde_json::from_str(&super::load("run-one").unwrap()).unwrap();
        assert_eq!(retained.run_id, "run-one");
        assert_eq!(retained.target, "target");
        assert_eq!(retained.agent, "security");
        assert_eq!(retained.model, "resolved");
        assert_eq!(retained.entries.len(), 4);
        let exec = &retained.entries[0];
        assert_eq!(exec.id, "ev-0001");
        assert_eq!(exec.output, "full output\n".repeat(2000));
        assert_eq!(exec.command.as_deref(), Some("probe target"));
        assert_eq!(exec.exit_code, Some(7));
        assert!(exec.prompt.is_none());
        assert!(super::load("other-run").is_err());
    });
}

/// Baseline: retained model calls preserve attribution and notes retain their text.
#[test]
fn baseline_llm_and_note_retention() {
    crate::test_support::isolated(|_| {
        let ledger = mixed_ledger();
        ledger
            .save("run-one", "target", "security", "resolved")
            .unwrap();
        let retained: LedgerDoc = serde_json::from_str(&super::load("run-one").unwrap()).unwrap();
        let llm = &retained.entries[1];
        assert_eq!(llm.id, ledger.phases()[1].evidence_id);
        assert_eq!(llm.phase, "verify");
        assert_eq!(llm.kind, "llm");
        assert_eq!(llm.prompt.as_deref(), Some("check evidence"));
        assert_eq!(llm.output, "candidate refuted");
        assert_eq!(llm.model.as_deref(), Some("resolved"));
        assert_eq!(llm.tokens_out, Some(42));
        assert!(llm.command.is_none());
        assert!(llm.exit_code.is_none());
        let note = &retained.entries[2];
        assert_eq!(note.id, "ev-0003");
        assert_eq!(note.kind, "note");
        assert_eq!(note.output, "No finding confirmed");
        assert!(note.prompt.is_none());
        assert!(note.tokens_out.is_none());
    });
}

/// Baseline: reports round-trip independently by run, with missing/corrupt errors.
#[test]
fn baseline_report_retention() {
    crate::test_support::isolated(|home| {
        assert!(super::load_report("run-one").is_err());
        let report = crate::test_support::assessment();
        super::save_report("run-one", &report).unwrap();
        let loaded = super::load_report("run-one").unwrap();
        assert_eq!(loaded.run_id, report.run_id);
        assert_eq!(loaded.target, report.target);
        assert_eq!(loaded.model, report.model);
        assert_eq!(loaded.summary, report.summary);
        assert!(super::load_report("other-run").is_err());
        std::fs::write(home.join("reports/run-one.json"), "{broken").unwrap();
        assert!(super::load_report("run-one").is_err());
    });
}

/// Baseline: blocked state directories return errors, not false retention success.
#[test]
fn baseline_retention_write_failures() {
    crate::test_support::isolated(|home| {
        std::fs::write(home.join("evidence"), "blocked").unwrap();
        assert!(Ledger::new().save("run", "t", "a", "m").is_err());
        std::fs::write(home.join("reports"), "blocked").unwrap();
        assert!(super::save_report("run", &crate::test_support::assessment()).is_err());
    });
}
