//! Terminal-aware progress reporting.
//!
//! When stderr is an interactive terminal, shows live in-place updates
//! (thinking spinners, token counts) using `\r` carriage returns.
//!
//! When stderr is piped or redirected (e.g., captured by an LLM tool runner),
//! all ephemeral progress is suppressed. Only the final completion line is
//! emitted via `tracing::info!`, which always appears regardless of mode.
//!
//! # Tracing Level Strategy
//!
//! - `info`  — Phase headers, completion summaries, final results.
//!   Safe for LLM consumption. Concise. One line per major event.
//! - `debug` — Cache hits/misses, API call parameters, section boundaries,
//!   per-call token counts. Useful for human debugging.
//! - `trace` — Raw SSE deltas, full JSON responses. Only for deep debugging.

use crate::api::anthropic::StreamEvent;
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Global flag — set once at startup, never changes.
static IS_INTERACTIVE: AtomicBool = AtomicBool::new(false);

/// Call once at startup to detect whether stderr is an interactive terminal.
pub fn init() {
    IS_INTERACTIVE.store(std::io::stderr().is_terminal(), Ordering::Relaxed);
}

/// Returns true if stderr is a real terminal (not piped, not captured).
pub fn is_interactive() -> bool {
    IS_INTERACTIVE.load(Ordering::Relaxed)
}

/// Tracks progress for a single LLM streaming call.
///
/// In interactive mode, updates a single line on stderr with thinking/generating
/// status and estimated token counts. In non-interactive mode, does nothing
/// until `finish()` is called, which emits a single `tracing::debug!` line.
pub struct StreamProgress {
    label: String,
    start: Instant,
    thinking_chars: usize,
    text_chars: usize,
    interactive: bool,
    dirty: bool,
}

impl StreamProgress {
    /// Create a new progress tracker for a streaming LLM call.
    ///
    /// `label` is a short description like "Surveying" or "Extracting Title IV".
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            start: Instant::now(),
            thinking_chars: 0,
            text_chars: 0,
            interactive: is_interactive(),
            dirty: false,
        }
    }

    /// Handle a streaming event. Call this from the `send_message_streaming` callback.
    pub fn on_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::ThinkingDelta(t) => {
                self.thinking_chars += t.len();
                if self.interactive {
                    let est = self.thinking_chars / 4;
                    eprint!("\r    \u{1f914} {}... thinking (~{} tok)", self.label, est);
                    let _ = std::io::stderr().flush();
                    self.dirty = true;
                }
            }
            StreamEvent::TextDelta(t) => {
                self.text_chars += t.len();
                if self.interactive {
                    let est = self.text_chars / 4;
                    eprint!(
                        "\r    \u{1f4dd} {}... generating (~{} tok)",
                        self.label, est
                    );
                    let _ = std::io::stderr().flush();
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    /// Estimated thinking tokens (chars / 4).
    pub fn thinking_tokens_est(&self) -> usize {
        self.thinking_chars / 4
    }

    /// Estimated output tokens (chars / 4).
    pub fn text_tokens_est(&self) -> usize {
        self.text_chars / 4
    }

    /// Elapsed time since creation.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }

    /// Clear the in-place progress line (interactive mode only).
    /// Call this before printing a final status line with tracing.
    pub fn clear_line(&mut self) {
        if self.interactive && self.dirty {
            // Overwrite the progress line with blanks, then return cursor
            eprint!("\r{:80}\r", "");
            let _ = std::io::stderr().flush();
            self.dirty = false;
        }
    }

    /// Finish progress tracking. Clears the in-place line if interactive.
    /// Returns a `ProgressResult` with the final stats for logging.
    pub fn finish(mut self) -> ProgressResult {
        self.clear_line();
        ProgressResult {
            label: self.label,
            elapsed: self.start.elapsed(),
            thinking_tokens_est: self.thinking_chars / 4,
            text_tokens_est: self.text_chars / 4,
        }
    }
}

/// Summary of a completed streaming call, for structured logging.
pub struct ProgressResult {
    pub label: String,
    pub elapsed: std::time::Duration,
    pub thinking_tokens_est: usize,
    pub text_tokens_est: usize,
}

impl std::fmt::Display for ProgressResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: ~{} thinking + ~{} output tokens [{:.1?}]",
            self.label, self.thinking_tokens_est, self.text_tokens_est, self.elapsed,
        )
    }
}

/// Create a callback closure suitable for `send_message_streaming`.
///
/// Usage:
/// ```ignore
/// let mut progress = StreamProgress::new("Surveying");
/// let message = client.send_message_streaming(&req, progress_callback!(progress)).await?;
/// let result = progress.finish();
/// tracing::debug!("{result}");
/// ```
#[macro_export]
macro_rules! progress_callback {
    ($progress:expr) => {
        |event: &$crate::api::anthropic::StreamEvent| {
            $progress.on_event(event);
        }
    };
}
