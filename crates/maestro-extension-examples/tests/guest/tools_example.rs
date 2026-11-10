//! The shipped tool selector loaded into the production engine.
use super::support::{plain_text, TERMINAL, Recorder, bind, verdict, fail_with, harness, load, local};
use maestro_extensions::{Presence, SessionManager, SessionStartEvent, SessionTreeEvent};
use maestro_extensions_wasmtime::test_support::{Harness, Loaded};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The variable that names the built tool-selector component.
const COMPONENT: &str = "MAESTRO_TOOLS_COMPONENT";

/// One inventory entry, as the application reports it.
fn tool(name: &str) -> Value {
    json!({"name": name, "description": format!("{name} tool"), "parameters": {},
        "sourceInfo": {"path": "/tools", "source": "local", "scope": "project", "origin": "top-level"}})
}

/// A loaded selector whose inventory, active tools and branch the test controls.
struct Instance {
    /// Keeps the engine and its session.
    harness: Harness,
    /// The loaded runner.
    loaded: Loaded,
    /// The UI, which also logs the active-tool updates and the persisted entries.
    ui: Rc<Recorder>,
    /// The tools the application reports.
    inventory: Rc<RefCell<Vec<Value>>>,
    /// The failure the inventory read returns.
    refusal: Rc<RefCell<Option<String>>>,
}

impl Instance {
    /// Loads the component over an application with `read` active and three tools.
    async fn new() -> Self {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let inventory = Rc::new(RefCell::new(vec![
            tool("read"),
            tool("bash"),
            tool("extra"),
        ]));
        let refusal = Rc::new(RefCell::new(None::<String>));
        bind(&loaded.runner, |actions, _| {
            let (tools, refused) = (inventory.clone(), refusal.clone());
            actions.get_all_tools = Rc::new(move || {
                fail_with(&refused)?;
                Ok(serde_json::from_value(json!(tools.borrow().clone())).unwrap())
            });
            actions.get_active_tools = Rc::new(|| Ok(vec!["read".to_owned()]));
            let log = ui.clone();
            actions.set_active_tools =
                Rc::new(move |names| log.record("setActiveTools", json!([names])));
            let log = ui.clone();
            actions.append_entry =
                Rc::new(move |kind, data| log.record("entry", json!([kind, data])));
        });
        Self {
            harness,
            loaded,
            ui,
            inventory,
            refusal,
        }
    }

    /// Puts the given custom entries on the current branch, oldest first.
    fn branch(&self, configs: &[Value]) {
        let mut history = SessionManager::in_memory(Some("/work")).unwrap();
        for enabled in configs {
            history
                .append_custom_entry(
                    "tools-config",
                    Presence::Present(json!({"enabledTools": enabled})),
                )
                .unwrap();
        }
        *self.harness.session_manager.borrow_mut() = history;
    }

    /// Delivers a session start or a tree change and returns the effects it had.
    async fn event(&self, name: &str) -> Vec<Value> {
        if name == "session_start" {
            let event = json!({"type": name, "reason": "resume"});
            verdict::<SessionStartEvent>(&self.loaded.runner, &event)
                .await
                .unwrap();
        } else {
            let event = json!({"type": name, "newLeafId": "leaf", "oldLeafId": "old"});
            verdict::<SessionTreeEvent>(&self.loaded.runner, &event)
                .await
                .unwrap();
        }
        self.ui.take()
    }

    /// Opens the selector with `inputs` and returns its failure, if any.
    async fn open(&self, inputs: &[&str]) -> Option<String> {
        self.ui.script(inputs);
        let command = self.loaded.runner.get_command("tools").unwrap().command;
        (command.handler)(
            "ignored arguments".into(),
            self.loaded.runner.create_command_context(),
        )
        .await
        .err()
        .map(|error| error.to_string())
    }
}

/// Both event kinds restore only the newest configuration on the selected branch.
#[test]
fn tools_restore_current_branch_and_filter_removed_tools() {
    local(async {
        let instance = Instance::new().await;
        instance.branch(&[
            json!(["read"]),
            json!(["extra", "removed", "extra", "bash"]),
        ]);
        assert_eq!(
            instance.event("session_start").await,
            [json!(["setActiveTools", [["extra", "bash"]]])]
        );
        instance.branch(&[json!([])]);
        assert_eq!(
            instance.event("session_tree").await,
            [json!(["setActiveTools", [[]]])]
        );
        instance.branch(&[]);
        assert!(
            instance.event("session_tree").await.is_empty(),
            "no saved selection leaves active tools untouched"
        );
    });
}

