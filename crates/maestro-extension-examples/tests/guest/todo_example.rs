//! The shipped todo component loaded into the production engine.
use super::support::{Shown, Failures, Instance, Recorder, call_tool, driven, emit, render_call, render_result, replace_branch, tool_result_message};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    ExtensionRunner, Local, SessionStartEvent, SessionTreeEvent,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// The variable that names the built todo component.
const COMPONENT: &str = "MAESTRO_TODO_COMPONENT";

/// One loaded todo component.
struct Todos {
    /// The loaded component.
    instance: Instance,
    /// The failures its callbacks reported.
    failures: Failures,
}

impl Todos {
    /// Loads a fresh component.
    async fn new() -> Self {
        let instance = Instance::new(COMPONENT).await;
        let failures = Failures::collect(instance.runner());
        Self { instance, failures }
    }

    /// The runner over the component.
    fn runner(&self) -> &Rc<ExtensionRunner<Local>> {
        self.instance.runner()
    }

    /// Calls the tool and returns its whole result.
    async fn tool(&self, params: Value) -> Value {
        call_tool(self.runner(), "todo", params).await.unwrap()
    }

    /// Replaces the session's branch with `entries` and delivers `name`.
    async fn event(&self, name: &str, entries: &[Value]) {
        replace_branch(&self.instance.harness.session_manager, entries);
        if name == "session_start" {
            emit::<SessionStartEvent>(self.runner(), &json!({"reason": "resume"})).await;
        } else {
            emit::<SessionTreeEvent>(
                self.runner(),
                &json!({"newLeafId": "leaf", "oldLeafId": "old"}),
            )
            .await;
        }
        assert_eq!(self.failures.take(), Vec::<String>::new());
    }

    /// The call row for `args`.
    fn call_row(&self, args: &Value) -> Vec<String> {
        render_call(self.runner(), "todo", args, 200)
    }

    /// The result row for `result`.
    fn result_row(&self, result: &Value, expanded: bool) -> Vec<String> {
        render_result(
            self.runner(),
            "todo",
            &Shown {
                expanded,
                ..Shown::new(&Value::Null, result)
            },
        )
    }
}

/// Checks both visible text and the independently saved state, including omission of errors.
fn answer(actual: &Value, text: &str, details: &Value) {
    assert_eq!(
        actual,
        &json!({"content":[{"type":"text","text":text}],"details":details})
    );
}

/// Every action preserves authored text, returns snapshots and resets identifiers on clear.
#[test]
fn todo_actions_keep_snapshots_and_validate_identifiers() {
    driven(async {
        let todos = Todos::new().await;
        answer(
            &todos.tool(json!({"action":"list"})).await,
            "No todos",
            &json!({"action":"list","todos":[],"nextId":1.0}),
        );
        for params in [json!({"action":"add"}), json!({"action":"add","text":""})] {
            answer(
                &todos.tool(params).await,
                "Error: text required for add",
                &json!({"action":"add","todos":[],"nextId":1.0,"error":"text required"}),
            );
        }
        let added = todos
            .tool(json!({"action":"add","text":"  ship 雪  "}))
            .await;
        let open = json!([{"id":1.0,"text":"  ship 雪  ","done":false}]);
        answer(
            &added,
            "Added todo #1:   ship 雪  ",
            &json!({"action":"add","todos":open,"nextId":2.0}),
        );
        answer(
            &todos.tool(json!({"action":"toggle"})).await,
            "Error: id required for toggle",
            &json!({"action":"toggle","todos":open,"nextId":2.0,"error":"id required"}),
        );
        for id in [-1.0, 0.0, 1.5, 99.0] {
            answer(
                &todos.tool(json!({"action":"toggle","id":id})).await,
                &format!("Todo #{id} not found"),
                &json!({"action":"toggle","todos":open,"nextId":2.0,"error":format!("#{id} not found")}),
            );
        }
        completed_and_clear(&todos, &added, &open).await;
    });
}

