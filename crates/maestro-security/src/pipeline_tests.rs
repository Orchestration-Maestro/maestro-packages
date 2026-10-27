//! Behaviour checks through engine runs with isolated bridge and runner fixtures.

use std::collections::VecDeque;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

use serde_json::{Value, json};

use crate::pipeline::PhaseKind;
use crate::{Agent, Assessment, Config, Engine, RunOptions, Verdict};

/// A local bridge and runner, retaining requests for boundary assertions.
struct Fixture {
    /// Bound URL shared by both clients.
    url: String,
    /// Requests observed in arrival order.
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    /// Stop signal so a failed assertion cannot strand the server.
    stop: Arc<AtomicBool>,
    /// Serving thread, joined when the fixture drops.
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl Fixture {
    /// Serve model replies in order and deterministic runner output.
    fn new(replies: Vec<Value>) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let url = format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&requests);
        let done = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            let mut replies: VecDeque<_> = replies.into();
            while !done.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve(stream, &seen, &mut replies)?,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(())
        });
        Ok(Self {
            url,
            requests,
            stop,
            worker: Some(worker),
        })
    }

    /// Point both engine clients at this fixture, never machine services.
    fn engine(&self) -> Engine {
        Engine::new(Config {
            bridge: self.url.clone(),
            runner: self.url.clone(),
            bridge_key: "fixture".into(),
            max_hypotheses: 4,
            default_model: "preferred".into(),
        })
    }

    /// Copy captured requests after a synchronous run completes.
    fn requests(&self) -> Result<Vec<(String, Value)>, String> {
        self.requests
            .lock()
            .map(|r| r.clone())
            .map_err(|e| e.to_string())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            assert!(matches!(worker.join(), Ok(Ok(()))), "fixture server failed");
        }
    }
}

/// Read one complete HTTP request, including its JSON body.
fn request(stream: &mut TcpStream) -> Result<(String, Value), String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut reader = std::io::BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let path = line
        .split_whitespace()
        .nth(1)
        .ok_or("missing request path")?
        .to_string();
    let mut length = 0;
    loop {
        line.clear();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().map_err(|e| e.to_string())?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    let value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).map_err(|e| e.to_string())?
    };
    Ok((path, value))
}

/// Answer one bridge or runner request without invoking external services.
fn serve(
    mut stream: TcpStream,
    seen: &Mutex<Vec<(String, Value)>>,
    replies: &mut VecDeque<Value>,
) -> Result<(), String> {
    let (path, body) = request(&mut stream)?;
    seen.lock()
        .map_err(|e| e.to_string())?
        .push((path.clone(), body));
    let response = match path.as_str() {
        "/health" => "{}".into(),
        "/models" => json!({"data":[{"id":"first"},{"id":"preferred"}]}).to_string(),
        "/exec" => "{\"type\":\"out\",\"data\":\"probe evidence\\n\"}\n{\"type\":\"exit\",\"code\":0}\n".into(),
        "/chat/completions" => json!({
            "choices":[{"message":{"content":replies.pop_front().unwrap_or(Value::Null).to_string()}}],
            "usage":{"completion_tokens":7}
        }).to_string(),
        _ => return Err(format!("unexpected request: {path}")),
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
        response.len()
    )
    .map_err(|e| e.to_string())
}

/// Run within a fresh state directory and remove only that directory afterward.
fn isolated(body: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let home = std::env::temp_dir().join(crate::pipeline::new_run_id("coverage"));
    let result = crate::evidence::with_home(&home, body);
    if home.exists() {
        std::fs::remove_dir_all(home).map_err(|e| e.to_string())?;
    }
    result
}

/// A role selecting only the phases relevant to a behaviour.
fn agent(phases: &'static [PhaseKind]) -> Agent {
    Agent {
        name: "fixture",
        description: "fixture",
        role: "Assess evidence",
        phases,
    }
}

/// Run selected phases with a caller-chosen id and optional model resolution.
fn run(fixture: &Fixture, phases: &'static [PhaseKind]) -> Result<Assessment, String> {
    fixture.engine().run_identified(
        &agent(phases),
        "example.test",
        "coverage-run",
        &mut RunOptions::default(),
    )
}

/// Model data for one hypothesis, with no inference delegated to the fixture.
fn hypothesis() -> Value {
    json!({"class":"configuration","claim":"exposed admin","command":"probe"})
}

