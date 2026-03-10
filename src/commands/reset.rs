// Moves HEAD to a commit and resets the index (mixed mode only)
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
            repo::resolve_head(&vrit_dir)?
                .ok_or("no commits yet")?
        }
    };

    let current_sha = repo::resolve_head(&vrit_dir)?;

    // If target differs from current HEAD, move the branch pointer
    if current_sha.as_deref() != Some(target_sha.as_str()) {
        repo::update_current_ref(&vrit_dir, &target_sha)?;
        eprintln!(
            "Warning: commits after {} may become unreachable (no reflog).",
            &target_sha[..7]
        );
    }

    // Reset index to match target commit's tree
    let entries = repo::commit_tree_entries(&vrit_dir, &target_sha)?;
    let mut index = Index::new();
    for (path, sha, mode) in &entries {
        index.add(IndexEntry {
            mode: *mode,
            sha: sha.clone(),
            path: path.clone(),
        });
    }
    index.save(&vrit_dir)?;

    Ok(())
}
