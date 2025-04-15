#include <dirent.h>
#include <libgen.h>
#include <openssl/sha.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#include <ctype.h>

#define MAX_PATH_LENGTH 4096
#define HASH_SIZE 65 // SHA-256 hex string (64 chars + null terminator)

int init_database(const char *db_path);
int crawl_directory(sqlite3 *db, const char *dir_path, int max_depth);
int process_file(sqlite3 *db, const char *file_path, struct stat *file_stat);
int check_file_changed(sqlite3 *db, const char *file_path, const char *hash,
                       time_t mtime, off_t size);
char *calculate_file_hash(const char *file_path);
int add_file_to_db(sqlite3 *db, const char *file_path, const char *hash,
                   struct stat *file_stat, const char *file_type,
                   const char *parent_dir, const char *file_name);
int update_file_in_db(sqlite3 *db, const char *file_path, const char *hash,
                      struct stat *file_stat);
char *get_file_type(const char *file_name);

int main(int argc, char **argv) {
  if (argc < 3) {
    fprintf(stderr,
            "Usage: %s <database_path> <directory_to_scan> [max_depth]\n",
            argv[0]);
    return 1;
  }

  const char *db_path = argv[1];
  const char *scan_dir = argv[2];
  int max_depth = -1; // -1 means no limit

  if (argc >= 4) {
    max_depth = atoi(argv[3]);
  }

  // Initialize the database
  if (init_database(db_path) != 0) {
    fprintf(stderr, "Failed to initialize database\n");
    return 1;
  }

  // Open database for writing
  sqlite3 *db;
  if (sqlite3_open(db_path, &db) != SQLITE_OK) {
    fprintf(stderr, "Cannot open database: %s\n", sqlite3_errmsg(db));
    return 1;
  }

  // Begin transaction for better performance
  sqlite3_exec(db, "BEGIN TRANSACTION", 0, 0, 0);

  // Crawl directory
  printf("Scanning directory: %s (max depth: %d)\n", scan_dir, max_depth);
  int result = crawl_directory(db, scan_dir, max_depth);

  // Commit transaction
  sqlite3_exec(db, "COMMIT", 0, 0, 0);

  // Close database
  sqlite3_close(db);

  if (result != 0) {
    fprintf(stderr, "Errors occurred during directory scan\n");
    return 1;
  }

  printf("Scan completed successfully\n");
  return 0;
}

// Initialize database with schema
int init_database(const char *db_path) {
  sqlite3 *db;
  char *err_msg = 0;
  int rc;

  // Open database (creates it if it doesn't exist)
  rc = sqlite3_open(db_path, &db);

  if (rc != SQLITE_OK) {
    fprintf(stderr, "Cannot open database: %s\n", sqlite3_errmsg(db));
    sqlite3_close(db);
    return 1;
  }

  // Create tables and indexes (schema should be loaded from schema.sql)
  // For simplicity, we'll include a simplified version here
  const char *sql =
      "CREATE TABLE IF NOT EXISTS files ("
      "id INTEGER PRIMARY KEY AUTOINCREMENT,"
      "file_path TEXT NOT NULL UNIQUE,"
      "file_name TEXT NOT NULL,"
      "file_size INTEGER,"
      "last_modified INTEGER,"
      "content_hash TEXT,"
      "file_type TEXT,"
      "indexed_at INTEGER,"
      "last_changed INTEGER,"
      "parent_dir TEXT"
      ");"

      "CREATE TABLE IF NOT EXISTS descriptions ("
      "id INTEGER PRIMARY KEY AUTOINCREMENT,"
      "file_id INTEGER NOT NULL,"
      "description TEXT,"
      "keywords TEXT,"
      "generated_at INTEGER,"
      "FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE"
      ");"

      "CREATE INDEX IF NOT EXISTS idx_files_hash ON files(content_hash);"
      "CREATE INDEX IF NOT EXISTS idx_files_path ON files(file_path);"
      "CREATE INDEX IF NOT EXISTS idx_files_type ON files(file_type);"
      "CREATE INDEX IF NOT EXISTS idx_files_parent ON files(parent_dir);"
      "CREATE INDEX IF NOT EXISTS idx_files_changed ON files(last_changed);";

  rc = sqlite3_exec(db, sql, 0, 0, &err_msg);

  if (rc != SQLITE_OK) {
    fprintf(stderr, "SQL error: %s\n", err_msg);
    sqlite3_free(err_msg);
    sqlite3_close(db);
    return 1;
  }

  // Create FTS5 virtual table for searching descriptions (if FTS5 is available)
  sql = "CREATE VIRTUAL TABLE IF NOT EXISTS description_fts USING fts5("
        "file_path, description, keywords, content='descriptions', "
        "content_rowid='id');";

  rc = sqlite3_exec(db, sql, 0, 0, &err_msg);

  // This might fail if FTS5 is not available, which is acceptable
  if (rc != SQLITE_OK) {
    fprintf(stderr, "Warning: Could not create FTS5 table: %s\n", err_msg);
    fprintf(stderr, "Search performance may be reduced\n");
    sqlite3_free(err_msg);
    // Don't return error here as FTS5 is optional
  }

  sqlite3_close(db);
  return 0;
}

