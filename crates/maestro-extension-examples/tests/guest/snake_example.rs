//! The shipped Snake command loaded into the production engine, driven through the terminal
//! input its view reads and the clock its ticks wait on.
use super::lane_b::{Bindings, Instance, Log, Recorder, replace_branch, stepped};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::{rc::Rc, time::Duration};

/// The variable that names the built game component.
const COMPONENT: &str = "MAESTRO_SNAKE_COMPONENT";

/// How many scheduling rounds a test gives the game to react before it reports a mismatch.
const DRAIN: usize = 3000;

/// How many scheduling rounds pass between two comparisons of the view.
const ROUND: usize = 20;

/// The wait between two ticks.
const TICK: Duration = Duration::from_millis(100);

/// Counts visible characters in the source's literal terminal styles.
fn visible(text: &str) -> usize {
    regex::Regex::new("\x1b\\[[0-9;]*m")
        .unwrap()
        .replace_all(text, "")
        .chars()
        .count()
}

/// A dim terminal border with the source's exact reset sequence.
fn dim(text: &str) -> String {
    format!("\x1b[2m{text}\x1b[22m")
}

/// Expected board rows from literal positions, independent of movement logic.
fn grid(game: &Value, columns: usize) -> Vec<String> {
    let snake = game["snake"].as_array().unwrap();
    (0..15)
        .map(|y| {
            let mut row = dim(" │");
            for x in 0..columns {
                let p = json!({"x":x,"y":y});
                row.push_str(if snake[0] == p {
                    "\x1b[32m██\x1b[0m"
                } else if snake[1..].contains(&p) {
                    "\x1b[32m▓▓\x1b[0m"
                } else if game["food"] == p {
                    "\x1b[31m◆ \x1b[0m"
                } else {
                    "  "
                });
            }
            row + &dim("│")
        })
        .collect()
}

/// Independently specified source output, including literal ANSI styles and footer wording.
fn frame(game: &Value, mode: &str, width: usize) -> Vec<String> {
    let columns = 40.min(width.saturating_sub(4) / 2);
    let size = columns * 2;
    let pad = |s: String| {
        let count = visible(&s);
        s + &" ".repeat(width.saturating_sub(count))
    };
    let boxed =
        |s: String| pad(dim(" │") + &s + &" ".repeat(size.saturating_sub(visible(&s))) + &dim("│"));
    let border = |l, r| pad(dim(&format!(" {l}{}{r}", "─".repeat(size))));
    let mut rows = vec![
        border("╭", "╮"),
        boxed(format!(
            "\x1b[1m\x1b[32mSNAKE\x1b[0m\x1b[22m │ Score: \x1b[1m\x1b[33m{}\x1b[0m\x1b[22m │ High: \x1b[1m\x1b[33m{}\x1b[0m\x1b[22m",
            game["score"], game["highScore"]
        )),
        border("├", "┤"),
    ];
    rows.extend(grid(game, columns).into_iter().map(pad));
    rows.push(border("├", "┤"));
    rows.push(boxed(match mode {
        "paused" => "\x1b[33m\x1b[1mPAUSED\x1b[22m\x1b[0m Press any key to continue, \x1b[1mQ\x1b[22m to quit",
        "over" => "\x1b[31m\x1b[1mGAME OVER!\x1b[22m\x1b[0m Press \x1b[1mR\x1b[22m to restart, \x1b[1mQ\x1b[22m to quit",
        _ => "↑↓←→ or WASD to move, \x1b[1mESC\x1b[22m pause, \x1b[1mQ\x1b[22m quit",
    }.into()));
    rows.push(border("╰", "╯"));
    rows
}

/// A worked saved game in the authored JSON format.
fn state(snake: &[(i32, i32)], food: (i32, i32), direction: &str, score: u64) -> Value {
    let point = |(x, y)| json!({"x":x,"y":y});
    json!({"snake":snake.iter().copied().map(point).collect::<Vec<_>>(), "food":point(food),
        "direction":direction,"nextDirection":direction,"score":score,"highScore":20,"gameOver":false})
}

/// A matching custom session entry, including an explicit null clearing entry.
fn entry(value: &Value) -> Value {
    json!({"customType":"snake-save","data":value})
}

/// A saved game without its food, which the game places at random.
fn without_food(game: &Value) -> Value {
    let mut game = game.clone();
    game.as_object_mut().unwrap().shift_remove("food");
    game
}

/// The loaded game, the saves it wrote and the failures it reported.
struct Game {
    /// The loaded component.
    instance: Instance,
    /// The custom entries the game saved.
    saves: Log,
}

