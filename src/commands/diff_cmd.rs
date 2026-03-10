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
        let old_content = head_entries
            .iter()
            .find(|(p, _)| p == &entry.path)
            .and_then(|(_, sha)| read_blob(vrit_dir, sha));

        let new_content = read_blob(vrit_dir, &entry.sha);

        show_diff(&entry.path, old_content.as_deref(), new_content.as_deref(), colored);
    }

    // Files in HEAD but not in index (staged deletions)
    for (path, sha) in &head_entries {
        if index.get(path).is_none() {
            let old_content = read_blob(vrit_dir, sha);
            show_diff(path, old_content.as_deref(), None, colored);
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
            let old_content = read_blob(vrit_dir, &entry.sha);
            show_diff(&entry.path, old_content.as_deref(), None, colored);
            continue;
        }

        let working_content = fs::read(&file_path).unwrap_or_default();
        let working_blob = Object::Blob(working_content.clone());

        if working_blob.sha() != entry.sha {
            let old_content = read_blob(vrit_dir, &entry.sha);
            let new_str = String::from_utf8(working_content).ok();
            show_diff(
                &entry.path,
                old_content.as_deref(),
                new_str.as_deref(),
                colored,
            );
        }
    }
    Ok(())
}

fn show_diff(path: &str, old: Option<&str>, new: Option<&str>, colored: bool) {
    let old_text = old.unwrap_or("");
    let new_text = new.unwrap_or("");

    // Binary check
    if is_binary(old_text.as_bytes()) || is_binary(new_text.as_bytes()) {
        println!("Binary files a/{path} and b/{path} differ");
        return;
    }

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

fn read_blob(vrit_dir: &std::path::Path, sha: &str) -> Option<String> {
    Object::read_from_store(vrit_dir, sha)
        .ok()
        .and_then(|obj| match obj {
            Object::Blob(data) => String::from_utf8(data).ok(),
            _ => None,
        })
}

fn atty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}
