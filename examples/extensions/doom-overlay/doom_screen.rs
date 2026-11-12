//! The software framebuffer the game draws into, and the drawing of one frame.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::doom_automap::Automap;
use doom_ui::{Finale, GameMenu, Intermission, Messages, Statusbar};
use gameplay::{MapObjFlag, Player};
use gamestate::{Game, subsystems::GameSubsystem};
use gamestate_traits::{GameState, SubsystemTrait};
use math::{Angle, Bam, FixedT, lookdir_to_radian};
use pic_data::{ByteOrder, PalLitCache, PixelFmt};
use render_common::{BufferSize, DrawBuffer, PixelTarget, RenderPspDef, RenderView, STBAR_HEIGHT};
use software25d::Software25D;
use ui_draw::UiCanvas;
use wad::types::{BLACK, WadPatch};

/// Framebuffer width in pixels.
pub const WIDTH: u16 = 640;
/// Framebuffer height in pixels.
pub const HEIGHT: u16 = 400;
/// The horizontal field of view the software renderer is built for, in radians.
const HFOV: f32 = 90f32.to_radians();
/// The lines of the original screen the interface is laid out on.
const OG_LINES: u16 = 200;

/// The subsystems that draw beside the player's view.
pub type Subsystems = GameSubsystem<Intermission, Statusbar, Messages, Finale>;

/// The framebuffer width as a pixel count.
fn columns() -> usize {
    usize::from(WIDTH)
}

/// The framebuffer height as a pixel count.
fn rows() -> usize {
    usize::from(HEIGHT)
}

/// A tightly packed `0xAARRGGBB` surface for the interface to draw into.
pub struct Screen {
    /// Row-major pixels.
    pixels: Vec<u32>,
    /// The surface size handed to the drawing code.
    size: BufferSize,
}
impl Screen {
    /// An opaque black surface.
    pub fn new() -> Self {
        Self {
            pixels: vec![PixelFmt::from_argb(BLACK, ByteOrder::Argb); columns() * rows()],
            size: BufferSize::new(columns(), rows()),
        }
    }

    /// The finished frame, row by row.
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
}
impl DrawBuffer for Screen {
    type Pixel = u32;

    fn size(&self) -> &BufferSize {
        &self.size
    }

    fn pitch(&self) -> usize {
        columns()
    }

    fn get_buf_index(&self, x: usize, y: usize) -> usize {
        y * columns() + x
    }

    fn set_pixel(&mut self, x: usize, y: usize, colour: u32) {
        if let Some(pixel) = self
            .pixels
            .get_mut(y * columns() + x)
            .filter(|_| x < columns())
        {
            *pixel = colour;
        }
    }

    fn buf_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }
}

/// Everything one frame is drawn from, besides the game itself.
pub struct Painter {
    /// The 2.5D renderer for the player's view.
    renderer: Software25D,
    /// The palette table, rebuilt when the game changes its palette.
    palettes: PalLitCache<u32>,
    /// Where the frame lands.
    pub screen: Screen,
    /// Pixel rows the status bar takes at the bottom.
    bar_rows: usize,
}
impl Painter {
    /// A renderer whose view leaves room for the status bar.
    pub fn new() -> Self {
        // The interface scales the status bar's 200-line height to the frame's height.
        let mut size = BufferSize::new(columns(), rows());
        size.set_statusbar_height(STBAR_HEIGHT * i32::from(HEIGHT / OG_LINES));
        let mut renderer = Software25D::new(HFOV, f32::from(WIDTH), f32::from(HEIGHT), true);
        renderer.set_view_height(size.view_height());
        Self {
            renderer,
            palettes: PalLitCache::new(),
            screen: Screen::new(),
            bar_rows: rows() - size.view_height_usize(),
        }
    }

    /// Pixel rows of the player's view, which the automap fills.
    pub fn map_rows(&self) -> usize {
        rows() - self.bar_rows
    }