impl Game {
    /// Loads the game over `history`.
    async fn new(history: &[Value]) -> Self {
        let instance = Instance::new(COMPONENT).await;
        let mut bindings = Bindings::new();
        let saves = bindings.record_entries(instance.harness.session_manager.clone());
        bindings.apply(instance.runner());
        let entries: Vec<Value> = history.iter().map(entry).collect();
        replace_branch(&instance.harness.session_manager, &entries);
        Self { instance, saves }
    }

    /// The runner over the game.
    fn runner(&self) -> &Rc<ExtensionRunner<Local>> {
        self.instance.runner()
    }

    /// The values the game saved, oldest first.
    fn saved(&self) -> Vec<Value> {
        self.saves
            .borrow()
            .iter()
            .map(|save| save["data"].clone())
            .collect()
    }

    /// Starts the command on a terminal `width` columns wide and waits for its view.
    async fn open(&self, width: usize) -> View {
        let ui = Recorder::install_interactive(self.runner(), width.max(1), 40);
        let runner = self.runner().clone();
        let command = tokio::task::spawn_local(async move {
            let command = runner.get_command("snake").unwrap().command;
            (command.handler)("ignored".into(), runner.create_command_context())
                .await
                .map_err(|error| error.to_string())
        });
        ui.shown(1).await;
        View { ui, command, width }
    }
}

/// Gives every task that can run the chance to, so a timer the game arms in reaction to input is
/// armed before the clock moves.
async fn settle() {
    rounds(DRAIN).await;
}

/// Lets every task that can run take `count` turns.
async fn rounds(count: usize) {
    for _ in 0..count {
        tokio::task::yield_now().await;
    }
}

/// The running command and its view.
struct View {
    /// The UI the view is shown on.
    ui: Rc<Recorder>,
    /// The running command.
    command: tokio::task::JoinHandle<Result<(), String>>,
    /// The width the view is drawn at.
    width: usize,
}

impl View {
    /// The view's rows, with their control sequences.
    fn rows(&self) -> Vec<String> {
        self.ui.component(0).try_render(self.width).unwrap()
    }

    /// Delivers terminal input and lets the game react to it.
    async fn press(&self, data: &str) {
        self.ui.press(0, data);
        settle().await;
    }

    /// Lets every task that can run do so, then asserts the view shows `expected`.
    async fn shows(&self, expected: &[String]) {
        for _ in 0..DRAIN / ROUND {
            if self.rows() == expected {
                return;
            }
            rounds(ROUND).await;
        }
        assert_eq!(self.rows(), expected);
    }

    /// Lets one tick's wait end and waits for the view to show `expected`.
    async fn tick_to(&self, expected: &[String]) {
        tokio::time::advance(TICK).await;
        self.shows(expected).await;
        settle().await;
    }

    /// Lets one tick's wait end and waits for the view to change.
    async fn tick_changes(&self) {
        let before = self.rows();
        tokio::time::advance(TICK).await;
        for _ in 0..DRAIN / ROUND {
            if self.rows() != before {
                settle().await;
                return;
            }
            rounds(ROUND).await;
        }
        assert_ne!(self.rows(), before, "the tick changed nothing");
    }

    /// Waits for the command to end.
    async fn finished(self) -> Result<(), String> {
        self.command.await.unwrap()
    }
}

/// Headless use returns without a view and without a save.
#[test]
fn snake_headless_command_requires_interactive_mode() {
    stepped(async {
        let game = Game::new(&[]).await;
        let commands = game.runner().get_registered_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.name, "snake");
        assert_eq!(json!(commands[0].command.description), "Play Snake!");
        let command = game.runner().get_command("snake").unwrap().command;
        (command.handler)("ignored".into(), game.runner().create_command_context())
            .await
            .unwrap();
        assert!(game.saved().is_empty());
    });
}

/// Restoring starts paused; quitting while paused leaves the latest save untouched.
#[test]
fn snake_restores_latest_save_paused_and_disposes_without_clearing() {
    stepped(async {
        for (key, width) in [("\x1b", 84), ("q", 20), ("Q", 0)] {
            let saved = state(&[(20, 7), (19, 7), (18, 7)], (21, 7), "right", 10);
            let game = Game::new(&[Value::Null, saved.clone()]).await;
            let view = game.open(width).await;
            let rows = view.rows();
            if width == 84 {
                assert_eq!(rows, frame(&saved, "paused", width), "{key:?} at {width}");
            }
            assert!(
                rows.iter().all(|row| visible(row) <= width),
                "{key:?} at {width}"
            );
            view.press(key).await;
            let ui = view.ui.clone();
            view.finished().await.unwrap();
            drop(ui);
            assert!(game.saved().is_empty());
        }
    });
}

