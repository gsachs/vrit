// Converts the current index into tree objects (handles nested directories)
use std::collections::BTreeMap;
use std::path::Path;

use crate::index::Index;
use crate::object::{Object, TreeEntry};
use crate::repo;

pub fn execute() -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let index = Index::load(&vrit_dir)?;
    let sha = write_tree_from_index(&index, &vrit_dir)?;
    println!("{sha}");
    Ok(())
}

/// Build tree objects from the index and write them to the store.
/// Returns the SHA of the root tree.
pub fn write_tree_from_index(index: &Index, vrit_dir: &Path) -> Result<String, String> {
    // Group entries by their top-level directory component
    let mut root_entries: Vec<TreeEntry> = Vec::new();
    let mut subdirs: BTreeMap<String, Vec<(String, &crate::index::IndexEntry)>> = BTreeMap::new();

    for entry in &index.entries {
        if let Some(slash_pos) = entry.path.find('/') {
            let dir = entry.path[..slash_pos].to_string();
            let rest = entry.path[slash_pos + 1..].to_string();
            subdirs.entry(dir).or_default().push((rest, entry));
        } else {
            root_entries.push(TreeEntry {
                mode: format!("{:o}", entry.mode),
                name: entry.path.clone(),
                sha: entry.sha.clone(),
            });
        }
    }

    // Recursively build subtrees
    for (dir_name, sub_entries) in &subdirs {
        let sub_index = Index {
            entries: sub_entries
                .iter()
                .map(|(rest_path, e)| crate::index::IndexEntry {
                    mode: e.mode,
                    sha: e.sha.clone(),
                    path: rest_path.clone(),
                })
                .collect(),
        };
        let sub_sha = write_tree_from_index(&sub_index, vrit_dir)?;
        root_entries.push(TreeEntry {
            mode: "40000".into(),
            name: dir_name.clone(),
            sha: sub_sha,
        });
    }

    // Sort entries by name (Git convention: directories compare with trailing /)
    root_entries.sort_by(|a, b| {
        let a_suffix = if a.mode == "40000" { "/" } else { "" };
        let b_suffix = if b.mode == "40000" { "/" } else { "" };
        (a.name.as_str(), a_suffix).cmp(&(b.name.as_str(), b_suffix))
    });

    let tree = Object::Tree(root_entries);
    tree.write_to_store(vrit_dir)
}
