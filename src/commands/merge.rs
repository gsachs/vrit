// Merges a branch into the current branch — fast-forward or three-way
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::branch::current_branch;
use crate::commands::commit::resolve_head;
use crate::commands::status::flatten_tree;
use crate::config::Config;
use crate::index::{Index, IndexEntry};
use crate::object::{CommitData, Object};
use crate::repo;

pub fn execute(branch: Option<&str>, abort: bool) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?
        .to_path_buf();

    if abort {
        return abort_merge(&vrit_dir, &repo_root);
    }

    let branch = branch.ok_or("must specify a branch to merge")?;

    // Refuse if dirty working tree
    check_clean_working_tree(&vrit_dir, &repo_root)?;

    let head_sha = resolve_head(&vrit_dir)?
        .ok_or("no commits yet — nothing to merge into")?;

    // Resolve the branch to a commit SHA
    let ref_path = vrit_dir.join("refs/heads").join(branch);
    let other_sha = if ref_path.exists() {
        fs::read_to_string(&ref_path)
            .map_err(|e| format!("cannot read branch ref: {e}"))?
            .trim()
            .to_string()
    } else {
        // Try as raw SHA
        let obj = Object::read_from_store(&vrit_dir, branch)
            .map_err(|_| format!("branch '{branch}' not found"))?;
        match obj {
            Object::Commit(_) => branch.to_string(),
            _ => return Err(format!("'{branch}' is not a commit")),
        }
    };

    // Merge with self
    if head_sha == other_sha {
        println!("Already up to date.");
        return Ok(());
    }

    // Find merge base
    let base = find_merge_base(&vrit_dir, &head_sha, &other_sha)?;

    // Fast-forward: current is ancestor of other
    if base.as_deref() == Some(head_sha.as_str()) {
        return fast_forward(&vrit_dir, &repo_root, branch, &other_sha);
    }

    // Already up to date: other is ancestor of current
    if base.as_deref() == Some(other_sha.as_str()) {
        println!("Already up to date.");
        return Ok(());
    }

    // Three-way merge
    let base_sha = base.ok_or("cannot find common ancestor — branches have no common history")?;
    three_way_merge(&vrit_dir, &repo_root, &head_sha, &other_sha, &base_sha, branch)
}

fn fast_forward(
    vrit_dir: &Path,
    repo_root: &Path,
    branch: &str,
    target_sha: &str,
) -> Result<(), String> {
    // Update working tree and index to target
    let target_entries = get_commit_tree(vrit_dir, target_sha)?;
    let current_entries = get_head_tree(vrit_dir)?;

    // Remove files not in target
    for (path, _) in &current_entries {
        if !target_entries.iter().any(|(p, _)| p == path) {
            let file_path = repo_root.join(path);
            let _ = fs::remove_file(&file_path);
        }
    }

    // Write target files
    let mut index = Index::new();
    for (path, sha) in &target_entries {
        write_blob_to_working_tree(vrit_dir, repo_root, path, sha)?;
        index.add(IndexEntry {
            mode: 0o100644,
            sha: sha.clone(),
            path: path.clone(),
        });
    }
    index.save(vrit_dir)?;

    // Update branch ref
    update_current_ref(vrit_dir, target_sha)?;

    println!("Fast-forward merge to {branch} ({})", &target_sha[..7]);
    Ok(())
}

