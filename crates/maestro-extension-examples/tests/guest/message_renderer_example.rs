//! The shipped status command and message renderer loaded into the production engine.
use super::support::{Bindings, Instance, local, render_message};
use serde_json::{Value, json};

/// The variable that names the built renderer component.
const COMPONENT: &str = "MAESTRO_MESSAGE_RENDERER_COMPONENT";

/// Width the transcript supplies to each rendered message.
const WIDTH: usize = 40;

/// Sends `args` to the status command and returns the messages the application received.
async fn sent(instance: &Instance, cases: &[&str]) -> Vec<Value> {
    let mut bindings = Bindings::new();
    let log = bindings.record_messages();
    bindings.apply(instance.runner());
    let command = instance.runner().get_command("status").unwrap().command;
    for args in cases {
        (command.handler)((*args).into(), instance.runner().create_command_context())
            .await
            .unwrap();
    }
    log.take()
}

/// The command registers with its usage, and the renderer under the sent custom type.
#[test]
fn message_renderer_registers_status_command_and_renderer() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        let commands = instance.runner().get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "status");
        assert_eq!(
            json!(commands[0].command.description),
            "Post a status note: /status [warn|error] <text>"
        );
        assert!(commands[0].command.get_argument_completions.is_none());
        assert!(
            instance
                .runner()
                .get_message_renderer_call::<Value>("status-update")
                .is_some()
        );
        assert!(
            instance
                .runner()
                .get_message_renderer_call::<Value>("other")
                .is_none()
        );
    });
}

/// A leading `warn` or `error` word selects the level; anything else sends the trimmed text
/// at the `info` level, and each message records a nonzero wall-clock timestamp.
#[test]
fn message_renderer_status_sends_level_defaults_and_timestamp() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        let cases = [
            ("  all   good \t", "info", "all   good"),
            ("warn  disk \n  low ", "warn", "disk low"),
            ("error", "error", "Status report"),
            ("\u{feff} error\u{a0}", "error", "Status report"),
            ("", "info", ""),
            ("warning x", "info", "warning x"),
            ("WARN x", "info", "WARN x"),
            ("warn\u{85}x", "info", "warn\u{85}x"),
        ];
        let args: Vec<&str> = cases.iter().map(|case| case.0).collect();
        let messages = sent(&instance, &args).await;
        assert_eq!(messages.len(), cases.len());
        let mut previous = 0.0;
        for (message, (_, level, content)) in messages.iter().zip(cases) {
            let input = &message["message"];
            assert_eq!(input["customType"], "status-update");
            assert_eq!(input["content"], *content);
            assert_eq!(input["display"], true);
            assert_eq!(input["details"]["level"], *level);
            assert_eq!(message["options"], Value::Null);
            let timestamp = input["details"]["timestamp"].as_f64().unwrap();
            assert!(
                timestamp > 1_700_000_000_000.0 && timestamp >= previous,
                "{timestamp}"
            );
            previous = timestamp;
        }
    });
}

/// A stored status message with the supplied content and, unless null, details.
fn stored(details: &Value, content: &Value) -> Value {
    let mut message = json!({"role":"custom", "customType":"status-update",
        "content":content, "display":true, "timestamp":1_767_323_045_678_u64});
    if !details.is_null() {
        message["details"] = details.clone();
    }
    message
}

/// Draws `message` and returns its rows.
fn drawn(instance: &Instance, message: &Value, expanded: bool) -> Vec<String> {
    render_message(instance.runner(), message, expanded, WIDTH)
        .expect("the renderer draws the message")
}

/// A message the command delivered draws its level and text, and adds the local time it was
/// sent only when expanded.
#[test]
fn message_renderer_draws_delivered_messages_collapsed_and_expanded() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        for (args, label, content) in [
            ("warn disk  low", "[WARN]", "disk low"),
            ("error", "[ERROR]", "Status report"),
            (" all good ", "[INFO]", "all good"),
        ] {
            let delivered = sent(&instance, &[args]).await;
            let mut message = delivered[0]["message"].clone();
            message["role"] = json!("custom");
            message["timestamp"] = message["details"]["timestamp"].clone();
            assert_eq!(
                drawn(&instance, &message, false),
                [format!("{label} {content}")]
            );
            let expanded = drawn(&instance, &message, true);
            assert_eq!(expanded.len(), 2);
            assert_eq!(expanded[0], format!("{label} {content}"));
            assert!(expanded[1].starts_with("sent "), "{expanded:?}");
        }
    });
}

/// Stored messages without a text level draw as `info`, text blocks join by line, and a
/// missing or zero timestamp shows no time even when expanded.
#[test]
fn message_renderer_defaults_level_and_skips_absent_timestamps() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        let blocks = json!([
            {"type":"text", "text":"first"},
            {"type":"image", "data":"AA==", "mimeType":"image/png"},
            {"type":"text", "text":"second"}
        ]);
        for (details, content, expected) in [
            (json!(null), blocks, vec!["[INFO] first", "second"]),
            (
                json!({"level":"error", "timestamp":0}),
                json!("x"),
                vec!["[ERROR] x"],
            ),
            (json!({"level":""}), json!("z"), vec!["[] z"]),
            (json!({"level":"straße"}), json!(""), vec!["[STRASSE]"]),
        ] {
            let rows = drawn(&instance, &stored(&details, &content), true);
            assert_eq!(rows, expected, "{details}");
        }
        let numeric_level = drawn(
            &instance,
            &stored(&json!({"level":7, "timestamp":0}), &json!("y")),
            true,
        );
        assert_eq!(numeric_level, ["[INFO] y"]);
    });
}

/// Any nonzero numeric timestamp shows a time, and a value past every instant shows the
/// calendar's invalid date.
#[test]
fn message_renderer_truncates_numeric_timestamps() {
    local(async {
        let instance = Instance::new(COMPONENT).await;
        for timestamp in [
            json!(1_767_323_045_678.0),
            json!(1_767_323_045_678.9),
            json!(-1.5),
            json!(0.5),
        ] {
            let rows = drawn(
                &instance,
                &stored(&json!({ "timestamp": timestamp }), &json!("x")),
                true,
            );
            assert_eq!(rows.len(), 2, "{timestamp}: {rows:?}");
            assert!(rows[1].starts_with("sent "), "{timestamp}: {rows:?}");
        }
        let beyond = drawn(
            &instance,
            &stored(&json!({ "timestamp": 1e300 }), &json!("x")),
            true,
        );
        assert_eq!(beyond, ["[INFO] x", "sent Invalid Date"]);
    });
}
