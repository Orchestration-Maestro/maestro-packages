//! Baseline checks through the two public page renderers.
use crate::evidence::{Entry, LedgerDoc};
use crate::report::{RunSpend, Verdict};
use crate::webui::{render, render_evidence};

/// Baseline: report data is escaped in text, tables and attack-graph branches.
#[test]
fn baseline_report_escapes_untrusted_data_in_every_section() {
    let mut report = sample_assessment();
    let hostile = "<script>\"&";
    report.target = hostile.into();
    report.agent = hostile.into();
    report.model = hostile.into();
    report.run_id = hostile.into();
    report.summary = hostile.into();
    report.findings[0].title = hostile.into();
    report.findings[0].severity = hostile.into();
    report.findings[0].impact = hostile.into();
    report.findings[0].evidence.command = hostile.into();
    report.refuted[0].hypothesis = hostile.into();
    report.refuted[0].evidence_id = hostile.into();
    report.threats[0].asset = hostile.into();
    report.threats[0].path = hostile.into();
    report.threats[0].impact = hostile.into();
    report.threats[0].hardening = hostile.into();
    report.unknown[0] = hostile.into();
    let page = render(&report);
    assert_eq!(page.run_id, hostile);
    assert_eq!(page.page, "report");
    assert!(!page.html.contains(hostile));
    assert!(page.html.contains("&lt;script&gt;&quot;&amp;"));
    assert!(!page.html.contains("<script>"));
}

/// Baseline: verdicts and spend remain visible, including rejected graph nodes.
#[test]
fn baseline_report_displays_verdicts_and_spend() {
    let mut report = sample_assessment();
    report.spend = Some(RunSpend {
        llm_calls: 2,
        tokens_out: 321,
        duration_secs: 7,
    });
    for (verdict, badge) in [
        (Verdict::Confirmed, "b-confirmed"),
        (Verdict::NeedsValidation, "b-needs"),
        (Verdict::Rejected, "b-rejected"),
    ] {
        report.findings[0].verdict = verdict;
        let html = render(&report).html;
        assert!(html.contains(&format!("class='badge {badge}'")));
        assert!(html.contains("0.80"));
        assert!(html.contains("321 tokens out"));
        assert!(html.contains("2 calls"));
        if matches!(verdict, Verdict::Rejected) {
            assert!(html.contains("class='gnode dead'>SQLi in login"));
        }
    }
}

/// Baseline: empty report collections render without invented findings or spend.
#[test]
fn baseline_report_handles_empty_sections() {
    let mut report = sample_assessment();
    report.findings.clear();
    report.refuted.clear();
    report.threats.clear();
    report.unknown.clear();
    let html = render(&report).html;
    assert!(!html.contains("<tbody>"));
    assert!(!html.contains("class='gnode "));
    assert!(!html.contains("tokens out"));
}

/// A ledger entry whose optional command and prompt control the displayed note.
fn entry(id: &str, kind: &str, command: Option<&str>, prompt: Option<&str>) -> Entry {
    Entry {
        id: id.into(),
        phase: "<recon>".into(),
        kind: kind.into(),
        command: command.map(str::to_string),
        prompt: prompt.map(str::to_string),
        output: "é".repeat(125),
        model: Some("<model>".into()),
        tokens_out: None,
        exit_code: None,
    }
}

/// Baseline: ledger notes prefer command then prompt, with a Unicode-safe fallback.
#[test]
fn baseline_evidence_escapes_notes_and_counts_only_matching_entry_kinds() {
    let doc = LedgerDoc {
        schema: crate::evidence::SCHEMA.into(),
        run_id: "<run>".into(),
        target: "<target>".into(),
        agent: "fixture".into(),
        model: "fixture".into(),
        entries: vec![
            entry("ev-1", "exec", Some("<command>"), Some("hidden prompt")),
            entry("ev-2", "llm", None, Some("<prompt>")),
            entry("ev-3", "note", None, None),
        ],
    };
    let page = render_evidence(&doc).expect("evidence page");
    assert_eq!(page.run_id, "<run>");
    assert_eq!(page.page, "evidence");
    for raw in [
        "<run>",
        "<target>",
        "<command>",
        "<prompt>",
        "<model>",
        "<recon>",
        "hidden prompt",
    ] {
        assert!(!page.html.contains(raw), "unescaped or wrong note: {raw}");
    }
    assert!(page.html.contains("&lt;command&gt;"));
    assert!(page.html.contains("&lt;prompt&gt;"));
    assert!(page.html.contains(&format!(">{}<", "é".repeat(120))));
    assert!(page.html.contains("1 llm calls"));
    assert!(page.html.contains("1 executions"));
}

/// Baseline: a retained ledger with no steps renders no rows.
#[test]
fn baseline_evidence_handles_an_empty_ledger() {
    let doc = LedgerDoc {
        schema: crate::evidence::SCHEMA.into(),
        run_id: "r".into(),
        target: "t".into(),
        agent: "a".into(),
        model: "m".into(),
        entries: Vec::new(),
    };
    assert!(
        !render_evidence(&doc)
            .expect("empty page")
            .html
            .contains("<tbody>")
    );
}

/// An assessment with one node in every graph branch.
fn sample_assessment() -> crate::report::Assessment {
    use crate::report::{Assessment, Evidence, Finding, Refuted, Threat, Verdict};
    Assessment {
        schema: crate::report::SCHEMA.into(),
        run_id: "r".into(),
        agent: "a".into(),
        target: "example.com".into(),
        model: "m".into(),
        phases: Vec::new(),
        findings: vec![Finding {
            id: "F-001".into(),
            fingerprint: "fp".into(),
            title: "SQLi in login".into(),
            class: "injection".into(),
            verdict: Verdict::Confirmed,
            severity: "high".into(),
            likelihood: String::new(),
            confidence: 0.8,
            trace: Vec::new(),
            conditions: Vec::new(),
            execution: None,
            mitre: None,
            poc: None,
            evidence_level: "CONFIRMED".into(),
            escalation: "admin".into(),
            evidence: Evidence {
                command: "curl -s /login".into(),
                output: "ok".into(),
                id: "ev-0001".into(),
            },
            impact: "db read".into(),
            fix: "params".into(),
            target: "example.com".into(),
            claimed_root_cause: None,
            blockers: Vec::new(),
            validation_plan: None,
            reason: None,
        }],
        unknown: vec!["tls config".into()],
        refuted: vec![Refuted {
            class: "xss".into(),
            hypothesis: "reflected xss".into(),
            evidence_id: "ev-0002".into(),
        }],
        threats: vec![Threat {
            asset: "db".into(),
            actor: "ext".into(),
            path: "sqli".into(),
            impact: "exfil".into(),
            likelihood: "medium".into(),
            hardening: "params".into(),
        }],
        runtime_mechanics: Vec::new(),
        spend: None,
        summary: "s".into(),
    }
}