/// Completion does not change earlier snapshots, and clear restarts identifiers.
async fn completed_and_clear(todos: &Todos, added: &Value, open: &Value) {
    let done = json!([{"id":1.0,"text":"  ship 雪  ","done":true}]);
    answer(
        &todos.tool(json!({"action":"toggle","id":1})).await,
        "Todo #1 completed",
        &json!({"action":"toggle","todos":done,"nextId":2.0}),
    );
    answer(
        &todos.tool(json!({"action":"list"})).await,
        "[x] #1:   ship 雪  ",
        &json!({"action":"list","todos":done,"nextId":2.0}),
    );
    assert_eq!(
        &added["details"]["todos"], open,
        "earlier result remains detached after toggle"
    );
    answer(
        &todos.tool(json!({"action":"toggle","id":1.0})).await,
        "Todo #1 uncompleted",
        &json!({"action":"toggle","todos":open,"nextId":2.0}),
    );
    answer(
        &todos.tool(json!({"action":"other"})).await,
        "Unknown action: other",
        &json!({"action":"list","todos":open,"nextId":2.0,"error":"unknown action: other"}),
    );
    answer(
        &todos.tool(json!({"action":"clear"})).await,
        "Cleared 1 todos",
        &json!({"action":"clear","todos":[],"nextId":1.0}),
    );
    answer(
        &todos.tool(json!({"action":"add","text":" "})).await,
        "Added todo #1:  ",
        &json!({"action":"add","todos":[{"id":1.0,"text":" ","done":false}],"nextId":2.0}),
    );
}

/// Session start and tree navigation replace local state with the last current-branch snapshot.
#[test]
fn todo_restores_only_current_branch_on_session_and_tree_changes() {
    driven(async {
        let todos = Todos::new().await;
        todos
            .tool(json!({"action":"add","text":"local only"}))
            .await;
        let first =
            json!({"action":"add","todos":[{"id":1.0,"text":"first","done":false}],"nextId":2.0});
        let last = json!({"action":"toggle","todos":[{"id":7.0,"text":"latest","done":true}],"nextId":8.0});
        let branch = [
            json!({"customType":"todo","data":last}),
            tool_result_message("one", "todo", Some(first.clone())),
            json!({"role":"user","content":"not state","timestamp":1}),
            tool_result_message("two", "todo", Some(last.clone())),
            tool_result_message("foreign", "other", Some(first.clone())),
            tool_result_message("no-details", "todo", None),
        ];
        todos.event("session_start", &branch).await;
        answer(
            &todos.tool(json!({"action":"list"})).await,
            "[x] #7: latest",
            &json!({"action":"list","todos":last["todos"],"nextId":8.0}),
        );
        let added = todos.tool(json!({"action":"add","text":"next"})).await;
        assert_eq!(added["content"][0]["text"], "Added todo #8: next");
        todos
            .event(
                "session_tree",
                &[tool_result_message("old", "todo", Some(first.clone()))],
            )
            .await;
        answer(
            &todos.tool(json!({"action":"list"})).await,
            "[ ] #1: first",
            &json!({"action":"list","todos":first["todos"],"nextId":2.0}),
        );
        todos.event("session_tree", &[]).await;
        answer(
            &todos.tool(json!({"action":"list"})).await,
            "No todos",
            &json!({"action":"list","todos":[],"nextId":1.0}),
        );
        let cleared = json!({"action":"clear","todos":[],"nextId":1.0});
        todos
            .event(
                "session_start",
                &[tool_result_message("clear", "todo", Some(cleared))],
            )
            .await;
        assert_eq!(
            todos.tool(json!({"action":"add","text":"restarted"})).await["content"][0]["text"],
            "Added todo #1: restarted"
        );
    });
}

/// Expanded lists retain every item; collapsed lists retain five and a remainder.
async fn rendered_list(todos: &Todos) {
    for _ in 0..5 {
        todos.tool(json!({"action":"add","text":"task"})).await;
    }
    let list = todos.tool(json!({"action":"list"})).await;
    let first_five = [
        "6 todo(s):",
        "✓ #1 task",
        "○ #2 task",
        "○ #3 task",
        "○ #4 task",
        "○ #5 task",
    ];
    let mut collapsed = first_five.to_vec();
    collapsed.push("... 1 more");
    assert_eq!(todos.result_row(&list, false), collapsed);
    let mut expanded = first_five.to_vec();
    expanded.push("○ #6 task");
    assert_eq!(todos.result_row(&list, true), expanded);
}

