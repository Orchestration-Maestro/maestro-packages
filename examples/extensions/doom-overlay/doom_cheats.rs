//! The cheat codes a player types while a level runs.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use game_config::{GameMission, GameMode, Skill, WeaponType};
use gameplay::{PlayerCheat, PowerType, english};
use gamestate::Game;
use gamestate_traits::{GameTraits, KeyCode};

/// What a completed code does to the console player.
#[derive(Clone, Copy)]
enum Effect {
    /// Toggles invulnerability.
    God,
    /// Armour, every weapon and full ammunition, without keys.
    Ammunition,
    /// Armour, every weapon, full ammunition and every key.
    Everything,
    /// Toggles walking through walls; the code depends on the game's edition.
    Noclip(GameMission),
    /// The chainsaw and invulnerability.
    Chainsaw,
    /// Toggles one power-up.
    Power(PowerType),
}

/// The codes, as the game documents them.
const CODES: [(&str, Effect); 12] = [
    ("iddqd", Effect::God),
    ("idfa", Effect::Ammunition),
    ("idkfa", Effect::Everything),
    ("idspispopd", Effect::Noclip(GameMission::Doom)),
    ("idclip", Effect::Noclip(GameMission::Doom2)),
    ("idchoppers", Effect::Chainsaw),
    ("idbeholdv", Effect::Power(PowerType::Invulnerability)),
    ("idbeholds", Effect::Power(PowerType::Strength)),
    ("idbeholdi", Effect::Power(PowerType::Invisibility)),
    ("idbeholdr", Effect::Power(PowerType::IronFeet)),
    ("idbeholda", Effect::Power(PowerType::Allmap)),
    ("idbeholdl", Effect::Power(PowerType::Infrared)),
];
/// The letters before the two digits of the level-change code.
const LEVEL_CODE: &str = "idclev";
/// The longest code, counting the two digits of the level change.
const LONGEST: usize = longest();
/// Armour points the ammunition codes give.
const ARMOUR_POINTS: i32 = 200;
/// The armour class of the strongest armour.
const ARMOUR_TYPE: i32 = 2;
/// The health an invulnerability code restores.
const FULL_HEALTH: i32 = 100;

/// The recent letters typed, matched against the codes.
#[derive(Default)]
pub struct Cheats {
    /// The last typed characters, at most as many as the longest code.
    typed: String,
}
impl Cheats {
    /// Forgets what was typed.
    pub fn reset(&mut self) {
        self.typed.clear();
    }

    /// Whether typing `next` now would carry a code forward instead of starting over.
    pub fn expects(&self, next: char) -> bool {
        (0..self.typed.len()).any(|start| {
            CODES.iter().any(|(code, _)| {
                code.strip_prefix(&self.typed[start..])
                    .is_some_and(|rest| rest.starts_with(next))
            })
        })
    }

    /// Takes one pressed key; a completed code changes the game. Modifier keys change
    /// nothing and any other key that is not a character ends the code in progress.
    pub fn type_key(&mut self, key: KeyCode, game: &mut Game) {
        if is_modifier(key) {
            return;
        }
        let Some(character) = key.to_char() else {
            self.reset();
            return;
        };
        if self.typed.len() == LONGEST {
            self.typed.remove(0);
        }
        self.typed.push(character);
        if game.is_netgame() {
            return;
        }
        if let Some(effect) = self.completed(game.game_mission()) {
            self.reset();
            if game.game_skill() != Skill::Nightmare {
                apply(effect, game);
            }
        } else if let Some((episode, map)) = self.level_change(game) {
            self.reset();
            game.defered_init_new(game.game_skill(), episode, map);
            game.players[game.consoleplayer].message = Some(english::STSTR_CLEV);
        }
    }

