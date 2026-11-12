//! The overlay's automap and cheat codes, played on a Freedoom level.
#![cfg(test)]
use super::{freedoom, key};
use crate::host::doom_wad::{Demo, Play, Tics, checked, controlled_io, loaded};
use maestro_extensions::{EventError, types::ExtensionComponent};
use std::{collections::HashSet, ops::Range, path::Path};

/// The frame's width in pixels; the render at this many terminal columns is the whole frame.
const FRAME_WIDTH: usize = 640;
/// Pixel rows of the player's view; the status bar takes the rest.
const VIEW_ROWS: usize = 336;
/// Pixel rows of the frame.
const FRAME_ROWS: usize = 400;
/// Game tics a message stays up, with a margin.
const MESSAGE_TICS: usize = 80;
/// The fewest pixels of one colour that make it part of a drawing.
const DRAWN: usize = 100;

/// One pixel as red, green and blue.
type Pixel = [u8; 3];

/// The whole frame, row by row, read back from a render at full resolution.
fn frame(built: &ExtensionComponent) -> Result<Vec<Pixel>, EventError> {
    let lines = built
        .component
        .try_render(FRAME_WIDTH)
        .map_err(|error| EventError::from(error.to_string()))?;
    let mut pixels = vec![[0; 3]; FRAME_WIDTH * FRAME_ROWS];
    for (row, line) in lines[..lines.len() - 1].iter().enumerate() {
        for (column, cell) in line.split('▀').enumerate().take(FRAME_WIDTH) {
            let numbers: Vec<u8> = cell
                .split(|c: char| !c.is_ascii_digit())
                .filter(|number| !number.is_empty())
                .map(|number| number.parse().unwrap())
                .collect();
            pixels[2 * row * FRAME_WIDTH + column] = [numbers[2], numbers[3], numbers[4]];
            pixels[(2 * row + 1) * FRAME_WIDTH + column] = [numbers[7], numbers[8], numbers[9]];
        }
    }
    Ok(pixels)
}

/// The pixels of some frame rows.
fn rows(frame: &[Pixel], rows: Range<usize>) -> &[Pixel] {
    &frame[rows.start * FRAME_WIDTH..rows.end * FRAME_WIDTH]
}

/// How many pixels of the rows differ between two frames.
fn changed(before: &[Pixel], after: &[Pixel], span: Range<usize>) -> usize {
    rows(before, span.clone())
        .iter()
        .zip(rows(after, span))
        .filter(|(a, b)| a != b)
        .count()
}

/// Whether the pixel is the map's red wall.
fn is_wall(pixel: Pixel) -> bool {
    pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60
}

/// Whether the pixel is the white of the player's arrow.
fn is_white(pixel: Pixel) -> bool {
    pixel.iter().all(|channel| *channel > 200)
}

/// How many pixels of the view rows have the property.
fn count(frame: &[Pixel], property: fn(Pixel) -> bool) -> usize {
    rows(frame, 0..VIEW_ROWS)
        .iter()
        .filter(|pixel| property(**pixel))
        .count()
}

/// Whether most of the view is black, as the map leaves it and the player's view does not.
fn mostly_black(frame: &[Pixel]) -> bool {
    count(frame, |pixel| pixel == [0; 3]) > FRAME_WIDTH * VIEW_ROWS * 9 / 10
}

/// The colours that make up a real share of the view rows.
fn drawn_colours(frame: &[Pixel]) -> HashSet<Pixel> {
    let mut counts = std::collections::HashMap::new();
    for pixel in rows(frame, 0..VIEW_ROWS) {
        *counts.entry(*pixel).or_insert(0_usize) += 1;
    }
    counts
        .into_iter()
        .filter(|(_, number)| *number >= DRAWN)
        .map(|(pixel, _)| pixel)
        .collect()
}

/// Types text as separate presses and releases, so no key stays held.
fn type_text(built: &ExtensionComponent, text: &str) -> Result<(), EventError> {
    for character in text.chars() {
        key(built, &character.to_string())?;
        key(built, &format!("\x1b[{};1:3u", u32::from(character)))?;
    }
    Ok(())
}

/// Asserts whether the first `rows` pixel rows differ between two frames.
fn map_moved(before: &[Pixel], after: &[Pixel], rows: usize, moved: bool, message: &str) {
    assert_eq!(changed(before, after, 0..rows) > 0, moved, "{message}");
}

/// Holds a key down for `count` game tics and lets it go.
async fn hold(
    built: &ExtensionComponent,
    tics: &Tics,
    character: char,
    count: usize,
) -> Result<(), EventError> {
    key(built, &character.to_string())?;
    tics.run(count).await?;
    key(built, &format!("\x1b[{};1:3u", u32::from(character)))
}

