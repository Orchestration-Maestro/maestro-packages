//! Drawing a registered tool's call and result the way the application's rows do.
use super::plain;
use maestro_extensions_wasmtime::test_support::maestro_extensions::{ExtensionRunner, Local};
use maestro_tui::{TerminalImage, test_support::manual_runtime::ManualRuntime};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

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

/// The dark theme tools render with.
fn dark() -> maestro_theme::LiveTheme {
    maestro_theme::LiveTheme::from_theme(
        maestro_theme::test_support::palette::palette("dark").unwrap(),
    )
}

/// The registered tool `name`, which must exist.
fn registered(
    runner: &ExtensionRunner<Local>,
    name: &str,
) -> maestro_extensions_wasmtime::test_support::maestro_extensions::RegisteredTool<Local> {
    runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.metadata().definition.name == name)
        .unwrap_or_else(|| panic!("no tool named {name}"))
}

/// A row's render context: completed arguments, the expansion and whether the call is partial.
fn row_context<'a>(
    args: &'a Value,
    rows: &'a maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRendererState,
    [expanded, partial]: [bool; 2],
) -> maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRenderContext<
    'a,
    maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRendererState,
> {
    use maestro_extensions_wasmtime::test_support::maestro_extensions::{
        ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    };
    let environment = Rc::new(ToolRenderEnvironment {
        runtime: ManualRuntime::new().handle(),
        readme_path: String::new(),
        live_theme: maestro_theme::LiveTheme::default(),
        highlighter: Rc::new(Plain),
        terminal_image: TerminalImage::default(),
    });
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "call".into(),
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

/// The unstyled rows, without their trailing padding, the registered tool `name` draws for a
/// call with `args`.
pub fn render_call(runner: &ExtensionRunner<Local>, name: &str, args: &Value) -> Vec<String> {
    let rows =
        maestro_extensions_wasmtime::test_support::maestro_extensions::ToolRendererState::default();
    let row = row_context(args, &rows, [false; 2]);
    registered(runner, name)
        .renderer_binding()
        .unwrap()
        .render_call
        .unwrap()(Some(args), &dark(), &row)
    .unwrap()
    .try_render(200)
    .unwrap()
    .iter()
    .map(|row| plain(row).trim_end_matches(' ').to_owned())
    .collect()
}

/// The unstyled rows the registered tool `name` draws for a result in the given state.
pub fn render_result(
    runner: &ExtensionRunner<Local>,
    name: &str,
    args: &Value,
    result: &Value,
    flags: [bool; 2],
) -> Vec<String> {
    use maestro_extensions_wasmtime::test_support::maestro_extensions::{
        ToolRenderResultLike, ToolRenderResultOptions, ToolRendererState,
    };
    let [expanded, partial] = flags;
    let content: Vec<_> = serde_json::from_value(result["content"].clone()).unwrap();
    let like = ToolRenderResultLike {
        content: &content,
        details: result.get("details"),
        terminate: None,
        is_error: Some(false),
    };
    let rows = ToolRendererState::default();
    let row = row_context(args, &rows, flags);
    registered(runner, name)
        .renderer_binding()
        .unwrap()
        .render_result
        .unwrap()
        .prepare(like)
        .unwrap()
        .render(
            ToolRenderResultOptions {
                expanded,
                is_partial: partial,
            },
            &dark(),
            &row,
        )
        .unwrap()
        .try_render(200)
        .unwrap()
        .iter()
        .map(|row| plain(row).trim_end_matches(' ').to_owned())
        .collect()
}
