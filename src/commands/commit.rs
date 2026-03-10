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
    let parent = repo::resolve_head(&vrit_dir)?;
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

    // Git convention: commit messages always end with a newline
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
    repo::update_current_ref(&vrit_dir, &sha)?;

    // Clean up merge state if present
    let _ = fs::remove_file(vrit_dir.join("MERGE_HEAD"));
    let _ = fs::remove_file(vrit_dir.join("MERGE_MSG"));

    let short_sha = &sha[..7];
    println!("[{short_sha}] {}", commit_summary(&sha, &vrit_dir));

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
