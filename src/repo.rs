// Repository discovery — finds the .vrit directory from any working tree path
use std::env;
use std::path::PathBuf;

const VRIT_DIR: &str = ".vrit";

/// Walk up from the current directory to find a .vrit directory.
pub fn find_vrit_dir() -> Result<PathBuf, String> {
    let mut dir = env::current_dir()
        .map_err(|e| format!("cannot determine current directory: {e}"))?;
    loop {
        let candidate = dir.join(VRIT_DIR);
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if !dir.pop() {
            return Err("not a vrit repository (or any parent up to mount point)".into());
        }
    }
}
