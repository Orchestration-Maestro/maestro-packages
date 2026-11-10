//! The shipped custom header loaded into the production engine.
use super::support::{Failures, Recorder, TERMINAL, dark, harness, load, local, verdict};
use maestro_extensions::SessionStartEvent;
use serde_json::{Value, json};

/// The variable that names the built custom-header component.
const COMPONENT: &str = "MAESTRO_CUSTOM_HEADER_COMPONENT";

/// Renders the installed header at `width`, with styling removed.
fn rows(ui: &Recorder, width: u16) -> Vec<String> {
    let factory = ui.header().expect("the extension installed a header");
    let built = factory.call((ui.stage().tui.clone(), dark())).unwrap();
    built
        .component
        .try_render(width.into())
        .unwrap()
        .iter()
        .map(|row| maestro_app::strip_ansi(row).unwrap())
        .collect()
}

/// The illustration rows every header carries above its version line.
fn illustration() -> Vec<String> {
    [
        "",
        "       ▄",
        "      ▀█▀",
        "       ▀",
        "  █▄       ▄█",
        "  █ ▀▄   ▄▀ █",
        "  █   ▀█▀   █",
        "",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Splits the header into its illustration and its last row, which names the host version.
fn assert_header(rows: &[String]) {
    assert_eq!(rows[..8], illustration());
    let version = rows[8]
        .strip_prefix("   Custom header v")
        .expect("the version line");
    assert!(
        version.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "{version}"
    );
    assert_eq!(rows.len(), 9);
}

/// Every start reason installs the illustration with the host version.
#[test]
fn custom_header_renders_host_version() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        for reason in ["startup", "reload", "new", "resume", "fork"] {
            let event = json!({"type":"session_start","reason":reason});
            assert_eq!(
                verdict::<SessionStartEvent>(&loaded.runner, &event).await,
                Ok(Value::Null)
            );
            assert_eq!(ui.take(), [json!(["setHeader", [true]])], "{reason}");
            for width in [1, 24, 80] {
                assert_header(&rows(&ui, width));
            }
        }
    });
}

/// The command restores the built-in slot before notifying, whether or not a custom header is
/// installed.
#[test]
fn custom_header_restores_builtin_header() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        let event = json!({"type":"session_start","reason":"new"});
        verdict::<SessionStartEvent>(&loaded.runner, &event)
            .await
            .unwrap();
        ui.take();
        let command = loaded.runner.get_command("builtin-header").unwrap().command;
        for _ in 0..2 {
            (command.handler)(
                "ignored arguments".into(),
                loaded.runner.create_command_context(),
            )
            .await
            .unwrap();
            assert!(ui.header().is_none());
            assert_eq!(
                ui.take(),
                [
                    json!(["setHeader", [false]]),
                    json!(["notify", ["Built-in header restored", "Info"]])
                ]
            );
        }
    });
}

/// A headless start installs nothing and reports no failure.
#[test]
fn custom_header_skips_headless_startup() {
    local(async {
        let harness = harness();
        let loaded = load(&harness, COMPONENT).await;
        let ui = Recorder::install_on(&loaded.runner, TERMINAL);
        ui.connect(&loaded.runner, false);
        let failures = Failures::collect(&loaded.runner);
        let event = json!({"type":"session_start","reason":"new"});
        assert_eq!(
            verdict::<SessionStartEvent>(&loaded.runner, &event).await,
            Ok(Value::Null)
        );
        assert!(failures.take().is_empty(), "a headless start reports no failure");
        assert!(ui.take().is_empty());
        assert!(ui.header().is_none());
    });
}