    /// The effect of the code just completed, if the edition accepts it.
    fn completed(&self, mission: GameMission) -> Option<Effect> {
        CODES
            .iter()
            .find(|(code, _)| self.typed.ends_with(code))
            .map(|&(_, effect)| effect)
            .filter(|effect| match effect {
                Effect::Noclip(edition) => {
                    (*edition == GameMission::Doom) == (mission == GameMission::Doom)
                }
                _ => true,
            })
    }

    /// The episode and map of a level-change code just completed, when the map exists.
    fn level_change(&self, game: &Game) -> Option<(usize, usize)> {
        let (head, tail) = self.typed.split_at(self.typed.len().checked_sub(2)?);
        if !head.ends_with(LEVEL_CODE) {
            return None;
        }
        let mut digits = tail.chars().map(|digit| {
            digit
                .to_digit(10)
                .and_then(|value| usize::try_from(value).ok())
        });
        let (first, second) = (digits.next()??, digits.next()??);
        let (episode, map, name) = if game.game_type.mode == GameMode::Commercial {
            let map = first * 10 + second;
            (1, map, format!("MAP{map:02}"))
        } else {
            (first, second, format!("E{first}M{second}"))
        };
        game.wad_data.lump_exists(&name).then_some((episode, map))
    }
}

/// The length of the longest code.
const fn longest() -> usize {
    let mut longest = LEVEL_CODE.len() + 2;
    let mut index = 0;
    while index < CODES.len() {
        if CODES[index].0.len() > longest {
            longest = CODES[index].0.len();
        }
        index += 1;
    }
    longest
}

/// Whether the key only modifies the keys typed with it.
const fn is_modifier(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::LCtrl
            | KeyCode::RCtrl
            | KeyCode::LShift
            | KeyCode::RShift
            | KeyCode::LAlt
            | KeyCode::RAlt
    )
}

/// Changes the console player and shows the game's message for it.
fn apply(effect: Effect, game: &mut Game) {
    let player = &mut game.players[game.consoleplayer];
    let message = match effect {
        Effect::God => {
            player.status.cheats.toggle(PlayerCheat::Godmode);
            if player.status.cheats.contains(PlayerCheat::Godmode) {
                restore_health(player);
                english::STSTR_DQDON
            } else {
                english::STSTR_DQDOFF
            }
        }
        Effect::Ammunition | Effect::Everything => {
            player.status.armorpoints = ARMOUR_POINTS;
            player.status.armortype = ARMOUR_TYPE;
            player.status.weaponowned.fill(true);
            let maximum = player.status.maxammo;
            player.status.ammo = maximum;
            if matches!(effect, Effect::Everything) {
                player.status.cards.fill(true);
                english::STSTR_KFAADDED
            } else {
                english::STSTR_FAADDED
            }
        }
        Effect::Noclip(_) => {
            player.status.cheats.toggle(PlayerCheat::Noclip);
            if player.status.cheats.contains(PlayerCheat::Noclip) {
                english::STSTR_NCON
            } else {
                english::STSTR_NCOFF
            }
        }
        Effect::Chainsaw => {
            player.status.weaponowned[WeaponType::Chainsaw as usize] = true;
            player.give_power(PowerType::Invulnerability);
            english::STSTR_CHOPPERS
        }
        Effect::Power(power) => {
            toggle_power(player, power);
            english::STSTR_BEHOLDX
        }
    };
    player.message = Some(message);
}

/// Sets the player and the player's map object to full health.
fn restore_health(player: &mut gameplay::Player) {
    if let Some(mobj) = player.mobj_mut() {
        mobj.health = FULL_HEALTH;
    }
    player.status.health = FULL_HEALTH;
}

/// Gives a power-up the player lacks; a timed one the player holds runs out at the
/// next tic, and strength is taken away.
fn toggle_power(player: &mut gameplay::Player, power: PowerType) {
    let held = &mut player.status.powers[power as usize];
    match power {
        PowerType::Allmap => *held = 1,
        PowerType::Strength if *held != 0 => *held = 0,
        _ if *held != 0 => *held = 1,
        _ => {
            player.give_power(power);
        }
    }
}
