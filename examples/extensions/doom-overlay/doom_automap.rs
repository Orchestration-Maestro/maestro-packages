//! The automap: the lines the player has seen, drawn around the player.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::doom_screen::{Screen, WIDTH};
use gameplay::PowerType;
use gamestate::Game;
use gamestate_traits::KeyCode;
use num_traits::ToPrimitive;
use render_common::DrawBuffer;

/// The line flag for a secret door, which the map shows as a wall.
const SECRET: u32 = 1 << 5;
/// The line flag for a line the map never shows.
const NEVER_SHOWN: u32 = 1 << 7;
/// The line flag the renderer sets on a line the player has seen.
const SEEN: u32 = 1 << 8;
/// The special number of a teleporter line.
const TELEPORTER: u32 = 39;

/// Palette index of a wall with nothing beyond it, red.
const WALL: usize = 176;
/// Palette index of a line where the floor height changes, brown.
const FLOOR_STEP: usize = 64;
/// Palette index of a line where the ceiling height changes, yellow.
const CEILING_STEP: usize = 231;
/// Palette index of a teleporter line.
const TELEPORT: usize = 184;
/// Palette index of a line the map power reveals before it is seen, grey.
const REVEALED: usize = 99;
/// Palette index of the player's arrow, white.
const PLAYER: usize = 209;

/// The map's size relative to the whole-level fit when it opens: the whole level fits at 0.7.
const OPENING_FIT: f32 = 0.7;
/// How much each game tic scales the map while a zoom key is held.
const ZOOM_RATE: f32 = 1.02;
/// Pixels per map unit at the closest zoom: two player radii fill the view height.
const PLAYER_DIAMETER: f32 = 32.0;
/// Half the length of the player's arrow in map units.
const ARROW: f32 = 18.0;
/// The arrow's lines as `[from, to]` points along the facing direction, in units of the arrow.
const ARROW_LINES: [[[f32; 2]; 2]; 7] = [
    [[-0.875, 0.0], [1.0, 0.0]],
    [[1.0, 0.0], [0.5, 0.25]],
    [[1.0, 0.0], [0.5, -0.25]],
    [[-0.875, 0.0], [-1.125, 0.25]],
    [[-0.875, 0.0], [-1.125, -0.25]],
    [[-0.625, 0.0], [-0.875, 0.25]],
    [[-0.625, 0.0], [-0.875, -0.25]],
];

/// Which way a held zoom key is scaling the map.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Zoom {
    /// No zoom key is held.
    #[default]
    Still,
    /// `=` is held: the map grows each tic.
    In,
    /// `-` is held: the map shrinks each tic.
    Out,
}

/// The level a map was opened on, which a different level replaces.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Level {
    /// The episode the level belongs to.
    episode: usize,
    /// The map within the episode.
    map: usize,
}

/// The widest and closest scales the map may take, in pixels per map unit.
struct Limits {
    /// The whole level fits the view.
    fit: f32,
    /// The player's diameter fills the view.
    closest: f32,
}
impl Limits {
    /// The limits of a level in a view of `rows` pixel rows.
    fn new(game: &Game, rows: usize) -> Option<Self> {
        let level = game.level.as_ref()?;
        let fit = fit_scale(
            level
                .level_data
                .linedefs
                .iter()
                .flat_map(|line| [line.v1.pos, line.v2.pos])
                .map(|point| [point.x, point.y]),
            rows,
        );
        Some(Self {
            fit,
            closest: rows_f32(rows) / PLAYER_DIAMETER,
        })
    }

    /// The scale a map opens at.
    fn opening(&self) -> f32 {
        self.within(self.fit / OPENING_FIT)
    }

    /// `scale` held between the widest and closest scales.
    fn within(&self, scale: f32) -> f32 {
        scale.min(self.closest).max(self.fit)
    }
}

/// Whether the map shows, and how far it is zoomed.
#[derive(Default)]
pub struct Automap {
    /// Whether the map replaces the player's view.
    shown: bool,
    /// Pixels per map unit once the map has opened on the level; a different level resets it.
    scale: Option<f32>,
    /// The zoom key held now.
    zoom: Zoom,
    /// The level the scale belongs to.
    level: Option<Level>,
}
impl Automap {
    /// Whether the map replaces the player's view.
    pub const fn is_shown(&self) -> bool {
        self.shown
    }

    /// Closes the map and forgets its scale, which belongs to the level that was running.
    pub const fn reset(&mut self) {
        self.shown = false;
        self.scale = None;
        self.zoom = Zoom::Still;
        self.level = None;
    }

