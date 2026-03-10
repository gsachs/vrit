// Integration tests for .vritignore
mod helpers;
use helpers::TestRepo;

#[test]
fn vritignore_excludes_matching_files() {
    let repo = TestRepo::new();
    repo.write_file(".vritignore", "*.log\nbuild/\n");
    repo.write_file("app.txt", "code");
    repo.write_file("debug.log", "log data");
    repo.write_file("build/output.bin", "binary");

    repo.run_ok(&["add", "."]);
    let status = repo.run_ok(&["status"]);

    assert!(status.contains("app.txt"));
    assert!(!status.contains("debug.log"));
    assert!(!status.contains("build/output.bin"));
}

#[test]
fn vritignore_comments_and_blank_lines() {
    let repo = TestRepo::new();
    repo.write_file(".vritignore", "# comment\n\n*.tmp\n");
    repo.write_file("data.tmp", "temp");
    repo.write_file("data.txt", "keep");

    repo.run_ok(&["add", "."]);
    let status = repo.run_ok(&["status"]);

    assert!(status.contains("data.txt"));
    assert!(!status.contains("data.tmp"));
}

#[test]
fn vrit_directory_always_ignored() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "content");
    repo.run_ok(&["add", "."]);
    let status = repo.run_ok(&["status"]);

    // .vrit directory itself should never appear in status
    assert!(!status.contains(".vrit/"));
    assert!(!status.contains("HEAD"));
    assert!(!status.contains("objects"));
}
