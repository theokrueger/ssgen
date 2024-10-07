//! ssgen
//!
//! Easy to use, highly flexible website builder, written in Rust
//! - Use YAML as a templating language to build your website
//! - High flexibility, yet easy to get started with
//! - Intelligent design becomes intuitive

/* IMPORTS */
use clap::Parser as ClapParser;
use glob::{glob_with, MatchOptions};
use indicatif::ProgressBar;
use pathdiff::diff_paths;
use std::{
    collections::HashMap, fs, path::PathBuf, sync::Arc, thread, thread::JoinHandle, time::Instant,
};

/* LOCAL IMPORTS */
mod args;
use args::{Args, Options};
mod pagenode;
use pagenode::PageNode;
mod parser;
use parser::Parser;

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

    /* METADATA */
    // parse the special "META.yaml" file
    let mut meta_file: PathBuf = o.input.clone();
    meta_file.push("META.yaml");
    let meta_vars: HashMap<Box<str>, Box<str>> = PageNode::consume_into_vars(
        if meta_file.exists() {
            info!(o, "META.yaml found! Parsing...");
            match fs::read_to_string(meta_file) {
                Ok(s) => {
                    let mut parser = Parser::new(o.clone());
                    parser.parse_yaml(s.as_str());
                    Parser::consume_into_root_node(parser)
                }
                Err(e) => {
                    panic!("Unable to read META.yaml despite file existing, please ensure permissions are correct: {e}");
                }
            }
        } else {
            info!(o, "META.yaml not found! Creating empty root node");
            PageNode::new(o.clone())
        },
    );

    /* THREADING */
    // one thread per page, scheduler will handle the hard part for us (TODO RIP memory usage)
    debug!(o, "Creating Page threads!");
    let pagebar = Arc::new(o.progress.add(ProgressBar::new(pages.len() as u64 + 1)));
    o.progress.set_move_cursor(true); // reduces flickering
    pagebar.tick();

    // create threads
    let mut handlers = Vec::<JoinHandle<()>>::new();
    pages.iter().for_each(|p| {
        let thread_pagefile = p.clone();
        let thread_o = o.clone();
        let thread_pagebar = pagebar.clone();
        let thread_meta_vars = meta_vars.clone();
        handlers.push(thread::spawn(move || {
            let mut parser = Parser::new_with_vars(thread_o.clone(), thread_meta_vars);
            let mut root_file = thread_pagefile.clone();
            root_file.pop();
            parser.set_root_dir(root_file.into());
            parser.add_progressbar(thread_pagebar);
            // read input
            info!(thread_o, "Reading file {}", thread_pagefile.display());
            match fs::read_to_string(thread_pagefile.clone()) {
                Ok(yaml) => parser.parse_yaml(yaml.as_str()),
                Err(e) => error!(
                    thread_o,
                    "Error reading file {f} | {e}",
                    f = thread_pagefile.display()
                ),
            }
            // write output
            let mut out_f = thread_o.output.clone();
            out_f.push(diff_paths(thread_pagefile, thread_o.input.clone()).unwrap());
            out_f.set_extension("html");
            let mut out_d = out_f.clone();
            out_d.pop(); // out_d now just directory containing file
            info!(thread_o, "Writing file {}", out_f.display());
            match fs::create_dir_all(out_d) {
                Ok(()) => match fs::write(out_f.clone(), format!("<!DOCTYPE html>\n{}", parser)) {
                    Ok(()) => (),
                    Err(e) => error!(
                        thread_o,
                        "Error writing file {f} | {e}",
                        f = out_f.display()
                    ),
                },
                Err(e) => error!(
                    thread_o,
                    "Error writing file {f} | {e}",
                    f = out_f.display()
                ),
            }
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
    pagebar.inc(1);
    pagebar.tick();
    info!(
        o,
        "Completed in {t} Seconds!",
        t = start_time.elapsed().as_secs_f32()
    );
    drop(o); // ensures logger gets flushed

    // for some reason we need to wait extra time on bebug builds for flush
    #[cfg(debug_assertions)]
    thread::sleep(std::time::Duration::from_millis(200));
}
