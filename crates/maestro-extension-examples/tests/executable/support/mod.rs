//! Disposable configuration, a cleared executable environment and the shipped theme.
use maestro_test_directory::Scratch;
use std::process::Command;

/// A root command with no ambient credentials or user settings.
pub struct Root {
    /// Owned temporary directories.
    pub scratch: Scratch,
}
impl Root {
    /// Create the user configuration root and project directory.
    pub fn new() -> Self {
        let scratch = Scratch::new().unwrap();
        for path in ["agent", "project"] {
            std::fs::create_dir(scratch.path().join(path)).unwrap();
        }
        Self { scratch }
    }
    /// Root executable invocation with supplied literal arguments.
    pub fn command(&self, args: &[&str]) -> Command {
        let executable = std::env::var_os("MAESTRO_EXECUTABLE")
            .expect("run just prepare-tests, then run these tests with cargo nextest");
        let mut command = Command::new(executable);
        command
            .env_clear()
            .current_dir(self.path("project"))
            .env("HOME", self.scratch.path())
            .envs(maestro_test_process::user_cache_home().map(|path| ("XDG_CACHE_HOME", path)))
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            // A Windows process without its system root cannot open sockets.
            .envs(std::env::var_os("SYSTEMROOT").map(|root| ("SYSTEMROOT", root)))
            .env("MAESTRO_CODING_AGENT_DIR", self.path("agent"))
            .env("MAESTRO_PACKAGE_DIR", crate::source::checkout())
            .env("MAESTRO_OFFLINE", "1")
            .env("NO_COLOR", "1")
            .args(args);
        command
    }
    /// A path under the disposable root.
    pub fn path(&self, path: &str) -> std::path::PathBuf {
        self.scratch.path().join(path)
    }
}

#[cfg(unix)]
pub mod native;
pub mod rpc;

impl Root {
    /// Configure one controlled model without ambient provider credentials.
    pub fn models(&self, base: &str) {
        std::fs::write(self.path("agent/models.json"), serde_json::json!({"providers": {"startup": {
            "baseUrl": base, "apiKey": "controlled", "api": "openai-completions",
            "models": [{"id": "controlled", "reasoning": true, "contextWindow": 200_000, "maxTokens":8192}],
        }}}).to_string()).unwrap();
    }
}
pub mod provider;

/// The application's host widgets over the default live theme.
pub fn widgets() -> std::rc::Rc<dyn maestro_app::ExtensionWidgets> {
    let highlighter = std::rc::Rc::new(
        maestro_theme::SyntectHighlighter::new().expect("bundled syntax definitions"),
    );
    std::rc::Rc::new(maestro_chat::ExtensionWidgetHost::new(
        maestro_theme::LiveTheme::default(),
        highlighter,
        maestro_tui::TerminalImage::default(),
    ))
}

/// SDK construction for compiled extension witnesses.
pub mod session;

/// The application's shipped dark theme, in truecolor.
pub fn dark() -> std::rc::Rc<maestro_theme::Theme> {
    maestro_theme::test_support::palette::palette("dark").unwrap()
}

/// A prepared shipped theme for loader decoration, editor styling and the example renderers.
pub fn theme() -> maestro_theme::LiveTheme {
    maestro_theme::LiveTheme::from_theme(dark())
}
