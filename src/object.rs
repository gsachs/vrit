// Content-addressable object model — blob, tree, commit, tag
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// Represents the four object types in vrit's content-addressable store.
#[derive(Debug, PartialEq)]
pub enum Object {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
    Commit(CommitData),
    Tag(TagData),
}

#[derive(Debug, PartialEq, Clone)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub sha: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct CommitData {
    pub tree: String,
    pub parents: Vec<String>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TagData {
    pub object: String,
    pub object_type: String,
    pub tag_name: String,
    pub tagger: String,
    pub message: String,
}

impl Object {
    pub fn type_str(&self) -> &str {
        match self {
            Object::Blob(_) => "blob",
            Object::Tree(_) => "tree",
            Object::Commit(_) => "commit",
            Object::Tag(_) => "tag",
        }
    }

    /// Serialize the object body (without the header).
    pub fn serialize_body(&self) -> Vec<u8> {
        match self {
            Object::Blob(data) => data.clone(),
            Object::Tree(entries) => serialize_tree(entries),
            Object::Commit(data) => serialize_commit(data),
            Object::Tag(data) => serialize_tag(data),
        }
    }

    /// Serialize with the Git-compatible header: "<type> <size>\0<body>"
    pub fn serialize(&self) -> Vec<u8> {
        let body = self.serialize_body();
        let header = format!("{} {}\0", self.type_str(), body.len());
        let mut result = header.into_bytes();
        result.extend_from_slice(&body);
        result
    }

    /// Compute SHA-1 of the full serialized form (header + body).
    pub fn sha(&self) -> String {
        let data = self.serialize();
        Self::hash_bytes(&data)
    }

    fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Write the object to the object store (zlib-compressed).
    pub fn write_to_store(&self, vrit_dir: &Path) -> Result<String, String> {
        let data = self.serialize();
        let sha = Self::hash_bytes(&data);

        validate_sha(&sha)?; // also prevents directory traversal via crafted SHA in path construction below
        let dir = vrit_dir.join("objects").join(&sha[..2]);
        let file = dir.join(&sha[2..]);

        if file.exists() {
            return Ok(sha);
        }

        fs::create_dir_all(&dir)
            .map_err(|e| format!("failed to create object directory: {e}"))?;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&data)
            .map_err(|e| format!("zlib compress failed: {e}"))?;
        let compressed = encoder
            .finish()
            .map_err(|e| format!("zlib finish failed: {e}"))?;

        // Atomic write: write to temp file, then rename
        let tmp = dir.join(format!("tmp_{}", &sha[2..]));
        fs::write(&tmp, &compressed)
            .map_err(|e| format!("failed to write object: {e}"))?;
        fs::rename(&tmp, &file)
            .map_err(|e| format!("failed to rename object file: {e}"))?;

        Ok(sha)
    }

    /// Read an object from the store by its SHA.
    pub fn read_from_store(vrit_dir: &Path, sha: &str) -> Result<Object, String> {
        validate_sha(sha)?;
        let path = vrit_dir.join("objects").join(&sha[..2]).join(&sha[2..]);
        let compressed = fs::read(&path)
            .map_err(|_| format!("object not found: {sha}"))?;

        // Cap decompressed size to prevent a small compressed payload from expanding into OOM
        const MAX_OBJECT_SIZE: u64 = 100 * 1024 * 1024; // 100 MB
        let decoder = ZlibDecoder::new(&compressed[..]);
        let mut data = Vec::new();
        decoder
            .take(MAX_OBJECT_SIZE)
            .read_to_end(&mut data)
            .map_err(|e| format!("zlib decompress failed: {e}"))?;
        if data.len() as u64 >= MAX_OBJECT_SIZE {
            return Err("object exceeds maximum size (100 MB)".into());
        }

        parse_object(&data)
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Object::Blob(data) => {
                write!(f, "{}", String::from_utf8_lossy(data))
            }
            Object::Tree(entries) => {
                for entry in entries {
                    writeln!(f, "{} {} {}", entry.mode, entry.sha, entry.name)?;
                }
                Ok(())
            }
            Object::Commit(data) => {
                writeln!(f, "tree {}", data.tree)?;
                for parent in &data.parents {
                    writeln!(f, "parent {parent}")?;
                }
                writeln!(f, "author {}", data.author)?;
                writeln!(f, "committer {}", data.committer)?;
                writeln!(f)?;
                write!(f, "{}", data.message)
            }
            Object::Tag(data) => {
                writeln!(f, "object {}", data.object)?;
                writeln!(f, "type {}", data.object_type)?;
                writeln!(f, "tag {}", data.tag_name)?;
                writeln!(f, "tagger {}", data.tagger)?;
                writeln!(f)?;
                write!(f, "{}", data.message)
            }
        }
    }
}

