// Integration tests for vrit init
mod helpers;
use helpers::TestRepo;

#[test]
fn init_creates_repository_structure() {
    let repo = TestRepo::new();
    assert!(repo.file_exists(".vrit"));
    assert!(repo.file_exists(".vrit/HEAD"));
    assert!(repo.file_exists(".vrit/objects"));
    assert!(repo.file_exists(".vrit/refs/heads"));
    assert!(repo.file_exists(".vrit/refs/tags"));
    assert!(repo.file_exists(".vrit/config"));

    let head = repo.read_head();
    assert_eq!(head, "ref: refs/heads/main");
}

#[test]
fn init_reinitialize_is_idempotent() {
    let repo = TestRepo::new();
    repo.write_file("test.txt", "data");
    repo.commit_all("first");

    let output = repo.run_ok(&["init"]);
    assert!(output.contains("Reinitialized"));

    // Objects and refs should still exist
    let log = repo.run_ok(&["log"]);
    assert!(log.contains("first"));
}
