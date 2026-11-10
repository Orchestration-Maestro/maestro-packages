//! The shipped Space Invaders command loaded into the production engine, driven through the
//! terminal input its view reads.
use super::support::{plain_rows, Bindings, Failures, Instance, Log, Recorder, replace_branch, stepped};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

/// The variable that names the built game component.
const COMPONENT: &str = "MAESTRO_SPACE_INVADERS_COMPONENT";

/// Frames and saved records captured from the authored game.
fn corpus() -> Value {
    serde_json::from_str(include_str!("space_invaders_frames.json")).unwrap()
}

/// The recorded frame named `name`.
fn frame(name: &str) -> Vec<String> {
    serde_json::from_value(corpus()["frames"][name].clone()).unwrap()
}

/// The recorded script named `name`.
fn script(name: &str) -> Value {
    corpus()["scripts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()
        .clone()
}

/// A custom save entry, including an explicit null discard.
fn entry(value: &Value) -> Value {
    json!({"customType":"space-invaders-save","data":value})
}

/// The loaded game, the saves it wrote and the failures it reported.
struct Game {
    /// The loaded component.
    instance: Instance,
    /// The custom entries the game saved.
    saves: Log,
    /// The failures the callbacks reported.
    failures: Failures,
    /// The number of next saves the application refuses.
    refusals: Rc<Cell<usize>>,
}

impl Game {
    /// Loads the game; the application refuses saves while `refusals` is above zero.
    async fn new(refusals: usize) -> Self {
        let instance = Instance::new(COMPONENT).await;
        let mut bindings = Bindings::new();
        let saves = bindings.record_entries(instance.harness.session_manager.clone());
        let recording = bindings.actions.append_entry.clone();
        let refusals = Rc::new(Cell::new(refusals));
        let left = refusals.clone();
        bindings.actions.append_entry = Rc::new(move |custom_type, data| {
            if left.get() > 0 {
                left.set(left.get() - 1);
                return Err("session append refused".into());
            }
            recording(custom_type, data)
        });
        bindings.apply(instance.runner());
        let failures = Failures::collect(instance.runner());
        Self {
            instance,
            saves,
            failures,
            refusals,
        }
    }

    /// The runner over the game.
    fn runner(&self) -> &Rc<ExtensionRunner<Local>> {
        self.instance.runner()
    }

    /// Replaces the session's history with `saves`.
    fn history(&self, saves: &[Value]) {
        let entries: Vec<Value> = saves.iter().map(entry).collect();
        replace_branch(&self.instance.harness.session_manager, &entries);
    }

    /// The values the game saved, oldest first.
    fn saved(&self) -> Vec<Value> {
        self.saves
            .borrow()
            .iter()
            .map(|save| save["data"].clone())
            .collect()
    }

    /// Starts the command on a terminal of `columns` and waits for its view.
    async fn open(
        &self,
        columns: usize,
    ) -> (Rc<Recorder>, tokio::task::JoinHandle<Result<(), String>>) {
        let ui = Recorder::install_on(self.runner(), (columns, 40));
        let runner = self.runner().clone();
        let command = tokio::task::spawn_local(async move {
            let command = runner.get_command("invaders").unwrap().command;
            (command.handler)("ignored".into(), runner.create_command_context())
                .await
                .map_err(|error| error.to_string())
        });
        ui.shown(1).await;
        (ui, command)
    }
}

/// The game registers one command and takes the interactive terminal only when there is one.
#[test]
fn space_invaders_component_runs_registered_command_and_restores_latest_save() {
    stepped(async {
        let corpus = corpus();
        let game = Game::new(0).await;
        let commands = game.runner().get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "invaders");
        assert_eq!(
            json!(commands[0].command.description),
            "Play Space Invaders!"
        );
        let command = game.runner().get_command("invaders").unwrap().command;
        (command.handler)("ignored".into(), game.runner().create_command_context())
            .await
            .unwrap();
        assert!(game.saved().is_empty(), "no terminal, no view and no save");
        let paused = script("resume")["start"].clone();
        let mut saved = Vec::new();
        for (history, shown, key) in [
            (vec![Value::Null, paused.clone()], "paused", "q"),
            (vec![paused.clone(), Value::Null], "initial", "\x1b"),
            (vec![], "initial", "q"),
        ] {
            let game = Game::new(0).await;
            game.history(&history);
            let (ui, view) = game.open(80).await;
            assert_eq!(ui.rows(0, 80), plain_rows(&frame(shown)), "{shown}");
            ui.press(0, key);
            view.await.unwrap().unwrap();
            assert!(game.failures.take().is_empty());
            saved.extend(game.saved());
        }
        assert_eq!(saved, [corpus["cases"][0]["start"].clone(), Value::Null]);
    });
}

/// Every styled row fits the supplied terminal, even when no game cell is visible.
#[test]
fn space_invaders_rows_fit_narrow_terminal_widths() {
    stepped(async {
        let corpus = corpus();
        for case in corpus["narrow_frames"].as_array().unwrap() {
            let columns = usize::try_from(case["columns"].as_u64().unwrap()).unwrap();
            let game = Game::new(0).await;
            game.history(&[script("resume")["start"].clone()]);
            let (ui, view) = game.open(columns.max(1)).await;
            let rows = ui.component(0).try_render(columns).unwrap();
            let expected: Vec<String> = serde_json::from_value(case["rows"].clone()).unwrap();
            assert_eq!(rows.len(), expected.len(), "{columns} columns");
            for row in &rows {
                let visible = maestro_app::strip_ansi(row).unwrap().chars().count();
                assert!(
                    visible <= columns,
                    "{columns} columns: {visible} cells in {row:?}"
                );
            }
            ui.press(0, "q");
            view.await.unwrap().unwrap();
            assert!(game.saved().is_empty());
        }
    });
}

/// A refused save on `rejected` leaves the view open; `retry` then saves or discards.
async fn rejects_then_retries(rejected: &str, retry: &str) {
    let corpus = corpus();
    let game = Game::new(1).await;
    game.history(&[]);
    let (ui, view) = game.open(80).await;
    assert_eq!(
        ui.try_press(0, rejected),
        Err("session append refused".into())
    );
    assert_eq!(game.refusals.get(), 0);
    assert!(
        !view.is_finished(),
        "the view stays open after a refused save"
    );
    assert_eq!(ui.rows(0, 80), plain_rows(&frame("initial")));
    ui.press(0, retry);
    view.await.unwrap().unwrap();
    let saved = if retry == "\x1b" {
        corpus["cases"][0]["start"].clone()
    } else {
        Value::Null
    };
    assert_eq!(game.saved(), [saved]);
}

/// Failed saves leave both retrying and discarding available on the retained view.
#[test]
fn space_invaders_rejected_save_keeps_input_usable() {
    stepped(async {
        for rejected in ["\x1b", "q"] {
            for retry in ["\x1b", "q"] {
                rejects_then_retries(rejected, retry).await;
            }
        }
    });
}
