// Saves and restores uncommitted changes via a stash stack
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::write_tree::write_tree_from_index;
use crate::config::Config;
use crate::index::{Index, IndexEntry};
use crate::object::{CommitData, Object};
use crate::repo;

pub fn execute_stash() -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?
        .to_path_buf();

    let head_sha = repo::resolve_head(&vrit_dir)?
        .ok_or("no commits yet — nothing to stash against")?;

    let head_entries_vec = repo::head_tree_entries(&vrit_dir)?;
    let head_entry_map: HashMap<String, String> = head_entries_vec
        .iter()
        .map(|(p, s, _)| (p.clone(), s.clone()))
        .collect();

    // Check if working tree is dirty
    let index = Index::load(&vrit_dir)?;
    let mut has_changes = false;

    for entry in &index.entries {
        let file_path = repo_root.join(&entry.path);
        if !file_path.exists() {
            has_changes = true;
            break;
        }
        let content = fs::read(&file_path)
            .map_err(|e| format!("cannot read '{}': {e}", entry.path))?;
        let blob = Object::Blob(content);
        if blob.sha() != entry.sha {
            has_changes = true;
            break;
        }
    }

    // Also check if index differs from HEAD
    if !has_changes {
        if index.entries.len() != head_entry_map.len() {
            has_changes = true;
        } else {
            for entry in &index.entries {
                let head_sha = head_entry_map.get(&entry.path).map(|s| s.as_str());
                if head_sha != Some(&entry.sha) {
                    has_changes = true;
                    break;
                }
            }
        }
    }

    if !has_changes {
        return Err("no local changes to save".into());
    }

    // Capture current state: write working tree files as blobs, build a tree
    let mut stash_index = Index::new();
    for entry in &index.entries {
        let file_path = repo_root.join(&entry.path);
        if file_path.exists() {
            let content = fs::read(&file_path)
                .map_err(|e| format!("cannot read '{}': {e}", entry.path))?;
            let blob = Object::Blob(content);
            let sha = blob.write_to_store(&vrit_dir)?;
            stash_index.add(IndexEntry {
                mode: entry.mode,
                sha,
                path: entry.path.clone(),
            });
        }
        // Deleted files are omitted from the stash tree
    }

    let stash_tree = write_tree_from_index(&stash_index, &vrit_dir)?;

    let config = Config::load(&vrit_dir)?;
    let name = config.require("user.name")?;
    let email = config.require("user.email")?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("time error: {e}"))?
        .as_secs();
    let author_line = format!("{name} <{email}> {timestamp} +0000");

    // Parent chain: HEAD is first parent, previous stash (if any) is second parent
    let stash_ref_path = vrit_dir.join("refs/stash");
    let mut parents = vec![head_sha.clone()];
    if stash_ref_path.exists() {
        let prev = fs::read_to_string(&stash_ref_path)
            .map_err(|e| format!("cannot read stash ref: {e}"))?
            .trim()
            .to_string();
        parents.push(prev);
    }

    let stash_commit = Object::Commit(CommitData {
        tree: stash_tree,
        parents,
        author: author_line.clone(),
        committer: author_line,
        message: "stash\n".to_string(),
    });
    let stash_sha = stash_commit.write_to_store(&vrit_dir)?;

    // Write stash ref
    fs::write(&stash_ref_path, format!("{stash_sha}\n"))
        .map_err(|e| format!("cannot write stash ref: {e}"))?;

    // Reset working tree and index to HEAD
    let mut new_index = Index::new();
    for (path, sha, mode) in &head_entries_vec {
        repo::write_blob_to_working_tree(&vrit_dir, &repo_root, path, sha, *mode)?;
        new_index.add(IndexEntry {
            mode: *mode,
            sha: sha.to_string(),
            path: path.to_string(),
        });
    }

    // Remove files tracked in stash but not in HEAD
    for entry in &stash_index.entries {
        if !head_entry_map.contains_key(&entry.path) {
            let file_path = repo_root.join(&entry.path);
            let _ = fs::remove_file(&file_path);
        }
    }

    new_index.save(&vrit_dir)?;

    println!("Saved working directory to stash");
    Ok(())
}

pub fn execute_pop() -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?
        .to_path_buf();

    let stash_ref_path = vrit_dir.join("refs/stash");
    if !stash_ref_path.exists() {
        return Err("no stash entries".into());
    }

    let stash_sha = fs::read_to_string(&stash_ref_path)
        .map_err(|e| format!("cannot read stash ref: {e}"))?
        .trim()
        .to_string();

    let obj = Object::read_from_store(&vrit_dir, &stash_sha)?;
    let cd = match obj {
        Object::Commit(cd) => cd,
        _ => return Err("stash ref does not point to a commit".into()),
    };

    // Restore stashed files to working tree
    let stash_entries = repo::flatten_tree(&vrit_dir, &cd.tree, "")?;
    let mut index = Index::load(&vrit_dir)?;

    for (path, sha, mode) in &stash_entries {
        repo::write_blob_to_working_tree(&vrit_dir, &repo_root, path, sha, *mode)?;
        index.add(IndexEntry {
            mode: *mode,
            sha: sha.to_string(),
            path: path.to_string(),
        });
    }
    index.save(&vrit_dir)?;

    // Update stash ref: pop to previous stash (second parent) or remove
    if cd.parents.len() > 1 {
        fs::write(&stash_ref_path, format!("{}\n", cd.parents[1]))
            .map_err(|e| format!("cannot update stash ref: {e}"))?;
    } else {
        fs::remove_file(&stash_ref_path)
            .map_err(|e| format!("cannot remove stash ref: {e}"))?;
    }

    println!("Applied stash and removed it");
    Ok(())
}

pub fn execute_list() -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let stash_ref_path = vrit_dir.join("refs/stash");

    if !stash_ref_path.exists() {
        return Ok(());
    }

    let mut sha = fs::read_to_string(&stash_ref_path)
        .map_err(|e| format!("cannot read stash ref: {e}"))?
        .trim()
        .to_string();

    let mut i = 0;
    loop {
        let obj = Object::read_from_store(&vrit_dir, &sha)?;
        let cd = match obj {
            Object::Commit(cd) => cd,
            _ => break,
        };

        let msg = cd.message.lines().next().unwrap_or("stash");
        println!("stash@{{{i}}}: {msg}");

        // Walk to previous stash via second parent
        if cd.parents.len() > 1 {
            sha = cd.parents[1].clone();
            i += 1;
        } else {
            break;
        }
    }

    Ok(())
}
