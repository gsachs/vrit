// Switches branches or restores files from HEAD
use std::fs;
use std::path::Path;

use crate::commands::commit::resolve_head;
use crate::commands::status::flatten_tree;
use crate::index::{Index, IndexEntry};
use crate::object::Object;
use crate::repo;

pub fn execute(target: &str, file: Option<&str>) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?
        .to_path_buf();

    // checkout -- <file>: restore a single file from HEAD
    if let Some(file_path) = file {
        return restore_file(&vrit_dir, &repo_root, file_path);
    }

    // Determine if target is a branch or a commit SHA
    let branch_ref = vrit_dir.join("refs/heads").join(target);
    if branch_ref.exists() {
        checkout_branch(&vrit_dir, &repo_root, target)
    } else {
        // Try as SHA (detached HEAD)
        checkout_detached(&vrit_dir, &repo_root, target)
    }
}

fn checkout_branch(
    vrit_dir: &Path,
    repo_root: &Path,
    branch: &str,
) -> Result<(), String> {
    let ref_path = vrit_dir.join("refs/heads").join(branch);
    let target_sha = fs::read_to_string(&ref_path)
        .map_err(|e| format!("cannot read branch ref: {e}"))?
        .trim()
        .to_string();

    // Check if we're already on this branch
    let head = fs::read_to_string(vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    let head = head.trim();
    if head == format!("ref: refs/heads/{branch}") {
        println!("Already on '{branch}'");
        return Ok(());
    }

    switch_to_commit(vrit_dir, repo_root, &target_sha)?;

    // Update HEAD to point to branch
    let tmp = vrit_dir.join("HEAD.tmp");
    fs::write(&tmp, format!("ref: refs/heads/{branch}\n"))
        .map_err(|e| format!("cannot write HEAD: {e}"))?;
    fs::rename(&tmp, vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot update HEAD: {e}"))?;

    println!("Switched to branch '{branch}'");
    Ok(())
}

fn checkout_detached(
    vrit_dir: &Path,
    repo_root: &Path,
    sha: &str,
) -> Result<(), String> {
    // Verify the SHA points to a valid commit
    let obj = Object::read_from_store(vrit_dir, sha)
        .map_err(|_| format!("pathspec '{sha}' did not match any branch or commit"))?;
    match obj {
        Object::Commit(_) => {}
        _ => return Err(format!("{sha} is not a commit")),
    }

    switch_to_commit(vrit_dir, repo_root, sha)?;

    // Update HEAD to raw SHA
    let tmp = vrit_dir.join("HEAD.tmp");
    fs::write(&tmp, format!("{sha}\n"))
        .map_err(|e| format!("cannot write HEAD: {e}"))?;
    fs::rename(&tmp, vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot update HEAD: {e}"))?;

    eprintln!("Warning: you are in 'detached HEAD' state.");
    eprintln!("Commits made here will become unreachable when you switch branches.");
    println!("HEAD is now at {}", &sha[..7.min(sha.len())]);
    Ok(())
}

fn switch_to_commit(
    vrit_dir: &Path,
    repo_root: &Path,
    target_sha: &str,
) -> Result<(), String> {
    let index = Index::load(vrit_dir)?;

    // Check for dirty tracked files that would be overwritten
    check_dirty_files(vrit_dir, repo_root, &index, target_sha)?;

    // Get current and target tree entries
    let current_entries = current_tree_entries(vrit_dir)?;
    let target_entries = target_tree_entries(vrit_dir, target_sha)?;

    // Remove files that are in current tree but not in target
    for (path, _) in &current_entries {
        if !target_entries.iter().any(|(p, _)| p == path) {
            let file_path = repo_root.join(path);
            if file_path.exists() {
                let _ = fs::remove_file(&file_path);
            }
            // Clean up empty parent directories
            clean_empty_parents(&file_path, repo_root);
        }
    }

    // Write files from target tree
    let mut new_index = Index::new();
    for (path, sha) in &target_entries {
        let obj = Object::read_from_store(vrit_dir, sha)?;
        if let Object::Blob(content) = obj {
            let file_path = repo_root.join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create directory: {e}"))?;
            }
            fs::write(&file_path, &content)
                .map_err(|e| format!("cannot write '{path}': {e}"))?;
        }
        new_index.add(IndexEntry {
            mode: 0o100644,
            sha: sha.clone(),
            path: path.clone(),
        });
    }

    new_index.save(vrit_dir)?;
    Ok(())
}

fn check_dirty_files(
    vrit_dir: &Path,
    repo_root: &Path,
    index: &Index,
    target_sha: &str,
) -> Result<(), String> {
    let target_entries = target_tree_entries(vrit_dir, target_sha)?;

    for entry in &index.entries {
        let file_path = repo_root.join(&entry.path);
        if !file_path.exists() {
            continue;
        }

        // Check if file has unstaged changes
        let content = fs::read(&file_path).unwrap_or_default();
        let blob = Object::Blob(content);
        if blob.sha() != entry.sha {
            // File is dirty — check if checkout would overwrite it
            let target_sha_for_path = target_entries
                .iter()
                .find(|(p, _)| p == &entry.path)
                .map(|(_, s)| s.as_str());
            if target_sha_for_path != Some(&entry.sha) {
                return Err(format!(
                    "Your local changes to '{}' would be overwritten by checkout.\n\
                     Please commit or stash your changes before switching branches.",
                    entry.path
                ));
            }
        }
    }
    Ok(())
}

fn restore_file(
    vrit_dir: &Path,
    repo_root: &Path,
    file_path: &str,
) -> Result<(), String> {
    let head_sha = resolve_head(vrit_dir)?
        .ok_or("no commits yet")?;
    let entries = target_tree_entries(vrit_dir, &head_sha)?;

    let (_, sha) = entries
        .iter()
        .find(|(p, _)| p == file_path)
        .ok_or(format!("pathspec '{file_path}' did not match any file in HEAD"))?;

    let obj = Object::read_from_store(vrit_dir, sha)?;
    if let Object::Blob(content) = obj {
        let path = repo_root.join(file_path);
        fs::write(&path, &content)
            .map_err(|e| format!("cannot write '{file_path}': {e}"))?;

        // Update index
        let mut index = Index::load(vrit_dir)?;
        index.add(IndexEntry {
            mode: 0o100644,
            sha: sha.clone(),
            path: file_path.to_string(),
        });
        index.save(vrit_dir)?;

        println!("Updated '{file_path}' from HEAD");
    }
    Ok(())
}

fn current_tree_entries(vrit_dir: &Path) -> Result<Vec<(String, String)>, String> {
    match resolve_head(vrit_dir)? {
        Some(sha) => target_tree_entries(vrit_dir, &sha),
        None => Ok(Vec::new()),
    }
}

fn target_tree_entries(
    vrit_dir: &Path,
    commit_sha: &str,
) -> Result<Vec<(String, String)>, String> {
    let obj = Object::read_from_store(vrit_dir, commit_sha)?;
    match obj {
        Object::Commit(cd) => flatten_tree(vrit_dir, &cd.tree, ""),
        _ => Err(format!("{commit_sha} is not a commit")),
    }
}

fn clean_empty_parents(path: &Path, stop_at: &Path) {
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == stop_at {
            break;
        }
        if fs::read_dir(d).map(|mut r| r.next().is_none()).unwrap_or(true) {
            let _ = fs::remove_dir(d);
        } else {
            break;
        }
        dir = d.parent();
    }
}
