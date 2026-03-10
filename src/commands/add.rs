// Stages files by hashing them as blobs and updating the index
use std::fs;
use std::path::Path;

use crate::ignore::IgnoreRules;
use crate::index::{Index, IndexEntry};
use crate::object::Object;
use crate::repo;

pub fn execute(paths: &[String]) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?;
    let ignore = IgnoreRules::load(repo_root);
    let mut index = Index::load(&vrit_dir)?;

    for path_str in paths {
        let path = Path::new(path_str);
        if !path.exists() {
            // File was deleted — remove from index
            let rel = normalize_path(path_str, repo_root)?;
            if index.remove(&rel) {
                continue;
            }
            return Err(format!("pathspec '{path_str}' did not match any files"));
        }

        if path.is_dir() {
            add_directory(path, repo_root, &vrit_dir, &ignore, &mut index)?;
        } else {
            add_file(path, repo_root, &vrit_dir, &ignore, &mut index)?;
        }
    }

    // Auto-remove index entries for files deleted on disk, but only when their
    // parent directory was explicitly passed to `add`. This prevents `add file.txt`
    // from accidentally unstaging unrelated deleted files elsewhere in the tree.
    let deleted: Vec<String> = index
        .entries
        .iter()
        .filter(|e| !repo_root.join(&e.path).exists())
        .map(|e| e.path.clone())
        .collect();
    for path in &deleted {
        for path_str in paths {
            let p = Path::new(path_str);
            if p.is_dir() {
                let prefix = normalize_path(path_str, repo_root)?;
                if path.starts_with(&prefix) || prefix == "." {
                    index.remove(path);
                    break;
                }
            }
        }
    }

    index.save(&vrit_dir)?;
    Ok(())
}

fn add_directory(
    dir: &Path,
    repo_root: &Path,
    vrit_dir: &Path,
    ignore: &IgnoreRules,
    index: &mut Index,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory '{}': {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("directory entry error: {e}"))?;
        let path = entry.path();
        let rel = normalize_path(&path.to_string_lossy(), repo_root)?;

        if ignore.is_ignored(&rel, path.is_dir()) {
            continue;
        }

        // Skip symlinks
        if path.symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }

        if path.is_dir() {
            add_directory(&path, repo_root, vrit_dir, ignore, index)?;
        } else {
            add_file(&path, repo_root, vrit_dir, ignore, index)?;
        }
    }
    Ok(())
}

fn add_file(
    path: &Path,
    repo_root: &Path,
    vrit_dir: &Path,
    ignore: &IgnoreRules,
    index: &mut Index,
) -> Result<(), String> {
    let rel = normalize_path(&path.to_string_lossy(), repo_root)?;

    if ignore.is_ignored(&rel, false) {
        return Ok(());
    }

    // Skip symlinks
    if path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Ok(());
    }

    let content = fs::read(path)
        .map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    let blob = Object::Blob(content);
    let sha = blob.write_to_store(vrit_dir)?;

    let mode = file_mode(path);
    index.add(IndexEntry {
        mode,
        sha,
        path: rel,
    });

    Ok(())
}

fn file_mode(path: &Path) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(path).ok();
        if let Some(m) = meta {
            if m.permissions().mode() & 0o111 != 0 {
                return 0o100755;
            }
        }
    }
    0o100644
}

fn normalize_path(path: &str, repo_root: &Path) -> Result<String, String> {
    let abs = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cannot get cwd: {e}"))?
            .join(path)
    };

    let abs = abs
        .canonicalize()
        .unwrap_or(abs);

    let root = repo_root
        .canonicalize()
        .unwrap_or(repo_root.to_path_buf());

    abs.strip_prefix(&root)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|_| format!("path '{}' is outside the repository", path))
}
