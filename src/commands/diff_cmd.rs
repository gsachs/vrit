// Shows differences between working tree and index, or index and HEAD
use std::fs;

use crate::commands::commit::resolve_head;
use crate::commands::status::flatten_tree;
use crate::diff::{self, is_binary};
use crate::index::Index;
use crate::object::Object;
use crate::repo;

pub fn execute(staged: bool) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let repo_root = vrit_dir
        .parent()
        .ok_or("cannot determine repository root")?
        .to_path_buf();
    let index = Index::load(&vrit_dir)?;
    let colored = atty_stdout();

    if staged {
        diff_staged(&vrit_dir, &index, colored)
    } else {
        diff_unstaged(&vrit_dir, &repo_root, &index, colored)
    }
}

fn diff_staged(
    vrit_dir: &std::path::Path,
    index: &Index,
    colored: bool,
) -> Result<(), String> {
    let head_entries = head_tree_map(vrit_dir)?;

    for entry in &index.entries {
        let old_bytes = head_entries
            .iter()
            .find(|(p, _)| p == &entry.path)
            .and_then(|(_, sha)| read_blob_bytes(vrit_dir, sha));

        let new_bytes = read_blob_bytes(vrit_dir, &entry.sha);

        show_diff_bytes(&entry.path, old_bytes.as_deref(), new_bytes.as_deref(), colored);
    }

    // Files in HEAD but not in index (staged deletions)
    for (path, sha) in &head_entries {
        if index.get(path).is_none() {
            let old_bytes = read_blob_bytes(vrit_dir, sha);
            show_diff_bytes(path, old_bytes.as_deref(), None, colored);
        }
    }

    Ok(())
}

fn diff_unstaged(
    vrit_dir: &std::path::Path,
    repo_root: &std::path::Path,
    index: &Index,
    colored: bool,
) -> Result<(), String> {
    for entry in &index.entries {
        let file_path = repo_root.join(&entry.path);
        if !file_path.exists() {
            // Deleted in working tree
            let old_bytes = read_blob_bytes(vrit_dir, &entry.sha);
            show_diff_bytes(&entry.path, old_bytes.as_deref(), None, colored);
            continue;
        }

        let working_content = fs::read(&file_path).unwrap_or_default();
        let working_blob = Object::Blob(working_content.clone());

        if working_blob.sha() != entry.sha {
            let old_bytes = read_blob_bytes(vrit_dir, &entry.sha);
            show_diff_bytes(
                &entry.path,
                old_bytes.as_deref(),
                Some(&working_content),
                colored,
            );
        }
    }
    Ok(())
}

fn show_diff_bytes(path: &str, old: Option<&[u8]>, new: Option<&[u8]>, colored: bool) {
    let old_bytes = old.unwrap_or(b"");
    let new_bytes = new.unwrap_or(b"");

    // Check binary on raw bytes before any UTF-8 conversion
    if is_binary(old_bytes) || is_binary(new_bytes) {
        println!("Binary files a/{path} and b/{path} differ");
        return;
    }

    let old_text = std::str::from_utf8(old_bytes).unwrap_or("");
    let new_text = std::str::from_utf8(new_bytes).unwrap_or("");

    let old_lines: Vec<&str> = if old_text.is_empty() {
        Vec::new()
    } else {
        old_text.lines().collect()
    };
    let new_lines: Vec<&str> = if new_text.is_empty() {
        Vec::new()
    } else {
        new_text.lines().collect()
    };

    let edits = diff::myers_diff(&old_lines, &new_lines);
    let output = diff::format_unified(path, path, &edits, colored);
    if !output.is_empty() {
        print!("{output}");
    }
}

fn head_tree_map(
    vrit_dir: &std::path::Path,
) -> Result<Vec<(String, String)>, String> {
    let head_sha = resolve_head(vrit_dir)?;
    match head_sha {
        Some(sha) => {
            let obj = Object::read_from_store(vrit_dir, &sha)?;
            match obj {
                Object::Commit(cd) => flatten_tree(vrit_dir, &cd.tree, ""),
                _ => Err("HEAD is not a commit".into()),
            }
        }
        None => Ok(Vec::new()),
    }
}

fn read_blob_bytes(vrit_dir: &std::path::Path, sha: &str) -> Option<Vec<u8>> {
    Object::read_from_store(vrit_dir, sha)
        .ok()
        .and_then(|obj| match obj {
            Object::Blob(data) => Some(data),
            _ => None,
        })
}

fn atty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}
