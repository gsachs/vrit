// Lists, creates, or deletes branches
use std::fs;

use crate::commands::commit::resolve_head;
use crate::repo;

pub fn execute(name: Option<&str>, delete: Option<&str>) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;

    if let Some(branch_name) = delete {
        return delete_branch(&vrit_dir, branch_name);
    }

    match name {
        Some(branch_name) => create_branch(&vrit_dir, branch_name),
        None => list_branches(&vrit_dir),
    }
}

fn list_branches(vrit_dir: &std::path::Path) -> Result<(), String> {
    let heads_dir = vrit_dir.join("refs/heads");
    if !heads_dir.exists() {
        return Ok(());
    }

    let current = current_branch(vrit_dir);

    let mut branches: Vec<String> = Vec::new();
    collect_branches(&heads_dir, "", &mut branches)?;
    branches.sort();

    for branch in &branches {
        if Some(branch.as_str()) == current.as_deref() {
            println!("* {branch}");
        } else {
            println!("  {branch}");
        }
    }
    Ok(())
}

fn collect_branches(
    dir: &std::path::Path,
    prefix: &str,
    branches: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("cannot read refs/heads: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("directory entry error: {e}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let full_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };

        if path.is_dir() {
            collect_branches(&path, &full_name, branches)?;
        } else {
            branches.push(full_name);
        }
    }
    Ok(())
}

fn create_branch(vrit_dir: &std::path::Path, name: &str) -> Result<(), String> {
    let ref_path = vrit_dir.join("refs/heads").join(name);
    if ref_path.exists() {
        return Err(format!("branch '{name}' already exists"));
    }

    let head_sha = resolve_head(vrit_dir)?
        .ok_or("cannot create branch: no commits yet")?;

    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create branch directory: {e}"))?;
    }

    fs::write(&ref_path, format!("{head_sha}\n"))
        .map_err(|e| format!("cannot write branch ref: {e}"))?;

    Ok(())
}

fn delete_branch(vrit_dir: &std::path::Path, name: &str) -> Result<(), String> {
    let current = current_branch(vrit_dir);
    if current.as_deref() == Some(name) {
        return Err(format!("cannot delete branch '{name}': it is the current branch"));
    }

    let ref_path = vrit_dir.join("refs/heads").join(name);
    if !ref_path.exists() {
        return Err(format!("branch '{name}' not found"));
    }

    fs::remove_file(&ref_path)
        .map_err(|e| format!("cannot delete branch ref: {e}"))?;

    println!("Deleted branch {name}");
    Ok(())
}

pub fn current_branch(vrit_dir: &std::path::Path) -> Option<String> {
    let head = fs::read_to_string(vrit_dir.join("HEAD")).ok()?;
    let head = head.trim();
    head.strip_prefix("ref: refs/heads/").map(|s| s.to_string())
}
