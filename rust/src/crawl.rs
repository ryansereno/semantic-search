//walks directory tree and reports every regular file in it.

use std::fs::{self, Metadata};
use std::io;
use std::path::Path;

/// Walks the tree under `root`, calling `on_file` for every regular file.
///
/// - Hidden entries (names starting with `.`) are skipped along with
///   everything inside them: `.git`, `.cache`, `.Trash` and friends.
/// - Symlinks are never followed, so there are no cycles and no wandering
///   outside `root`.
/// - Only regular files are reported. FIFOs, sockets and devices are skipped.
/// - Files are `stat`ed, never opened, so a crawl is cheap and won't make
///   iCloud download the contents of cloud-only files.
/// - Errors (usually permission denied) are printed and skipped, so one
///   unreadable directory doesn't stop the crawl.
pub fn walk(root: &Path, mut on_file: impl FnMut(&Path, &Metadata)) {
    // Directories still to visit. A stack instead of recursion means a very
    // deep tree can't overflow the call stack.
    let mut pending = vec![root.to_path_buf()];

    while let Some(dir) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) => {
                skip(&dir, &err);
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    skip(&dir, &err);
                    continue;
                }
            };
            if entry.file_name().as_encoded_bytes().starts_with(b".") {
                continue;
            }

            let path = entry.path();
            // Like lstat(2): a symlink reports as a symlink, not as its target.
            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(err) => {
                    skip(&path, &err);
                    continue;
                }
            };

            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() {
                on_file(&path, &meta);
            }
        }
    }
}

fn skip(path: &Path, err: &io::Error) {
    eprintln!("skipping {}: {err}", path.display());
}
