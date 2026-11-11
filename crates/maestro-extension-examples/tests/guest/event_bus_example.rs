//! The shipped event-bus factory loaded into the production engine.
use super::support::{Recorder, TERMINAL, harness, load, local, verdict};
use maestro_extensions::Local;
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded,
    maestro_extensions::{SessionStartEvent, Subscription},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built event-bus component.
const COMPONENT: &str = "MAESTRO_EVENT_BUS_COMPONENT";

/// The channels whose traffic the tests record.
const CHANNELS: [&str; 2] = ["my:notification", "other:event"];

/// The recorded registration and case observations.
fn corpus() -> Value {
    serde_json::from_str(include_str!("event_bus_expected.json")).unwrap()
}

/// One loaded factory, the bus traffic around it and the notifications it showed.
struct Session {
    /// The engine and the bus every component shares.
    harness: Harness,
    /// The loaded runner.
    loaded: Loaded,
    /// The notifications the listener showed.
    ui: Rc<Recorder>,
    /// Every emission any party put on the recorded channels, in order.
    emissions: Rc<RefCell<Vec<Value>>>,
    /// Keeps the recording listeners subscribed.
    _taps: Vec<Subscription<Local>>,
}

impl Session {
    /// Loads the factory with recording listeners on the bus, so load-time emissions show.
    async fn new() -> Self {
        let harness = harness();
        let emissions = Rc::new(RefCell::new(Vec::new()));
        let bus = harness.bus.for_extension();
        let taps = CHANNELS
            .into_iter()
            .map(|channel| {
                let log = emissions.clone();
                bus.on(
                    channel,
                    Rc::new(move |data, _| {
                        log.borrow_mut()
                            .push(json!({"channel": channel, "data": *data}));
                        Ok(None)
                    }),
                )
            })
            .collect();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        Self {
            harness,
            loaded,
            ui,
            emissions,
            _taps: taps,
        }
    }

    /// Delivers a session start to both callbacks, as the application does.
    async fn start(&self) {
        let event = json!({"type": "session_start", "reason": "new"});
        verdict::<SessionStartEvent>(&self.loaded.runner, &event)
            .await
            .unwrap();
    }

    /// Starts a session and forgets what its start emitted and showed, so a case observes only
    /// what follows.
    async fn begin(&self) {
        self.start().await;
        self.emissions.borrow_mut().clear();
        self.ui.take();
    }

    /// Runs the emit command with `args`.
    async fn command(&self, args: &str) {
        let command = self.loaded.runner.get_command("emit").unwrap().command;
        (command.handler)(args.into(), self.loaded.runner.create_command_context())
            .await
            .unwrap();
    }

    /// Puts `data` on `channel` as another party does.
    async fn send(&self, channel: &str, data: &Value) {
        self.harness
            .bus
            .for_extension()
            .emit(channel, data.clone())
            .await;
    }

    /// The notifications shown since the last call, as message and severity.
    fn notified(&self) -> Vec<Value> {
        self.ui
            .take()
            .iter()
            .map(|request| json!({"message": request[1][0], "kind": request[1][1].as_str().map(str::to_lowercase)}))
            .collect()
    }
}

/// Performs the operation a recorded case names.
async fn perform(session: &Session, row: &Value) {
    let (name, input) = (&row["name"], &row["input"]);
    match row["operation"].as_str().unwrap() {
        "command" => {
            session.begin().await;
            session.command(input.as_str().unwrap()).await;
        }
        "before_start" => {
            session.command(input.as_str().unwrap()).await;
            assert!(
                session.notified().is_empty(),
                "{name}: reported before the session started"
            );
            session.start().await;
            session.command(input.as_str().unwrap()).await;
        }
        "external" => external(session, name, input).await,
        _ => {
            for _ in input.as_array().unwrap() {
                session.start().await;
            }
            session
                .send(
                    "my:notification",
                    &json!({"message": "later", "from": "peer"}),
                )
                .await;
        }
    }
}

/// Delivers a peer payload, first before any session started when the case says so.
async fn external(session: &Session, name: &Value, input: &Value) {
    let channel = input["channel"].as_str().unwrap();
    if input["before"] == true {
        session.send(channel, &input["data"]).await;
        assert!(
            session.notified().is_empty(),
            "{name}: reported before the session started"
        );
        session.start().await;
    } else {
        session.begin().await;
    }
    session.send(channel, &input["data"]).await;
}

/// Runs one recorded case and checks the emissions and notifications it produces.
async fn run(row: &Value) {
    let (name, session) = (&row["name"], Session::new().await);
    assert!(
        session.emissions.borrow().is_empty(),
        "{name}: the factory emitted while it loaded"
    );
    perform(&session, row).await;
    let expected = &row["expected"];
    assert_eq!(
        json!(session.emissions.borrow().clone()),
        expected["emissions"],
        "{name}: emissions"
    );
    let owned: Vec<Value> = expected["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|notice| json!({"message": notice["message"], "kind": notice["kind"]}))
        .collect();
    assert_eq!(session.notified(), owned, "{name}: notifications");
}

/// Replays the recorded cases `keep` selects.
fn replay(keep: impl Fn(&Value) -> bool) {
    local(async {
        let corpus = corpus();
        let rows: Vec<&Value> = corpus["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| keep(row))
            .collect();
        assert!(!rows.is_empty());
        for row in rows {
            run(row).await;
        }
    });
}

/// Whether a case delivers a peer payload before any session started.
fn external_before_start(row: &Value) -> bool {
    row["operation"] == "external" && row["input"]["before"] == true
}

/// The factory registers the recorded listener and command and emits nothing while loading.
#[test]
fn event_bus_registers_channels_and_command() {
    local(async {
        let session = Session::new().await;
        let recorded = &corpus()["metadata"];
        let commands = session.loaded.runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        let command = &commands[0].command;
        assert_eq!(json!(command.name), recorded["command"][0]["name"]);
        assert_eq!(
            json!(command.description),
            recorded["command"][0]["description"]
        );
        assert!(session.loaded.runner.has_handlers("session_start"));
        assert!(
            session.emissions.borrow().is_empty(),
            "the factory emitted while it loaded"
        );
    });
}

/// The command trims its message and defaults an empty one.
#[test]
fn event_bus_command_preserves_message_and_default() {
    replay(|row| row["operation"] == "command");
}

/// A listener that fires before any session started shows nothing until one starts.
#[test]
fn event_bus_listener_waits_for_session_context() {
    replay(|row| external_before_start(row) || row["operation"] == "before_start");
}

/// Each session start replaces the context the listener reports through.
#[test]
fn event_bus_session_start_replaces_current_context() {
    replay(|row| row["operation"] == "lifecycle");
}

/// A payload from another party keeps its literal fields, extra members and channel.
#[test]
fn event_bus_foreign_delivery_keeps_literal_fields() {
    replay(|row| row["operation"] == "external" && !external_before_start(row));
}