fn three_way_merge(
    vrit_dir: &Path,
    repo_root: &Path,
    head_sha: &str,
    other_sha: &str,
    base_sha: &str,
    branch_name: &str,
) -> Result<(), String> {
    let base_entries = get_commit_tree(vrit_dir, base_sha)?;
    let head_entries = get_commit_tree(vrit_dir, head_sha)?;
    let other_entries = get_commit_tree(vrit_dir, other_sha)?;

    // Collect all paths
    let mut all_paths: HashSet<String> = HashSet::new();
    for (p, _) in &base_entries {
        all_paths.insert(p.clone());
    }
    for (p, _) in &head_entries {
        all_paths.insert(p.clone());
    }
    for (p, _) in &other_entries {
        all_paths.insert(p.clone());
    }

    let mut index = Index::new();
    let mut has_conflicts = false;
    let current_name = current_branch(vrit_dir).unwrap_or_else(|| head_sha[..7].to_string());

    let mut sorted_paths: Vec<String> = all_paths.into_iter().collect();
    sorted_paths.sort();

    for path in &sorted_paths {
        let base_sha_opt = find_sha(&base_entries, path);
        let head_sha_opt = find_sha(&head_entries, path);
        let other_sha_opt = find_sha(&other_entries, path);

        match (base_sha_opt, head_sha_opt, other_sha_opt) {
            // Unchanged in both sides
            (_, Some(h), Some(o)) if h == o => {
                write_blob_to_working_tree(vrit_dir, repo_root, path, h)?;
                index.add(make_entry(path, h));
            }
            // Changed only in head side
            (Some(b), Some(h), Some(o)) if b == o && b != h => {
                write_blob_to_working_tree(vrit_dir, repo_root, path, h)?;
                index.add(make_entry(path, h));
            }
            // Changed only in other side
            (Some(b), Some(h), Some(o)) if b == h && b != o => {
                write_blob_to_working_tree(vrit_dir, repo_root, path, o)?;
                index.add(make_entry(path, o));
            }
            // Added only in head
            (None, Some(h), None) => {
                write_blob_to_working_tree(vrit_dir, repo_root, path, h)?;
                index.add(make_entry(path, h));
            }
            // Added only in other
            (None, None, Some(o)) => {
                write_blob_to_working_tree(vrit_dir, repo_root, path, o)?;
                index.add(make_entry(path, o));
            }
            // Deleted in one side, unchanged in other
            (Some(b), None, Some(o)) if b == o => {
                let file_path = repo_root.join(path);
                let _ = fs::remove_file(&file_path);
            }
            (Some(b), Some(h), None) if b == h => {
                let file_path = repo_root.join(path);
                let _ = fs::remove_file(&file_path);
            }
            // Deleted one side, modified other — conflict
            (Some(_), None, Some(o)) => {
                let content = read_blob_content(vrit_dir, o)?;
                let file_path = repo_root.join(path);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).ok();
                }
                fs::write(&file_path, &content)
                    .map_err(|e| format!("cannot write '{path}': {e}"))?;
                let blob = Object::Blob(content.into_bytes());
                let sha = blob.write_to_store(vrit_dir)?;
                index.add(make_entry(path, &sha));
                has_conflicts = true;
                println!("CONFLICT (modify/delete): {path} deleted in {current_name} and modified in {branch_name}");
            }
            (Some(_), Some(h), None) => {
                write_blob_to_working_tree(vrit_dir, repo_root, path, h)?;
                index.add(make_entry(path, h));
                has_conflicts = true;
                println!("CONFLICT (modify/delete): {path} modified in {current_name} and deleted in {branch_name}");
            }
            // Both sides changed differently (including add/add) — conflict
            (_, Some(h), Some(o)) => {
                let head_content = read_blob_content(vrit_dir, h)?;
                let other_content = read_blob_content(vrit_dir, o)?;
                let merged = write_conflict_markers(
                    &head_content,
                    &other_content,
                    &current_name,
                    branch_name,
                );
                let file_path = repo_root.join(path);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).ok();
                }
                fs::write(&file_path, &merged)
                    .map_err(|e| format!("cannot write '{path}': {e}"))?;
                let blob = Object::Blob(merged.into_bytes());
                let conflict_sha = blob.write_to_store(vrit_dir)?;
                index.add(make_entry(path, &conflict_sha));
                has_conflicts = true;
                println!("CONFLICT (content): Merge conflict in {path}");
            }
            // Deleted in both or not present — skip
            _ => {}
        }
    }

    index.save(vrit_dir)?;

    if has_conflicts {
        // Write merge state
        fs::write(
            vrit_dir.join("MERGE_HEAD"),
            format!("{other_sha}\n"),
        )
        .map_err(|e| format!("cannot write MERGE_HEAD: {e}"))?;

        let msg = format!("Merge branch '{branch_name}' into {current_name}\n");
        fs::write(vrit_dir.join("MERGE_MSG"), &msg)
            .map_err(|e| format!("cannot write MERGE_MSG: {e}"))?;

        println!("Automatic merge failed; fix conflicts and then commit the result.");
    } else {
        // Auto-commit clean merge
        auto_commit_merge(vrit_dir, head_sha, other_sha, branch_name)?;
    }

    Ok(())
}

fn auto_commit_merge(
    vrit_dir: &Path,
    head_sha: &str,
    other_sha: &str,
    branch_name: &str,
) -> Result<(), String> {
    let config = Config::load(vrit_dir)?;
    let name = config.require("user.name")?;
    let email = config.require("user.email")?;

    let index = Index::load(vrit_dir)?;
    let tree_sha = crate::commands::write_tree::write_tree_from_index(&index, vrit_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("time error: {e}"))?
        .as_secs();
    let author_line = format!("{name} <{email}> {timestamp} +0000");

    let current_name = current_branch(vrit_dir).unwrap_or_else(|| head_sha[..7].to_string());
    let message = format!("Merge branch '{branch_name}' into {current_name}\n");

    let commit = Object::Commit(CommitData {
        tree: tree_sha,
        parents: vec![head_sha.to_string(), other_sha.to_string()],
        author: author_line.clone(),
        committer: author_line,
        message,
    });
    let sha = commit.write_to_store(vrit_dir)?;
    update_current_ref(vrit_dir, &sha)?;

    println!("Merge made by the 'recursive' strategy.");
    Ok(())
}

