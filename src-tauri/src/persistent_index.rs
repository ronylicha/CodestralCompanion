use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use rusqlite::{Connection, params};
use sha2::{Sha256, Digest};

/// Persistent code index using SQLite
pub struct PersistentIndex {
    conn: Connection,
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct IndexedFileInfo {
    pub id: i64,
    pub relative_path: String,
    pub absolute_path: String,
    pub extension: String,
    pub content_hash: String,
    pub size: u64,
    pub modified_at: i64,
    pub description: Option<String>,
}

impl PersistentIndex {
    /// Open or create an index database in the project's .codestral folder
    pub fn open(project_root: &Path) -> Result<Self, String> {
        let codestral_dir = project_root.join(".codestral");
        fs::create_dir_all(&codestral_dir)
            .map_err(|e| format!("Cannot create .codestral directory: {}", e))?;
        
        let db_path = codestral_dir.join("index.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Cannot open index database: {}", e))?;
        
        // Create tables if needed
        conn.execute_batch(r"
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY,
                relative_path TEXT UNIQUE NOT NULL,
                absolute_path TEXT NOT NULL,
                extension TEXT,
                content_hash TEXT NOT NULL,
                size INTEGER,
                modified_at INTEGER,
                indexed_at INTEGER,
                description TEXT,
                content TEXT
            );
            
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY,
                file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
                tag TEXT NOT NULL
            );
            
