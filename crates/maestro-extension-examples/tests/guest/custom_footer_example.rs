//! The shipped footer toggle loaded into the production engine over a started terminal.
//!
//! Footer rows are compared without the theme's styling, at the widths the terminal supplies.
use super::support::{Metadata, Recorder, TERMINAL, drain, driven, harness, load, plain_text};
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded,
    maestro_extensions::{
        ExtensionContextActions, ExtensionExecution, Local,
        types::{ExtensionComponent, ProviderActions},
    },
    unconfigured_context_actions,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The variable that names the built footer component.
const COMPONENT: &str = "MAESTRO_CUSTOM_FOOTER_COMPONENT";

/// One assistant message with the supplied usage; cache counts must not enter the totals.
fn assistant(input: u32, output: u32, total: f64) -> Value {
    json!({"role": "assistant", "content": [], "api": "openai-responses",
        "provider": "controlled", "model": "old-model", "timestamp": 0, "stopReason": "stop",
        "usage": {"input": input, "output": output, "cacheRead": 999_999, "cacheWrite": 888_888,
            "totalTokens": 9_999_999, "cost": {"input": 7, "output": 8, "cacheRead": 9,
                "cacheWrite": 10, "total": total}}})
}

/// The model the host reports, with identity `id`.
fn model(id: &str) -> Arc<RwLock<maestro_models::Model>> {
    Arc::new(RwLock::new(
        serde_json::from_value(
            json!({"id": id, "name": "Controlled", "api": "openai-responses",
            "provider": "controlled", "baseUrl": "https://invalid.example", "reasoning": false,
            "input": ["text"], "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
            "contextWindow": 10000, "maxTokens": 1000}),
        )
        .unwrap(),
    ))
}

/// The loaded component, its terminal, footer metadata and the model the host reports.
struct Footer {
    /// The engine the component lives in.
    harness: Harness,
    /// The runner over the component.
    loaded: Loaded,
    /// The UI that holds the installed footer.
    ui: Rc<Recorder>,
    /// The branch the footer shows, and the callbacks it registered.
    metadata: Rc<Metadata>,
    /// The model identity the host reports; `None` reports no model.
    model: Rc<RefCell<Option<String>>>,
}

impl Footer {
    /// Loads the component over a session with no history.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let reported = Rc::new(RefCell::new(None::<String>));
        let read = reported.clone();
        loaded
            .runner
            .bind_core(
                Local::unbound_actions(),
                ExtensionContextActions {
                    get_model: Rc::new(move || Ok(read.borrow().as_deref().map(model))),
                    ..unconfigured_context_actions()
                },
                ProviderActions::default(),
            )
            .unwrap();
        Self {
            harness,
            loaded,
            ui,
            metadata: Rc::default(),
            model: reported,
        }
    }

    /// Runs the footer toggle command.
    async fn toggle(&self) {
        let command = self.loaded.runner.get_command("footer").unwrap().command;
        (command.handler)(
            "ignored".into(),
            self.loaded.runner.create_command_context(),
        )
        .await
        .unwrap();
    }

    /// Replaces the session's history with assistant messages of the given usage.
    fn history(&self, usages: &[(u32, u32, f64)]) {
        let mut session = self.harness.session_manager.borrow_mut();
        for (input, output, total) in usages {
            session
                .append_message(assistant(*input, *output, *total))
                .unwrap();
        }
    }

    /// The footer the host builds from the installed factory.
    fn build(&self) -> ExtensionComponent {
        self.ui.build_footer(self.metadata.clone())
    }
}

/// Renders the footer `width` cells wide as unstyled rows.
fn rows(footer: &ExtensionComponent, width: usize) -> Vec<String> {
    footer
        .component
        .try_render(width)
        .unwrap()
        .iter()
        .map(|row| plain_text(row))
        .collect()
}

/// The toggle installs one branch listener and removes it when restoring the host footer.
#[test]
fn custom_footer_component_toggles_and_disposes() {
    driven(async {
        let footer = Footer::new().await;
        let commands = footer.loaded.runner.get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "footer");
        assert_eq!(
            commands[0].command.description.as_deref(),
            Some("Toggle custom footer")
        );
        assert!(commands[0].command.get_argument_completions.is_none());
        for _ in 0..2 {
            footer.toggle().await;
            let seen = footer.ui.take();
            assert_eq!(seen[0][1], json!([true]));
            assert_eq!(
                seen[1][1],
                json!(["Custom footer enabled", "Info"])
            );
            let installed = footer.build();
            assert_eq!(footer.metadata.listeners.borrow().len(), 1);
            let screen = footer.ui.stage();
            screen.runtime.settle().unwrap();
            drain(&screen.redraws);
            let notify = footer.metadata.listeners.borrow()[0].clone();
            notify();
            screen.redraws.notified().await;
            footer.toggle().await;
            let seen = footer.ui.take();
            assert_eq!(seen[0][1], json!([false]));
            assert_eq!(
                seen[1][1],
                json!(["Default footer restored", "Info"])
            );
            (installed.dispose.unwrap()).call(()).unwrap();
            assert!(footer.metadata.listeners.borrow().is_empty());
        }
    });
}

/// The retained footer rereads history, model and branch, and clips by cell width.
#[test]
fn custom_footer_component_renders_live_history_and_clips() {
    driven(async {
        let footer = Footer::new().await;
        footer.history(&[(400, 400, 0.1), (599, 600, 0.0254)]);
        *footer.model.borrow_mut() = Some("model-雪-x".into());
        *footer.metadata.branch.borrow_mut() = Some("main".into());
        footer.toggle().await;
        let built = footer.build();
        for (row, width) in [
            ("↑999 ↓1.0k $0.125      model-雪-x (main)", 40),
            ("↑999 ↓1...", 10),
        ] {
            assert_eq!(rows(&built, width), [row], "width {width}");
        }
        footer.history(&[(1000, 1000, 0.1), (501, 999, 0.0254)]);
        *footer.metadata.branch.borrow_mut() = Some("topic".into());
        // Four assistant messages in all: 2500 input, 2999 output and 0.2508 in cost.
        assert_eq!(
            rows(&built, 40),
            ["↑2.5k ↓3.0k $0.251    model-雪-x (topic)"]
        );
        for (model, branch, width, row) in [(None, None, 0, ""), (Some(""), Some(""), 1, ".")] {
            *footer.model.borrow_mut() = model.map(str::to_owned);
            *footer.metadata.branch.borrow_mut() = branch.map(str::to_owned);
            assert_eq!(rows(&built, width), [row], "width {width}");
        }
    });
}

/// An exact decimal tie rounds upward in the thousands label.
#[test]
fn custom_footer_component_rounds_token_ties_up() {
    driven(async { rounded(1250, 0, 0.0, "↑1.3k ↓0 $0.000").await });
}

/// An exact decimal tie rounds upward in the dollar label.
#[test]
fn custom_footer_component_rounds_cost_ties_up() {
    driven(async { rounded(0, 0, 0.0625, "↑0 ↓0 $0.063").await });
}

/// Renders one usage record at a width that clips to the left side, against a worked label.
async fn rounded(input: u32, output: u32, cost: f64, left: &str) {
    let footer = Footer::new().await;
    footer.history(&[(input, output, cost)]);
    footer.toggle().await;
    let built = footer.build();
    let row = rows(&built, 80);
    assert_eq!(
        row,
        [format!(
            "{left}{}no-model",
            " ".repeat(80 - left.chars().count() - 8)
        )]
    );
}
