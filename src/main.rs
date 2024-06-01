//! ssgen
//!
//! Easy to use, highly flexible website builder, written in Rust
//! - Use YAML as a templating language to build your website
//! - High flexibility, yet easy to get started with
//! - Intelligent design becomes intuitive

/* IMPORTS */
use clap::Parser;
use indicatif::ProgressBar;
use std::{sync::Arc, time::Instant};

/* LOCAL IMPORTS */
mod args;
use args::{Args, Options};

/* MAIN */
fn main() {
    /* SETUP */
    let start_time = Instant::now();
    let o: Arc<Options> = Arc::new(Args::parse().build_options());
    info!(o, "Starting SSGen...");

    /* CLEANUP */
    info!(
        o,
        "Completed in {t} Seconds!",
        t = start_time.elapsed().as_secs_f32()
    );
    drop(o); // ensures logger gets flushed
}
