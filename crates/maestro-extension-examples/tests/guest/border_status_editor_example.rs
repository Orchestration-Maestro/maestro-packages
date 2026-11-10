//! The shipped border status editor's spinner, driven through its lifecycle hooks.
use super::support::{Bindings, Failures, Instance, Recorder, emit, stepped};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AgentStartEvent, SessionStartEvent, types::AgentEndEvent,
};
use serde_json::json;
use std::rc::Rc;

/// The variable that names the built editor component.
const COMPONENT: &str = "MAESTRO_BORDER_STATUS_EDITOR_COMPONENT";

/// The ten frames in display order.
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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
        let ui = Recorder::install_on(instance.runner(), (80, 24));
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

    /// Waits until the top border shows `frame`, then asserts it shows no other.
    async fn shows(&self, frame: &str) {
        while !self.top().contains(frame) {
            self.ui.redrawn().await;
        }
        assert_eq!(self.shown(), [frame]);
    }

    /// Delivers `agent_start`.
    async fn start(&self) {
        emit::<AgentStartEvent>(self.instance.runner(), &json!({})).await;
    }

    /// Delivers `agent_end`.
    async fn end(&self) {
        emit::<AgentEndEvent>(self.instance.runner(), &json!({"messages": []})).await;
    }
}

/// Working shows the first frame, and the end of work clears the spinner.
#[test]
fn border_status_editor_work_shows_the_first_frame_and_its_end_clears_it() {
    stepped(async {
        let session = Session::new().await;
        assert!(session.shown().is_empty());
        session.start().await;
        session.shows(FRAMES[0]).await;
        session.end().await;
        assert!(
            session.shown().is_empty(),
            "ending clears the spinner: {:?}",
            session.top()
        );
        assert!(session.failures.take().is_empty());
    });
}
