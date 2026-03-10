// Creates a new .vrit repository in the current directory
use std::fs;
use std::path::Path;

const DEFAULT_HEAD: &str = "ref: refs/heads/main\n";
const DEFAULT_CONFIG: &str = "\
# vrit configuration
# Required: set your identity before committing
# user.name = Your Name
# user.email = you@example.com
";

pub fn execute() -> Result<(), String> {
    let vrit_dir = Path::new(".vrit");

    if vrit_dir.exists() {
        println!("Reinitialized existing vrit repository in {}", dunce_current_dir());
        return Ok(());
    }

    let dirs = [
        ".vrit",
        ".vrit/objects",
        ".vrit/refs",
        ".vrit/refs/heads",
        ".vrit/refs/tags",
    ];

    for dir in &dirs {
        fs::create_dir_all(dir)
            .map_err(|e| format!("failed to create {dir}: {e}"))?;
    }

    fs::write(vrit_dir.join("HEAD"), DEFAULT_HEAD)
        .map_err(|e| format!("failed to write HEAD: {e}"))?;
    fs::write(vrit_dir.join("config"), DEFAULT_CONFIG)
        .map_err(|e| format!("failed to write config: {e}"))?;

    println!("Initialized empty vrit repository in {}", dunce_current_dir());
    println!("Edit .vrit/config to set user.name and user.email before committing.");

    Ok(())
}

fn dunce_current_dir() -> String {
    std::env::current_dir()
        .map(|p| p.join(".vrit").display().to_string())
        .unwrap_or_else(|_| ".vrit".into())
}
