//! The extension examples as built components on Maestro's production engine, through the public
//! test harness.
#[path = "support/lane_a.rs"]
mod lane_a;
#[path = "support/lane_b.rs"]
mod lane_b;
#[path = "support/lane_c.rs"]
mod lane_c;
mod support;

mod bookmark_example;
mod border_status_editor_example;
mod commands_example;
mod confirm_destructive_example;
mod custom_footer_example;
mod custom_header_example;
mod dirty_repo_guard_example;
mod dynamic_tools_example;
mod event_bus_example;
mod file_trigger_example;
mod git_checkpoint_example;
mod github_issue_autocomplete_example;
mod hello_example;
mod hidden_thinking_label_example;
mod input_transform_example;
mod mac_system_theme_example;
mod message_renderer_example;
mod modal_editor_example;
mod model_status_example;
mod notify_example;
mod permission_gate_example;
mod pirate_example;
mod preset_example;
mod prompt_customizer_example;
mod protected_paths_example;
mod question_example;
mod questionnaire_example;
mod rainbow_editor_example;
mod reload_runtime_example;
mod render_example;
mod send_user_message_example;
mod session_name_example;
mod shutdown_example;
mod snake_example;
mod space_invaders_example;
mod status_line_example;
mod structured_output_example;
mod summarize_example;
mod system_prompt_header_example;
mod tic_tac_toe_example;
mod timed_confirm_example;
mod titlebar_spinner_example;
mod todo_example;
mod tools_example;
mod trigger_compact_example;
mod truncated_tool_example;
mod ui_example;
mod utils_trim_example;
mod widget_placement_example;
mod with_deps_example;
mod working_indicator_example;
mod working_message_example;
