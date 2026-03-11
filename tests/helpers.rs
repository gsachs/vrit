// Shared test harness for integration tests — spawns vrit as a subprocess in temp dirs
use std::path::PathBuf;
use std::process::{Command, Output};

pub struct TestRepo {
    pub dir: PathBuf,
    vrit_bin: PathBuf,
}

impl TestRepo {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let dir = dir.keep();
        let vrit_bin = PathBuf::from(env!("CARGO_BIN_EXE_vrit"));

        let repo = Self { dir, vrit_bin };
        repo.run_ok(&["init"]);
        repo.write_config("Test User", "test@example.com");
        repo
    }

    pub fn write_config(&self, name: &str, email: &str) {
        let config_path = self.dir.join(".vrit/config");
        std::fs::write(
            &config_path,
            format!("user.name = {name}\nuser.email = {email}\n"),
        )
        .expect("failed to write config");
    }

    pub fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.vrit_bin)
            .args(args)
            .current_dir(&self.dir)
            .env("NO_COLOR", "1")
            .output()
            .expect("failed to execute vrit")
    }

    pub fn run_ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        assert!(
            output.status.success(),
            "vrit {args:?} failed.\nstdout: {stdout}\nstderr: {stderr}"
        );
        stdout
    }

    pub fn run_err(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            !output.status.success(),
            "vrit {args:?} should have failed but succeeded.\nstdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        format!("{stderr}{stdout}")
    }

    pub fn write_file(&self, path: &str, content: &str) {
        let file_path = self.dir.join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create parent dirs");
        }
        std::fs::write(&file_path, content).expect("failed to write file");
    }

    pub fn read_file(&self, path: &str) -> String {
        std::fs::read_to_string(self.dir.join(path)).expect("failed to read file")
    }

    pub fn file_exists(&self, path: &str) -> bool {
        self.dir.join(path).exists()
    }

    pub fn remove_file(&self, path: &str) {
        std::fs::remove_file(self.dir.join(path)).expect("failed to remove file");
    }

    /// Stage and commit with a message, returning stdout
    pub fn commit_all(&self, message: &str) -> String {
        self.run_ok(&["add", "."]);
        self.run_ok(&["commit", "-m", message])
    }

    /// Read a ref file (e.g., "refs/heads/main") and return the SHA
    pub fn read_ref(&self, ref_path: &str) -> String {
        let full = self.dir.join(".vrit").join(ref_path);
        std::fs::read_to_string(&full)
            .expect(&format!("failed to read ref {ref_path}"))
            .trim()
            .to_string()
    }

    /// Read HEAD content
    pub fn read_head(&self) -> String {
        self.read_file(".vrit/HEAD").trim().to_string()
    }

    /// Write arbitrary bytes to a path relative to repo root (for gray-box corruption tests)
    pub fn write_raw(&self, path: &str, bytes: &[u8]) {
        let file_path = self.dir.join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create parent dirs");
        }
        std::fs::write(&file_path, bytes).expect("failed to write raw bytes");
    }

    /// Read raw bytes from a path relative to repo root
    pub fn read_raw(&self, path: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(path)).expect("failed to read raw bytes")
    }

    /// Assert command fails without a panic backtrace in output
    pub fn run_err_no_panic(&self, args: &[&str]) -> String {
        let output = self.run(args);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let combined = format!("{stderr}{stdout}");
        assert!(
            !output.status.success(),
            "vrit {args:?} should have failed but succeeded.\nstdout: {stdout}"
        );
        assert!(
            !combined.contains("panicked at"),
            "vrit {args:?} panicked instead of returning an error:\n{combined}"
        );
        combined
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
