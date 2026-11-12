//! A frame-writer runtime that tells a test when the overlay asked for a frame, which it does once per game tic.
use maestro_extensions::EventError;
use maestro_tui::{
    interface::{LocalFuture, LogContext, RenderCallback, RenderTimer, TuiRuntime},
    test_support::manual_runtime::ManualRuntime,
};
use std::{rc::Rc, time::Duration};
use tokio::sync::{Notify, watch};

/// The controlled runtime of the test terminal, and the signal that a frame was asked for.
#[derive(Clone)]
pub struct Tics {
    /// Holds the scheduled frames until the test runs them.
    manual: ManualRuntime,
    /// Signalled each time the overlay asks for a frame.
    requested: Rc<Notify>,
    /// Whether the open view has closed.
    closed: Rc<watch::Sender<bool>>,
}
impl Tics {
    /// A runtime with nothing scheduled.
    pub fn new() -> Self {
        Self {
            manual: ManualRuntime::new(),
            requested: Rc::new(Notify::new()),
            closed: Rc::new(watch::Sender::new(false)),
        }
    }

    /// The runtime as the frame writer receives it.
    pub fn handle(&self) -> Rc<dyn TuiRuntime> {
        Rc::new(self.clone())
    }

    /// Records that a view opened, so an earlier view's close does not end a play on this one.
    pub fn open(&self) {
        self.closed.send_replace(false);
    }

    /// Records that the view closed, so a test waiting on a tic fails instead of hanging.
    pub fn close(&self) {
        self.closed.send_replace(true);
    }

    /// Lets the game run `count` more tics: waits for each frame request, then draws that frame.
    ///
    /// # Errors
    /// Returns an error when the view closes while the test waits.
    pub async fn run(&self, count: usize) -> Result<(), EventError> {
        let mut closed = self.closed.subscribe();
        for _ in 0..count {
            tokio::select! {
                () = self.requested.notified() => self.manual.settle().unwrap(),
                _ = closed.wait_for(|closed| *closed) => {
                    return Err(EventError::from("the view closed while the test waited for a game tic"));
                }
            }
        }
        Ok(())
    }
}
impl TuiRuntime for Tics {
    fn now(&self) -> Duration {
        self.manual.now()
    }

    fn schedule(&self, delay: Duration, callback: RenderCallback) -> Box<dyn RenderTimer> {
        let timer = self.manual.schedule(delay, callback);
        self.requested.notify_one();
        timer
    }

    fn spawn_local(&self, future: LocalFuture) {
        self.manual.spawn_local(future);
    }

    fn log_context(&self) -> LogContext {
        self.manual.log_context()
    }
}
