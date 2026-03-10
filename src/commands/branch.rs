// Lists, creates, or deletes branches
use std::fs;

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

    let current = repo::current_branch(vrit_dir);

    let mut branches: Vec<String> = Vec::new();
    repo::collect_refs(&heads_dir, "", &mut branches)?;
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

fn create_branch(vrit_dir: &std::path::Path, name: &str) -> Result<(), String> {
    repo::validate_ref_name(name)?;
    let ref_path = vrit_dir.join("refs/heads").join(name);
    if ref_path.exists() {
        return Err(format!("branch '{name}' already exists"));
    }

    let head_sha = repo::resolve_head(vrit_dir)?
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
    repo::validate_ref_name(name)?;
    let current = repo::current_branch(vrit_dir);
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