/// Settings reflect all available tools, and each input applies and persists before closing.
#[test]
fn tools_selector_applies_and_persists_each_change() {
    local(async {
        let instance = Instance::new().await;
        instance.branch(&[]);
        assert!(instance.event("session_start").await.is_empty());
        assert_eq!(
            instance
                .open(&["\r", "\x1b[B", "\r", "\x1b[A", "\r", "\x1b"])
                .await,
            None
        );
        let states: Vec<[String; 3]> = instance
            .ui
            .frames()
            .iter()
            .map(|frame| states(frame))
            .collect();
        let on = |a: &str, b: &str, c: &str| [a.to_owned(), b.to_owned(), c.to_owned()];
        assert_eq!(
            states,
            [
                on("enabled", "disabled", "disabled"),
                on("disabled", "disabled", "disabled"),
                on("disabled", "disabled", "disabled"),
                on("disabled", "enabled", "disabled"),
                on("disabled", "enabled", "disabled"),
                on("enabled", "enabled", "disabled"),
                on("enabled", "enabled", "disabled"),
            ]
        );
        let requests = instance.ui.take();
        assert_eq!(
            requests,
            [
                json!(["custom", []]),
                json!(["setActiveTools", [[]]]),
                json!(["entry", ["tools-config", {"enabledTools": []}]]),
                json!(["setActiveTools", [["bash"]]]),
                json!(["entry", ["tools-config", {"enabledTools": ["bash"]}]]),
                json!(["setActiveTools", [["bash", "read"]]]),
                json!(["entry", ["tools-config", {"enabledTools": ["bash", "read"]}]]),
            ]
        );
    });
}

/// Each row of a frame that names one of the three tools ends with that tool's state.
fn states(frame: &[String]) -> [String; 3] {
    let frame: Vec<String> = frame.iter().map(|row| plain_text(row)).collect();
    assert_eq!(frame[0].trim(), "Tool Configuration");
    ["read", "bash", "extra"].map(|name| {
        let row = frame
            .iter()
            .find(|row| row.contains(name))
            .unwrap_or_else(|| panic!("no row for {name}: {frame:?}"));
        row.trim().rsplit(' ').next().unwrap().to_owned()
    })
}

/// The selector refreshes the inventory at command time, including empty and long inventories,
/// and closing without a change writes nothing.
#[test]
fn tools_selector_refreshes_inventory_and_limits_visible_rows() {
    local(async {
        let instance = Instance::new().await;
        instance.branch(&[]);
        assert!(instance.event("session_start").await.is_empty());
        for count in [0, 16] {
            *instance.inventory.borrow_mut() = (0..count)
                .map(|index| tool(&format!("new-{index}")))
                .collect();
            assert_eq!(instance.open(&["\x1b"]).await, None);
            let frames = instance.ui.frames();
            let listed = frames[0].iter().filter(|row| row.contains("new-")).count();
            assert_eq!(listed, count.min(15), "{count} tools");
            assert_eq!(
                instance.ui.take(),
                [json!(["custom", []])],
                "closing without changes writes nothing"
            );
        }
    });
}

/// Host failures stay visible and do not turn into successful commands.
#[test]
fn tools_selector_propagates_host_rejections() {
    local(async {
        let instance = Instance::new().await;
        instance.branch(&[]);
        assert!(instance.event("session_start").await.is_empty());
        *instance.refusal.borrow_mut() = Some("inventory refused".into());
        assert_eq!(
            instance.open(&[]).await,
            Some("inventory refused".to_owned())
        );
        *instance.refusal.borrow_mut() = None;
        instance.ui.reject("custom", "selector refused");
        assert_eq!(
            instance.open(&[]).await,
            Some("selector refused".to_owned())
        );
        assert_eq!(
            instance.ui.take(),
            [json!(["custom", []])],
            "nothing is written"
        );
    });
}