fn abort_merge(vrit_dir: &Path, repo_root: &Path) -> Result<(), String> {
    if !vrit_dir.join("MERGE_HEAD").exists() {
        return Err("not currently merging — nothing to abort".into());
    }

    // Reset index and working tree to HEAD
    let head_sha = resolve_head(vrit_dir)?
        .ok_or("no HEAD commit")?;
    let entries = get_commit_tree(vrit_dir, &head_sha)?;

    // Restore all files from HEAD
    let mut index = Index::new();
    for (path, sha) in &entries {
        write_blob_to_working_tree(vrit_dir, repo_root, path, sha)?;
        index.add(make_entry(path, sha));
    }
    index.save(vrit_dir)?;

    // Clean up merge state
    let _ = fs::remove_file(vrit_dir.join("MERGE_HEAD"));
    let _ = fs::remove_file(vrit_dir.join("MERGE_MSG"));

    println!("Merge aborted — working tree restored to HEAD.");
    Ok(())
}

/// Find the lowest common ancestor of two commits using BFS.
pub fn find_merge_base(
    vrit_dir: &Path,
    sha1: &str,
    sha2: &str,
) -> Result<Option<String>, String> {
    let ancestors1 = collect_ancestors(vrit_dir, sha1)?;

    // BFS from sha2, first hit in ancestors1 is the merge base
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    queue.push_back(sha2.to_string());

    while let Some(sha) = queue.pop_front() {
        if ancestors1.contains(&sha) {
            return Ok(Some(sha));
        }
        if visited.contains(&sha) {
            continue;
        }
        visited.insert(sha.clone());

        let obj = Object::read_from_store(vrit_dir, &sha)?;
        if let Object::Commit(cd) = obj {
            for parent in &cd.parents {
                if !visited.contains(parent) {
                    queue.push_back(parent.clone());
                }
            }
        }
    }

    Ok(None)
}

fn collect_ancestors(vrit_dir: &Path, sha: &str) -> Result<HashSet<String>, String> {
    let mut ancestors = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(sha.to_string());

    while let Some(s) = queue.pop_front() {
        if ancestors.contains(&s) {
            continue;
        }
        ancestors.insert(s.clone());

        let obj = Object::read_from_store(vrit_dir, &s)?;
        if let Object::Commit(cd) = obj {
            for parent in &cd.parents {
                queue.push_back(parent.clone());
            }
        }
    }

    Ok(ancestors)
}

fn check_clean_working_tree(vrit_dir: &Path, repo_root: &Path) -> Result<(), String> {
    let index = Index::load(vrit_dir)?;
    for entry in &index.entries {
        let file_path = repo_root.join(&entry.path);
        if !file_path.exists() {
            continue;
        }
        let content = fs::read(&file_path).unwrap_or_default();
        let blob = Object::Blob(content);
        if blob.sha() != entry.sha {
            return Err(
                "Please commit or stash your changes before merging.".into()
            );
        }
    }
    Ok(())
}

fn write_conflict_markers(
    head_content: &str,
    other_content: &str,
    head_name: &str,
    other_name: &str,
) -> String {
    format!(
        "<<<<<<< {head_name}\n{head_content}=======\n{other_content}>>>>>>> {other_name}\n"
    )
}

fn update_current_ref(vrit_dir: &Path, sha: &str) -> Result<(), String> {
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

fn get_head_tree(vrit_dir: &Path) -> Result<Vec<(String, String)>, String> {
    match resolve_head(vrit_dir)? {
        Some(sha) => get_commit_tree(vrit_dir, &sha),
        None => Ok(Vec::new()),
    }
}

fn get_commit_tree(
    vrit_dir: &Path,
    commit_sha: &str,
) -> Result<Vec<(String, String)>, String> {
    let obj = Object::read_from_store(vrit_dir, commit_sha)?;
    match obj {
        Object::Commit(cd) => flatten_tree(vrit_dir, &cd.tree, ""),
        _ => Err(format!("{commit_sha} is not a commit")),
    }
}

fn find_sha<'a>(entries: &'a [(String, String)], path: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|(p, _)| p == path)
        .map(|(_, s)| s.as_str())
}

fn make_entry(path: &str, sha: &str) -> IndexEntry {
    IndexEntry {
        mode: 0o100644,
        sha: sha.to_string(),
        path: path.to_string(),
    }
}

fn write_blob_to_working_tree(
    vrit_dir: &Path,
    repo_root: &Path,
    path: &str,
    sha: &str,
) -> Result<(), String> {
    let obj = Object::read_from_store(vrit_dir, sha)?;
    if let Object::Blob(content) = obj {
        let file_path = repo_root.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&file_path, &content)
            .map_err(|e| format!("cannot write '{path}': {e}"))?;
    }
    Ok(())
}

fn read_blob_content(vrit_dir: &Path, sha: &str) -> Result<String, String> {
    let obj = Object::read_from_store(vrit_dir, sha)?;
    match obj {
        Object::Blob(data) => String::from_utf8(data)
            .map_err(|_| format!("blob {sha} is not valid UTF-8")),
        _ => Err(format!("{sha} is not a blob")),
    }
}