/// Starts a new game on the first episode and lets the first level run a few tics.
async fn start_level(built: &ExtensionComponent, tics: &Tics) -> Result<Vec<Pixel>, EventError> {
    for data in ["\x1b", "\r", "\r", "\r"] {
        key(built, data)?;
    }
    tics.run(3).await?;
    frame(built)
}

/// Runs a real-engine level scenario; returns whether the case is one of these.
pub(super) async fn run(root: &Path, case: &str) -> bool {
    let play: Play = match case {
        "automap-tab" => Box::new(|built, tics| Box::pin(automap_tab(built, tics))),
        "automap-zoom" => Box::new(|built, tics| Box::pin(automap_zoom(built, tics))),
        "automap-reload" => Box::new(|built, tics| Box::pin(automap_reload(built, tics))),
        "automap-new-game" => Box::new(|built, tics| Box::pin(automap_new_game(built, tics))),
        "automap-load" => Box::new(|built, tics| Box::pin(automap_load(built, tics))),
        "automap-end" => Box::new(|built, tics| Box::pin(automap_end(built, tics))),
        "automap-beholda" => Box::new(|built, tics| Box::pin(automap_beholda(built, tics))),
        "cheat-iddqd" => Box::new(|built, tics| Box::pin(cheat_iddqd(built, tics))),
        "cheat-idkfa" => Box::new(|built, tics| Box::pin(cheat_idkfa(built, tics))),
        "cheat-idclev" => Box::new(|built, tics| Box::pin(cheat_idclev(built, tics))),
        _ => return false,
    };
    let demo: Demo = loaded(root, controlled_io("none"), false).await;
    demo.play(play);
    demo.command(&freedoom()).await;
    demo.shutdown().await;
    true
}

/// Tab swaps the view for a map of the seen lines with the player on it, keeps the status bar,
/// and brings the view back.
async fn automap_tab(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    let view = start_level(built, tics).await?;
    assert!(!mostly_black(&view), "the player's view fills the screen");
    key(built, "\t")?;
    let map = frame(built)?;
    assert!(mostly_black(&map), "the map leaves most of the view black");
    assert!(count(&map, is_wall) > 0, "the seen walls are drawn");
    assert!(count(&map, is_white) > 0, "the player's arrow is drawn");
    let bar = FRAME_WIDTH * (FRAME_ROWS - VIEW_ROWS);
    assert!(
        changed(&view, &map, VIEW_ROWS..FRAME_ROWS) * 100 < bar,
        "the status bar stays"
    );
    key(built, "\t")?;
    assert!(frame(built)? == view, "the second Tab returns the view");
    Ok(())
}

/// Holding `=` zooms the map in and holding `-` zooms it out, each as long as it is held and
/// no further than the whole level or the player's size; the scale stays when the map reopens.
async fn automap_zoom(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    let view = start_level(built, tics).await?;
    hold(built, tics, '=', 3).await?;
    map_moved(
        &view,
        &frame(built)?,
        FRAME_ROWS,
        false,
        "= does nothing without the map",
    );
    key(built, "\t")?;
    tics.run(1).await?;
    let opened = frame(built)?;
    hold(built, tics, '=', 5).await?;
    let near = frame(built)?;
    map_moved(&opened, &near, VIEW_ROWS, true, "holding = zooms in");
    hold(built, tics, '=', 5).await?;
    let held = frame(built)?;
    map_moved(
        &near,
        &held,
        VIEW_ROWS,
        true,
        "the map keeps zooming while the key is held",
    );
    tics.run(5).await?;
    map_moved(
        &held,
        &frame(built)?,
        VIEW_ROWS,
        false,
        "releasing stops the zoom",
    );
    automap_zoom_limits(built, tics, &held).await
}

/// Zooming out stops at the whole level and zooming in starts at once from there; the map
/// reopens at the scale it was left at.
async fn automap_zoom_limits(
    built: &ExtensionComponent,
    tics: &Tics,
    held: &[Pixel],
) -> Result<(), EventError> {
    hold(built, tics, '-', 40).await?;
    let widest = frame(built)?;
    map_moved(held, &widest, VIEW_ROWS, true, "holding - zooms out");
    hold(built, tics, '-', 10).await?;
    let still = frame(built)?;
    map_moved(
        &widest,
        &still,
        VIEW_ROWS,
        false,
        "the whole level is the widest view",
    );
    hold(built, tics, '=', 5).await?;
    let zoomed = frame(built)?;
    map_moved(
        &widest,
        &zoomed,
        VIEW_ROWS,
        true,
        "zooming in starts at once from the widest view",
    );
    key(built, "\t")?;
    key(built, "\t")?;
    map_moved(
        &zoomed,
        &frame(built)?,
        VIEW_ROWS,
        false,
        "the map reopens at the scale it was left at",
    );
    Ok(())
}

