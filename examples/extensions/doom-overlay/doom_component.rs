//! Half-block game frames and the lifetime of one visible overlay.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    doom_engine::DoomEngine,
    doom_keys::map_key_to_doom,
    doom_screen::{HEIGHT, WIDTH},
};
use gamestate_traits::KeyCode;
use maestro_extensions_wasm::{
    Background, BackgroundTask, CallbackEmitter, ComponentView, CustomDone, ExtensionAPI,
    ExtensionResult, TUI, is_key_release, matches_key,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::Write,
    rc::Rc,
    time::Duration,
};

/// Controls shown beneath the framebuffer.
const FOOTER: &str =
    " DOOM | Q=Pause | WASD=Move | Shift+WASD=Run | Space=Use | F=Fire | 1-7=Weapons";
/// Visibility and the work and input it owns.
#[derive(Default)]
struct ViewState {
    /// Whether disposal has already stopped the view.
    closed: bool,
    /// Codes admitted for each held primary key, including its run modifier.
    held: HashMap<KeyCode, Vec<KeyCode>>,
    /// Dropping this handle cancels the pending native sleep.
    task: Option<BackgroundTask>,
}
/// One visible view sharing the retained engine, not another engine instance.
#[derive(Clone)]
pub struct DoomOverlayComponent {
    /// Host key and native-wait operations.
    api: ExtensionAPI,
    /// The host's frame writer.
    tui: TUI,
    /// Engine retained across invocations.
    engine: Rc<DoomEngine>,
    /// First accepted close completes the custom view.
    done: CustomDone<()>,
    /// Shared state of the component, its tail and its disposer.
    state: Rc<RefCell<ViewState>>,
}
impl DoomOverlayComponent {
    /// Creates an overlay without starting another engine.
    pub fn new(api: ExtensionAPI, tui: TUI, engine: Rc<DoomEngine>, done: CustomDone<()>) -> Self {
        Self {
            api,
            tui,
            engine,
            done,
            state: Rc::default(),
        }
    }
    /// Starts one cancellable 35 Hz loop on the existing background owner.
    /// # Errors
    /// Returns a background admission failure.
    pub async fn start(&self, background: &Background) -> ExtensionResult<()> {
        let task = background.spawn(Box::pin(self.clone().run())).await?;
        self.state.borrow_mut().task = Some(task);
        Ok(())
    }
    /// Advances only while this view is visible.
    async fn run(self) -> ExtensionResult<()> {
        let io = self.api.native_io();
        loop {
            io.sleep(Duration::from_nanos(1_000_000_000 / 35), None)
                .await?;
            if self.state.borrow().closed {
                return Ok(());
            }
            if !self.engine.tick() {
                return self.close();
            }
            self.tui.request_render(false)?;
        }
    }
    /// Stops the timer and releases held input exactly once.
    /// # Errors
    /// Returns the host's refusal of closing the custom view.
    pub fn close(&self) -> ExtensionResult<()> {
        let (held, task) = {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return Ok(());
            }
            state.closed = true;
            (std::mem::take(&mut state.held), state.task.take())
        };
        drop(task);
        for code in held.into_values().flatten().collect::<HashSet<_>>() {
            self.engine.push_key(false, code);
        }
        (self.done)(())
    }
    /// A release uses the codes from its press even when modifier reporting changed.
    fn input(&self, codes: Vec<KeyCode>, released: bool) {
        let Some(primary) = codes.first().copied() else {
            return;
        };
        let unheld_codes = {
            let mut state = self.state.borrow_mut();
            let old = state.held.remove(&primary).unwrap_or_default();
            if !released {
                state.held.insert(primary, codes.clone());
            }
            old.into_iter()
                .filter(|code| !state.held.values().any(|held| held.contains(code)))
                .collect::<Vec<_>>()
        };
        for code in unheld_codes {
            self.engine.push_key(false, code);
        }
        if !released {
            for code in codes {
                self.engine.push_key(true, code);
            }
        }
    }
}
impl ComponentView for DoomOverlayComponent {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let mut lines = render_half_block(&self.engine.frame(), width)?;
        let columns = usize::try_from(width).map_err(|e| e.to_string())?;
        lines.push(format!(
            "\x1b[2m{}\x1b[0m",
            &FOOTER[..columns.min(FOOTER.len())]
        ));
        Ok(lines)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.state.borrow().closed {
            return Ok(());
        }
        let released = is_key_release(&self.api, data)?;
        if !released
            && (matches!(data, "q" | "Q")
                || matches_key(&self.api, data, "q")?
                || matches_key(&self.api, data, "shift+q")?)
            && !self.engine.continues_cheat('q')
        {
            return self.close();
        }
        self.input(map_key_to_doom(&self.api, data)?, released);
        Ok(())
    }
    fn wants_key_release(&self, _: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Ok(true)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}
/// Samples both pixels of each cell without stretching the terminal aspect ratio.
fn render_half_block(frame: &[u32], width: u32) -> ExtensionResult<Vec<String>> {
    let rows = (width / 16 * 5 + width % 16 * 5 / 16).max(10);
    let (frame_width, frame_height) = (u64::from(WIDTH), u64::from(HEIGHT));
    let mut lines = Vec::new();
    for row in 0..rows {
        let y1 = u64::from(row) * frame_height / u64::from(rows);
        let y2 = (u64::from(row) * 2 + 1) * frame_height / (u64::from(rows) * 2);
        let mut line = String::new();
        for column in 0..width {
            let x = u64::from(column) * frame_width / u64::from(width);
            let (top, bottom) = (color(frame, x, y1)?, color(frame, x, y2)?);
            write!(
                line,
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                top[0], top[1], top[2], bottom[0], bottom[1], bottom[2]
            )
            .map_err(|e| e.to_string())?;
        }
        lines.push(format!("{line}\x1b[0m"));
    }
    Ok(lines)
}
/// Reads one RGB triplet from a `0xAARRGGBB` pixel.
fn color(frame: &[u32], x: u64, y: u64) -> ExtensionResult<[u8; 3]> {
    let offset = usize::try_from(y * u64::from(WIDTH) + x).map_err(|e| e.to_string())?;
    let pixel = frame
        .get(offset)
        .ok_or("The frame is smaller than its size")?;
    let [_, red, green, blue] = pixel.to_be_bytes();
    Ok([red, green, blue])
}
