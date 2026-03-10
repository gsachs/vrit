// Reads key-value pairs from .vrit/config
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct Config {
    values: HashMap<String, String>,
}

impl Config {
    pub fn load(vrit_dir: &Path) -> Result<Config, String> {
        let path = vrit_dir.join("config");
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("cannot read config: {e}"))?;

        let mut values = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                values.insert(key.trim().to_string(), val.trim().to_string());
            }
        }
        Ok(Config { values })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn require(&self, key: &str) -> Result<&str, String> {
        self.get(key).ok_or_else(|| {
            format!("{key} is not set in .vrit/config — please add it before committing")
        })
    }
}
