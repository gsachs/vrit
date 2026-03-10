// Creates a commit object from the current index state
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::write_tree::write_tree_from_index;
use crate::config::Config;
use crate::index::Index;
use crate::object::{CommitData, Object};
use crate::repo;

pub fn execute(message: &str) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let config = Config::load(&vrit_dir)?;

    let name = config.require("user.name")?;
    let email = config.require("user.email")?;

    let index = Index::load(&vrit_dir)?;
    if index.entries.is_empty() {
        return Err("nothing to commit (empty index)".into());
    }

    let tree_sha = write_tree_from_index(&index, &vrit_dir)?;

    // Check if anything changed since last commit
    let parent = resolve_head(&vrit_dir)?;
    if let Some(ref parent_sha) = parent {
        let parent_obj = Object::read_from_store(&vrit_dir, parent_sha)?;
        if let Object::Commit(ref cd) = parent_obj {
            if cd.tree == tree_sha {
                return Err("nothing to commit (no changes)".into());
            }
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system time error: {e}"))?
        .as_secs();
    let author_line = format!("{name} <{email}> {timestamp} +0000");

    let parents = if let Some(p) = parent {
        vec![p]
    } else {
        vec![]
    };

    // Check for merge state (MERGE_HEAD)
    let merge_head_path = vrit_dir.join("MERGE_HEAD");
    let mut all_parents = parents;
    if merge_head_path.exists() {
        let merge_sha = fs::read_to_string(&merge_head_path)
            .map_err(|e| format!("cannot read MERGE_HEAD: {e}"))?
            .trim()
            .to_string();
        all_parents.push(merge_sha);
    }

    let message = if message.ends_with('\n') {
        message.to_string()
    } else {
        format!("{message}\n")
    };

    let commit = Object::Commit(CommitData {
        tree: tree_sha,
        parents: all_parents,
        author: author_line.clone(),
        committer: author_line,
        message,
    });
    let sha = commit.write_to_store(&vrit_dir)?;

    // Update the current branch ref (or HEAD directly if detached)
    update_ref(&vrit_dir, &sha)?;

    // Clean up merge state if present
    let _ = fs::remove_file(vrit_dir.join("MERGE_HEAD"));
    let _ = fs::remove_file(vrit_dir.join("MERGE_MSG"));

    let short_sha = &sha[..7];
    println!("[{short_sha}] {}", commit_summary(&sha, &vrit_dir));

    Ok(())
}

/// Resolve HEAD to a commit SHA, or None for an unborn branch.
pub fn resolve_head(vrit_dir: &std::path::Path) -> Result<Option<String>, String> {
    let head_content = fs::read_to_string(vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    let head = head_content.trim();

    if let Some(ref_path) = head.strip_prefix("ref: ") {
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

fn update_ref(vrit_dir: &std::path::Path, sha: &str) -> Result<(), String> {
    let head_content = fs::read_to_string(vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    let head = head_content.trim();

    if let Some(ref_path) = head.strip_prefix("ref: ") {
        let ref_file = vrit_dir.join(ref_path);
        if let Some(parent) = ref_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create ref directory: {e}"))?;
        }
        // Atomic write
        let tmp = ref_file.with_extension("tmp");
        fs::write(&tmp, format!("{sha}\n"))
            .map_err(|e| format!("cannot write ref: {e}"))?;
        fs::rename(&tmp, &ref_file)
            .map_err(|e| format!("cannot update ref: {e}"))?;
    } else {
        // Detached HEAD
        let tmp = vrit_dir.join("HEAD.tmp");
        fs::write(&tmp, format!("{sha}\n"))
            .map_err(|e| format!("cannot write HEAD: {e}"))?;
        fs::rename(&tmp, vrit_dir.join("HEAD"))
            .map_err(|e| format!("cannot update HEAD: {e}"))?;
    }
    Ok(())
}

fn commit_summary(sha: &str, vrit_dir: &std::path::Path) -> String {
    Object::read_from_store(vrit_dir, sha)
        .ok()
        .and_then(|obj| {
            if let Object::Commit(cd) = obj {
                Some(cd.message.lines().next().unwrap_or("").to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}
