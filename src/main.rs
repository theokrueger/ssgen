//! ssgen
//!
//! Easy to use, highly flexible website builder, written in Rust
//! - Use YAML as a templating language to build your website
//! - High flexibility, yet easy to get started with
//! - Intelligent design becomes intuitive

/* IMPORTS */
use clap::Parser;
use glob::{glob_with, MatchOptions};
use indicatif::ProgressBar;
use std::{path::PathBuf, sync::Arc, thread, thread::JoinHandle, time::Instant};

/* LOCAL IMPORTS */
mod args;
use args::{Args, Options};

/* MAIN */
fn main() {
    /* SETUP */
    let start_time = Instant::now();
    let o: Arc<Options> = Arc::new(Args::parse().build_options());
    info!(o, "Starting SSGen...");

    /* PARSE PAGES */
    info!(o, "Walking input directory");
    // match any file in input directory that ends with .page (case insensitive)
    // safe because Options contains canonical paths
    let match_pages = o.input.clone().into_os_string().into_string().unwrap() + "/**/*.page";
    let mut pages = Vec::<PathBuf>::new();
    let walkspin = o.progress.add(ProgressBar::new_spinner());
    for entry in glob_with(
        match_pages.as_str(),
        MatchOptions {
            case_sensitive: false,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        },
    )
    .unwrap()
    {
        match entry {
            Ok(path) => {
                debug!(o, "Found file {}", path.display());
                walkspin.tick();
                pages.push(path);
            }
            Err(e) => error!(o, "Error finding file {}", e),
        }
    }
    walkspin.finish();

    // create one thread per page, let the scheduler handle the hard part lol
    debug!(o, "Creating Page threads!");
    let pagebar = Arc::new(o.progress.add(ProgressBar::new(pages.len() as u64 + 1)));
    pagebar.tick();

    let mut handlers = Vec::<JoinHandle<()>>::new();
    pages.iter().for_each(|p| {
        let thread_pagefile = p.clone();
        let thread_o = o.clone();
        let thread_pagebar = pagebar.clone();
        handlers.push(thread::spawn(move || {
            for i in 1..10 {
                info!(thread_o, "hi {} from {}!", i, thread_pagefile.display());
                thread::sleep(std::time::Duration::from_millis(100));
            }
            thread_pagebar.inc(1);
        }))
    });

    // collect threads
    debug!(o, "Collecting Page threads!");
    loop {
        match handlers.pop() {
            Some(t) => {
                t.join().unwrap();
            }
            None => break,
        };
    }

    /* CLEANUP */
    info!(
        o,
        "Completed in {t} Seconds!",
        t = start_time.elapsed().as_secs_f32()
    );
    drop(o); // ensures logger gets flushed
}
