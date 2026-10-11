# tracker

Local semantic search over the files on your machine. Rust + SQLite, Unix-first (macOS and Linux).

## Run

```bash
cd rust
cargo run --release -- ~/Documents
```

Crawls the directory and records every regular file in `~/.tracker/tracker.db`, then prints how many files were new, changed, unchanged or deleted since the last crawl. To look inside:

```bash
sqlite3 ~/.tracker/tracker.db "SELECT CAST(path AS TEXT), byte_count, last_crawl FROM files LIMIT 10"
```

To start over (e.g. after a schema change), delete `~/.tracker/tracker.db`.

## Roadmap

1. [x] **Crawl**: walk a directory tree and print file metadata
2. [x] **Store**: keep that metadata in SQLite; detect new, changed and deleted files between runs
3. [ ] **Keyword search**: extract text from plain-text files, split it into chunks, index it with FTS5
4. [ ] **Semantic search**: embed the chunks, search by vector, merge with keyword results
5. [ ] **More**: PDFs and images, file watching (FSEvents/inotify), background indexing
