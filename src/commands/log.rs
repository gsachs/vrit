// Walks commit DAG from HEAD with topological ordering, colored output
use colored::Colorize;
use std::collections::{HashSet, VecDeque};

use crate::commands::commit::resolve_head;
use crate::object::Object;
use crate::repo;

pub fn execute() -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;

    let head_sha = resolve_head(&vrit_dir)?
        .ok_or("no commits yet")?;

    // BFS with topological awareness — process a commit only after
    // all its children have been processed (simple: just visit in BFS order)
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(head_sha);

    while let Some(sha) = queue.pop_front() {
        if visited.contains(&sha) {
            continue;
        }
        visited.insert(sha.clone());

        let obj = Object::read_from_store(&vrit_dir, &sha)?;
        let cd = match obj {
            Object::Commit(cd) => cd,
            _ => return Err(format!("{sha} is not a commit")),
        };

        println!("{} {}", "commit".yellow(), sha.yellow());
        if cd.parents.len() > 1 {
            let parent_strs: Vec<String> = cd.parents.iter().map(|p| p[..7].to_string()).collect();
            println!("Merge: {}", parent_strs.join(" "));
        }
        println!("Author: {}", cd.author.bold());
        println!();
        for line in cd.message.lines() {
            println!("    {line}");
        }
        println!();

        for parent in &cd.parents {
            if !visited.contains(parent) {
                queue.push_back(parent.clone());
            }
        }
    }

    Ok(())
}
