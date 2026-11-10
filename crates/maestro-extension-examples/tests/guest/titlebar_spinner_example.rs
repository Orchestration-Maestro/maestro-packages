//! The shipped title animation loaded into the production engine, driven by the real clock.
use super::lane_a::{Ui, event, failures, harness_in, local_io};
use super::support::load;
use maestro_extensions_wasmtime::test_support::{
    Harness, Loaded,
    maestro_extensions::{AgentEndEvent, AgentStartEvent, SessionShutdownEvent},
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The variable that names the built title component.
const COMPONENT: &str = "MAESTRO_TITLEBAR_SPINNER_COMPONENT";
/// The animation frames in display order.
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// One loaded instance with the session name the host reports and the title requests it makes.
struct Titlebar {
    /// The engine the instance lives in.
    harness: Harness,
    /// The runner over the loaded component.
    loaded: Loaded,
    /// The title requests.
    ui: Rc<Ui>,
    /// The session name the host reports.
    name: Rc<RefCell<Option<String>>>,
    /// The callback failures the runner reported.
    failures: Rc<RefCell<Vec<String>>>,
    /// How many title requests the test has consumed.
    read: Cell<usize>,
}

impl Titlebar {
    /// Loads the component into a session working in `cwd`.
    async fn new(cwd: &str) -> Self {
        let harness = harness_in(cwd);
        let loaded = load(&harness, COMPONENT).await;
        let name = Rc::new(RefCell::new(None));
        let reported = name.clone();
        harness.runtime.actions.write().unwrap().get_session_name =
            Rc::new(move || Ok(reported.borrow().clone()));
        let ui = Ui::install(&loaded.runner);
        let failures = failures(&loaded.runner);
        Self {
            harness,
            loaded,
            ui,
            name,
            failures,
            read: Cell::new(0),
        }
    }

    /// Reports `name` as the session's name from now on.
    fn rename(&self, name: Option<&str>) {
        *self.name.borrow_mut() = name.map(str::to_owned);
    }

    /// Delivers the lifecycle event `name`.
    async fn deliver(&self, name: &str) {
        let runner = &self.loaded.runner;
        match name {
            "agent_start" => {
                let mut start: AgentStartEvent = event(json!({"type": "agent_start"}));
                runner.emit(&mut start).await.unwrap();
            }
            "agent_end" => {
                let mut end: AgentEndEvent = event(json!({"type": "agent_end", "messages": []}));
                runner.emit(&mut end).await.unwrap();
            }
            _ => {
                let mut shutdown: SessionShutdownEvent =
                    event(json!({"type": "session_shutdown", "reason": "quit"}));
                runner.emit(&mut shutdown).await.unwrap();
            }
        }
    }

    /// Waits for the next unread title request and returns its text.
    async fn next(&self) -> String {
        let index = self.read.get();
        self.ui.until(index + 1).await;
        self.read.set(index + 1);
        self.title(index)
    }

    /// The text of the title request recorded at `index`.
    fn title(&self, index: usize) -> String {
        let request = &self.ui.seen()[index];
        assert_eq!(request["method"], "setTitle");
        request["operands"][0].as_str().unwrap().to_owned()
    }

    /// The newest title the component set.
    fn latest(&self) -> String {
        self.title(self.ui.seen().len() - 1)
    }
}

/// All ten authored frames, wraparound and the live session name on every frame.
#[test]
fn titlebar_spinner_cadence_and_live_title() {
    local_io(async {
        let bar = Titlebar::new("/work/project/").await;
        bar.rename(Some("named 雪"));
        bar.deliver("agent_start").await;
        assert_eq!(bar.next().await, "Maestro - named 雪 - project");
        for index in 0..=10 {
            match index {
                1 => bar.rename(Some("renamed")),
                2 => bar.rename(Some("")),
                _ => {}
            }
            let suffix = match index {
                0 => "named 雪 - project",
                1 => "renamed - project",
                _ => "project",
            };
            assert_eq!(
                bar.next().await,
                format!("{} Maestro - {suffix}", FRAMES[index % FRAMES.len()])
            );
        }
        bar.deliver("agent_end").await;
        assert_eq!(bar.latest(), "Maestro - project");
        assert!(bar.failures.borrow().is_empty());
    });
}

/// Restart, end and shutdown stop the animation and restore the current base title.
#[test]
fn titlebar_spinner_lifecycle_cancels_pending_waits() {
    local_io(async {
        let bar = Titlebar::new("/work/project/").await;
        for ending in ["agent_start", "agent_end", "session_shutdown"] {
            bar.deliver("agent_start").await;
            bar.next().await;
            bar.next().await;
            bar.rename(Some("after"));
            bar.deliver(ending).await;
            assert_eq!(bar.latest(), "Maestro - after - project", "{ending}");
            if ending == "agent_start" {
                assert_eq!(bar.next().await, "Maestro - after - project");
                assert_eq!(bar.next().await, "⠋ Maestro - after - project");
                bar.deliver("agent_end").await;
                assert_eq!(bar.latest(), "Maestro - after - project");
            }
            bar.read.set(bar.ui.seen().len());
        }
        assert!(bar.failures.borrow().is_empty());
    });
}

/// The title names the host's basename of the working directory, keeping a literal backslash.
#[test]
fn titlebar_spinner_uses_host_basename() {
    local_io(async {
        let bar = Titlebar::new(r"/work/a\b").await;
        bar.deliver("agent_end").await;
        assert_eq!(bar.next().await, r"Maestro - a\b");
    });
}

/// Root, empty and trailing-slash directories restore without inventing a directory name; a
/// refused title or name lookup is reported and starts no animation.
#[test]
fn titlebar_spinner_base_titles_and_rejections() {
    local_io(async {
        for (cwd, expected) in [
            ("/", "Maestro - "),
            ("", "Maestro - "),
            ("/a/b///", "Maestro - b"),
        ] {
            let bar = Titlebar::new(cwd).await;
            bar.deliver("agent_end").await;
            assert_eq!(bar.next().await, expected, "{cwd:?}");
        }
        let bar = Titlebar::new("/a/b").await;
        bar.ui.reject("setTitle", "title refused");
        bar.deliver("agent_start").await;
        assert_eq!(*bar.failures.borrow(), ["title refused"]);
        assert_eq!(bar.ui.seen().len(), 1, "no animation after a refused title");
        bar.harness
            .runtime
            .actions
            .write()
            .unwrap()
            .get_session_name = Rc::new(|| Err("name refused".into()));
        bar.deliver("agent_start").await;
        assert_eq!(*bar.failures.borrow(), ["title refused", "name refused"]);
        bar.deliver("session_shutdown").await;
    });
}

/// The built component registers its three lifecycle hooks and runs a whole lifecycle.
#[test]
fn titlebar_spinner_component_runs_lifecycle() {
    local_io(async {
        let bar = Titlebar::new("/work/project").await;
        for name in ["agent_start", "agent_end", "session_shutdown"] {
            assert!(bar.loaded.runner.has_handlers(name), "{name}");
        }
        for name in ["agent_start", "agent_end", "session_shutdown"] {
            bar.deliver(name).await;
        }
        assert_eq!(bar.latest(), "Maestro - project");
        assert!(bar.failures.borrow().is_empty());
    });
}
