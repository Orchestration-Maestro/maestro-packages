//! The extension examples loaded as components in Maestro's native host.
mod canonical_ui;
mod components;
mod example_process;
mod model_support;
mod session;
mod support;

/// The shared silent command operations the examples' witnesses bind.
mod context_actions {
    pub use maestro_extensions::test_support::silent_command_operations;
}

use maestro_tui::test_support::{manual_runtime, recording_terminal};

mod auto_commit_on_exit_example;
mod bash_spawn_hook_example;
mod border_status_editor_example;
mod custom_compaction_example;
mod custom_gateway_provider_example;
mod doom_game;
mod doom_wad;
mod dynamic_resources_example;
mod extension_guide;
mod freedoom_archive;
mod git_checkpoint_example;
mod handoff_example;
mod inline_bash_example;
mod interactive_shell_example;
mod minimal_mode_example;
mod overlay_qa_tests_example;
mod overlay_test_example;
mod project_rules_example;
mod summarize_example;
mod tool_override_example;