/// Narrow frames fit the terminal and keep the board's height.
#[test]
fn snake_narrow_frames_fit_without_changing_collision_boundaries() {
    stepped(async {
        let saved = state(&[(38, 7), (37, 7), (36, 7)], (0, 0), "right", 10);
        for width in [0, 1, 2, 3, 20, 40, 42, 43, 45, 46, 84] {
            let game = Game::new(std::slice::from_ref(&saved)).await;
            let view = game.open(width).await;
            let rows = view.rows();
            assert_eq!(rows.len(), frame(&saved, "paused", width).len(), "{width}");
            for row in &rows {
                assert!(visible(row) <= width, "{width}: {row:?}");
            }
            view.press("q").await;
            view.finished().await.unwrap();
            assert!(game.saved().is_empty(), "{width}");
        }
    });
}

/// A snake at the wall dies on the next tick at the full board's width.
#[test]
fn snake_wall_collision_ends_the_game_and_clears_the_save() {
    stepped(async {
        let start = state(&[(38, 7), (37, 7), (36, 7)], (0, 0), "right", 10);
        let moved = state(&[(39, 7), (38, 7), (37, 7)], (0, 0), "right", 10);
        let mut over = moved.clone();
        over["gameOver"] = json!(true);
        let game = Game::new(std::slice::from_ref(&start)).await;
        let view = game.open(84).await;
        assert_eq!(view.rows(), frame(&start, "paused", 84));
        view.press("x").await;
        view.tick_to(&frame(&moved, "playing", 84)).await;
        view.tick_to(&frame(&over, "over", 84)).await;
        view.press("q").await;
        view.finished().await.unwrap();
        assert_eq!(game.saved(), [Value::Null]);
    });
}

/// Tick completions grow the body, apply turns, reject reversal and persist the moved state.
#[test]
fn snake_ticks_move_grow_turn_and_save_through_session_entries() {
    stepped(async {
        let start = state(&[(20, 7), (19, 7), (18, 7)], (21, 7), "right", 0);
        let game = Game::new(std::slice::from_ref(&start)).await;
        let view = game.open(84).await;
        view.press("x").await;
        view.tick_changes().await;
        assert!(
            view.rows()[1].contains("Score: \x1b[1m\x1b[33m10"),
            "the food was eaten"
        );
        let queued = state(&[(21, 6), (21, 7), (20, 7), (19, 7)], (0, 0), "up", 10);
        view.press("W").await;
        view.tick_changes().await;
        view.press("\x1b[B").await;
        view.press("a").await;
        view.press("\x1b").await;
        view.finished().await.unwrap();
        let saved = game.saved();
        assert_eq!(saved.len(), 1);
        let mut expected = queued;
        expected["nextDirection"] = json!("left");
        assert_eq!(without_food(&saved[0]), without_food(&expected));
        let food = &saved[0]["food"];
        let (x, y) = (food["x"].as_i64().unwrap(), food["y"].as_i64().unwrap());
        assert!(
            (0..40).contains(&x) && (0..15).contains(&y),
            "the food lies on the board"
        );
        assert!(
            !saved[0]["snake"].as_array().unwrap().contains(food),
            "the food avoids the body"
        );
    });
}

/// Wall and self collisions stop movement; restarting retains the high score and clears the save.
#[test]
fn snake_collisions_restart_and_clear_saved_state() {
    stepped(async {
        for (snake, direction) in [
            (vec![(39, 7), (38, 7), (37, 7)], "right"),
            (vec![(20, 7), (20, 8), (21, 8), (21, 7), (22, 7)], "right"),
        ] {
            let paused = state(&snake, (5, 5), direction, 10);
            let mut over = paused.clone();
            over["gameOver"] = json!(true);
            let game = Game::new(std::slice::from_ref(&paused)).await;
            let view = game.open(84).await;
            view.press("x").await;
            view.tick_to(&frame(&over, "over", 84)).await;
            view.press("R").await;
            let fresh = state(&[(20, 7), (19, 7), (18, 7)], (0, 0), "right", 0);
            let header = &frame(&fresh, "playing", 84)[1];
            assert_eq!(
                &view.rows()[1],
                header,
                "the score restarts and the high score stays"
            );
            assert!(
                view.rows()[19].contains("ESC"),
                "a restarted game is running"
            );
            view.press("q").await;
            view.finished().await.unwrap();
            assert_eq!(game.saved(), [Value::Null, Value::Null]);
        }
    });
}

