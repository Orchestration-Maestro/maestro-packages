//! The rows the application draws for tools and messages, and text without the terminal control
//! sequences a theme adds.
use maestro_extensions::{
    ExtensionRunner, Local, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    ToolRenderResultLike, ToolRenderResultOptions, ToolRendererState, types::RegisteredTool,
};
use maestro_theme::LiveTheme;
use maestro_tui::{TerminalImage, test_support::manual_runtime::ManualRuntime};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The application's shipped dark theme, published live.
pub fn dark() -> LiveTheme {
    LiveTheme::from_theme(maestro_theme::test_support::palette::palette("dark").unwrap())
}

/// Text without terminal control sequences.
pub fn plain_text(styled: &str) -> String {
    maestro_app::strip_ansi(styled).unwrap()
}

/// Rows without terminal control sequences and without the spaces that pad them.
pub fn plain_rows(rows: &[String]) -> Vec<String> {
    rows.iter()
        .map(|row| plain_text(row).trim_end_matches(' ').to_owned())
        .collect()
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

/// The registered tool `name`, which must exist.
fn registered(runner: &ExtensionRunner<Local>, name: &str) -> RegisteredTool<Local> {
    runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap_or_else(|| panic!("no tool named {name}"))
}

/// What a result row shows: the arguments of its call, the result and the row's state.
pub struct Shown<'a> {
    /// The completed arguments of the call.
    pub args: &'a Value,
    /// The result as `{"content": [..], "details": ..}`.
    pub result: &'a Value,
    /// Whether the row is expanded.
    pub expanded: bool,
    /// Whether the result is still partial.
    pub partial: bool,
    /// The width the row is drawn at.
    pub width: usize,
}

impl<'a> Shown<'a> {
    /// A collapsed, complete result row drawn 200 cells wide.
    pub fn new(args: &'a Value, result: &'a Value) -> Self {
        Self {
            args,
            result,
            expanded: false,
            partial: false,
            width: 200,
        }
    }
}

/// A row's render context: completed arguments, no images and a redraw that does nothing.
fn row_context<'a>(
    args: &'a Value,
    rows: &'a ToolRendererState,
    (expanded, partial): (bool, bool),
) -> ToolRenderContext<'a, ToolRendererState> {
    let environment = Rc::new(ToolRenderEnvironment {
        runtime: ManualRuntime::new().handle(),
        readme_path: String::new(),
        live_theme: dark(),
        highlighter: Rc::new(Plain),
        terminal_image: TerminalImage::default(),
    });
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "row".into(),
        invalidate: Rc::new(|| {}),
        environment,
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/work".into(),
        phase: ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: partial,
        },
        expanded,
        show_images: false,
        is_error: false,
    }
}

/// Renders the call row of the registered tool `name` for `args`, without control sequences.
pub fn render_call(
    runner: &ExtensionRunner<Local>,
    name: &str,
    args: &Value,
    width: usize,
) -> Vec<String> {
    let state = ToolRendererState::default();
    let context = row_context(args, &state, (false, false));
    let view = registered(runner, name)
        .definition
        .renderer_binding()
        .unwrap()
        .render_call
        .expect("a call renderer")(Some(args), &dark(), &context)
    .unwrap();
    plain_rows(&view.try_render(width).unwrap())
}

/// Renders the result row of the registered tool `name`, without control sequences.
pub fn render_result(runner: &ExtensionRunner<Local>, name: &str, shown: &Shown) -> Vec<String> {
    let state = ToolRendererState::default();
    let context = row_context(shown.args, &state, (shown.expanded, shown.partial));
    let blocks: Vec<maestro_app::UserBlock> = serde_json::from_value(
        shown
            .result
            .get("content")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .unwrap();
    let details = shown.result.get("details").filter(|details| !details.is_null());
    let view = registered(runner, name)
        .definition
        .renderer_binding()
        .unwrap()
        .render_result
        .expect("a result renderer")
        .prepare(ToolRenderResultLike {
            content: &blocks,
            details,
            terminate: None,
            is_error: Some(false),
        })
        .unwrap()
        .render(
            ToolRenderResultOptions {
                expanded: shown.expanded,
                is_partial: shown.partial,
            },
            &dark(),
            &context,
        )
        .unwrap();
    plain_rows(&view.try_render(shown.width).unwrap())
}

/// Draws the stored custom message `message` with the renderer registered for its type and
/// returns the non-empty rows, without control sequences. `None` when no renderer drew it.
pub fn render_message(
    runner: &ExtensionRunner<Local>,
    message: &Value,
    expanded: bool,
    width: usize,
) -> Option<Vec<String>> {
    let custom_type = message["customType"].as_str().unwrap();
    let renderer = runner.get_message_renderer_call::<Value>(custom_type)?;
    let view = renderer
        .call(
            Rc::new(serde_json::from_value(message.clone()).unwrap()),
            maestro_app::MessageRenderOptions { expanded },
            dark(),
        )
        .unwrap()?;
    Some(
        plain_rows(&view.try_render(width).unwrap())
            .into_iter()
            .map(|row| row.trim().to_owned())
            .filter(|row| !row.is_empty())
            .collect(),
    )
}
