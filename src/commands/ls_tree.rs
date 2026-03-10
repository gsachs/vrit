// Lists the entries of a tree object
use crate::object::Object;
use crate::repo;

pub fn execute(sha: &str) -> Result<(), String> {
    let vrit_dir = repo::find_vrit_dir()?;
    let obj = Object::read_from_store(&vrit_dir, sha)?;

    match obj {
        Object::Tree(entries) => {
            for entry in &entries {
                let obj_type = resolve_type(&vrit_dir, &entry.sha);
                println!("{} {} {}\t{}", entry.mode, obj_type, entry.sha, entry.name);
            }
            Ok(())
        }
        _ => Err(format!("object {sha} is not a tree")),
    }
}

fn resolve_type(vrit_dir: &std::path::Path, sha: &str) -> String {
    Object::read_from_store(vrit_dir, sha)
        .map(|o| o.type_str().to_string())
        .unwrap_or_else(|_| "unknown".into())
}
