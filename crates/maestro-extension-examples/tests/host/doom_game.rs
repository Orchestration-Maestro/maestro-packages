//! The overlay playing Freedoom: its frames, menu, retained game and private files.
#![cfg(test)]
use super::doom_wad::{Demo, Scenario, checked, controlled_io};

mod level;
use maestro_extensions::{EventError, types::ExtensionComponent};
use std::path::Path;

/// A terminal width that gives the overlay a typical view.
const WIDTH: usize = 80;
/// One terminal cell of black.
const BLACK_CELL: &str = "\x1b[38;2;0;0;0m\x1b[48;2;0;0;0m▀";

/// The IWAD the test preparation fetched and checked.
pub(super) fn freedoom() -> String {
    std::env::var("MAESTRO_FREEDOOM_WAD")
        .expect("MAESTRO_FREEDOOM_WAD is unset: run just prepare-tests, then cargo nextest")
}

/// Renders the view as terminal rows.
fn rows(built: &ExtensionComponent) -> Result<Vec<String>, EventError> {
    built
        .component
        .try_render(WIDTH)
        .map_err(|error| EventError::from(error.to_string()))
}

/// Sends one key to the view.
pub(super) fn key(built: &ExtensionComponent, data: &str) -> Result<(), EventError> {
    built
        .component
        .input_handler()
        .unwrap()
        .try_handle_input(data)
        .map_err(|error| EventError::from(error.to_string()))
}

/// Whether every game cell of the frame is black; the last row is the controls footer.
fn all_black(rows: &[String]) -> bool {
    rows[..rows.len() - 1]
        .iter()
        .all(|row| row.replace(BLACK_CELL, "") == "\x1b[0m")
}

