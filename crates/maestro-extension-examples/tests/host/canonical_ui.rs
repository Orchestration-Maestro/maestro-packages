//! The application UI collaborators the host tests render through.
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

/// Footer metadata whose branch listeners the test runs.
#[derive(Default)]
pub(super) struct Metadata {
    /// Current branch.
    branch: RefCell<Option<String>>,
    /// Live extension statuses.
    statuses: maestro_extensions::footer::ExtensionStatuses,
    /// Branch listeners in registration order.
    listeners: RefCell<Vec<Rc<dyn Fn()>>>,
}
impl maestro_extensions::footer::ReadonlyFooterDataProvider for Metadata {
    fn get_git_branch(&self) -> Option<String> {
        self.branch.borrow().clone()
    }
    fn get_extension_statuses(&self) -> maestro_extensions::footer::ExtensionStatuses {
        self.statuses.clone()
    }
    fn get_available_provider_count(&self) -> f64 {
        3.0
    }
    fn on_branch_change(&self, callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        self.listeners.borrow_mut().push(callback);
        Box::new(|| {})
    }
}

/// A terminal writer over a recording terminal and a manual runtime.
pub(super) fn screen() -> (
    maestro_tui::TUI,
    super::recording_terminal::RecordingTerminal,
) {
    let terminal = super::recording_terminal::RecordingTerminal::new(40, 12);
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        super::manual_runtime::ManualRuntime::new().handle(),
        maestro_tui::TerminalImage::default(),
        Some(false),
    );
    (tui, terminal)
}

/// Supports no language, so rendering never classifies code.
struct Plain;
impl maestro_theme::SyntaxHighlighter for Plain {
    fn supports_language(&self, _: &str) -> bool {
        false
    }
    fn highlight(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Vec::new())
    }
}

/// A row's render context: completed arguments, an expanded row and a redraw counter.
pub(super) fn row_context<'a>(
    args: &'a Value,
    rows: &'a maestro_extensions::ToolRendererState,
    redraws: &Rc<std::cell::Cell<u32>>,
) -> maestro_extensions::ToolRenderContext<'a, maestro_extensions::ToolRendererState> {
    let counted = redraws.clone();
    let environment = Rc::new(maestro_extensions::ToolRenderEnvironment {
        runtime: super::manual_runtime::ManualRuntime::new().handle(),
        readme_path: String::new(),
        live_theme: maestro_theme::LiveTheme::default(),
        highlighter: Rc::new(Plain),
        terminal_image: maestro_tui::TerminalImage::default(),
    });
    maestro_extensions::ToolRenderContext {
        args: Some(args),
        tool_call_id: "n1".into(),
        invalidate: Rc::new(move || counted.set(counted.get() + 1)),
        environment,
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/work".into(),
        phase: maestro_extensions::ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: false,
        },
        expanded: true,
        show_images: false,
        is_error: false,
    }
}
