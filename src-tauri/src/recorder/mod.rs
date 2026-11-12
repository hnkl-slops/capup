//! Records an area of a display, without the app window, into an mp4 file with the OS's hardware encoder.

#[cfg_attr(target_os = "macos", path = "mac.rs")]
#[cfg_attr(target_os = "windows", path = "windows.rs")]
mod backend;

use std::{
    path::Path,
    sync::mpsc,
    thread::{self, JoinHandle},
};

pub use backend::Target;

use crate::region::Region;

/// A running recording.
pub struct Recorder {
    stop_tx: mpsc::Sender<()>,
    worker: JoinHandle<Result<(), String>>,
}

impl Recorder {
    /// Starts recording the region into the mp4 file at `output`.
    /// Returns once the capture has started, or with the reason it could not.
    pub fn start(region: Region, output: &Path) -> Result<Self, String> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let output = output.to_path_buf();
        // The native capture objects cannot be sent across threads, so they live on one thread from start to finish.
        let worker = thread::spawn(move || backend::record(region, &output, ready_tx, stop_rx));

        match ready_rx.recv() {
            Ok(()) => Ok(Self { stop_tx, worker }),
            Err(_) => Err(join(worker)
                .err()
                .unwrap_or_else(|| "The recorder stopped before it started".into())),
        }
    }

    /// Stops recording and waits until the mp4 file is finalized.
    pub fn finish(self) -> Result<(), String> {
        let _ = self.stop_tx.send(());
        join(self.worker)
    }
}

fn join(worker: JoinHandle<Result<(), String>>) -> Result<(), String> {
    worker
        .join()
        .map_err(|_| "The recorder thread panicked".to_string())?
}