/// Replies for one verification and an optional deepening response.
fn verification_replies(level: &str) -> Vec<Value> {
    vec![
        json!({"hypotheses":[hypothesis()]}),
        json!({"command":"probe","refutation_command":"counterprobe"}),
        json!({"evidence_level":level,"reasoning":"observed admin","unknown":["authentication"],"counterevidence":"alternate input"}),
        json!({"escalation":"admin access","impact":"read accounts","fix":"require auth","severity":"high","confidence":1.7,"trace":["entry","sink"],"conditions":["network"],"likelihood":"high","mitre":"T1078","execution":{"attacker_perspective":"remote","payloads":["probe"],"instructions":"probe","observed_result":"admin"}}),
    ]
}

/// Indirect evidence remains a candidate, never a deepened confirmed finding.
#[test]
fn likely_findings_are_not_deepened() -> Result<(), String> {
    isolated(|| {
        let fixture = Fixture::new(verification_replies("LIKELY"))?;
        let report = run(
            &fixture,
            &[PhaseKind::Hypotheses, PhaseKind::Verify, PhaseKind::Deepen],
        )?;
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].verdict, Verdict::NeedsValidation);
        assert!(
            report.findings[0].escalation.is_empty(),
            "indirect evidence was deepened"
        );
        let chats = fixture
            .requests()?
            .iter()
            .filter(|(p, _)| p == "/chat/completions")
            .count();
        assert_eq!(chats, 3, "LIKELY must not request deepening");
        Ok(())
    })
}

/// Recon passes target data literally to tools, never through shell expansion.
#[test]
fn recon_target_is_one_literal_shell_argument() -> Result<(), String> {
    isolated(|| {
        let fixture = Fixture::new(vec![])?;
        let target = "https://example.test/a'b?x=$(printf injected)&y=2";
        fixture.engine().run_identified(
            &agent(&[PhaseKind::Recon]),
            target,
            "literal-target",
            &mut RunOptions::default(),
        )?;
        for (_, body) in fixture.requests()?.iter().filter(|(p, _)| p == "/exec") {
            let command = body["command"].as_str().ok_or("no runner command")?;
            let shell = format!(
                "nmap() {{ printf '[%s]\\n' \"$@\"; }}; curl() {{ nmap \"$@\"; }}; httpx() {{ nmap \"$@\"; }}; {command}"
            );
            let output = std::process::Command::new("sh")
                .args(["-c", &shell])
                .output()
                .map_err(|e| e.to_string())?;
            assert!(
                output.status.success(),
                "recon command is not valid shell: {command}"
            );
            assert!(
                String::from_utf8_lossy(&output.stdout).contains(&format!("[{target}]")),
                "target was interpreted by shell: {command}"
            );
        }
        Ok(())
    })
}

/// Confirmed findings carry execution evidence, active refutation and deepening.
#[test]
fn baseline_confirmed_finding_retains_evidence_and_deepening() -> Result<(), String> {
    isolated(|| {
        let fixture = Fixture::new(verification_replies("CONFIRMED"))?;
        let report = run(
            &fixture,
            &[PhaseKind::Hypotheses, PhaseKind::Verify, PhaseKind::Deepen],
        )?;
        let finding = report.findings.first().ok_or("missing finding")?;
        assert_eq!(finding.verdict, Verdict::Confirmed);
        assert_eq!(finding.evidence.command, "probe");
        assert_eq!(finding.evidence.output, "probe evidence\n");
        assert_eq!(finding.impact, "read accounts");
        assert_eq!(finding.fix, "require auth");
        assert_eq!(finding.confidence, 1.0);
        assert_eq!(finding.trace, ["entry", "sink"]);
        assert_eq!(finding.conditions, ["network"]);
        assert_eq!(finding.mitre.as_deref(), Some("T1078"));
        assert!(finding.execution.is_some());
        let ledger: Value = serde_json::from_str(&crate::evidence_for("coverage-run")?)
            .map_err(|e| e.to_string())?;
        let entries = ledger["entries"].as_array().ok_or("no entries")?;
        assert!(entries.iter().any(|e| e["id"] == finding.evidence.id));
        assert!(entries.iter().any(|e| e["phase"] == "counterevidence"));
        assert_eq!(
            crate::load_report("coverage-run")?.findings[0].fix,
            "require auth"
        );
        Ok(())
    })
}