/// The private roots the overlay created under the controlled temporary directory.
fn roots(root: &Path) -> Vec<std::path::PathBuf> {
    let mut roots: Vec<_> = std::fs::read_dir(root.join("temporary"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    roots.sort();
    roots
}

/// A view that checks the opening frame against the title screen and the menu.
fn title_then_menu() -> Scenario {
    Box::new(|built| {
        let title = rows(built)?;
        assert!(!all_black(&title), "the title screen is drawn at once");
        key(built, "\x1b")?;
        assert_ne!(title, rows(built)?, "Escape opens the menu over the title");
        Ok(())
    })
}

/// Opens the game on Freedoom and returns the loaded demo.
async fn started(root: &Path, scenario: Scenario) -> Demo {
    let demo = super::doom_wad::loaded(root, controlled_io("none"), false).await;
    demo.view(scenario);
    demo.command(&freedoom()).await;
    demo
}

/// Runs a real-engine scenario; returns whether the case is one of these.
pub(super) async fn run(root: &Path, case: &str) -> bool {
    if level::run(root, case).await {
        return true;
    }
    match case {
        "title-and-menu" => {
            let demo = started(root, title_then_menu()).await;
            demo.shutdown().await;
        }
        "private-root" => private_root(root).await,
        "resume" => resume(root).await,
        "rediscover" => rediscover(root).await,
        "function-keys" => function_keys(root).await,
        "switch" => switch(root).await,
        "quit" => quit(root).await,
        _ => return false,
    }
    true
}

/// The engine's files live in one root that shutdown removes.
async fn private_root(root: &Path) {
    let temporary = root.join("temporary");
    let expected = std::fs::read(freedoom()).unwrap();
    let inside = temporary.clone();
    let demo = started(
        root,
        Box::new(move |_| {
            let entries: Vec<_> = std::fs::read_dir(&inside)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            assert_eq!(entries.len(), 1, "one root holds the game");
            assert_eq!(
                std::fs::read(entries[0].join("doom1.wad")).unwrap(),
                expected
            );
            Ok(())
        }),
    )
    .await;
    assert_eq!(roots(root).len(), 1, "the root outlives the view");
    demo.shutdown().await;
    assert!(roots(root).is_empty(), "shutdown removes the root");
}

/// Opening the same WAD again resumes the same game instead of starting another.
async fn resume(root: &Path) {
    let demo = started(root, Box::new(|_| Ok(()))).await;
    let first = roots(root);
    demo.view(Box::new(|_| Ok(())));
    demo.command(&freedoom()).await;
    assert_eq!(roots(root), first, "the retained game keeps its root");
    let notices = demo.notices();
    assert!(
        notices.contains(&"Resuming DOOM...".to_owned()),
        "{notices:?}"
    );
    assert_eq!(
        notices
            .iter()
            .filter(|n| n.starts_with("Loading DOOM from"))
            .count(),
        1,
        "{notices:?}"
    );
    demo.shutdown().await;
}

/// Opening with no path looks for a WAD again; a different one starts a new game.
async fn rediscover(root: &Path) {
    let demo = started(root, Box::new(|_| Ok(()))).await;
    let first = roots(root);
    std::fs::copy(freedoom(), root.join("project/doom1.wad")).unwrap();
    demo.view(Box::new(|_| Ok(())));
    demo.command("").await;
    let notices = demo.notices();
    assert!(
        !notices.contains(&"Resuming DOOM...".to_owned()),
        "{notices:?}"
    );
    assert!(
        notices
            .last()
            .is_some_and(|n| n.starts_with("Loading DOOM from ") && n.ends_with("doom1.wad...")),
        "{notices:?}"
    );
    assert_ne!(roots(root), first, "the found WAD started a new game");
    demo.shutdown().await;
}

/// Function keys are not game input, so F1 leaves the title screen as it was.
async fn function_keys(root: &Path) {
    let demo = started(
        root,
        Box::new(|built| {
            let title = rows(built)?;
            key(built, "\x1bOP")?;
            assert_eq!(title, rows(built)?, "F1 does not open the help screen");
            Ok(())
        }),
    )
    .await;
    demo.shutdown().await;
}

/// A different WAD replaces the game and removes the old root.
async fn switch(root: &Path) {
    let (first, second) = (root.join("first.wad"), root.join("second.wad"));
    std::fs::copy(freedoom(), &first).unwrap();
    std::fs::copy(freedoom(), &second).unwrap();
    let demo = super::doom_wad::loaded(root, controlled_io("none"), false).await;
    demo.view(Box::new(|_| Ok(())));
    demo.command(first.to_str().unwrap()).await;
    let old = roots(root);
    demo.view(Box::new(|_| Ok(())));
    demo.command(second.to_str().unwrap()).await;
    let new = roots(root);
    assert_eq!(new.len(), 1, "only the new game keeps files");
    assert_ne!(new, old, "the second WAD started a new game");
    let notices = demo.notices();
    assert!(
        !notices.contains(&"Resuming DOOM...".to_owned()),
        "{notices:?}"
    );
    demo.shutdown().await;
}

/// Quitting from the menu stops the view and the next open starts a new game.
async fn quit(root: &Path) {
    let demo = super::doom_wad::loaded(root, controlled_io("none"), false).await;
    demo.ui.until_closed.set(true);
    demo.view(Box::new(|built| {
        for data in ["\x1b", "\x1b[A", "\r", "y"] {
            key(built, data)?;
        }
        Ok(())
    }));
    demo.command(&freedoom()).await;
    assert_eq!(
        demo.notices(),
        [
            "Loading DOOM...".to_owned(),
            format!("Loading DOOM from {}...", freedoom())
        ],
        "the game closes without a notice"
    );
    let ended = roots(root);
    demo.view(Box::new(|_| Ok(())));
    demo.command(&freedoom()).await;
    assert_ne!(roots(root), ended, "the quit game is replaced, not resumed");
    demo.shutdown().await;
}

/// The title screen is drawn at once and Escape opens the menu over it.
#[test]
fn doom_overlay_draws_the_title_and_opens_the_menu() {
    checked(
        "doom_overlay_draws_the_title_and_opens_the_menu",
        &["title-and-menu"],
    );
}
/// The game's files stay in one private root that shutdown removes.
#[test]
fn doom_overlay_files_are_private_and_deleted_on_shutdown() {
    checked(
        "doom_overlay_files_are_private_and_deleted_on_shutdown",
        &["private-root"],
    );
}
/// Opening the same WAD again resumes the retained game.
#[test]
fn doom_overlay_resumes_the_retained_game() {
    checked("doom_overlay_resumes_the_retained_game", &["resume"]);
}
/// Opening with no path looks for a WAD again instead of reusing the retained one.
#[test]
fn doom_overlay_rediscovers_the_wad_without_an_argument() {
    checked(
        "doom_overlay_rediscovers_the_wad_without_an_argument",
        &["rediscover"],
    );
}
/// F1 does not reach the game.
#[test]
fn doom_overlay_ignores_function_keys() {
    checked("doom_overlay_ignores_function_keys", &["function-keys"]);
}
/// Opening a different WAD replaces the retained game.
#[test]
fn doom_overlay_replaces_the_game_for_another_wad() {
    checked(
        "doom_overlay_replaces_the_game_for_another_wad",
        &["switch"],
    );
}
/// Quitting the game from its menu closes the view and the next open starts afresh.
#[test]
fn doom_overlay_menu_quit_stops_the_game() {
    checked("doom_overlay_menu_quit_stops_the_game", &["quit"]);
}