/// A new level takes the map down: warping from inside a level shows the new level's view.
async fn automap_reload(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    start_level(built, tics).await?;
    key(built, "\t")?;
    tics.run(1).await?;
    assert!(mostly_black(&frame(built)?), "the map is open");
    type_text(built, "idclev12")?;
    tics.run(5).await?;
    assert!(
        !mostly_black(&frame(built)?),
        "the new level shows the player's view"
    );
    Ok(())
}

/// The menu keys that start a new game while a level runs.
const NEW_GAME: [&str; 4] = ["\x1b", "n", "\r", "\r"];
/// The menu keys that load the first save slot.
const LOAD_GAME: [&str; 3] = ["\x1b", "l", "\r"];

/// Presses menu keys and lets the game run `count` tics.
async fn press(
    built: &ExtensionComponent,
    tics: &Tics,
    keys: &[&str],
    count: usize,
) -> Result<(), EventError> {
    for data in keys {
        key(built, data)?;
    }
    tics.run(count).await
}

/// Opens the map on the running level, zooms it in and leaves it open; then reloads the same map
/// with `reload`. The reload closes the map, so Tab opens it again, at the scale it was left at.
async fn keeps_scale_over_reload(
    built: &ExtensionComponent,
    tics: &Tics,
    reload: &[&str],
    level: &str,
) -> Result<(), EventError> {
    press(built, tics, reload, 5).await?;
    // Drawing the player's view marks the lines it sees, which the map then shows.
    frame(built)?;
    key(built, "\t")?;
    tics.run(1).await?;
    let opened = frame(built)?;
    assert!(mostly_black(&opened), "Tab opens the map on the {level}");
    hold(built, tics, '=', 5).await?;
    let zoomed = frame(built)?;
    press(built, tics, reload, 5).await?;
    // Drawing the player's view marks the lines it sees, which the map then shows.
    frame(built)?;
    key(built, "\t")?;
    tics.run(1).await?;
    let map = frame(built)?;
    assert!(mostly_black(&map), "Tab opens the map again on the {level}");
    assert!(
        changed(&zoomed, &map, 0..VIEW_ROWS) * 2 < changed(&opened, &zoomed, 0..VIEW_ROWS),
        "the {level}'s map reopens at the zoomed scale"
    );
    Ok(())
}

/// A new game on the map that is running closes the map and keeps its scale.
async fn automap_new_game(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    start_level(built, tics).await?;
    keeps_scale_over_reload(built, tics, &NEW_GAME, "new game").await
}

/// Loading a save of the map that is running closes the map and keeps its scale.
async fn automap_load(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    start_level(built, tics).await?;
    // The save menu opens on the first slot, which Enter starts naming.
    press(built, tics, &["\x1b", "s", "\r", "a", "\r"], 3).await?;
    keeps_scale_over_reload(built, tics, &LOAD_GAME, "loaded game").await
}

/// Ending the game closes the map, so Tab opens it again on the next level.
async fn automap_end(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    start_level(built, tics).await?;
    key(built, "\t")?;
    for data in ["\x1b", "o", "e"] {
        key(built, data)?;
    }
    tics.run(3).await?;
    // The menu opens on Options after End Game, so Escape goes up to the main menu.
    for data in ["\x1b", "\x1b", "n", "\r", "\r"] {
        key(built, data)?;
    }
    tics.run(3).await?;
    key(built, "\t")?;
    assert!(
        mostly_black(&frame(built)?),
        "Tab opens the map on the new level"
    );
    Ok(())
}

/// `idbeholda` adds the lines the player has not seen to the map; a word that is not a code does not.
async fn automap_beholda(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    start_level(built, tics).await?;
    key(built, "\t")?;
    tics.run(1).await?;
    let seen = drawn_colours(&frame(built)?);
    type_text(built, "idbeholdx")?;
    tics.run(MESSAGE_TICS).await?;
    assert_eq!(
        drawn_colours(&frame(built)?),
        seen,
        "idbeholdx is not a code"
    );
    type_text(built, "idbeholda")?;
    tics.run(MESSAGE_TICS).await?;
    assert!(
        drawn_colours(&frame(built)?).difference(&seen).count() > 0,
        "the unseen lines are drawn in a colour of their own"
    );
    Ok(())
}