/// Refuted and possible hypotheses never become findings or trigger refutation.
#[test]
fn baseline_unproven_hypotheses_stay_out_of_findings() -> Result<(), String> {
    for level in ["REFUTED", "POSSIBLE"] {
        isolated(|| {
            let fixture = Fixture::new(verification_replies(level))?;
            let report = run(
                &fixture,
                &[PhaseKind::Hypotheses, PhaseKind::Verify, PhaseKind::Deepen],
            )?;
            assert!(report.findings.is_empty());
            assert_eq!(report.refuted.len(), 1);
            assert_eq!(report.refuted[0].hypothesis, "exposed admin");
            assert_eq!(report.unknown, ["authentication"]);
            assert_eq!(
                fixture
                    .requests()?
                    .iter()
                    .filter(|(p, _)| p == "/exec")
                    .count(),
                1
            );
            Ok(())
        })?;
    }
    Ok(())
}

/// A full run orders phases, checkpoints evidence and uses the preferred model.
#[test]
fn baseline_full_run_checkpoints_and_retains_report() -> Result<(), String> {
    isolated(|| {
        let fixture = Fixture::new(vec![
            json!({"services":[]}),
            json!({"threats":[{"asset":"accounts","actor":"remote","path":"admin","impact":"read","likelihood":"high","hardening":"require auth"}]}),
            json!({"summary":"No demonstrated findings.","runtime_mechanics":["observed redirect"]}),
        ])?;
        let mut checkpoints = Vec::new();
        let mut checkpoint = |ledger: &crate::Ledger, target: &str, role: &str| {
            checkpoints.push((
                ledger
                    .phases()
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>(),
                target.to_string(),
                role.to_string(),
            ));
        };
        let mut opts = RunOptions {
            checkpoint: Some(&mut checkpoint),
            ..RunOptions::default()
        };
        let report = fixture.engine().run_checkpointed(
            crate::resolve_agent(Some("maestro.cyber")),
            "https://example.test",
            &mut opts,
        )?;
        assert_eq!(report.model, "preferred");
        assert_eq!(report.summary, "No demonstrated findings.");
        assert_eq!(report.runtime_mechanics, ["observed redirect"]);
        assert_eq!(report.threats.len(), 1);
        assert_eq!(report.threats[0].hardening, "require auth");
        assert_eq!(checkpoints.len(), 4);
        assert!(
            checkpoints.iter().all(
                |(_, target, role)| target == "https://example.test" && role == "maestro.cyber"
            )
        );
        assert_eq!(checkpoints[0].0, ["recon", "recon", "recon"]);
        assert_eq!(crate::load_report(&report.run_id)?.summary, report.summary);
        assert!(crate::evidence_for(&report.run_id)?.contains("probe evidence"));
        Ok(())
    })
}

/// Malformed model records are discarded and the configured hypothesis cap wins.
#[test]
fn baseline_hypotheses_filter_invalid_entries_and_enforce_limit() -> Result<(), String> {
    isolated(|| {
        let mut hypotheses = vec![
            json!({"claim":"missing class"}),
            json!({"class":"x","claim":"blank","command":"  "}),
        ];
        hypotheses.extend((0..8).map(|_| hypothesis()));
        let mut replies = vec![json!({"hypotheses":hypotheses})];
        for _ in 0..4 {
            replies.extend([
                json!({"command":"probe"}),
                json!({"evidence_level":"REFUTED"}),
            ]);
        }
        let fixture = Fixture::new(replies)?;
        let report = run(&fixture, &[PhaseKind::Hypotheses, PhaseKind::Verify])?;
        assert_eq!(report.refuted.len(), 4);
        assert!(report.findings.is_empty());
        Ok(())
    })
}

/// Bad model documents degrade to empty results and a deterministic report.
#[test]
fn baseline_malformed_optional_phases_preserve_report() -> Result<(), String> {
    isolated(|| {
        let fixture = Fixture::new(vec![
            json!("not hypotheses"),
            json!({"threats":[{"asset":"incomplete"}]}),
            json!({"summary":7,"runtime_mechanics":[3,"redirect"]}),
        ])?;
        let report = run(
            &fixture,
            &[PhaseKind::Hypotheses, PhaseKind::Threat, PhaseKind::Report],
        )?;
        assert!(report.findings.is_empty());
        assert!(report.threats.is_empty());
        assert!(report.summary.contains("0 findings, 0 hypotheses refuted"));
        assert_eq!(report.runtime_mechanics, ["redirect"]);
        Ok(())
    })
}

/// Blank targets fail at the assessment boundary without contacting services.
#[test]
fn baseline_blank_target_is_rejected() {
    let result = crate::assess(&crate::AssessParams {
        target: " \n\t".into(),
        ..crate::AssessParams::default()
    });
    assert!(matches!(result, Err(error) if error == "target is required"));
}
