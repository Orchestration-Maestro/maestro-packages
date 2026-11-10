//! A started terminal on a manual runtime that announces each redraw it is asked to schedule.
use maestro_tui::{
    TUI, TerminalImage,
    interface::{LocalFuture, LogContext, RenderCallback, RenderTimer, TuiRuntime},
    test_support::{manual_runtime::ManualRuntime, recording_terminal::RecordingTerminal},
};
use std::rc::Rc;
use tokio::sync::Notify;

/// A started terminal on a manual runtime whose scheduled work the test runs.
#[derive(Clone)]
pub struct Stage {
    /// The frame writer that factories receive.
    pub tui: TUI,
    /// The terminal under the frame writer; it records what the writer draws.
    pub terminal: RecordingTerminal,
    /// The runtime whose scheduled work `settle` runs.
    pub runtime: ManualRuntime,
    /// Wakes a waiter whenever the frame writer schedules a redraw.
    pub redraws: Rc<Notify>,
}

/// The manual runtime, announcing each redraw it is asked to schedule.
struct Announcing {
    /// The runtime that keeps the scheduled work.
    inner: Rc<dyn TuiRuntime>,
    /// Wakes the test.
    redraws: Rc<Notify>,
}

impl TuiRuntime for Announcing {
    fn now(&self) -> std::time::Duration {
        self.inner.now()
    }

    fn schedule(
        &self,
        delay: std::time::Duration,
        callback: RenderCallback,
    ) -> Box<dyn RenderTimer> {
        self.redraws.notify_one();
        self.inner.schedule(delay, callback)
    }

    fn spawn_local(&self, future: LocalFuture) {
        self.inner.spawn_local(future);
    }

    fn log_context(&self) -> LogContext {
        self.inner.log_context()
    }
}

/// Consumes a redraw announcement that arrived before anyone waited for it.
pub fn drain(redraws: &Notify) {
    let mut waiting = std::pin::pin!(redraws.notified());
    let _ = waiting
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
}

impl Stage {
    /// Starts a terminal of `columns` by `rows`.
    pub fn new(columns: usize, rows: usize) -> Self {
        let terminal = RecordingTerminal::new(columns, rows);
        let runtime = ManualRuntime::new();
        let redraws = Rc::new(Notify::new());
        let announcing = Announcing {
            inner: runtime.handle(),
            redraws: redraws.clone(),
        };
        let tui = TUI::new(
            terminal.handle(),
            Rc::new(announcing),
            TerminalImage::default(),
            Some(false),
        );
        tui.start().unwrap();
        runtime.settle().unwrap();
        drain(&redraws);
        Self {
            tui,
            terminal,
            runtime,
            redraws,
        }
    }

    /// Runs the work the frame writer scheduled.
    pub fn settle(&self) {
        self.runtime.settle().unwrap();
    }
}
