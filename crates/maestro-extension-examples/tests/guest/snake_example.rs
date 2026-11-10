//! The shipped Snake command loaded into the production engine, driven through the terminal
//! input its view reads.
use super::support::{Bindings, Instance, Log, Recorder, replace_branch, stepped};
use maestro_extensions_wasmtime::test_support::maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::rc::Rc;

/// The variable that names the built game component.
const COMPONENT: &str = "MAESTRO_SNAKE_COMPONENT";

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
        let ui = Recorder::install_on(self.runner(), (width.max(1), 40));
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

    /// Delivers terminal input to the view.
    fn press(&self, data: &str) {
        self.ui.press(0, data);
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
            view.press(key);
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
            view.press("q");
            view.finished().await.unwrap();
            assert!(game.saved().is_empty(), "{width}");
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
            view.press("\x1b");
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
            Recorder::install_on(game.runner(), (84, 40));
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

/// Presses `key` in a running game and checks the turn it queues: the saved game carries it.
async fn turns(key: &str, direction: &str) {
    let start = if direction == "down" {
        state(&[(20, 7), (19, 7), (18, 7)], (0, 0), "right", 0)
    } else {
        state(&[(20, 7), (20, 8), (20, 9)], (0, 0), "up", 0)
    };
    let mut queued = start.clone();
    queued["nextDirection"] = json!(direction);
    let game = Game::new(std::slice::from_ref(&start)).await;
    let view = game.open(84).await;
    view.press("x");
    view.press(key);
    view.press("\x1b");
    view.finished().await.unwrap();
    assert_eq!(game.saved(), [queued], "{key:?}");
}

/// Both arrow keys and either letter case queue their turn.
#[test]
fn snake_controls_apply_arrow_and_letter_turns() {
    stepped(async {
        let cases = [
            (["w", "W", "\x1b[A"], "up"),
            (["s", "S", "\x1b[B"], "down"),
            (["a", "A", "\x1b[D"], "left"),
            (["d", "D", "\x1b[C"], "right"),
        ];
        for (keys, direction) in cases {
            for key in keys {
                turns(key, direction).await;
            }
        }
    });
}