/// `iddqd` changes the status bar's face, though it holds the Q that closes the overlay; `iddqx` does not.
async fn cheat_iddqd(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    let idle = start_level(built, tics).await?;
    type_text(built, "iddqx")?;
    tics.run(3).await?;
    let word = frame(built)?;
    assert_eq!(
        changed(&idle, &word, VIEW_ROWS..FRAME_ROWS),
        0,
        "iddqx is not a code"
    );
    type_text(built, "iddqd")?;
    tics.run(3).await?;
    assert_ne!(
        changed(&word, &frame(built)?, VIEW_ROWS..FRAME_ROWS),
        0,
        "iddqd is"
    );
    Ok(())
}

/// `idkfa` changes the status bar's ammunition, weapons and keys; `idkf` does not.
async fn cheat_idkfa(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    let idle = start_level(built, tics).await?;
    type_text(built, "idkf")?;
    tics.run(3).await?;
    let part = frame(built)?;
    assert_eq!(
        changed(&idle, &part, VIEW_ROWS..FRAME_ROWS),
        0,
        "idkf is not a code"
    );
    type_text(built, "a")?;
    tics.run(3).await?;
    assert_ne!(
        changed(&part, &frame(built)?, VIEW_ROWS..FRAME_ROWS),
        0,
        "idkfa is"
    );
    Ok(())
}

/// `idclev12` warps to the second map of the first episode; a map the WAD lacks is ignored.
async fn cheat_idclev(built: &ExtensionComponent, tics: &Tics) -> Result<(), EventError> {
    let first = start_level(built, tics).await?;
    type_text(built, "idclev99")?;
    tics.run(5).await?;
    let ignored = frame(built)?;
    let half = FRAME_WIDTH * VIEW_ROWS / 2;
    assert!(
        changed(&first, &ignored, 0..VIEW_ROWS) < half,
        "a missing map changes nothing"
    );
    type_text(built, "idclev12")?;
    tics.run(5).await?;
    assert!(
        changed(&ignored, &frame(built)?, 0..VIEW_ROWS) > half,
        "the second map replaces the first"
    );
    Ok(())
}

/// Tab shows the automap and Tab again returns to the player's view.
#[test]
fn doom_overlay_tab_toggles_the_automap() {
    checked("doom_overlay_tab_toggles_the_automap", &["automap-tab"]);
}
/// The equals and minus keys zoom the automap in and out.
#[test]
fn doom_overlay_automap_zooms_with_equals_and_minus() {
    checked(
        "doom_overlay_automap_zooms_with_equals_and_minus",
        &["automap-zoom"],
    );
}
/// A level that loads while the map is open takes the map down.
#[test]
fn doom_overlay_automap_closes_when_a_new_level_loads() {
    checked(
        "doom_overlay_automap_closes_when_a_new_level_loads",
        &["automap-reload"],
    );
}
/// A new game on the map that is running keeps the automap's scale.
#[test]
fn doom_overlay_automap_keeps_its_scale_when_a_new_game_starts_on_the_same_map() {
    checked(
        "doom_overlay_automap_keeps_its_scale_when_a_new_game_starts_on_the_same_map",
        &["automap-new-game"],
    );
}
/// A loaded save of the map that is running keeps the automap's scale.
#[test]
fn doom_overlay_automap_keeps_its_scale_when_a_save_loads_on_the_same_map() {
    checked(
        "doom_overlay_automap_keeps_its_scale_when_a_save_loads_on_the_same_map",
        &["automap-load"],
    );
}
/// Ending the game closes the automap.
#[test]
fn doom_overlay_automap_closes_when_the_game_ends() {
    checked(
        "doom_overlay_automap_closes_when_the_game_ends",
        &["automap-end"],
    );
}
/// The full-map power shows unseen lines on the automap.
#[test]
fn doom_overlay_idbeholda_shows_unseen_lines_on_the_automap() {
    checked(
        "doom_overlay_idbeholda_shows_unseen_lines_on_the_automap",
        &["automap-beholda"],
    );
}
/// The invulnerability code works, Q included.
#[test]
fn doom_overlay_iddqd_toggles_invulnerability() {
    checked(
        "doom_overlay_iddqd_toggles_invulnerability",
        &["cheat-iddqd"],
    );
}
/// The weapons and keys code works.
#[test]
fn doom_overlay_idkfa_gives_weapons_ammunition_and_keys() {
    checked(
        "doom_overlay_idkfa_gives_weapons_ammunition_and_keys",
        &["cheat-idkfa"],
    );
}
/// The level code warps to an existing map and ignores a missing one.
#[test]
fn doom_overlay_idclev_warps_to_the_map() {
    checked("doom_overlay_idclev_warps_to_the_map", &["cheat-idclev"]);
}
