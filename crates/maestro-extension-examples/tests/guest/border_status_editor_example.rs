//! The shipped border status editor's spinner, driven through its lifecycle hooks and the paused
//! clock its waits run on.
use super::lane_b::{Bindings, Failures, Instance, Recorder, emit, stepped};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AgentStartEvent, SessionShutdownEvent, SessionStartEvent, types::AgentEndEvent,
};
use serde_json::json;
use std::{rc::Rc, time::Duration};

/// The variable that names the built editor component.
const COMPONENT: &str = "MAESTRO_BORDER_STATUS_EDITOR_COMPONENT";

/// The ten frames in display order.
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How many scheduling rounds a test gives the spinner to react before it reports a mismatch.
const DRAIN: usize = 300;

/// Gives every task that can run the chance to, so a wait the spinner arms in reaction to an
/// event is armed before the clock moves.
async fn settle() {
    for _ in 0..DRAIN {
        tokio::task::yield_now().await;
    }
}

/// The wait between two frames.
const STEP: Duration = Duration::from_millis(80);

/// The started session: the loaded example, its UI and the failures it reported.
struct Session {
    /// The loaded component.
    instance: Instance,
    /// The interactive UI the editor is installed on.
    ui: Rc<Recorder>,
    /// The failures the callbacks reported.
    failures: Failures,
}

impl Session {
    /// Loads the example, starts the session and lets its branch read finish.
    async fn new() -> Self {
        let instance = Instance::new(COMPONENT).await;
        let ui = Recorder::install_interactive(instance.runner(), 80, 24);
        let mut bindings = Bindings::new();
        bindings.actions.get_thinking_level =
            Rc::new(|| Ok(maestro_models::ModelThinkingLevel::High));
        bindings.context.get_model = Rc::new(|| Ok(None));
        bindings.context.get_context_usage = Rc::new(|| Ok(None));
        bindings.apply(instance.runner());
        let failures = Failures::collect(instance.runner());
        emit::<SessionStartEvent>(instance.runner(), &json!({"reason": "startup"})).await;
        ui.settle();
        Self {
            instance,
            ui,
            failures,
        }
    }

    /// The top border the editor renders 40 columns wide.
    fn top(&self) -> String {
        self.ui.editor_rows(40)[0].clone()
    }

    /// The frames the top border shows.
    fn shown(&self) -> Vec<&'static str> {
        let top = self.top();
        FRAMES
            .iter()
            .filter(|frame| top.contains(*frame))
            .copied()
            .collect()
    }

    /// Waits until the top border shows `frame`, then lets the spinner arm its next wait.
    async fn shows(&self, frame: &str) {
        for _ in 0..DRAIN {
            if self.top().contains(frame) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(self.shown(), [frame]);
        settle().await;
    }

    /// Lets one wait end.
    async fn step(&self) {
        tokio::time::advance(STEP).await;
    }

    /// Delivers `agent_start` and lets the spinner arm its first wait.
    async fn start(&self) {
        emit::<AgentStartEvent>(self.instance.runner(), &json!({})).await;
        settle().await;
    }

    /// Delivers `agent_end` and lets the spinner stop.
    async fn end(&self) {
        emit::<AgentEndEvent>(self.instance.runner(), &json!({"messages": []})).await;
        settle().await;
    }
}

/// Working shows the ten frames in order, one per wait, wraps after the last, and the end of
/// work clears the spinner.
#[test]
fn border_status_editor_spinner_follows_each_wait() {
    stepped(async {
        let session = Session::new().await;
        assert!(session.shown().is_empty());
        session.start().await;
        session.shows(FRAMES[0]).await;
        for frame in FRAMES.iter().skip(1).chain(&FRAMES[..2]) {
            session.step().await;
            session.shows(frame).await;
        }
        session.end().await;
        assert!(
            session.shown().is_empty(),
            "ending clears the spinner: {:?}",
            session.top()
        );
        assert!(session.failures.take().is_empty());
    });
}

/// A second start drops the running wait and continues from the frame shown.
#[test]
fn border_status_editor_restart_keeps_the_frame() {
    stepped(async {
        let session = Session::new().await;
        session.start().await;
        session.shows(FRAMES[0]).await;
        for frame in &FRAMES[1..4] {
            session.step().await;
            session.shows(frame).await;
        }
        session.start().await;
        session.shows(FRAMES[3]).await;
        session.step().await;
        session.shows(FRAMES[4]).await;
        session.end().await;
        assert!(session.shown().is_empty(), "{:?}", session.top());
    });
}

/// Shutdown stops the spinner: no further frame follows a wait, even when the session keeps
/// receiving agent events.
#[test]
fn border_status_editor_shutdown_stops_the_spinner() {
    stepped(async {
        let session = Session::new().await;
        session.start().await;
        session.shows(FRAMES[0]).await;
        session.step().await;
        session.shows(FRAMES[1]).await;
        emit::<SessionShutdownEvent>(session.instance.runner(), &json!({"reason": "quit"})).await;
        let written = session.ui.writes().len();
        session.step().await;
        session.end().await;
        assert_eq!(
            session.ui.writes().len(),
            written,
            "no terminal write after shutdown"
        );
    });
}
