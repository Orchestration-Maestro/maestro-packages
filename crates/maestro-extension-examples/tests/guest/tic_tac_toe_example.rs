//! The shipped game loaded into the production engine over an in-memory session.
use super::lane_b::{
    Bindings, Failures, Instance, Log, Recorder, emit, replace_branch, timed, tool_result_message,
};
use super::support::{before_agent_start, call_tool};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    ExtensionRunner, Local, SessionStartEvent, SessionTreeEvent,
};
use serde_json::{Value, json};
use std::{rc::Rc, time::Duration};

/// The variable that names the built game component.
const COMPONENT: &str = "MAESTRO_TIC_TAC_TOE_COMPONENT";

/// The loaded game, the session it reads and the application operations it reached.
struct Game {
    /// The loaded component.
    instance: Instance,
    /// The custom entries the game saved.
    saves: Log,
    /// The custom messages the game sent.
    messages: Log,
    /// The failures the callbacks reported.
    failures: Failures,
}

impl Game {
    /// Loads the game with its saves written to the session, as an application writes them.
    async fn new() -> Self {
        let instance = Instance::new(COMPONENT).await;
        let mut bindings = Bindings::new();
        let saves = bindings.record_entries(instance.harness.session_manager.clone());
        let messages = bindings.record_messages();
        bindings.apply(instance.runner());
        let failures = Failures::collect(instance.runner());
        Self {
            instance,
            saves,
            messages,
            failures,
        }
    }

    /// The runner over the game.
    fn runner(&self) -> &Rc<ExtensionRunner<Local>> {
        self.instance.runner()
    }

    /// Calls a game tool.
    async fn tool(&self, name: &str, params: Value) -> Result<Value, String> {
        call_tool(self.runner(), name, params).await
    }

    /// The board the agent sees.
    async fn board(&self) -> Value {
        self.tool("tic_tac_toe_see_board", json!({})).await.unwrap()
    }

    /// Replaces the session's branch with `entries` and delivers `name` so the game rebuilds from
    /// it; returns the failure the rebuild reported.
    async fn restore(&self, name: &str, entries: &[Entry]) -> Result<(), String> {
        let branch: Vec<Value> = entries
            .iter()
            .map(|entry| match entry {
                Entry::Save(details) => json!({"customType": "tic-tac-toe-save", "data": details}),
                Entry::Result(details) => {
                    tool_result_message("call", "tic_tac_toe", Some(details.clone()))
                }
            })
            .collect();
        replace_branch(&self.instance.harness.session_manager, &branch);
        if name == "session_start" {
            emit::<SessionStartEvent>(self.runner(), &json!({"reason": "resume"})).await;
        } else {
            emit::<SessionTreeEvent>(
                self.runner(),
                &json!({"newLeafId": "new", "oldLeafId": "old"}),
            )
            .await;
        }
        self.failures.take().pop().map_or(Ok(()), Err)
    }
}

/// One thing on the session's branch.
enum Entry {
    /// An ordinary custom save.
    Save(Value),
    /// A successful game tool result carrying a snapshot.
    Result(Value),
}

