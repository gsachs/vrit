// Repository discovery and shared ref/tree operations
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::object::Object;

const VRIT_DIR: &str = ".vrit";

/// Validates that a branch or tag name is safe (no path traversal).
pub fn validate_ref_name(name: &str) -> Result<(), String> {
    if name.contains("..") || name.starts_with('/') || name.starts_with('-')
        || name.contains('\0') || name.ends_with('/') || name.ends_with('.')
        || name.contains("//") {
        return Err(format!("invalid ref name: '{name}'"));
    }
    Ok(())
}

/// Walk up from the current directory to find a .vrit directory.
pub fn find_vrit_dir() -> Result<PathBuf, String> {
    let mut dir = env::current_dir()
        .map_err(|e| format!("cannot determine current directory: {e}"))?;
    loop {
        let candidate = dir.join(VRIT_DIR);
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if !dir.pop() {
            return Err("not a vrit repository (or any parent up to mount point)".into());
        }
    }
}

/// Resolve HEAD to a commit SHA, or None for an unborn branch.
pub fn resolve_head(vrit_dir: &Path) -> Result<Option<String>, String> {
    let head_content = fs::read_to_string(vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    let head = head_content.trim();

    if let Some(ref_path) = head.strip_prefix("ref: ") {
        if !ref_path.starts_with("refs/") || ref_path.contains("..") {
            return Err(format!("invalid HEAD ref path: {ref_path}"));
        }
        let ref_file = vrit_dir.join(ref_path);
        if ref_file.exists() {
            let sha = fs::read_to_string(&ref_file)
                .map_err(|e| format!("cannot read ref {ref_path}: {e}"))?;
            Ok(Some(sha.trim().to_string()))
        } else {
            Ok(None) // unborn branch
        }
    } else {
        // Detached HEAD — raw SHA
        Ok(Some(head.to_string()))
    }
}

/// Update the ref that HEAD points to (or HEAD itself if detached).
pub fn update_current_ref(vrit_dir: &Path, sha: &str) -> Result<(), String> {
    let head_content = fs::read_to_string(vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    let head = head_content.trim();

    if let Some(ref_path) = head.strip_prefix("ref: ") {
        let ref_file = vrit_dir.join(ref_path);
        if let Some(parent) = ref_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create ref directory: {e}"))?;
        }
        let tmp = ref_file.with_extension("tmp");
        fs::write(&tmp, format!("{sha}\n"))
            .map_err(|e| format!("cannot write ref: {e}"))?;
        fs::rename(&tmp, &ref_file)
            .map_err(|e| format!("cannot update ref: {e}"))?;
    } else {
        let tmp = vrit_dir.join("HEAD.tmp");
        fs::write(&tmp, format!("{sha}\n"))
            .map_err(|e| format!("cannot write HEAD: {e}"))?;
        fs::rename(&tmp, vrit_dir.join("HEAD"))
            .map_err(|e| format!("cannot update HEAD: {e}"))?;
    }
    Ok(())
}

/// Return the current branch name, or None if HEAD is detached.
pub fn current_branch(vrit_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(vrit_dir.join("HEAD")).ok()?;
    let head = head.trim();
    head.strip_prefix("ref: refs/heads/").map(|s| s.to_string())
}

/// Recursively flatten a tree object into (path, blob_sha, mode) triples.
pub fn flatten_tree(
    vrit_dir: &Path,
    tree_sha: &str,
    prefix: &str,
) -> Result<Vec<(String, String, u32)>, String> {
    let obj = Object::read_from_store(vrit_dir, tree_sha)?;
    let entries = match obj {
        Object::Tree(e) => e,
        _ => return Err(format!("{tree_sha} is not a tree")),
    };

    let mut result = Vec::new();
    for entry in &entries {
        let full_path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };

        if entry.mode == "40000" {
            result.extend(flatten_tree(vrit_dir, &entry.sha, &full_path)?);
        } else {
            let mode = u32::from_str_radix(&entry.mode, 8).unwrap_or(0o100644);
            result.push((full_path, entry.sha.clone(), mode));
        }
    }
    Ok(result)
}

/// Read a commit and flatten its tree into (path, blob_sha, mode) triples.
pub fn commit_tree_entries(
    vrit_dir: &Path,
    commit_sha: &str,
) -> Result<Vec<(String, String, u32)>, String> {
    let obj = Object::read_from_store(vrit_dir, commit_sha)?;
    match obj {
        Object::Commit(cd) => flatten_tree(vrit_dir, &cd.tree, ""),
        _ => Err(format!("{commit_sha} is not a commit")),
    }
}

/// Resolve HEAD and flatten its tree. Returns empty vec for unborn branches.
pub fn head_tree_entries(vrit_dir: &Path) -> Result<Vec<(String, String, u32)>, String> {
    match resolve_head(vrit_dir)? {
        Some(sha) => commit_tree_entries(vrit_dir, &sha),
        None => Ok(Vec::new()),
    }
}

/// Like `commit_tree_entries`, but returns a HashMap keyed by path.
pub fn commit_tree_entries_map(
    vrit_dir: &Path,
    commit_sha: &str,
) -> Result<HashMap<String, (String, u32)>, String> {
    Ok(commit_tree_entries(vrit_dir, commit_sha)?
        .into_iter()
        .map(|(p, s, m)| (p, (s, m)))
        .collect())
}

/// Recursively collect ref names from a directory (used for branches and tags).
pub fn collect_refs(
    dir: &Path,
    prefix: &str,
    results: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("directory entry error: {e}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let full_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };

        if path.is_dir() {
            collect_refs(&path, &full_name, results)?;
        } else {
            results.push(full_name);
        }
    }
    Ok(())
}

/// Write a blob from the object store to a file in the working tree, applying mode.
pub fn write_blob_to_working_tree(
    vrit_dir: &Path,
    repo_root: &Path,
    path: &str,
    sha: &str,
    mode: u32,
) -> Result<(), String> {
    let obj = Object::read_from_store(vrit_dir, sha)?;
    let content = match obj {
        Object::Blob(data) => data,
        _ => return Err(format!("expected blob for '{path}', got {}", obj.type_str())),
    };

    // Path traversal check: resolve against repo root without requiring file to exist.
    // Normalize by resolving the parent (which must exist after create_dir_all) and
    // appending the file name, avoiding canonicalize on the not-yet-created file.
    let file_path = repo_root.join(path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).ok();
        let parent_canonical = parent.canonicalize()
            .map_err(|e| format!("cannot resolve parent directory for '{path}': {e}"))?;
        let root_canonical = repo_root.canonicalize()
            .map_err(|e| format!("cannot resolve repo root: {e}"))?;
        if !parent_canonical.starts_with(&root_canonical) {
            return Err(format!("refusing to write outside repository: {path}"));
        }
    }

    fs::write(&file_path, &content)
        .map_err(|e| format!("cannot write '{path}': {e}"))?;
    apply_file_mode(&file_path, mode)?;
    Ok(())
}

/// Set file permissions based on the stored mode (executable bit on Unix).
fn apply_file_mode(file_path: &Path, mode: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let is_executable = mode == 0o100755;
        let metadata = fs::metadata(file_path)
            .map_err(|e| format!("cannot read metadata for '{}': {e}", file_path.display()))?;
        let mut perms = metadata.permissions();
        let current = perms.mode();
        let new_mode = if is_executable {
            current | 0o111
        } else {
            current & !0o111
        };
        perms.set_mode(new_mode);
        fs::set_permissions(file_path, perms)
            .map_err(|e| format!("cannot set permissions for '{}': {e}", file_path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (file_path, mode);
    }
    Ok(())
}