            CREATE INDEX IF NOT EXISTS idx_files_path ON files(relative_path);
            CREATE INDEX IF NOT EXISTS idx_files_hash ON files(content_hash);
            CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);
        ").map_err(|e| format!("Cannot create tables: {}", e))?;
        
        Ok(Self {
            conn,
            root: project_root.to_path_buf(),
        })
    }
    
    /// Calculate SHA256 hash of file content
    fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    /// Get modification time as unix timestamp
    fn get_mtime(path: &Path) -> i64 {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).map_err(|_| std::io::Error::other("time error")))
            .unwrap_or(0)
    }
    
    /// Check if a file needs reindexing (hash changed or not in db)
    pub fn needs_reindex(&self, relative_path: &str, content: &str) -> bool {
        let hash = Self::hash_content(content);
        let result: Result<String, _> = self.conn.query_row(
            "SELECT content_hash FROM files WHERE relative_path = ?",
            params![relative_path],
            |row| row.get(0),
        );
        
        match result {
            Ok(stored_hash) => stored_hash != hash,
            Err(_) => true, // Not in DB
        }
    }
    
    /// Index or update a file
    pub fn index_file(
        &self,
        absolute_path: &Path,
        relative_path: &str,
        content: &str,
    ) -> Result<i64, String> {
        let hash = Self::hash_content(content);
        let extension = absolute_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let size = content.len() as u64;
        let mtime = Self::get_mtime(absolute_path);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        
        self.conn.execute(
            r"INSERT INTO files (relative_path, absolute_path, extension, content_hash, size, modified_at, indexed_at, content)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
              ON CONFLICT(relative_path) DO UPDATE SET
                absolute_path = excluded.absolute_path,
                extension = excluded.extension,
                content_hash = excluded.content_hash,
                size = excluded.size,
                modified_at = excluded.modified_at,
                indexed_at = excluded.indexed_at,
                content = excluded.content",
            params![
                relative_path,
                absolute_path.to_string_lossy().to_string(),
                extension,
                hash,
                size as i64,
                mtime,
                now,
                content
            ],
        ).map_err(|e| format!("Cannot index file: {}", e))?;
        
        Ok(self.conn.last_insert_rowid())
    }
    
    /// Add tags to a file
    pub fn add_tags(&self, file_id: i64, tags: &[&str]) -> Result<(), String> {
        for tag in tags {
            self.conn.execute(
                "INSERT OR IGNORE INTO tags (file_id, tag) VALUES (?1, ?2)",
                params![file_id, tag],
            ).map_err(|e| format!("Cannot add tag: {}", e))?;
        }
        Ok(())
    }
    
    /// Set description for a file
    pub fn set_description(&self, relative_path: &str, description: &str) -> Result<(), String> {
        self.conn.execute(
            "UPDATE files SET description = ?1 WHERE relative_path = ?2",
            params![description, relative_path],
        ).map_err(|e| format!("Cannot set description: {}", e))?;
        Ok(())
    }
    
    /// Get all indexed files
    pub fn list_files(&self) -> Result<Vec<IndexedFileInfo>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, relative_path, absolute_path, extension, content_hash, size, modified_at, description FROM files ORDER BY relative_path"
        ).map_err(|e| format!("Query error: {}", e))?;
        
        let rows = stmt.query_map([], |row| {
            Ok(IndexedFileInfo {
                id: row.get(0)?,
                relative_path: row.get(1)?,
                absolute_path: row.get(2)?,
                extension: row.get(3)?,
                content_hash: row.get(4)?,
                size: row.get::<_, i64>(5)? as u64,
                modified_at: row.get(6)?,
                description: row.get(7)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;
        
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Row error: {}", e))
    }
    
    /// Search files by path pattern
    pub fn search_by_path(&self, pattern: &str) -> Result<Vec<IndexedFileInfo>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, relative_path, absolute_path, extension, content_hash, size, modified_at, description 
             FROM files WHERE relative_path LIKE ?1 ORDER BY relative_path"
        ).map_err(|e| format!("Query error: {}", e))?;
        
        let rows = stmt.query_map(params![format!("%{}%", pattern)], |row| {
            Ok(IndexedFileInfo {
                id: row.get(0)?,
                relative_path: row.get(1)?,
                absolute_path: row.get(2)?,
                extension: row.get(3)?,
                content_hash: row.get(4)?,
                size: row.get::<_, i64>(5)? as u64,
                modified_at: row.get(6)?,
                description: row.get(7)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;
        
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Row error: {}", e))
    }
    
    /// Search files by tag
    pub fn search_by_tag(&self, tag: &str) -> Result<Vec<IndexedFileInfo>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.relative_path, f.absolute_path, f.extension, f.content_hash, f.size, f.modified_at, f.description 
             FROM files f JOIN tags t ON f.id = t.file_id WHERE t.tag = ?1 ORDER BY f.relative_path"
        ).map_err(|e| format!("Query error: {}", e))?;
        
        let rows = stmt.query_map(params![tag], |row| {
            Ok(IndexedFileInfo {
                id: row.get(0)?,
                relative_path: row.get(1)?,
                absolute_path: row.get(2)?,
                extension: row.get(3)?,
                content_hash: row.get(4)?,
                size: row.get::<_, i64>(5)? as u64,
                modified_at: row.get(6)?,
                description: row.get(7)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;
        
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Row error: {}", e))
    }
    
    /// Get file content by relative path
    pub fn get_content(&self, relative_path: &str) -> Result<Option<String>, String> {
        let result: Result<String, _> = self.conn.query_row(
            "SELECT content FROM files WHERE relative_path = ?",
            params![relative_path],
            |row| row.get(0),
        );
        
        match result {
            Ok(content) => Ok(Some(content)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query error: {}", e)),
        }
    }
    
    /// Get statistics
    pub fn stats(&self) -> Result<(usize, u64), String> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM files",
            [],
            |row| row.get(0),
        ).map_err(|e| format!("Query error: {}", e))?;
        
        let size: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(size), 0) FROM files",
            [],
            |row| row.get(0),
        ).map_err(|e| format!("Query error: {}", e))?;
        
        Ok((count as usize, size as u64))
    }
    
    /// Remove files not in the provided list (cleanup stale entries)
    pub fn cleanup_stale(&self, current_paths: &[String]) -> Result<usize, String> {
        if current_paths.is_empty() {
            return Ok(0);
        }
        
        // Get all paths in DB
        let mut stmt = self.conn.prepare("SELECT relative_path FROM files")
            .map_err(|e| format!("Query error: {}", e))?;
        
        let db_paths: Vec<String> = stmt.query_map([], |row| row.get(0))
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();
        
        let current_set: std::collections::HashSet<&String> = current_paths.iter().collect();
        let mut deleted = 0;
        
        for path in db_paths {
            if !current_set.contains(&path) {
                self.conn.execute("DELETE FROM files WHERE relative_path = ?", params![path])
                    .map_err(|e| format!("Delete error: {}", e))?;
                deleted += 1;
            }
        }
        
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_persistent_index() {
        let dir = tempdir().unwrap();
        let index = PersistentIndex::open(dir.path()).unwrap();
        
        // Index a file
        let id = index.index_file(
            Path::new("/test/file.rs"),
            "file.rs",
            "fn main() {}"
        ).unwrap();
        
        // Check stats
        let (count, _) = index.stats().unwrap();
        assert_eq!(count, 1);
        
        // Check content
        let content = index.get_content("file.rs").unwrap();
        assert_eq!(content, Some("fn main() {}".to_string()));
        
        // Check needs_reindex
        assert!(!index.needs_reindex("file.rs", "fn main() {}"));
        assert!(index.needs_reindex("file.rs", "fn main() { println!(); }"));
    }
}
