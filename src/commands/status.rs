// Shows working tree status: staged, modified, untracked files
use colored::Colorize;
use std::collections::{BTreeSet, HashMap};
use std::fs;

use crate::ignore::IgnoreRules;
use crate::index::Index;
use crate::object::Object;
use crate::repo;

pub fn execute() -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?
        .to_path_buf();
    let index = Index::load(&vrit_dir)?;
    let ignore = IgnoreRules::load(&repo_root);

    // Check for detached HEAD
    let head_content = fs::read_to_string(vrit_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    let head_trimmed = head_content.trim();
    if !head_trimmed.starts_with("ref: ") {
        println!(
            "{}",
            format!("HEAD detached at {}", &head_trimmed[..7.min(head_trimmed.len())]).red()
        );
    } else if let Some(branch) = head_trimmed.strip_prefix("ref: refs/heads/") {
        println!("On branch {branch}");
    }

    // Check merge state
    if vrit_dir.join("MERGE_HEAD").exists() {
        println!("You have unmerged changes.");
    }

    // Get HEAD tree entries for comparison
    let head_entries: HashMap<String, String> = repo::head_tree_entries(&vrit_dir)?.into_iter().map(|(p, s, _)| (p, s)).collect();

    // Staged changes: diff HEAD tree vs index
    let mut staged_new: Vec<String> = Vec::new();
    let mut staged_modified: Vec<String> = Vec::new();
    let mut staged_deleted: Vec<String> = Vec::new();

    let index_paths: BTreeSet<&str> = index.entries.iter().map(|e| e.path.as_str()).collect();

    for entry in &index.entries {
        if let Some(head_sha) = head_entries.get(&entry.path) {
            if head_sha != &entry.sha {
                staged_modified.push(entry.path.clone());
            }
        } else {
            staged_new.push(entry.path.clone());
        }
    }
    for path in head_entries.keys() {
        if !index_paths.contains(path.as_str()) {
            staged_deleted.push(path.clone());
        }
    }

    // Unstaged changes: diff index vs working tree
    let mut modified: Vec<String> = Vec::new();
    let mut deleted: Vec<String> = Vec::new();

    for entry in &index.entries {
        let file_path = repo_root.join(&entry.path);
        if !file_path.exists() {
            deleted.push(entry.path.clone());
        } else {
            let content = match fs::read(&file_path) {
                Ok(c) => c,
                Err(_) => {
                    modified.push(entry.path.clone());
                    continue;
                }
            };
            let blob = Object::Blob(content);
            if blob.sha() != entry.sha {
                modified.push(entry.path.clone());
            }
        }
    }

    // Untracked files
    let mut untracked = find_untracked(&repo_root, &repo_root, &index, &ignore)?;
    untracked.sort();

    // Display
    let has_staged = !staged_new.is_empty() || !staged_modified.is_empty() || !staged_deleted.is_empty();
    let has_unstaged = !modified.is_empty() || !deleted.is_empty();
    let has_untracked = !untracked.is_empty();

    if has_staged {
        println!("Changes to be committed:");
        for path in &staged_new {
            println!("  {}", format!("new file:   {path}").green());
        }
        for path in &staged_modified {
            println!("  {}", format!("modified:   {path}").green());
        }
        for path in &staged_deleted {
            println!("  {}", format!("deleted:    {path}").green());
        }
        println!();
    }

    if has_unstaged {
        println!("Changes not staged for commit:");
        for path in &modified {
            println!("  {}", format!("modified:   {path}").red());
        }
        for path in &deleted {
            println!("  {}", format!("deleted:    {path}").red());
        }
        println!();
    }

    if has_untracked {
        println!("Untracked files:");
        for path in &untracked {
            println!("  {}", path.red());
        }
        println!();
    }

    if !has_staged && !has_unstaged && !has_untracked {
        println!("nothing to commit, working tree clean");
    }

    Ok(())
}

fn find_untracked(
    dir: &std::path::Path,
    repo_root: &std::path::Path,
    index: &Index,
    ignore: &IgnoreRules,
) -> Result<Vec<String>, String> {
    let mut untracked = Vec::new();

    let read_dir = fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory: {e}"))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("directory entry error: {e}"))?;
        let path = entry.path();

        let rel = path
            .strip_prefix(repo_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if rel.is_empty() {
            continue;
        }

        let is_dir = path.is_dir();
        if ignore.is_ignored(&rel, is_dir) {
            continue;
        }

        if is_dir {
            let sub = find_untracked(&path, repo_root, index, ignore)?;
            untracked.extend(sub);
        } else if index.get(&rel).is_none() {
            untracked.push(rel);
        }
    }

    Ok(untracked)
}