/// Call rows, all result actions, errors, fallback content and collapsed lists keep source text.
#[test]
fn todo_renderers_preserve_call_and_result_output() {
    driven(async {
        let todos = Todos::new().await;
        assert_eq!(
            todos.call_row(&json!({"action":"add","text":"task","id":2})),
            ["todo add \"task\" #2"]
        );
        assert_eq!(todos.call_row(&json!(null)), ["todo"]);
        assert_eq!(
            todos.result_row(&json!({"content":[{"type":"text","text":"raw"}]}), false),
            ["raw"]
        );
        assert!(
            todos
                .result_row(&json!({"content":[],"details":null}), false)
                .is_empty()
        );
        let empty = todos.tool(json!({"action":"list"})).await;
        assert_eq!(todos.result_row(&empty, false), ["No todos"]);
        let error = todos.tool(json!({"action":"add"})).await;
        assert_eq!(todos.result_row(&error, false), ["Error: text required"]);
        let added = todos.tool(json!({"action":"add","text":"task"})).await;
        assert_eq!(todos.result_row(&added, false), ["✓ Added #1 task"]);
        let toggled = todos.tool(json!({"action":"toggle","id":1})).await;
        assert_eq!(todos.result_row(&toggled, true), ["✓ Todo #1 completed"]);
        rendered_list(&todos).await;
        let cleared = todos.tool(json!({"action":"clear"})).await;
        assert_eq!(todos.result_row(&cleared, false), ["✓ Cleared all todos"]);
    });
}

/// The list command registers with its description and returns without a view when no UI is
/// installed.
#[test]
fn todo_command_returns_without_a_view_when_headless() {
    driven(async {
        let todos = Todos::new().await;
        let commands = todos.runner().get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "todos");
        assert_eq!(
            json!(commands[0].command.description),
            "Show all todos on the current branch"
        );
        assert!(commands[0].command.get_argument_completions.is_none());
        let command = todos.runner().get_command("todos").unwrap().command;
        (command.handler)("ignored".into(), todos.runner().create_command_context())
            .await
            .unwrap();
    });
}

/// Starts the list command and returns once its view is shown.
async fn open_list(
    todos: &Todos,
    ui: &Recorder,
) -> (usize, tokio::task::JoinHandle<Result<(), String>>) {
    let runner = todos.runner().clone();
    let opened = ui.opened();
    let view = tokio::task::spawn_local(async move {
        let command = runner.get_command("todos").unwrap().command;
        (command.handler)(String::new(), runner.create_command_context())
            .await
            .map_err(|error| error.to_string())
    });
    ui.shown(opened + 1).await;
    (opened, view)
}

/// An interactive list shows the snapshot, ignores other keys and closes on either close key.
#[test]
fn todo_command_shows_interactive_lists() {
    driven(async {
        let todos = Todos::new().await;
        let ui = Recorder::install_on(todos.runner(), (80, 24));
        for (populated, close) in [(false, "\x1b"), (true, "\x03")] {
            if populated {
                todos.tool(json!({"action":"add","text":"done"})).await;
                todos.tool(json!({"action":"toggle","id":1})).await;
                todos.tool(json!({"action":"add","text":"open"})).await;
            }
            let (opened, view) = open_list(&todos, &ui).await;
            let text = ui.rows(opened, 80).join("\n");
            assert!(text.contains("Todos"), "{text}");
            assert!(text.contains("Press Escape to close"), "{text}");
            let expected: &[&str] = if populated {
                &["1/2 completed", "✓ #1 done", "○ #2 open"]
            } else {
                &["No todos yet. Ask the agent to add some!"]
            };
            for part in expected {
                assert!(text.contains(part), "{text}");
            }
            ui.press(opened, "x");
            assert!(!view.is_finished(), "an unrelated key keeps the view open");
            ui.press(opened, close);
            view.await.unwrap().unwrap();
        }
    });
}

/// A refused view surfaces the application's failure.
#[test]
fn todo_command_reports_a_refused_view() {
    driven(async {
        let todos = Todos::new().await;
        let ui = Recorder::install_on(todos.runner(), (80, 24));
        ui.reject("custom", "view refused");
        let command = todos.runner().get_command("todos").unwrap().command;
        let refused =
            (command.handler)(String::new(), todos.runner().create_command_context()).await;
        assert_eq!(
            refused.err().map(|error| error.to_string()),
            Some("view refused".into())
        );
    });
}

/// Numeric identifiers keep the JSON boundary's exponent and signed-zero display.
#[test]
fn todo_identifier_text_uses_json_number_format() {
    driven(async {
        let todos = Todos::new().await;
        for (id, text) in [(1e21, "1e+21"), (1e-7, "1e-7"), (-0.0, "0")] {
            let params = json!({"action":"toggle","id":id});
            answer(
                &todos.tool(params.clone()).await,
                &format!("Todo #{text} not found"),
                &json!({"action":"toggle","todos":[],"nextId":1.0,"error":format!("#{text} not found")}),
            );
            assert_eq!(todos.call_row(&params), [format!("todo toggle #{text}")]);
        }
    });
}