    /// Resets the map when the running level is not the one it was opened on. When the same
    /// map was loaded again (`reloaded`) by a new game, a warp or a loaded save, the map
    /// closes and keeps its scale.
    pub fn sync(&mut self, game: &Game, reloaded: bool) {
        let current = game.level.as_ref().map(|level| Level {
            episode: level.options.episode,
            map: level.options.map,
        });
        if self.level.zip(current).is_some_and(|(was, now)| was != now) {
            self.reset();
        } else if reloaded {
            self.shown = false;
            self.zoom = Zoom::Still;
        }
        self.level = current;
    }

    /// Takes a pressed key: Tab opens or closes the map, and `=` and `-` zoom it while held.
    pub fn responder(&mut self, key: KeyCode) -> bool {
        match (key, self.shown) {
            (KeyCode::Tab, shown) => {
                self.shown = !shown;
                self.zoom = Zoom::Still;
                true
            }
            (KeyCode::Equals, true) => {
                self.zoom = Zoom::In;
                true
            }
            (KeyCode::Minus, true) => {
                self.zoom = Zoom::Out;
                true
            }
            _ => false,
        }
    }

    /// Takes a released key: letting go of `=` or `-` stops the zoom.
    pub fn release(&mut self, key: KeyCode) {
        if matches!(key, KeyCode::Equals | KeyCode::Minus) {
            self.zoom = Zoom::Still;
        }
    }

    /// Scales the map one tic while a zoom key is held, within the view's limits.
    pub fn tick(&mut self, game: &Game, rows: usize) {
        let rate = match self.zoom {
            Zoom::Still => return,
            Zoom::In => ZOOM_RATE,
            Zoom::Out => ZOOM_RATE.recip(),
        };
        if let Some(limits) = self.shown.then(|| Limits::new(game, rows)).flatten() {
            let scale = self.scale.unwrap_or_else(|| limits.opening());
            self.scale = Some(limits.within(scale * rate));
        }
    }

    /// Draws the map into the top `rows` pixel rows of `screen`, centred on the console player.
    pub fn draw(&self, game: &Game, screen: &mut Screen, rows: usize) {
        let (Some(limits), Some(level)) = (Limits::new(game, rows), game.level.as_ref()) else {
            return;
        };
        let player = &game.players[game.consoleplayer];
        let Some(mobj) = player.mobj() else {
            return;
        };
        let palette = &game.pic_data.wad_palette().0;
        let revealed = player.status.powers[PowerType::Allmap as usize] != 0;
        let projection = Projection {
            centre: [mobj.x.to_f32(), mobj.y.to_f32()],
            scale: self
                .scale
                .map_or_else(|| limits.opening(), |scale| limits.within(scale)),
            rows,
        };
        for line in level.level_data.linedefs.iter() {
            let bits = line.flags.bits();
            let steps = line.backsector.as_ref().map(|back| {
                (
                    back.floorheight != line.frontsector.floorheight,
                    back.ceilingheight != line.frontsector.ceilingheight,
                )
            });
            let colour = line_colour(bits, line.default_special, steps, revealed);
            if let Some(colour) = colour {
                let (from, to) = (
                    projection.point(line.v1.pos.x, line.v1.pos.y),
                    projection.point(line.v2.pos.x, line.v2.pos.y),
                );
                draw_line(screen, rows, [from, to], palette[colour]);
            }
        }
        projection.arrow(screen, mobj.angle.sin_cos(), palette[PLAYER]);
    }
}

/// The palette index of a line the map shows, or none when it stays hidden.
fn line_colour(
    bits: u32,
    special: u32,
    steps: Option<(bool, bool)>,
    revealed: bool,
) -> Option<usize> {
    if bits & NEVER_SHOWN != 0 {
        None
    } else if bits & SEEN != 0 {
        seen_colour(bits, special, steps)
    } else {
        revealed.then_some(REVEALED)
    }
}

/// The palette index of a seen line, or none for a two-sided line with no height change.
fn seen_colour(bits: u32, special: u32, steps: Option<(bool, bool)>) -> Option<usize> {
    let Some((floor, ceiling)) = steps else {
        return Some(WALL);
    };
    if special == TELEPORTER {
        Some(TELEPORT)
    } else if bits & SECRET != 0 {
        Some(WALL)
    } else if floor {
        Some(FLOOR_STEP)
    } else if ceiling {
        Some(CEILING_STEP)
    } else {
        None
    }
}

