//walk directory tree and report every file in it.
//skip hidden files and node_modules, don't follow symlinks, don't trigger cloud-dir downloads,
//skip sockets and devices
//returns the dirs it couldn't fully read, so their files aren't mistaken for deleted.
//stops early if on_file returns an error.

use std::error::Error;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};

pub fn walk(
    root: &Path,
    mut on_file: impl FnMut(&Path, &Metadata) -> Result<(), Box<dyn Error>>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut pending = vec![root.to_path_buf()];
    let mut unreadable = Vec::new();

    while let Some(dir) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) => {
                skip(&dir, &err);
                unreadable.push(dir);
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    skip(&dir, &err);
                    unreadable.push(dir.clone());
                    continue;
                }
            };
            let name = entry.file_name();
            if name.as_encoded_bytes().starts_with(b".") || name == "node_modules" {
                continue;
            }

            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(err) => {
                    skip(&path, &err);
                    unreadable.push(dir.clone());
                    continue;
                }
            };

            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() {
                on_file(&path, &meta)?;
            }
        }
    }

    Ok(unreadable)
}

fn skip(path: &Path, err: &io::Error) {
    eprintln!("skipping {}: {err}", path.display());
}
