//! Live domain bindings, toolkit widgets and prepared components shared by the host tests.
#![cfg(test)]
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, types::ProviderActions,
};
use std::rc::Rc;

/// Binds a prompt reader without replacing command-specific operations.
pub fn bind_prompt(runner: &ExtensionRunner<Local>, prompt: &'static str) {
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_system_prompt: Rc::new(move || Ok(prompt.into())),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// The user bindings a test manager with a file reloads.
pub fn reloaded_bindings() -> maestro_tui::KeybindingsConfig {
    vec![(
        "tui.select.up".to_owned(),
        Some(maestro_tui::KeybindingKeys::Single("f2".to_owned())),
    )]
}

/// Toolkit widgets over `theme`, whose managers with a file reload [`reloaded_bindings`].
pub fn widgets_over(
    theme: maestro_theme::LiveTheme,
) -> Rc<dyn maestro_extensions::ExtensionWidgets> {
    let widgets = maestro_extensions::test_support::ToolkitWidgets::new(theme).unwrap();
    Rc::new(widgets.reloading(reloaded_bindings()))
}

/// Toolkit widgets over an unpublished live theme.
pub fn widgets() -> Rc<dyn maestro_extensions::ExtensionWidgets> {
    widgets_over(maestro_theme::LiveTheme::default())
}

/// The production artifact factory over toolkit widgets and a fixed installation version.
pub fn artifact(path: String) -> maestro_extensions::ExtensionFactory<Local> {
    maestro_extensions_wasmtime::artifact_factory(
        path,
        widgets(),
        maestro_extensions_wasmtime::Installation {
            version: "installed-component-host".into(),
            agent_dir: maestro_test_process::run_agent_directory().unwrap_or_default(),
        },
    )
}

/// The prepared component the setup variable `variable` names.
///
/// # Errors
/// Returns an error naming the variable when the setup did not export it.
pub fn component(variable: &str) -> Result<std::path::PathBuf, String> {
    std::env::var_os(variable)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| format!("{variable} is unset: run just prepare-tests, then cargo nextest"))
}

/// The application's shipped dark theme, in truecolor.
pub fn dark() -> Rc<maestro_theme::Theme> {
    maestro_theme::test_support::palette::palette("dark").unwrap()
}

/// The suggestion future of a native completion provider.
pub type Pending<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<
                    Option<maestro_tui::AutocompleteSuggestions>,
                    maestro_tui::candidates::CompletionError,
                >,
            > + 'a,
    >,
>;
