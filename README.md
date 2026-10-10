# tracker

Local semantic search over the files on your machine. Rust + SQLite, Unix-first (macOS and Linux).

## Run

```bash
cargo run --release -- ~/Documents
```

Prints `size  mtime  path` for every regular file under the directory, then a file count and timing on stderr.

## Roadmap

1. [x] **Crawl**: walk a directory tree and print file metadata
2. [ ] **Store**: keep that metadata in SQLite; detect new, changed and deleted files between runs
3. [ ] **Keyword search**: extract text from plain-text files, split it into chunks, index it with FTS5
4. [ ] **Semantic search**: embed the chunks, search by vector, merge with keyword results
5. [ ] **More**: PDFs and images, file watching (FSEvents/inotify), background indexing
