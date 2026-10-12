# tracker

Local semantic search over the files on your machine. Rust + SQLite, Unix-first (macOS and Linux).

## Run

```bash
cd rust
cargo run --release -- crawl ~/Documents
cargo run --release -- search quarterly estimate
```

`crawl` records every regular file in `~/.tracker/tracker.db` and prints how many were new, changed, unchanged or deleted since the last crawl. New and changed text files (by extension, up to 1 MB) are read, split into chunks of whole lines, and added to a word index. Identical files share one copy of their text.

`search` lists the chunks containing every word, best matches first, as `path:line` plus a snippet. To look inside the database:

```bash
sqlite3 ~/.tracker/tracker.db "SELECT CAST(path AS TEXT), byte_count, last_crawl FROM files LIMIT 10"
```

To start over (e.g. after a schema change), delete `~/.tracker/tracker.db`.

## Roadmap

1. [x] **Crawl**: walk a directory tree and print file metadata
2. [x] **Store**: keep that metadata in SQLite; detect new, changed and deleted files between runs
3. [x] **Keyword search**: extract text from plain-text files, split it into chunks, index it with FTS5
4. [ ] **Semantic search**: embed the chunks, search by vector, merge with keyword results
5. [ ] **More**: PDFs and images, file watching (FSEvents/inotify), background indexing
