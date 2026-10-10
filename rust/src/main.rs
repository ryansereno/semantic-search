mod crawl;

use std::env;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

fn main() {
    // Usage: tracker [DIR]  (defaults to the current directory).
    // args_os rather than args: paths aren't guaranteed to be valid UTF-8.
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let started = Instant::now();
    let mut count: u64 = 0;
    let mut out = io::stdout().lock();

    crawl::walk(&root, |path, meta| {
        count += 1;
        let size = meta.len(); // bytes
        let mtime = meta.mtime(); // seconds since 1970
        let written = writeln!(out, "{size:>12}  {mtime}  {}", path.display());
        if written.is_err() {
            // stdout was closed, e.g. piped into `head`. Stop quietly.
            process::exit(0);
        }
    });

    // The summary goes to stderr so stdout stays clean for piping.
    eprintln!("{count} files in {:.2?}", started.elapsed());
}
