// Merges a branch into the current branch — fast-forward or three-way
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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

    let head_sha = repo::resolve_head(&vrit_dir)?
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
    let target_entries = repo::commit_tree_entries(vrit_dir, target_sha)?;
    let target_paths: HashSet<&String> = target_entries.iter().map(|(p, _, _)| p).collect();
    let current_entries = repo::head_tree_entries(vrit_dir)?;

    // Remove files not in target
    for (path, _, _) in &current_entries {
        if !target_paths.contains(path) {
            let file_path = repo_root.join(path);
            let _ = fs::remove_file(&file_path);
        }
    }

    // Write target files
    let mut index = Index::new();
    for (path, sha, mode) in &target_entries {
        repo::write_blob_to_working_tree(vrit_dir, repo_root, path, sha, *mode)?;
        index.add(IndexEntry {
            mode: *mode,
            sha: sha.clone(),
            path: path.clone(),
        });
    }
    index.save(vrit_dir)?;

    // Update branch ref
    repo::update_current_ref(vrit_dir, target_sha)?;

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
    let base_entries = repo::commit_tree_entries_map(vrit_dir, base_sha)?;
    let head_entries = repo::commit_tree_entries_map(vrit_dir, head_sha)?;
    let other_entries = repo::commit_tree_entries_map(vrit_dir, other_sha)?;

    // Collect all paths
    let mut all_paths: HashSet<&String> = HashSet::new();
    all_paths.extend(base_entries.keys());
    all_paths.extend(head_entries.keys());
    all_paths.extend(other_entries.keys());

    let mut index = Index::new();
    let mut has_conflicts = false;
    let current_name = repo::current_branch(vrit_dir).unwrap_or_else(|| head_sha[..7].to_string());

    let mut sorted_paths: Vec<&String> = all_paths.into_iter().collect();
    sorted_paths.sort();

    for path in &sorted_paths {
        let base_sha_opt = base_entries.get(*path).map(|(s, _)| s.as_str());
        let head_val = head_entries.get(*path);
        let other_val = other_entries.get(*path);
        let head_sha_opt = head_val.map(|(s, _)| s.as_str());
        let other_sha_opt = other_val.map(|(s, _)| s.as_str());
        let resolved_mode = head_val.map(|(_, m)| *m)
            .or_else(|| other_val.map(|(_, m)| *m))
            .unwrap_or(0o100644);

        match (base_sha_opt, head_sha_opt, other_sha_opt) {
            // Unchanged in both sides
            (_, Some(h), Some(o)) if h == o => {
                repo::write_blob_to_working_tree(vrit_dir, repo_root, path, h, resolved_mode)?;
                index.add(make_entry(path, h, resolved_mode));
            }
            // Changed only in head side
            (Some(b), Some(h), Some(o)) if b == o && b != h => {
                let mode = head_val.map(|(_, m)| *m).unwrap_or(0o100644);
                repo::write_blob_to_working_tree(vrit_dir, repo_root, path, h, mode)?;
                index.add(make_entry(path, h, mode));
            }
            // Changed only in other side
            (Some(b), Some(h), Some(o)) if b == h && b != o => {
                let mode = other_val.map(|(_, m)| *m).unwrap_or(0o100644);
                repo::write_blob_to_working_tree(vrit_dir, repo_root, path, o, mode)?;
                index.add(make_entry(path, o, mode));
            }
            // Added only in head
            (None, Some(h), None) => {
                let mode = head_val.map(|(_, m)| *m).unwrap_or(0o100644);
                repo::write_blob_to_working_tree(vrit_dir, repo_root, path, h, mode)?;
                index.add(make_entry(path, h, mode));
            }
            // Added only in other
            (None, None, Some(o)) => {
                let mode = other_val.map(|(_, m)| *m).unwrap_or(0o100644);
                repo::write_blob_to_working_tree(vrit_dir, repo_root, path, o, mode)?;
                index.add(make_entry(path, o, mode));
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
                index.add(make_entry(path, &sha, resolved_mode));
                has_conflicts = true;
                println!("CONFLICT (modify/delete): {path} deleted in {current_name} and modified in {branch_name}");
            }
            (Some(_), Some(h), None) => {
                let mode = head_val.map(|(_, m)| *m).unwrap_or(0o100644);
                repo::write_blob_to_working_tree(vrit_dir, repo_root, path, h, mode)?;
                index.add(make_entry(path, h, mode));
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
                index.add(make_entry(path, &conflict_sha, resolved_mode));
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

    let current_name = repo::current_branch(vrit_dir).unwrap_or_else(|| head_sha[..7].to_string());
    let message = format!("Merge branch '{branch_name}' into {current_name}\n");

    let commit = Object::Commit(CommitData {
        tree: tree_sha,
        parents: vec![head_sha.to_string(), other_sha.to_string()],
        author: author_line.clone(),
        committer: author_line,
        message,
    });
    let sha = commit.write_to_store(vrit_dir)?;
    repo::update_current_ref(vrit_dir, &sha)?;

    println!("Merge made by the 'recursive' strategy.");
    Ok(())
}

fn abort_merge(vrit_dir: &Path, repo_root: &Path) -> Result<(), String> {
    if !vrit_dir.join("MERGE_HEAD").exists() {
        return Err("not currently merging — nothing to abort".into());
    }

    // Reset index and working tree to HEAD
    let head_sha = repo::resolve_head(vrit_dir)?
        .ok_or("no HEAD commit")?;
    let entries = repo::commit_tree_entries(vrit_dir, &head_sha)?;

    // Restore all files from HEAD
    let mut index = Index::new();
    for (path, sha, mode) in &entries {
        repo::write_blob_to_working_tree(vrit_dir, repo_root, path, sha, *mode)?;
        index.add(make_entry(path, sha, *mode));
    }
    index.save(vrit_dir)?;

    // Clean up merge state
    let _ = fs::remove_file(vrit_dir.join("MERGE_HEAD"));
    let _ = fs::remove_file(vrit_dir.join("MERGE_MSG"));

    println!("Merge aborted — working tree restored to HEAD.");
    Ok(())
}

/// Find the lowest common ancestor via BFS: collect all ancestors of sha1,
/// then BFS from sha2 — the first hit is the LCA because BFS explores by distance.
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
        let content = fs::read(&file_path)
            .map_err(|e| format!("cannot read '{}': {e}", entry.path))?;
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

fn make_entry(path: &str, sha: &str, mode: u32) -> IndexEntry {
    IndexEntry {
        mode,
        sha: sha.to_string(),
        path: path.to_string(),
    }
}

fn read_blob_content(vrit_dir: &Path, sha: &str) -> Result<String, String> {
    let obj = Object::read_from_store(vrit_dir, sha)?;
    match obj {
        Object::Blob(data) => String::from_utf8(data)
            .map_err(|_| format!("blob {sha} is not valid UTF-8")),
        _ => Err(format!("{sha} is not a blob")),
    }
}
