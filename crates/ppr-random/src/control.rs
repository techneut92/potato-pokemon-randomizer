use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared flag to stop a run. Cloning shares the flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Asks the run to stop at the next check.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// True once [`cancel`](Self::cancel) was called.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Progress of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Label of the step that is running: one of the pipeline's labels, or a label the caller adds for its own
    /// steps, like the number of manual edits it applies.
    pub step: Cow<'static, str>,
    /// 0 to 100.
    pub percent: u8,
}

/// Receives progress updates (Observer pattern). The CLI draws a bar, the desktop app sends a Tauri event.
pub trait ProgressSink {
    /// Called when a step starts and when the run completes.
    fn report(&mut self, progress: Progress);
}

impl<F: FnMut(Progress)> ProgressSink for F {
    fn report(&mut self, progress: Progress) {
        self(progress);
    }
}
