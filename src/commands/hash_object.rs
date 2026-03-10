// Computes SHA-1 for a file and optionally writes it to the object store
use std::fs;

use crate::object::Object;
use crate::repo;

pub fn execute(file: &str, write: bool) -> Result<(), String> {
    let content = fs::read(file)
        .map_err(|e| format!("cannot read '{file}': {e}"))?;

    let blob = Object::Blob(content);

    if write {
        let vrit_dir = repo::find_vrit_dir()?;
        let sha = blob.write_to_store(&vrit_dir)?;
        println!("{sha}");
    } else {
        println!("{}", blob.sha());
    }

    Ok(())
}