// Recursively crawl a directory to find files
int crawl_directory(sqlite3 *db, const char *dir_path, int max_depth) {
  if (max_depth == 0) {
    return 0; // Reached max depth
  }

  DIR *dir;
  struct dirent *entry;
  struct stat file_stat;
  char file_path[MAX_PATH_LENGTH];

  if (!(dir = opendir(dir_path))) {
    perror("opendir");
    return 1;
  }

  while ((entry = readdir(dir)) != NULL) {
    if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
      continue;
    }

    // Skip hidden files and directories
    if (entry->d_name[0] == '.') {
      continue;
    }

    snprintf(file_path, sizeof(file_path), "%s/%s", dir_path, entry->d_name);

    if (lstat(file_path, &file_stat) == 0) {
      if (S_ISDIR(file_stat.st_mode)) {
        // Recursively process subdirectory
        int new_depth = max_depth > 0 ? max_depth - 1 : -1;
        crawl_directory(db, file_path, new_depth);
      } else if (S_ISREG(file_stat.st_mode)) {
        // Process regular file
        process_file(db, file_path, &file_stat);
      }
    }
  }

  closedir(dir);
  return 0;
}

// Process a single file
int process_file(sqlite3 *db, const char *file_path, struct stat *file_stat) {
  // Skip files that are too large (> 100MB by default)
  const long max_size = 100 * 1024 * 1024;
  if (file_stat->st_size > max_size) {
    printf("Skipping large file: %s (%ld bytes)\n", file_path,
           (long)file_stat->st_size);
    return 0;
  }

  // Extract filename and parent directory
  char path_copy[MAX_PATH_LENGTH];
  strncpy(path_copy, file_path, MAX_PATH_LENGTH);
  path_copy[MAX_PATH_LENGTH - 1] = '\0';
  char *file_name = basename(path_copy);

  char dir_copy[MAX_PATH_LENGTH];
  strncpy(dir_copy, file_path, MAX_PATH_LENGTH);
  dir_copy[MAX_PATH_LENGTH - 1] = '\0';
  char *parent_dir = dirname(dir_copy);

  // Get file type from extension
  char *file_type = get_file_type(file_name);

  // Calculate file hash for change detection
  char *hash = calculate_file_hash(file_path);
  if (!hash) {
    fprintf(stderr, "Failed to calculate hash for %s\n", file_path);
    return 1;
  }

  // Check if file has changed or is new
  int changed = check_file_changed(db, file_path, hash, file_stat->st_mtime,
                                   file_stat->st_size);

  if (changed == 1) {
    // File exists and has changed - update it
    update_file_in_db(db, file_path, hash, file_stat);
    printf("Updated: %s\n", file_path);
  } else if (changed == 0) {
    // File is new - add it
    add_file_to_db(db, file_path, hash, file_stat, file_type, parent_dir,
                   file_name);
    printf("Added: %s\n", file_path);
  } else {
    // File exists but hasn't changed - do nothing
    printf("Unchanged: %s\n", file_path);
  }

  free(hash);
  free(file_type);
  return 0;
}

// Check if a file has changed by comparing hash and metadata
int check_file_changed(sqlite3 *db, const char *file_path, const char *hash,
                       time_t mtime, off_t size) {
  sqlite3_stmt *stmt;
  int result = -1; // -1: unchanged, 0: new file, 1: changed

  const char *sql = "SELECT id, content_hash, last_modified, file_size FROM "
                    "files WHERE file_path = ?";

  if (sqlite3_prepare_v2(db, sql, -1, &stmt, NULL) != SQLITE_OK) {
    fprintf(stderr, "Failed to prepare statement: %s\n", sqlite3_errmsg(db));
    return 0; // Assume it's a new file on error
  }

  sqlite3_bind_text(stmt, 1, file_path, -1, SQLITE_STATIC);

  if (sqlite3_step(stmt) == SQLITE_ROW) {
    // File exists in database
    const char *old_hash = (const char *)sqlite3_column_text(stmt, 1);
    time_t old_mtime = sqlite3_column_int64(stmt, 2);
    off_t old_size = sqlite3_column_int64(stmt, 3);

    // Check if file has changed
    if (strcmp(hash, old_hash) != 0 || mtime != old_mtime || size != old_size) {
      result = 1; // File has changed
    } else {
      result = -1; // File is unchanged
    }
  } else {
    // File does not exist in database
    result = 0; // New file
  }

  sqlite3_finalize(stmt);
  return result;
}

