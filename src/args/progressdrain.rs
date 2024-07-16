//! Terminal Drain for slog that supports a progress bar
//!
//! Ensures no collisions between a Multiprogress progress bar and printed text
//! Most likely slower than a slog_term drain
//! ```
//! use indicatif::{MultiProgress, ProgressBar};
//! use slog::{o, info, Level};
//! use std::sync::Arc;
//!
//! let prog = Arc::new(MultiProgress::new());
//! let drain = ProgressDrain::new(prog.clone(), Level::Info);
//! let drain = slog_async::Async::new(drain).build().fuse();
//! let log = slog::Logger::root(drain, o!());
//!
//! info!(log, "log loop test");
//! let pg = prog.add(ProgressBar::new(100));
//! for i in 0..100 {
//!     std::thread::sleep(std::time::Duration::from_micros(1000));
//!     info!(log, "iteration {}", i);
//!     pg.inc(1);
//! }
//! ```

/* IMPORTS */
use colored::Colorize;
use indicatif::MultiProgress;
use slog::{Drain, Level, Never, OwnedKVList, Record};
use std::{result::Result, sync::Arc};

/* PROGRESSDRAIN */
/// Slog Drain with progressbar support using indicatif::MultiProgress
pub struct ProgressDrain {
    /// MultiProgress clone for pausing progressbar during printouts
    progress: Arc<MultiProgress>,
    /// Verbosity level to log at
    level: Level,
}

impl ProgressDrain {
    /// Create a new ProgressDrain from given arguments
    pub fn new(prog: Arc<MultiProgress>, level: Level) -> ProgressDrain {
        return ProgressDrain {
            progress: prog,
            level: level,
        };
    }
}

impl Drain for ProgressDrain {
    type Ok = ();
    type Err = Never;

    /// Log to stdout while not interrupting progressbar
    fn log(&self, record: &Record, _: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        if self.level < record.level() {
            return Ok(());
        };
        let levelstr = format!("{}", record.level());
        let s = format!(
            "[{level}] {message}",
            level = match record.level() {
                Level::Error => levelstr.red(),
                Level::Warning => levelstr.yellow(),
                Level::Info => levelstr.blue(),
                Level::Debug => levelstr.green(),
                _ => levelstr.into(),
            },
            message = record.msg()
        );

        // debug build log formatting
        #[cfg(debug_assertions)]
        let s = s + format!(
            " {location}",
            location = format!(
                "{file}:{line}:{column}",
                file = record.file(),
                line = record.line(),
                column = record.column()
            )
        )
        .italic()
        .bold()
        .white()
        .to_string()
        .as_str();

        self.progress.println(s).unwrap();
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test logging cabailities of ProgressDrain
    #[test]
    fn test_log() {
        use indicatif::MultiProgress;
        use slog::{o, Level};
        use std::sync::Arc;
        let prog = Arc::new(MultiProgress::new());
        let drain = ProgressDrain::new(prog.clone(), Level::Trace);
        let log = slog::Logger::root(drain, o!());
        slog::crit!(log, "Critical test");
        slog::error!(log, "Error test");
        slog::warn!(log, "Warning test");
        slog::info!(log, "Info test");
        slog::debug!(log, "Debug test");
        slog::trace!(log, "Trace test");
    }

    /// Ensure progress bar works properly
    #[test]
    fn test_progress() {
        use indicatif::{MultiProgress, ProgressBar};
        use slog::{o, Level};
        use std::sync::Arc;
        let prog = Arc::new(MultiProgress::new());
        let drain = ProgressDrain::new(prog.clone(), Level::Critical);
        let log = slog::Logger::root(drain, o!());

        slog::info!(log, "log loop test");
        let pg = prog.add(ProgressBar::new(10));
        for i in 0..10 {
            slog::info!(log, "iteration {}", i);
            pg.inc(1);
        }
    }
}
