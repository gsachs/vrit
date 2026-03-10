// Integration tests for vrit add, commit, status, rm
mod helpers;
use helpers::TestRepo;

#[test]
fn add_and_commit_single_file() {
    let repo = TestRepo::new();
    repo.write_file("hello.txt", "hello world\n");
    repo.run_ok(&["add", "hello.txt"]);

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("new file:   hello.txt"));

    let out = repo.run_ok(&["commit", "-m", "initial"]);
    assert!(out.contains("initial"));

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("nothing to commit"));
}

#[test]
fn add_directory_recursively() {
    let repo = TestRepo::new();
    repo.write_file("dir/a.txt", "a");
    repo.write_file("dir/sub/b.txt", "b");
    repo.run_ok(&["add", "dir"]);

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("dir/a.txt"));
    assert!(status.contains("dir/sub/b.txt"));
}

#[test]
fn commit_detects_no_changes() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "content");
    repo.commit_all("first");

    let err = repo.run_err(&["commit", "-m", "duplicate"]);
    assert!(err.contains("nothing to commit"));
}

#[test]
fn add_detects_deleted_files() {
    let repo = TestRepo::new();
    repo.write_file("gone.txt", "soon deleted");
    repo.commit_all("add file");

    repo.remove_file("gone.txt");
    repo.run_ok(&["add", "."]);

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("deleted:    gone.txt"));
}

#[test]
fn rm_removes_from_index_and_disk() {
    let repo = TestRepo::new();
    repo.write_file("removeme.txt", "bye");
    repo.commit_all("add");

    repo.run_ok(&["rm", "removeme.txt"]);
    assert!(!repo.file_exists("removeme.txt"));

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("deleted:    removeme.txt"));
}

#[test]
fn status_shows_modified_files() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.write_file("f.txt", "v2");
    let status = repo.run_ok(&["status"]);
    assert!(status.contains("modified:   f.txt"));
}

#[test]
fn status_shows_untracked_files() {
    let repo = TestRepo::new();
    repo.write_file("tracked.txt", "t");
    repo.commit_all("first");
    repo.write_file("new.txt", "untracked");

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("Untracked files:"));
    assert!(status.contains("new.txt"));
}

#[test]
fn commit_requires_config() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.into_path();
    let vrit_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_vrit"));

    // init
    std::process::Command::new(&vrit_bin)
        .args(["init"])
        .current_dir(&dir)
        .output()
        .unwrap();

    // Write a file and add it
    std::fs::write(dir.join("f.txt"), "x").unwrap();
    std::process::Command::new(&vrit_bin)
        .args(["add", "f.txt"])
        .current_dir(&dir)
        .output()
        .unwrap();

    // Commit without setting user.name/email should fail
    let output = std::process::Command::new(&vrit_bin)
        .args(["commit", "-m", "test"])
        .current_dir(&dir)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("user.name") || combined.contains("user.email"),
        "expected config error, got: {combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