fn serialize_tree(entries: &[TreeEntry]) -> Vec<u8> {
    let mut buf = Vec::new();
    for entry in entries {
        // Git tree format: "<mode> <name>\0<20-byte SHA>"
        buf.extend_from_slice(entry.mode.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(entry.name.as_bytes());
        buf.push(0);
        let sha_bytes = hex_to_bytes(&entry.sha)
            .expect("tree entry SHA should be valid hex");
        buf.extend_from_slice(&sha_bytes);
    }
    buf
}

fn serialize_commit(data: &CommitData) -> Vec<u8> {
    let mut s = format!("tree {}\n", data.tree);
    for parent in &data.parents {
        s.push_str(&format!("parent {parent}\n"));
    }
    s.push_str(&format!("author {}\n", data.author));
    s.push_str(&format!("committer {}\n", data.committer));
    s.push_str(&format!("\n{}", data.message));
    s.into_bytes()
}

fn serialize_tag(data: &TagData) -> Vec<u8> {
    let mut s = format!("object {}\n", data.object);
    s.push_str(&format!("type {}\n", data.object_type));
    s.push_str(&format!("tag {}\n", data.tag_name));
    s.push_str(&format!("tagger {}\n", data.tagger));
    s.push_str(&format!("\n{}", data.message));
    s.into_bytes()
}

/// Parse a raw object (header + body) into an Object.
fn parse_object(data: &[u8]) -> Result<Object, String> {
    let null_pos = data
        .iter()
        .position(|&b| b == 0)
        .ok_or("invalid object: no null byte in header")?;
    let header = std::str::from_utf8(&data[..null_pos])
        .map_err(|_| "invalid object header")?;

    let (obj_type, size_str) = header
        .split_once(' ')
        .ok_or("invalid object header format")?;
    let size: usize = size_str
        .parse()
        .map_err(|_| "invalid object size")?;

    let body = &data[null_pos + 1..];
    if body.len() != size {
        return Err(format!(
            "object size mismatch: header says {size}, body is {}",
            body.len()
        ));
    }

    match obj_type {
        "blob" => Ok(Object::Blob(body.to_vec())),
        "tree" => parse_tree(body),
        "commit" => parse_commit(body),
        "tag" => parse_tag(body),
        _ => Err(format!("unknown object type: {obj_type}")),
    }
}

fn parse_tree(data: &[u8]) -> Result<Object, String> {
    let mut entries = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let space_pos = data[i..]
            .iter()
            .position(|&b| b == b' ')
            .ok_or("invalid tree entry: no space")?
            + i;
        let null_pos = data[space_pos..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("invalid tree entry: no null")?
            + space_pos;

        let mode = std::str::from_utf8(&data[i..space_pos])
            .map_err(|_| "invalid mode in tree entry")?
            .to_string();
        let name = std::str::from_utf8(&data[space_pos + 1..null_pos])
            .map_err(|_| "invalid name in tree entry")?
            .to_string();

        // Reject traversal sequences that could escape the repo when tree entries are written to disk
        if name.contains("..") || name.starts_with('/') || name.contains('\0') {
            return Err(format!("invalid tree entry name: {name}"));
        }

        if null_pos + 1 + 20 > data.len() {
            return Err("tree entry truncated".into());
        }
        let sha = bytes_to_hex(&data[null_pos + 1..null_pos + 21]);

        entries.push(TreeEntry { mode, name, sha });
        i = null_pos + 21;
    }
    Ok(Object::Tree(entries))
}

fn parse_commit(data: &[u8]) -> Result<Object, String> {
    let text = std::str::from_utf8(data)
        .map_err(|_| "invalid commit: not UTF-8")?;

    let mut tree = String::new();
    let mut parents = Vec::new();
    let mut author = String::new();
    let mut committer = String::new();

    let (headers, message) = text
        .split_once("\n\n")
        .ok_or("invalid commit: no blank line separating headers and message")?;

    for line in headers.lines() {
        if let Some(val) = line.strip_prefix("tree ") {
            tree = val.to_string();
        } else if let Some(val) = line.strip_prefix("parent ") {
            parents.push(val.to_string());
        } else if let Some(val) = line.strip_prefix("author ") {
            author = val.to_string();
        } else if let Some(val) = line.strip_prefix("committer ") {
            committer = val.to_string();
        }
    }

    Ok(Object::Commit(CommitData {
        tree,
        parents,
        author,
        committer,
        message: message.to_string(),
    }))
}

fn parse_tag(data: &[u8]) -> Result<Object, String> {
    let text = std::str::from_utf8(data)
        .map_err(|_| "invalid tag: not UTF-8")?;

    let mut object = String::new();
    let mut object_type = String::new();
    let mut tag_name = String::new();
    let mut tagger = String::new();

    let (headers, message) = text
        .split_once("\n\n")
        .ok_or("invalid tag: no blank line separating headers and message")?;

    for line in headers.lines() {
        if let Some(val) = line.strip_prefix("object ") {
            object = val.to_string();
        } else if let Some(val) = line.strip_prefix("type ") {
            object_type = val.to_string();
        } else if let Some(val) = line.strip_prefix("tag ") {
            tag_name = val.to_string();
        } else if let Some(val) = line.strip_prefix("tagger ") {
            tagger = val.to_string();
        }
    }

    Ok(Object::Tag(TagData {
        object,
        object_type,
        tag_name,
        tagger,
        message: message.to_string(),
    }))
}

fn validate_sha(sha: &str) -> Result<(), String> {
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("invalid SHA: {sha}"));
    }
    Ok(())
}

pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err(format!("hex string has odd length: {}", hex.len()));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte at position {i}: {}", &hex[i..i + 2]))
        })
        .collect()
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(s, "{b:02x}").unwrap();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_roundtrip() {
        let content = b"hello world\n";
        let blob = Object::Blob(content.to_vec());
        let serialized = blob.serialize();
        let parsed = parse_object(&serialized).unwrap();
        assert_eq!(blob, parsed);
    }

    #[test]
    fn blob_sha_matches_git() {
        // Verified against: printf 'hello world\n' | git hash-object --stdin
        let blob = Object::Blob(b"hello world\n".to_vec());
        assert_eq!(blob.sha(), "3b18e512dba79e4c8300dd08aeb37f8e728b8dad");
    }

    #[test]
    fn tree_serialization_roundtrip() {
        let entries = vec![
            TreeEntry {
                mode: "100644".into(),
                name: "hello.txt".into(),
                sha: "3b18e512dba79e4c8300dd08aeb37f8e728b8dad".into(),
            },
            TreeEntry {
                mode: "40000".into(),
                name: "subdir".into(),
                sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            },
        ];
        let tree = Object::Tree(entries.clone());
        let serialized = tree.serialize();
        let parsed = parse_object(&serialized).unwrap();
        assert_eq!(Object::Tree(entries), parsed);
    }

    #[test]
    fn commit_roundtrip() {
        let commit = Object::Commit(CommitData {
            tree: "abc123".repeat(7)[..40].to_string(),
            parents: vec![],
            author: "Test User <test@example.com> 1234567890 +0000".into(),
            committer: "Test User <test@example.com> 1234567890 +0000".into(),
            message: "Initial commit\n".into(),
        });
        let serialized = commit.serialize();
        let parsed = parse_object(&serialized).unwrap();
        assert_eq!(commit, parsed);
    }

    #[test]
    fn tag_roundtrip() {
        let tag = Object::Tag(TagData {
            object: "abc123".repeat(7)[..40].to_string(),
            object_type: "commit".into(),
            tag_name: "v1.0".into(),
            tagger: "Test User <test@example.com> 1234567890 +0000".into(),
            message: "Release v1.0\n".into(),
        });
        let serialized = tag.serialize();
        let parsed = parse_object(&serialized).unwrap();
        assert_eq!(tag, parsed);
    }

    #[test]
    fn hex_bytes_roundtrip() {
        let hex = "95d09f2b10159347eece71399a7e2e907ea3df4f";
        let bytes = hex_to_bytes(hex).unwrap();
        assert_eq!(bytes.len(), 20);
        assert_eq!(bytes_to_hex(&bytes), hex);
    }
}
