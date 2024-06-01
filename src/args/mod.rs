//! Arguments module for ssgen
//!
//! Parses command line arguments for ssgen into a struct for ease of access
//!
//! # Usage
//! ```
//! use args::{Args, Options};
//! let a: Args = Args::parse();
//! let o: Options = a.build_options();
//!
//! info!(o, "Options created!");
//! ```

/* IMPORTS */
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar};
use slog::{o, Drain, Level, Logger};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

/* LOCAL IMPORTS */
mod progressdrain;
use progressdrain::ProgressDrain;

/* MACROS */
/// Wrapper for slog error!() macro to fix indicatif progress bar duplication
#[macro_export]
macro_rules! error {
    ($target:expr, $($arg:tt)+) => (slog::error!($target.logger, $($arg)+));
}

/// Wrapper for slog warn!() macro to fix indicatif progress bar duplication
#[macro_export]
macro_rules! warn {
    ($target:expr, $($arg:tt)+) => (slog::warn!($target.logger, $($arg)+));
}

/// Wrapper for slog info!() macro to fix indicatif progress bar duplication
#[macro_export]
macro_rules! info {
    ($target:expr, $($arg:tt)+) => (slog::info!($target.logger, $($arg)+));
}

/// Wrapper for slog debug!() macro to fix indicatif progress bar duplication
#[macro_export]
macro_rules! debug {
    ($target:expr, $($arg:tt)+) => (slog::debug!($target.logger, $($arg)+));
}

/* OPTIONS */
/// Options struct for program settings
///
/// This struct should always be buit from Args like so:
/// ```
/// let o: Options = Args::parse().build_options();
/// ```
pub struct Options {
    /// Output directory for generated HTML
    pub output: PathBuf,

    /// Input directory for page files
    pub input: PathBuf,

    /// Global logger
    pub logger: Box<Logger>,

    /// Global progress bar
    pub progress: Arc<MultiProgress>,
}

/* ARGS */
/// Command-line arugments
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Output directory for generated HTML
    #[arg(short, long, value_name = "FILE", default_value = "./")]
    output: Box<Path>,

    /// Input directory for page files
    #[arg(short, long, value_name = "FILE")]
    input: Box<Path>,

    /// Show verbose messages
    #[arg(short, long)]
    verbose: bool,

    /// Suppress warning messages
    #[arg(short, long)]
    quiet: bool,

    /// Show debug messages
    #[arg(short, long)]
    debug: bool,

    /// Silence output
    #[arg(short, long)]
    silent: bool,
}

impl Args {
    /// Convert command-line arguments into usable Options struct
    ///
    /// Registers variables that are derived from arguments and creates global objects like the logger
    /// Is also a sanity check for arguments that ensures basic functionality won't be interrupted
    /// Does the following:
    /// - Set up logger
    /// - Canonicalise paths
    /// - Ensure input directory is not the same as output directory
    pub fn build_options(self) -> Options {
        // Set up logger

        let prog = Arc::new(MultiProgress::new());
        let drain = ProgressDrain::new(
            prog.clone(),
            if self.debug {
                Level::Debug
            } else if self.verbose {
                Level::Info
            } else if self.quiet {
                Level::Error
            } else if self.silent {
                Level::Critical
            } else {
                Level::Warning
            },
        );
        let drain = slog_async::Async::new(drain).build().fuse();
        let log = slog::Logger::root(drain, o!());

        slog::debug!(log, "Logger built!");

        let mut exit = false;

        // canonicalise paths TODO
        slog::debug!(log, "Canonicalising paths...");
        let input = match fs::canonicalize(&self.input) {
            Ok(p) => p,
            Err(e) => {
                slog::error!(log, "Error canonicalizing output path {}", e);
                exit = true;
                self.input.to_path_buf()
            }
        };
        let output = match fs::canonicalize(&self.output) {
            Ok(p) => p,
            Err(e) => {
                slog::error!(log, "Error canonicalizing input path {}", e);
                exit = true;
                self.output.to_path_buf()
            }
        };

        // sanity check
        if output == input {
            slog::error!(log, "Output directory is the same as Input directory!");
            exit = true;
        }

        if exit {
            slog::error!(
                log,
                "Sanity check failed! Please fix the above issues to proceed."
            );
            drop(log);
            #[cfg(not(test))]
            std::process::exit(0x1);
            #[cfg(test)]
            panic!("Sanity check fail panic");
        }

        // done
        return Options {
            input: input,
            output: output,
            logger: Box::new(log),
            progress: prog,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test wrapper macros debug!() info!() warn!() and error!()
    #[test]
    fn test_macros() {
        let o: Options = Args::parse_from(["", "--input", "./", "--output", "/tmp/", "--silent"])
            .build_options();
        debug!(o, "Test debug");
        info!(o, "Test info");
        warn!(o, "Test warn");
        error!(o, "Test error");
    }

    /// Ensure built options struct makes sense
    #[test]
    #[should_panic(expected = "Sanity check fail panic")]
    fn test_arguments() {
        // different loglevels
        let _: Options = Args::parse_from(["", "-i", "./", "-o", "/tmp", "-s"]).build_options();
        let _: Options = Args::parse_from(["", "-i", "./", "-o", "/tmp", "-q"]).build_options();
        let _: Options = Args::parse_from(["", "-i", "./", "-o", "/tmp", "-v"]).build_options();
        let _: Options = Args::parse_from(["", "-i", "./", "-o", "/tmp", "-d"]).build_options();
        let _: Options = Args::parse_from(["", "-i", "./", "-o", "/tmp"]).build_options();

        // fail sanity check
        let _: Options =
            Args::parse_from(["", "-i", "/ROOT/NONEXIST", "-o", "/ROOT/NONEXIST"]).build_options();
    }
}
