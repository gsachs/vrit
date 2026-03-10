// Moves HEAD to a commit and resets the index (mixed mode only)
use std::fs;

use crate::commands::commit::resolve_head;
use crate::commands::status::flatten_tree;
use crate::index::{Index, IndexEntry};
use crate::object::Object;
use crate::repo;

pub fn execute(commit: Option<&str>) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;

    let target_sha = match commit {
        Some(sha) => {
            // Verify it's a valid commit
            let obj = Object::read_from_store(&vrit_dir, sha)
                .map_err(|_| format!("commit '{sha}' not found"))?;
            match obj {
                Object::Commit(_) => sha.to_string(),
                _ => return Err(format!("'{sha}' is not a commit")),
            }
        }
        None => {
            // Default to HEAD — just unstage everything
            resolve_head(&vrit_dir)?
                .ok_or("no commits yet")?
        }
    };

    let current_sha = resolve_head(&vrit_dir)?;

    // If target differs from current HEAD, move the branch pointer
    if current_sha.as_deref() != Some(target_sha.as_str()) {
        update_current_ref(&vrit_dir, &target_sha)?;
        eprintln!(
            "Warning: commits after {} may become unreachable (no reflog).",
            &target_sha[..7]
        );
    }

    // Reset index to match target commit's tree
    let entries = flatten_tree_for_commit(&vrit_dir, &target_sha)?;
    let mut index = Index::new();
    for (path, sha) in &entries {
        index.add(IndexEntry {
            mode: 0o100644,
            sha: sha.clone(),
            path: path.clone(),
        });
    }
    index.save(&vrit_dir)?;

    Ok(())
}

fn flatten_tree_for_commit(
    vrit_dir: &std::path::Path,
    commit_sha: &str,
) -> Result<Vec<(String, String)>, String> {
    let obj = Object::read_from_store(vrit_dir, commit_sha)?;
    match obj {
        Object::Commit(cd) => flatten_tree(vrit_dir, &cd.tree, ""),
        _ => Err(format!("{commit_sha} is not a commit")),
    }
}

fn update_current_ref(vrit_dir: &std::path::Path, sha: &str) -> Result<(), String> {
    let head = fs::read_to_string(vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    let head = head.trim();

    if let Some(ref_path) = head.strip_prefix("ref: ") {
        let full_path = vrit_dir.join(ref_path);
        let tmp = full_path.with_extension("tmp");
        fs::write(&tmp, format!("{sha}\n"))
            .map_err(|e| format!("cannot write ref: {e}"))?;
        fs::rename(&tmp, &full_path)
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
