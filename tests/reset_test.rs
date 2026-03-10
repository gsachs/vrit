// Integration tests for vrit reset
mod helpers;
use helpers::TestRepo;

#[test]
fn reset_unstages_changes() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.write_file("f.txt", "v2");
    repo.run_ok(&["add", "f.txt"]);

    // Verify it's staged
    let status = repo.run_ok(&["status"]);
    assert!(status.contains("Changes to be committed:"));

    repo.run_ok(&["reset"]);

    // Should now be unstaged (modified, not staged)
    let status = repo.run_ok(&["status"]);
    assert!(status.contains("Changes not staged for commit:"));
    assert!(status.contains("modified:   f.txt"));

    // Working tree should be untouched
    assert_eq!(repo.read_file("f.txt"), "v2");
}

#[test]
fn reset_to_previous_commit() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");
    let first_sha = repo.read_ref("refs/heads/main");

    repo.write_file("f.txt", "v2");
    repo.commit_all("second");

    repo.run_ok(&["reset", &first_sha]);

    // HEAD should now point to first commit
    let current_sha = repo.read_ref("refs/heads/main");
    assert_eq!(current_sha, first_sha);

    // Working tree should be unchanged (mixed reset)
    assert_eq!(repo.read_file("f.txt"), "v2");

    // Log should only show first commit
    let log = repo.run_ok(&["log"]);
    assert!(log.contains("first"));
    assert!(!log.contains("second"));
}

#[test]
fn reset_preserves_working_tree() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.write_file("f.txt", "v3");
    repo.run_ok(&["add", "f.txt"]);
    repo.run_ok(&["reset"]);

    // File content should be preserved
    assert_eq!(repo.read_file("f.txt"), "v3");
}

#[test]
fn reset_invalid_commit_errors() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["reset", "0000000000000000000000000000000000000000"]);
    assert!(err.contains("not found") || err.contains("object not found"));
}
