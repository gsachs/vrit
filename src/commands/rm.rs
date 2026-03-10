// Removes a file from the index and optionally from the working tree
use std::fs;

use crate::index::Index;
use crate::repo;

pub fn execute(path: &str) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?;
    let mut index = Index::load(&vrit_dir)?;

    if !index.remove(path) {
        return Err(format!("pathspec '{path}' did not match any tracked files"));
    }

    // Remove from working tree if it exists
    let file_path = repo_root.join(path);
    if file_path.exists() {
        fs::remove_file(&file_path)
            .map_err(|e| format!("cannot remove '{}': {e}", file_path.display()))?;
    }

    index.save(&vrit_dir)?;
    println!("rm '{path}'");
    Ok(())
}