    /// Draws the current game state, then the menu over it.
    pub fn draw(
        &mut self,
        game: &mut Game,
        machines: &mut Subsystems,
        menu: &mut GameMenu,
        automap: &Automap,
    ) {
        match game.gamestate {
            GameState::Level => {
                if automap.is_shown() {
                    let map_rows = self.map_rows();
                    self.screen
                        .pixels
                        .fill(PixelFmt::from_argb(BLACK, ByteOrder::Argb));
                    automap.draw(game, &mut self.screen, map_rows);
                } else {
                    self.draw_level(game);
                }
                machines.statusbar.draw(&mut self.screen);
                machines.hud_msgs.draw(&mut self.screen);
            }
            GameState::Intermission => machines.intermission.draw(&mut self.screen),
            GameState::Finale => machines.finale.draw(&mut self.screen),
            GameState::DemoScreen => self.draw_title(game),
            GameState::ForceWipe => {}
        }
        menu.draw(&mut self.screen);
    }

    /// The player's view, with the status bar area cleared first.
    fn draw_level(&mut self, game: &mut Game) {
        let Some(level) = game
            .level
            .as_mut()
            .filter(|_| game.players_in_game[game.consoleplayer])
        else {
            return;
        };
        level.level_data.apply_render_interpolation(1.0);
        let player = &game.players[game.consoleplayer];
        if let Some(view) = render_view(player, game.game_tic) {
            game.pic_data.set_player_palette(
                player.status.damagecount,
                player.status.bonuscount,
                player.status.powers[gameplay::PowerType::Strength as usize],
                player.status.powers[gameplay::PowerType::IronFeet as usize],
            );
            let table = self.palettes.get(
                game.pic_data.palette_generation(),
                game.pic_data.palettes(),
                ByteOrder::Argb,
            );
            let view_rows = rows() - self.bar_rows;
            self.screen.pixels[view_rows * columns()..]
                .fill(PixelFmt::from_argb(BLACK, ByteOrder::Argb));
            let mut target = PixelTarget::new(
                &mut self.screen.pixels,
                BufferSize::new(columns(), rows()),
                columns(),
                table,
                game.pic_data.use_palette(),
            );
            self.renderer
                .draw_view(&view, &level.level_data, &mut game.pic_data, &mut target);
        }
        level.level_data.restore_render_interpolation();
    }

    /// The title page, refreshed when the game changes pages.
    fn draw_title(&mut self, game: &mut Game) {
        if game.page.cache.name != game.page.name
            && let Some(lump) = game.wad_data.get_lump(game.page.name)
        {
            game.page.cache = WadPatch::from_lump(lump);
        }
        self.screen
            .pixels
            .fill(PixelFmt::from_argb(BLACK, ByteOrder::Argb));
        let viewport = self.screen.ui_layout().centered();
        let palette = game.pic_data.wad_palette();
        UiCanvas::new(&mut self.screen, palette, viewport).patch(&game.page.cache, [0.0, 0.0]);
    }
}

/// The view of the console player at the end of the last tic, or none while no map object exists.
fn render_view(player: &Player, game_tic: u32) -> Option<RenderView> {
    let mobj = player.mobj()?;
    let psprites = std::array::from_fn(|index| {
        let sprite = &player.psprites[index];
        sprite
            .state
            .map_or_else(RenderPspDef::default, |state| RenderPspDef {
                active: true,
                sprite: state.sprite as usize,
                frame: state.frame,
                sx: sprite.sx,
                sy: sprite.sy,
            })
    });
    Some(RenderView {
        x: mobj.x,
        y: mobj.y,
        z: mobj.z,
        viewz: player.viewz,
        viewheight: player.viewheight,
        angle: Angle::<Bam>::from_bam(mobj.angle.to_bam()),
        lookdir: lookdir_to_radian(i16::try_from(player.lookdir).map_or(0.0, f32::from)),
        fixedcolormap: usize::try_from(player.fixedcolormap).unwrap_or_default(),
        extralight: player.extralight,
        is_shadow: mobj.flags.contains(MapObjFlag::Shadow),
        psprites,
        sector_lightlevel: mobj.subsector.sector.lightlevel,
        player_mobj_id: std::ptr::from_ref(mobj).addr(),
        frac: 1.0,
        frac_fp: FixedT::ONE,
        game_tic,
        head_rotation: None,
        weapon_anchor: None,
        teleport: None,
    })
}
