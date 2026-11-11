//! The shipped compaction-trigger example loaded into the production engine.
use super::support::{Failures, Recorder, bind, fail_with, harness, load, local, verdict};
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded,
    maestro_extensions::{CompactionResult, ContextUsage, TurnEndEvent},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built compaction-trigger component.
const COMPONENT: &str = "MAESTRO_TRIGGER_COMPACT_COMPONENT";

/// The compaction result the host reports through a completion callback.
const COMPLETION: &str = r#"{"summary":"summary","firstKeptEntryId":"kept","tokensBefore":1}"#;

/// The recorded rows of one group, which a group selection must find at least one of.
fn rows(group: &str) -> Vec<Value> {
    let corpus: Value =
        serde_json::from_str(include_str!("trigger_compact_expected.json")).unwrap();
    let selected: Vec<Value> = corpus["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == group)
        .cloned()
        .collect();
    assert!(!selected.is_empty(), "recorded rows of {group}");
    selected
}

/// A compaction request the host admitted, with the callbacks it holds for the outcome.
struct Admitted {
    /// Reports the finished compaction.
    complete: Rc<dyn Fn(CompactionResult) -> Result<(), maestro_extensions::EventError>>,
    /// Reports the failure.
    error: Rc<dyn Fn(maestro_extensions::EventError) -> Result<(), maestro_extensions::EventError>>,
}

/// Serves the context usage and the compaction admission of one activation.
fn serve(
    loaded: &Loaded,
    ui: &Rc<Recorder>,
    usage: &Rc<RefCell<Option<ContextUsage>>>,
    admitted: &Rc<RefCell<Vec<Admitted>>>,
    refusal: &Rc<RefCell<Option<String>>>,
) {
    bind(&loaded.runner, |_, context| {
        let reading = usage.clone();
        context.get_context_usage = Rc::new(move || Ok(*reading.borrow()));
        let (log, held, refuse) = (ui.clone(), admitted.clone(), refusal.clone());
        context.compact = Rc::new(move |options| {
            fail_with(&refuse)?;
            let options = options.unwrap_or_default();
            log.record(
                "compact",
                json!({
                    "instructions": options.custom_instructions,
                    "complete": options.on_complete.is_some(),
                    "error": options.on_error.is_some(),
                }),
            )?;
            held.borrow_mut().push(Admitted {
                complete: options.on_complete.unwrap(),
                error: options.on_error.unwrap(),
            });
            Ok(())
        });
    });
}

/// One activation of the factory, with the host operations it reads and the requests it makes.
struct Activation {
    /// Keeps the engine alive.
    _harness: Harness,
    /// The loaded runner.
    loaded: Loaded,
    /// The usage the host reports.
    usage: Rc<RefCell<Option<ContextUsage>>>,
    /// The requests the host admitted, in order.
    admitted: Rc<RefCell<Vec<Admitted>>>,
    /// The failures the runner reported for turns.
    failures: Failures,
}

/// The activations of one replay and the effects they share.
struct Replay {
    /// The factory activations, in start order.
    activations: Vec<Activation>,
    /// The notifications and admissions every activation records, in order.
    ui: Rc<Recorder>,
    /// The failure the next admission returns.
    refusal: Rc<RefCell<Option<String>>>,
}

impl Replay {
    /// Starts `count` activations that share one effect log.
    async fn start(count: usize, ui_available: bool) -> Self {
        let ui = Recorder::detached(None);
        let refusal = Rc::new(RefCell::new(None::<String>));
        let mut activations = Vec::new();
        for _ in 0..count {
            let harness = harness();
            let loaded = load(&harness, COMPONENT).await;
            ui.connect(&loaded.runner, ui_available);
            let usage = Rc::new(RefCell::new(None));
            let admitted = Rc::new(RefCell::new(Vec::new()));
            serve(&loaded, &ui, &usage, &admitted, &refusal);
            let failures = Failures::collect(&loaded.runner);
            activations.push(Activation {
                _harness: harness,
                loaded,
                usage,
                admitted,
                failures,
            });
        }
        Self {
            activations,
            ui,
            refusal,
        }
    }

    /// Delivers one turn with the supplied estimate.
    async fn turn(&self, instance: usize, step: &Value) -> Result<(), String> {
        let active = &self.activations[instance];
        let context_window = 200_000.0;
        *active.usage.borrow_mut() = match step["usage"].as_str() {
            Some("absent" | "null") => None,
            _ => Some(ContextUsage {
                tokens: step["tokens"].as_f64(),
                context_window,
                percent: None,
            }),
        };
        let event = json!({"type":"turn_end","turnIndex":0,"message":{"role":"assistant","content":[],
            "api":"controlled","provider":"controlled","model":"controlled",
            "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
            "stopReason":"stop","timestamp":0},"toolResults":[]});
        verdict::<TurnEndEvent>(&active.loaded.runner, &event).await?;
        active
            .failures
            .take()
            .into_iter()
            .next()
            .map_or(Ok(()), Err)
    }

    /// Runs the command of one activation.
    async fn command(&self, instance: usize, args: &str) -> Result<(), String> {
        let runner = &self.activations[instance].loaded.runner;
        let command = runner.get_command("trigger-compact").unwrap().command;
        (command.handler)(args.into(), runner.create_command_context())
            .await
            .map_err(|error| error.to_string())
    }

    /// Runs one recorded step and returns its outcome in the recorded form.
    async fn step(&self, step: &Value) -> Value {
        let instance = usize::try_from(step["instance"].as_u64().unwrap_or(0)).unwrap();
        let request = usize::try_from(step["request"].as_u64().unwrap_or(0)).unwrap();
        let outcome = match step["op"].as_str().unwrap() {
            "turn" => self.turn(instance, step).await,
            "command" => {
                self.command(instance, step["args"].as_str().unwrap_or_default())
                    .await
            }
            "ui" => {
                let on = step["value"].as_bool().unwrap();
                self.activations
                    .iter()
                    .for_each(|active| self.ui.connect(&active.loaded.runner, on));
                Ok(())
            }
            "reject" => {
                let message = step["message"].as_str().unwrap();
                match step["method"].as_str().unwrap() {
                    "compact" => *self.refusal.borrow_mut() = Some(message.to_owned()),
                    method => self.ui.reject(method, message),
                }
                Ok(())
            }
            "complete" => {
                let complete = self.activations[instance].admitted.borrow()[request]
                    .complete
                    .clone();
                complete(serde_json::from_str(COMPLETION).unwrap())
                    .map_err(|error| error.to_string())
            }
            _ => {
                let error = self.activations[instance].admitted.borrow()[request]
                    .error
                    .clone();
                error(step["message"].as_str().unwrap().into()).map_err(|error| error.to_string())
            }
        };
        match outcome {
            Ok(()) => json!({"ok": null}),
            Err(error) => json!({"error": error}),
        }
    }
}

/// The request a recorded effect is, in the log's spelling.
fn request(effect: &Value) -> Value {
    if let Some(notice) = effect.get("notify") {
        let kind = match notice["kind"].as_str().unwrap() {
            "info" => "Info",
            "error" => "Error",
            other => panic!("unexpected severity {other}"),
        };
        json!(["notify", [notice["message"], kind]])
    } else {
        json!(["compact", effect["compact"]])
    }
}

/// The command and hook the factory registers once per activation, with the recorded metadata.
fn assert_registrations(replay: &Replay, row: &Value) {
    let name = &row["name"];
    for active in &replay.activations {
        assert!(
            active.loaded.runner.has_handlers("turn_end"),
            "{name}: hook"
        );
        let commands = active.loaded.runner.get_registered_commands();
        let recorded: Vec<&Value> = row["registrations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry.get("command").is_some())
            .take(1)
            .collect();
        assert_eq!(commands.len(), recorded.len(), "{name}: commands");
        assert_eq!(
            json!(commands[0].command.name),
            recorded[0]["command"],
            "{name}"
        );
        assert_eq!(
            json!(commands[0].command.description),
            recorded[0]["description"],
            "{name}"
        );
    }
}

/// Replays one recorded row and asserts every observation it records.
async fn replay(row: &Value) {
    let name = &row["name"];
    let events = row["registrations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry.get("event").is_some())
        .count();
    let state = Replay::start(events, row["initialUi"].as_bool().unwrap()).await;
    assert_registrations(&state, row);
    let effects: Vec<Value> = row["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(request)
        .collect();
    let mut seen = Vec::new();
    for (index, step) in row["steps"].as_array().unwrap().iter().enumerate() {
        let outcome = state.step(step).await;
        assert_eq!(outcome, row["outcomes"][index], "{name}: step {index}");
        seen.extend(state.ui.take());
        let reached = usize::try_from(row["checkpoints"][index].as_u64().unwrap()).unwrap();
        assert_eq!(
            seen,
            effects[..reached],
            "{name}: effects after step {index}"
        );
    }
    assert_eq!(seen, effects, "{name}: effects");
}

/// Replays every row of `group`.
fn replay_group(group: &str) {
    local(async {
        for row in rows(group) {
            replay(&row).await;
        }
    });
}

/// The factory registers one turn hook and one command with the recorded description.
#[test]
fn trigger_compact_registers_hook_and_command() {
    local(async {
        let state = Replay::start(1, false).await;
        let row = &rows("commands")[0];
        assert_registrations(&state, row);
        assert!(
            state.ui.take().is_empty(),
            "the factory reaches nothing while starting"
        );
    });
}

/// Compaction starts when the estimate crosses its limit, not while it stays across.
#[test]
fn trigger_compact_crossings_keep_previous_usage() {
    replay_group("crossings");
}

/// Unknown usage of every form keeps the last estimate and never starts a compaction.
#[test]
fn trigger_compact_unknown_usage_keeps_last_estimate() {
    replay_group("unknown");
}

/// The command passes its instructions and the crossing state through to the host.
#[test]
fn trigger_compact_command_preserves_instructions_and_crossing_state() {
    replay_group("commands");
}

/// Notifications follow the UI that is current when the outcome arrives.
#[test]
fn trigger_compact_notifications_follow_current_ui() {
    replay_group("notifications");
}

/// Refused host operations surface to the caller and keep the earlier requests.
#[test]
fn trigger_compact_host_rejections_preserve_prior_requests() {
    replay_group("rejections");
}

/// Two activations of the factory keep separate usage.
#[test]
fn trigger_compact_factory_instances_keep_separate_usage() {
    replay_group("instances");
}
