// Binary index (staging area) — sorted list of tracked file entries
use std::fs;
use std::io::{self, Read};
use std::path::Path;

const INDEX_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct IndexEntry {
    pub mode: u32,
    pub sha: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct Index {
    pub entries: Vec<IndexEntry>,
}

impl Index {
    pub fn new() -> Index {
        Index {
            entries: Vec::new(),
        }
    }

    /// Read the index from .vrit/index, or return an empty index if it doesn't exist.
    pub fn load(vrit_dir: &Path) -> Result<Index, String> {
        let path = vrit_dir.join("index");
        if !path.exists() {
            return Ok(Index::new());
        }
        let data = fs::read(&path)
            .map_err(|e| format!("cannot read index: {e}"))?;
        Self::deserialize(&data)
    }

    /// Write the index to .vrit/index using atomic rename.
    pub fn save(&self, vrit_dir: &Path) -> Result<(), String> {
        let data = self.serialize();
        let path = vrit_dir.join("index");
        let tmp = vrit_dir.join("index.tmp");
        fs::write(&tmp, &data)
            .map_err(|e| format!("cannot write index: {e}"))?;
        fs::rename(&tmp, &path)
            .map_err(|e| format!("cannot rename index: {e}"))?;
        Ok(())
    }

    /// Add or update an entry. Keeps entries sorted by path.
    pub fn add(&mut self, entry: IndexEntry) {
        match self.entries.binary_search_by(|e| e.path.cmp(&entry.path)) {
            Ok(i) => self.entries[i] = entry,
            Err(i) => self.entries.insert(i, entry),
        }
    }

    /// Remove an entry by path. Returns true if it existed.
    pub fn remove(&mut self, path: &str) -> bool {
        if let Ok(i) = self.entries.binary_search_by(|e| e.path.cmp(&path.to_string())) {
            self.entries.remove(i);
            true
        } else {
            false
        }
    }

    /// Remove all entries under a directory prefix.
    pub fn remove_dir(&mut self, dir: &str) {
        let prefix = if dir.ends_with('/') {
            dir.to_string()
        } else {
            format!("{dir}/")
        };
        self.entries.retain(|e| !e.path.starts_with(&prefix));
    }

    pub fn get(&self, path: &str) -> Option<&IndexEntry> {
        self.entries
            .binary_search_by(|e| e.path.cmp(&path.to_string()))
            .ok()
            .map(|i| &self.entries[i])
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(INDEX_VERSION);

        let count = self.entries.len() as u32;
        buf.extend_from_slice(&count.to_be_bytes());

        for entry in &self.entries {
            buf.extend_from_slice(&entry.mode.to_be_bytes());

            let sha_bytes = crate::object::hex_to_bytes(&entry.sha);
            buf.extend_from_slice(&sha_bytes);

            let path_bytes = entry.path.as_bytes();
            let path_len = path_bytes.len() as u16;
            buf.extend_from_slice(&path_len.to_be_bytes());
            buf.extend_from_slice(path_bytes);
        }
        buf
    }

    fn deserialize(data: &[u8]) -> Result<Index, String> {
        let mut cursor = io::Cursor::new(data);
        let mut byte = [0u8; 1];

        cursor.read_exact(&mut byte).map_err(|_| "truncated index")?;
        if byte[0] != INDEX_VERSION {
            return Err(format!("unsupported index version: {}", byte[0]));
        }

        let mut count_buf = [0u8; 4];
        cursor
            .read_exact(&mut count_buf)
            .map_err(|_| "truncated index header")?;
        let count = u32::from_be_bytes(count_buf) as usize;

        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let mut mode_buf = [0u8; 4];
            cursor
                .read_exact(&mut mode_buf)
                .map_err(|_| "truncated index entry")?;
            let mode = u32::from_be_bytes(mode_buf);

            let mut sha_buf = [0u8; 20];
            cursor
                .read_exact(&mut sha_buf)
                .map_err(|_| "truncated index entry sha")?;
            let sha = crate::object::bytes_to_hex(&sha_buf);

            let mut len_buf = [0u8; 2];
            cursor
                .read_exact(&mut len_buf)
                .map_err(|_| "truncated index entry path length")?;
            let path_len = u16::from_be_bytes(len_buf) as usize;

            let mut path_buf = vec![0u8; path_len];
            cursor
                .read_exact(&mut path_buf)
                .map_err(|_| "truncated index entry path")?;
            let path = String::from_utf8(path_buf)
                .map_err(|_| "invalid UTF-8 in index path")?;

            entries.push(IndexEntry { mode, sha, path });
        }

        Ok(Index { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_empty() {
        let index = Index::new();
        let data = index.serialize();
        let parsed = Index::deserialize(&data).unwrap();
        assert_eq!(parsed.entries.len(), 0);
    }

    #[test]
    fn roundtrip_with_entries() {
        let mut index = Index::new();
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "3b18e512dba79e4c8300dd08aeb37f8e728b8dad".into(),
            path: "hello.txt".into(),
        });
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            path: "src/main.rs".into(),
        });

        let data = index.serialize();
        let parsed = Index::deserialize(&data).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].path, "hello.txt");
        assert_eq!(parsed.entries[1].path, "src/main.rs");
        assert_eq!(parsed.entries[0].sha, "3b18e512dba79e4c8300dd08aeb37f8e728b8dad");
    }

    #[test]
    fn add_updates_existing() {
        let mut index = Index::new();
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "aaaa".repeat(10),
            path: "foo.txt".into(),
        });
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "bbbb".repeat(10),
            path: "foo.txt".into(),
        });
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].sha, "bbbb".repeat(10));
    }

    #[test]
    fn remove_entry() {
        let mut index = Index::new();
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "aaaa".repeat(10),
            path: "foo.txt".into(),
        });
        assert!(index.remove("foo.txt"));
        assert!(!index.remove("foo.txt"));
        assert_eq!(index.entries.len(), 0);
    }

    #[test]
    fn entries_stay_sorted() {
        let mut index = Index::new();
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "aaaa".repeat(10),
            path: "z.txt".into(),
        });
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "bbbb".repeat(10),
            path: "a.txt".into(),
        });
        index.add(IndexEntry {
            mode: 0o100644,
            sha: "cccc".repeat(10),
            path: "m.txt".into(),
        });
        let paths: Vec<&str> = index.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["a.txt", "m.txt", "z.txt"]);
    }
}