/// Null saves start fresh; finished saves start fresh with their high score.
#[test]
fn snake_new_games_and_completed_saves_start_running() {
    stepped(async {
        let mut over = state(&[(1, 1)], (2, 1), "right", 10);
        over["gameOver"] = json!(true);
        for history in [vec![], vec![over.clone(), Value::Null], vec![over.clone()]] {
            let high = if history.len() == 1 { 20 } else { 0 };
            let game = Game::new(&history).await;
            let view = game.open(84).await;
            assert!(
                view.rows().iter().any(|row| row.contains("ESC")),
                "a new game is running"
            );
            view.press("\x1b").await;
            view.finished().await.unwrap();
            let saved = game.saved();
            assert_eq!(saved.len(), 1);
            let mut expected = state(&[(20, 7), (19, 7), (18, 7)], (0, 0), "right", 0);
            expected["highScore"] = json!(high);
            assert_eq!(without_food(&saved[0]), without_food(&expected));
        }
    });
}

/// Invalid persisted coordinates are rejected before the view opens.
#[test]
fn snake_rejects_invalid_saved_positions() {
    stepped(async {
        for snake in [vec![], vec![(-1, 0)], vec![(40, 14)], vec![(0, 15)]] {
            let game = Game::new(&[state(&snake, (5, 5), "right", 0)]).await;
            Recorder::install_interactive(game.runner(), 84, 40);
            let command = game.runner().get_command("snake").unwrap().command;
            let outcome =
                (command.handler)("ignored".into(), game.runner().create_command_context()).await;
            assert_eq!(
                outcome.err().map(|error| error.to_string()),
                Some("Invalid Snake save".into())
            );
            assert!(game.saved().is_empty());
        }
    });
}

/// The last food completes a full board instead of retrying an impossible random spawn.
#[test]
fn snake_filled_board_ends_without_spawning_food() {
    stepped(async {
        let mut snake = vec![(38, 14)];
        snake.extend(
            (0..15)
                .flat_map(|y| (0..40).map(move |x| (x, y)))
                .filter(|p| ![(38, 14), (39, 14)].contains(p)),
        );
        let start = state(&snake, (39, 14), "right", 20);
        snake.insert(0, (39, 14));
        let mut over = state(&snake, (39, 14), "right", 30);
        over["gameOver"] = json!(true);
        over["highScore"] = json!(30);
        let game = Game::new(&[start]).await;
        let view = game.open(84).await;
        view.press("x").await;
        view.tick_to(&frame(&over, "over", 84)).await;
        view.press("\x1b").await;
        view.finished().await.unwrap();
        assert_eq!(game.saved(), [over]);
    });
}

/// Presses `key` before a tick and checks the turn it makes at the next tick boundary.
async fn turns(key: &str, direction: &str, head: (i32, i32)) {
    let start = if direction == "down" {
        state(&[(20, 7), (19, 7), (18, 7)], (0, 0), "right", 0)
    } else {
        state(&[(20, 7), (20, 8), (20, 9)], (0, 0), "up", 0)
    };
    let body = start["snake"][1].clone();
    let mut moved = state(&[head, (20, 7), (0, 0)], (0, 0), direction, 0);
    moved["snake"][2] = body;
    let game = Game::new(std::slice::from_ref(&start)).await;
    let view = game.open(84).await;
    view.press("x").await;
    view.press(key).await;
    view.tick_to(&frame(&moved, "playing", 84)).await;
    view.press("\x1b").await;
    view.finished().await.unwrap();
    assert_eq!(game.saved(), [moved], "{key:?}");
}

/// Both arrow keys and either letter case turn only at the next tick boundary.
#[test]
fn snake_controls_apply_arrow_and_letter_turns() {
    stepped(async {
        let cases = [
            (["w", "W", "\x1b[A"], "up", (20, 6)),
            (["s", "S", "\x1b[B"], "down", (20, 8)),
            (["a", "A", "\x1b[D"], "left", (19, 7)),
            (["d", "D", "\x1b[C"], "right", (21, 7)),
        ];
        for (keys, direction, head) in cases {
            for key in keys {
                turns(key, direction, head).await;
            }
        }
    });
}

/// The initial food sampler never hides the food inside the starting body.
#[test]
fn snake_initial_food_avoids_the_whole_body() {
    stepped(async {
        for _ in 0..20 {
            let game = Game::new(&[]).await;
            let view = game.open(84).await;
            let rows = view.rows().join("\n");
            assert_eq!(rows.matches('◆').count(), 1, "one food cell is visible");
            view.press("q").await;
            view.finished().await.unwrap();
            assert_eq!(game.saved(), [Value::Null]);
        }
    });
}