// Calculate SHA-256 hash of file content
char *calculate_file_hash(const char *file_path) {
  FILE *file = fopen(file_path, "rb");
  if (!file) {
    perror("fopen");
    return NULL;
  }

  unsigned char hash[SHA256_DIGEST_LENGTH];
  SHA256_CTX sha256;
  SHA256_Init(&sha256);

  const int buf_size = 8192;
  unsigned char *buffer = malloc(buf_size);
  if (!buffer) {
    fclose(file);
    return NULL;
  }

  int bytes_read = 0;
  while ((bytes_read = fread(buffer, 1, buf_size, file)) > 0) {
    SHA256_Update(&sha256, buffer, bytes_read);
  }

  SHA256_Final(hash, &sha256);
  fclose(file);
  free(buffer);

  char *hash_str = malloc(HASH_SIZE);
  if (!hash_str) {
    return NULL;
  }

  for (int i = 0; i < SHA256_DIGEST_LENGTH; i++) {
    sprintf(&hash_str[i * 2], "%02x", hash[i]);
  }
  hash_str[64] = '\0';

  return hash_str;
}

// Add a new file to the database
int add_file_to_db(sqlite3 *db, const char *file_path, const char *hash,
                   struct stat *file_stat, const char *file_type,
                   const char *parent_dir, const char *file_name) {
  sqlite3_stmt *stmt;
  int result = 0;
  time_t current_time = time(NULL);

  const char *sql =
      "INSERT INTO files "
      "(file_path, file_name, file_size, last_modified, content_hash, "
      "file_type, indexed_at, last_changed, parent_dir) "
      "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

  if (sqlite3_prepare_v2(db, sql, -1, &stmt, NULL) != SQLITE_OK) {
    fprintf(stderr, "Failed to prepare statement: %s\n", sqlite3_errmsg(db));
    return 1;
  }

  sqlite3_bind_text(stmt, 1, file_path, -1, SQLITE_STATIC);
  sqlite3_bind_text(stmt, 2, file_name, -1, SQLITE_STATIC);
  sqlite3_bind_int64(stmt, 3, file_stat->st_size);
  sqlite3_bind_int64(stmt, 4, file_stat->st_mtime);
  sqlite3_bind_text(stmt, 5, hash, -1, SQLITE_STATIC);
  sqlite3_bind_text(stmt, 6, file_type, -1, SQLITE_STATIC);
  sqlite3_bind_int64(stmt, 7, current_time);
  sqlite3_bind_int64(stmt, 8, current_time);
  sqlite3_bind_text(stmt, 9, parent_dir, -1, SQLITE_STATIC);

  if (sqlite3_step(stmt) != SQLITE_DONE) {
    fprintf(stderr, "Failed to insert file: %s\n", sqlite3_errmsg(db));
    result = 1;
  }

  sqlite3_finalize(stmt);
  return result;
}

// Update an existing file in the database
int update_file_in_db(sqlite3 *db, const char *file_path, const char *hash,
                      struct stat *file_stat) {
  sqlite3_stmt *stmt;
  int result = 0;
  time_t current_time = time(NULL);

  const char *sql = "UPDATE files SET "
                    "file_size = ?, last_modified = ?, content_hash = ?, "
                    "indexed_at = ?, last_changed = ? "
                    "WHERE file_path = ?";

  if (sqlite3_prepare_v2(db, sql, -1, &stmt, NULL) != SQLITE_OK) {
    fprintf(stderr, "Failed to prepare statement: %s\n", sqlite3_errmsg(db));
    return 1;
  }

  sqlite3_bind_int64(stmt, 1, file_stat->st_size);
  sqlite3_bind_int64(stmt, 2, file_stat->st_mtime);
  sqlite3_bind_text(stmt, 3, hash, -1, SQLITE_STATIC);
  sqlite3_bind_int64(stmt, 4, current_time);
  sqlite3_bind_int64(stmt, 5, current_time);
  sqlite3_bind_text(stmt, 6, file_path, -1, SQLITE_STATIC);

  if (sqlite3_step(stmt) != SQLITE_DONE) {
    fprintf(stderr, "Failed to update file: %s\n", sqlite3_errmsg(db));
    result = 1;
  }

  sqlite3_finalize(stmt);
  return result;
}

// Get file type from filename
char *get_file_type(const char *file_name) {
  char *ext = strrchr(file_name, '.');
  if (ext != NULL) {
    ext++; // Skip the dot
    char *type = malloc(strlen(ext) + 1);
    if (type) {
      strcpy(type, ext);
      // Convert to lowercase
      for (char *p = type; *p; p++) {
        *p = tolower(*p);
      }
      return type;
    }
  }

  // No extension or memory allocation failed
  char *unknown = malloc(8);
  if (unknown) {
    strcpy(unknown, "unknown");
    return unknown;
  }
  return NULL;
}
