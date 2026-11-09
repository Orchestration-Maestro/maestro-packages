//! Recording console effects with selectable write or flush failure.
use std::io::{self, Write};

/// Console calls retained for assertions while a real session emits events.
#[derive(Default)]
pub struct RecordingWriter {
    /// Ordered byte writes and flushes, including failed attempts.
    pub calls: Vec<String>,
    /// Whether every byte write is refused.
    pub refuse_writes: bool,
    /// Whether every flush is refused.
    pub refuse_flushes: bool,
}

impl Write for RecordingWriter {
    /// Record a byte write before optionally refusing it.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls.push(String::from_utf8_lossy(bytes).into_owned());
        if self.refuse_writes {
            return Err(io::Error::other(format!(
                "write failure {}",
                self.calls.len()
            )));
        }
        Ok(bytes.len())
    }

    /// Record a flush before optionally refusing it.
    fn flush(&mut self) -> io::Result<()> {
        self.calls.push("flush".into());
        if self.refuse_flushes {
            return Err(io::Error::other(format!(
                "flush failure {}",
                self.calls.len()
            )));
        }
        Ok(())
    }
}
