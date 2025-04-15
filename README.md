# Semantic File Search

A native, system-wide semantic file indexer written in C.

Recursively scan a specified directory, calculate file hashes, and store file descriptions in SQLite.

---

- Recursive directory crawling
- SHA-256 file hashing
- SQLite metadata and description storage
- Change detection using hash + mtime + size
- Basic file type inference from extension

---

## Build

### Dependencies

- `sqlite3`
- `openssl`

### macOS (with Homebrew)

```bash
brew install sqlite3 openssl
gcc tracker.c -o indexer \
  -I$(brew --prefix openssl)/include \
  -L$(brew --prefix openssl)/lib \
  -lsqlite3 -lssl -lcrypto \
  -Wall -Wextra -Wno-deprecated-declarations
```

### Linux (Debian/Ubuntu)

```bash
sudo apt install build-essential libsqlite3-dev libssl-dev
gcc tracker.c -o indexer \
  -lsqlite3 -lssl -lcrypto \
  -Wall -Wextra
```

### Usage

```bash
./indexer <database_path> <directory_to_scan> [max_depth]
```

### Example

```bash
./indexer myindex.db ~/Desktop/test_scan 1
```

### Planned Features
- AI-based description generation (CLIP, LLMs)
- File format-aware parsing (PDF, DOCX, code, etc)
- CLI search frontend (possibly in Rust or C)
- Live file system change tracking (libgit2 or inotify/FSEvents)
- Ignore rules (.searchignore)
- Fuzzy text search via ripgrep