/// The agent sees only its cursor; an out-of-turn play leaves the board untouched.
#[test]
fn tic_tac_toe_initial_board_and_out_of_turn_play() {
    timed(async {
        let game = Game::new().await;
        let initial = game.board().await;
        assert_eq!(
            initial["details"],
            json!({
                "board":[[" "," "," "],[" "," "," "],[" "," "," "]],
                "agentCursorRow":0,"agentCursorCol":0,"status":"playing","currentTurn":"X"
            })
        );
        assert!(
            initial["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("<[0,0]>")
        );
        assert_eq!(
            game.tool("tic_tac_toe", json!({"action":"play"})).await,
            Err("It is not your turn.".into())
        );
        assert_eq!(game.board().await, initial);
    });
}

/// Rejected session naming never enables game guidance for a later ordinary prompt.
#[test]
fn tic_tac_toe_failed_setup_leaves_prompt_unchanged() {
    timed(async {
        let game = Game::new().await;
        Recorder::install(game.runner());
        let mut bindings = Bindings::new();
        bindings.actions.set_session_name = Rc::new(|_| Err("session naming refused".into()));
        bindings.apply(game.runner());
        let command = game.runner().get_command("tic-tac-toe").unwrap().command;
        let outcome =
            (command.handler)(String::new(), game.runner().create_command_context()).await;
        assert_eq!(
            outcome.err().map(|error| error.to_string()),
            Some("session naming refused".into())
        );
        let event = json!({"type":"before_agent_start","prompt":"ordinary prompt",
            "systemPrompt":"original instructions","systemPromptOptions":{"cwd":"/work"}});
        assert_eq!(before_agent_start(game.runner(), &event).await, Value::Null);
    });
}

/// Moves await the authored delay, clamp, retain failed cursors and persist detached snapshots.
#[test]
fn tic_tac_toe_atomic_moves_restore_and_persist_on_the_branch() {
    timed(async {
        let game = Game::new().await;
        let board = json!({"board":[["X"," "," "],[" "," "," "],[" "," "," "]],"agentCursorRow":0,"agentCursorCol":0,"status":"playing","currentTurn":"O"});
        game.restore("session_start", &[Entry::Save(board.clone())])
            .await
            .unwrap();
        assert_eq!(game.board().await["details"], board);
        let refused = game
            .tool("tic_tac_toe", json!({"action":"play"}))
            .await
            .unwrap_err();
        assert!(refused.contains("already X"));
        let started = tokio::time::Instant::now();
        for (action, row, col) in [
            ("move_up", 0, 0),
            ("move_left", 0, 0),
            ("move_down", 1, 0),
            ("move_down", 2, 0),
            ("move_down", 2, 0),
            ("move_right", 2, 1),
            ("move_right", 2, 2),
            ("move_right", 2, 2),
            ("move_up", 1, 2),
        ] {
            let result = game
                .tool("tic_tac_toe", json!({"action":action}))
                .await
                .unwrap();
            assert_eq!(result["details"]["agentCursorRow"], row);
            assert_eq!(result["details"]["agentCursorCol"], col);
        }
        assert_eq!(
            started.elapsed(),
            Duration::from_millis(300) * 9,
            "one authored wait per move"
        );
        let played = game
            .tool("tic_tac_toe", json!({"action":"play"}))
            .await
            .unwrap();
        assert_eq!(
            played["details"]["board"],
            json!([["X", " ", " "], [" ", " ", "O"], [" ", " ", " "]])
        );
        assert_eq!(played["details"]["agentCursorRow"], 0);
        assert_eq!(played["details"]["agentCursorCol"], 0);
        assert_eq!(played["details"]["currentTurn"], "X");
        replays(&game, &board, &played).await;
    });
}

/// Checks that the game saved each accepted state and rebuilds only from the branch it is given.
async fn replays(game: &Game, board: &Value, played: &Value) {
    let saves: Vec<Value> = game
        .saves
        .borrow()
        .iter()
        .map(|save| save["data"].clone())
        .collect();
    assert_eq!(
        saves.len(),
        11,
        "the refused occupied play, nine moves and the play"
    );
    assert_eq!(saves[0], *board);
    assert_eq!(saves[10], played["details"]);
    game.restore(
        "session_tree",
        &[
            Entry::Save(board.clone()),
            Entry::Result(played["details"].clone()),
        ],
    )
    .await
    .unwrap();
    assert_eq!(game.board().await["details"], played["details"]);
    game.restore("session_tree", &[]).await.unwrap();
    assert_eq!(
        game.board().await["details"]["board"],
        json!([[" ", " ", " "], [" ", " ", " "], [" ", " ", " "]])
    );
}

/// Independently worked final moves covering all eight lines and a draw.
const FINISHES: [([[&str; 3]; 3], usize, usize, &str); 9] = [
    (
        [["O", "O", " "], [" ", "X", " "], ["X", " ", " "]],
        0,
        2,
        "win_O",
    ),
    (
        [["X", " ", " "], ["O", "O", " "], ["X", " ", " "]],
        1,
        2,
        "win_O",
    ),
    (
        [["X", " ", " "], [" ", "X", " "], ["O", "O", " "]],
        2,
        2,
        "win_O",
    ),
    (
        [["O", "X", " "], ["O", "X", " "], [" ", " ", " "]],
        2,
        0,
        "win_O",
    ),
    (
        [["X", "O", " "], ["X", "O", " "], [" ", " ", " "]],
        2,
        1,
        "win_O",
    ),
    (
        [["X", " ", "O"], ["X", " ", "O"], [" ", " ", " "]],
        2,
        2,
        "win_O",
    ),
    (
        [["O", "X", " "], ["X", "O", " "], [" ", " ", " "]],
        2,
        2,
        "win_O",
    ),
    (
        [["X", "X", "O"], [" ", "O", " "], [" ", " ", " "]],
        2,
        0,
        "win_O",
    ),
    (
        [["X", "O", "X"], ["X", "O", "O"], ["O", "X", " "]],
        2,
        2,
        "draw",
    ),
];

/// Every row, column and diagonal can finish, and a full board without a line draws.
#[test]
fn tic_tac_toe_finishes_wins_and_draws_once() {
    timed(async {
        let game = Game::new().await;
        for (board, row, col, status) in FINISHES {
            let details = json!({"board":board,"agentCursorRow":row,"agentCursorCol":col,"status":"playing","currentTurn":"O"});
            game.restore("session_tree", &[Entry::Save(details)])
                .await
                .unwrap();
            let before = game.messages.borrow().len();
            let result = game
                .tool("tic_tac_toe", json!({"action":"play"}))
                .await
                .unwrap();
            assert_eq!(result["details"]["status"], status);
            assert_eq!(result["details"]["agentCursorRow"], 0);
            assert_eq!(result["details"]["agentCursorCol"], 0);
            assert_eq!(
                game.tool("tic_tac_toe", json!({"action":"play"})).await,
                Err(format!("Game is over ({status})."))
            );
            let messages = game.messages.borrow();
            assert_eq!(messages.len(), before + 1);
            let last = &messages[messages.len() - 1]["message"];
            assert_eq!(last["customType"], "tic-tac-toe-game-over");
            assert_eq!(last["details"], result["details"]);
        }
    });
}

/// Malformed saves cannot reach board indexing or replace the last accepted state.
#[test]
fn tic_tac_toe_rejects_invalid_snapshots_without_replacing_state() {
    timed(async {
        let game = Game::new().await;
        let initial = game.board().await;
        for (field, value) in [
            ("agentCursorRow", json!(3)),
            ("agentCursorCol", json!(9)),
            ("currentTurn", json!("Z")),
            ("status", json!("unknown")),
            (
                "board",
                json!([["Z", " ", " "], [" ", " ", " "], [" ", " ", " "]]),
            ),
        ] {
            let mut invalid = initial["details"].clone();
            invalid[field] = value;
            assert!(
                game.restore("session_tree", &[Entry::Save(invalid)])
                    .await
                    .is_err()
            );
            assert_eq!(game.board().await, initial);
        }
        assert_eq!(
            game.tool("tic_tac_toe", json!({"action":"jump"})).await,
            Err("Unknown action: jump".into())
        );
    });
}
