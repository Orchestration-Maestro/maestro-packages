//! The game, its input and its frame, retained across overlay views.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    doom_screen::{Painter, Subsystems},
    wad_check::check_iwad,
};
use doom_ui::{Finale, GameMenu, Intermission, Messages, Statusbar};
use game_config::GameOptions;
use gamestate::{Game, prepare_wad, subsystems::GameSubsystem};
use gamestate_traits::{ConfigKey, ConfigTraits, GameState, KeyCode, SubsystemTrait};
use input::{InputState, config::InputConfig};
use maestro_extensions_wasm::ExtensionResult;
use std::{cell::RefCell, path::Path, sync::mpsc};
use wad::WadData;

/// The IWAD copy the game reads, inside the private root.
const WAD_FILE: &str = "doom1.wad";

/// The game and everything that draws or listens beside it.
struct State {
    /// The running game.
    game: Game,
    /// Held keys, turned into one tic command per tic.
    input: InputState,
    /// The menu drawn over the game.
    menu: GameMenu,
    /// The status bar, messages, intermission and finale.
    machines: Subsystems,
    /// Draws the frame.
    painter: Painter,
}

/// One initialized game; a different WAD needs a new engine.
pub struct DoomEngine {
    /// Mutated by ticks, keys and drawing, which never overlap.
    state: RefCell<State>,
}
impl DoomEngine {
    /// Starts a game on the title screen from supplied bytes, inside `root`.
    /// # Errors
    /// Returns a damaged or unsuitable WAD, or a file that cannot be written.
    pub fn new(wad: &[u8], root: &str) -> ExtensionResult<Self> {
        check_iwad(wad)?;
        let path = Path::new(root).join(WAD_FILE);
        std::fs::write(&path, wad).map_err(|error| error.to_string())?;
        // Saves go to a relative directory, so the game must start inside its private root.
        std::env::set_current_dir(root).map_err(|error| error.to_string())?;
        let data = WadData::try_new(&path).map_err(|error| error.to_string())?;
        let (options, data) = prepare_wad(GameOptions::default(), data);
        let (sound, _unheard) = mpsc::channel();
        let mut game = Game::new(options, data, sound, None, config());
        let mut menu = GameMenu::new(game.game_type.mode, &game.wad_data);
        menu.init(&game);
        let machines = GameSubsystem {
            statusbar: Statusbar::new(game.game_type.mode, &game.wad_data),
            intermission: Intermission::new(game.game_type.mode, &game.wad_data, &game.umapinfo),
            hud_msgs: Messages::new(&game.wad_data),
            finale: Finale::new(&game.wad_data),
        };
        game.start_title();
        let engine = Self {
            state: RefCell::new(State {
                game,
                input: InputState::new((&InputConfig::default()).into()),
                menu,
                machines,
                painter: Painter::new(),
            }),
        };
        // The first tic puts the title page up, so the first frame is not blank.
        engine.tick();
        Ok(engine)
    }

    /// Advances one game tic. Returns whether the game is still running.
    pub fn tick(&self) -> bool {
        let mut guard = self.state.borrow_mut();
        let State {
            game,
            input,
            menu,
            machines,
            ..
        } = &mut *guard;
        if game.demo.advance {
            game.do_advance_demo();
        }
        let menu_active = menu.ticker(game);
        if menu_active && game.gamestate == GameState::Level && !game.demo.playback {
            machines.statusbar.ticker(game);
            machines.hud_msgs.ticker(game);
            game.frozen = true;
        } else {
            game.ticker(machines);
            game.frozen = false;
        }
        game.game_tic += 1;
        let console = game.consoleplayer;
        game.netcmds[console][0] = input.events.build_tic_cmd(&input.config);
        // Screen wipes are only visual, and gameplay would wait for one that is never drawn.
        game.wipe_game_state = game.gamestate;
        game.clear_config_dirty();
        game.running()
    }

    /// Whether the game has quit, so the next open must start a new one.
    pub fn has_ended(&self) -> bool {
        !self.state.borrow().game.running()
    }

    /// Draws the current frame and returns it as `0xAARRGGBB` pixels.
    pub fn frame(&self) -> Vec<u32> {
        let mut guard = self.state.borrow_mut();
        let State {
            game,
            machines,
            menu,
            painter,
            ..
        } = &mut *guard;
        painter.draw(game, machines, menu);
        painter.screen.pixels().to_vec()
    }

    /// Delivers a press, which the menu or a finale may take, or a release.
    pub fn push_key(&self, pressed: bool, key: KeyCode) {
        let mut guard = self.state.borrow_mut();
        let State {
            game,
            input,
            menu,
            machines,
            ..
        } = &mut *guard;
        if pressed && !responded(key, game, menu, machines) {
            input.events.set_kb(key);
        } else {
            input.events.unset_kb(key);
        }
    }
}

/// Gives a pressed key to the menu, then the messages, then the screen that has focus.
fn responded(
    key: KeyCode,
    game: &mut Game,
    menu: &mut GameMenu,
    machines: &mut Subsystems,
) -> bool {
    if menu.responder(key, game) || machines.hud_msgs.responder(key, game) {
        return true;
    }
    match game.gamestate {
        GameState::Intermission if game.level.is_none() => {
            machines.intermission.responder(key, game)
        }
        GameState::Finale if game.level.is_none() => machines.finale.responder(key, game),
        _ => false,
    }
}

/// The game's settings: a status bar, stacked messages, and no sound.
fn config() -> [i32; ConfigKey::KeyCount as usize] {
    let mut values = [0; ConfigKey::KeyCount as usize];
    values[ConfigKey::HiRes as usize] = 1;
    values[ConfigKey::HudSize as usize] = 1;
    values[ConfigKey::HudMsgMode as usize] = 1;
    values[ConfigKey::HudMsgTime as usize] = 2;
    values[ConfigKey::MenuDim as usize] = 1;
    values[ConfigKey::MouseSensitivity as usize] = 5;
    values
}