/// A pixel row count as a float.
fn rows_f32(rows: usize) -> f32 {
    f32::from(u16::try_from(rows).unwrap_or(u16::MAX))
}

/// The scale at which the whole level fits the view.
fn fit_scale(points: impl Iterator<Item = [f32; 2]>, rows: usize) -> f32 {
    let (low, high) = points.fold(([f32::MAX; 2], [f32::MIN; 2]), |(low, high), [x, y]| {
        (
            [low[0].min(x), low[1].min(y)],
            [high[0].max(x), high[1].max(y)],
        )
    });
    let width = (high[0] - low[0]).max(1.0);
    let height = (high[1] - low[1]).max(1.0);
    (f32::from(WIDTH) / width).min(rows_f32(rows) / height)
}

/// Where map points land in the view, which follows the player.
struct Projection {
    /// The map point at the middle of the view.
    centre: [f32; 2],
    /// Pixels per map unit.
    scale: f32,
    /// Pixel rows of the view.
    rows: usize,
}
impl Projection {
    /// The view pixel of a map point; the map's up is the view's up.
    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        [
            f32::from(WIDTH) / 2.0 + (x - self.centre[0]) * self.scale,
            rows_f32(self.rows) / 2.0 - (y - self.centre[1]) * self.scale,
        ]
    }

    /// Draws the player's arrow facing the way of `(sin, cos)` at the middle of the view.
    fn arrow(&self, screen: &mut Screen, (sin, cos): (f32, f32), colour: u32) {
        for [from, to] in ARROW_LINES {
            let turn = |[along, across]: [f32; 2]| {
                let (along, across) = (along * ARROW, across * ARROW);
                self.point(
                    self.centre[0] + along * cos - across * sin,
                    self.centre[1] + along * sin + across * cos,
                )
            };
            draw_line(screen, self.rows, [turn(from), turn(to)], colour);
        }
    }
}

/// Draws a line two pixels thick, clipped to the view.
fn draw_line(screen: &mut Screen, rows: usize, [from, to]: [[f32; 2]; 2], colour: u32) {
    let bounds = [f32::from(WIDTH) - 2.0, rows_f32(rows) - 2.0];
    let Some([from, to]) = clip(from, to, bounds) else {
        return;
    };
    let (Some(mut x), Some(mut y), Some(x_end), Some(y_end)) = (
        from[0].round().to_i32(),
        from[1].round().to_i32(),
        to[0].round().to_i32(),
        to[1].round().to_i32(),
    ) else {
        return;
    };
    let (dx, dy) = ((x_end - x).abs(), -(y_end - y).abs());
    let (step_x, step_y) = ((x_end - x).signum(), (y_end - y).signum());
    let mut error = dx + dy;
    loop {
        for (offset_x, offset_y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            if let (Ok(column), Ok(row)) =
                (usize::try_from(x + offset_x), usize::try_from(y + offset_y))
            {
                screen.set_pixel(column, row, colour);
            }
        }
        if x == x_end && y == y_end {
            return;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += step_x;
        }
        if doubled <= dx {
            error += dx;
            y += step_y;
        }
    }
}

/// The part of a segment inside the box from the origin to `bounds`, if any.
fn clip(from: [f32; 2], to: [f32; 2], bounds: [f32; 2]) -> Option<[[f32; 2]; 2]> {
    let mut range = (0.0_f32, 1.0_f32);
    for axis in 0..2 {
        let delta = to[axis] - from[axis];
        range = narrow(range, from[axis], -delta)?;
        range = narrow(range, bounds[axis] - from[axis], delta)?;
    }
    let at = |t: f32| {
        [
            from[0] + (to[0] - from[0]) * t,
            from[1] + (to[1] - from[1]) * t,
        ]
    };
    (range.0 <= range.1).then(|| [at(range.0), at(range.1)])
}

/// Narrows the visible fractions of a segment by one edge of the box, `distance` from the
/// segment's start and `direction` the way the segment heads across it; none when the
/// segment runs outside along the edge.
fn narrow((first, last): (f32, f32), distance: f32, direction: f32) -> Option<(f32, f32)> {
    if direction.abs() <= f32::EPSILON {
        return (distance >= 0.0).then_some((first, last));
    }
    let at = distance / direction;
    Some(if direction < 0.0 {
        (first.max(at), last)
    } else {
        (first, last.min(at))
    })
}
