mod crawl;

use std::env;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

fn main() {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let started = Instant::now();
    let mut count: u64 = 0;
    let mut out = io::stdout().lock();

    crawl::walk(&root, |path, meta| {
        count += 1;
        let byte_count = meta.len(); 
        let last_modified = meta.mtime(); 
        let written = writeln!(out, "{byte_count:>12}  {last_modified}  {}", path.display());
        if written.is_err() {
            // stdout was closed, e.g. piped into `head`. Stop quietly.
            process::exit(0);
        }
    });

    // The summary goes to stderr so stdout stays clean for piping.
    eprintln!("{count} files in {:.2?}", started.elapsed());
}
