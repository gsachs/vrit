// Integration tests for vrit stash, stash pop, stash list
mod helpers;
use helpers::TestRepo;

#[test]
fn stash_saves_and_restores_changes() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.write_file("f.txt", "dirty work");
    assert_eq!(repo.read_file("f.txt"), "dirty work");

    repo.run_ok(&["stash"]);
    assert_eq!(repo.read_file("f.txt"), "v1", "stash should reset to HEAD");

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("nothing to commit"));

    repo.run_ok(&["stash", "pop"]);
    assert_eq!(repo.read_file("f.txt"), "dirty work", "pop should restore changes");
}

#[test]
fn stash_on_clean_tree_errors() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["stash"]);
    assert!(err.contains("no local changes"));
}

#[test]
fn stash_pop_on_empty_stack_errors() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["stash", "pop"]);
    assert!(err.contains("no stash entries"));
}

#[test]
fn stash_multiple_lifo_order() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base");
    repo.commit_all("base");

    repo.write_file("f.txt", "change A");
    repo.run_ok(&["stash"]);
    repo.write_file("f.txt", "change B");
    repo.run_ok(&["stash"]);
    repo.write_file("f.txt", "change C");
    repo.run_ok(&["stash"]);

    // List should show 3 entries
    let list = repo.run_ok(&["stash", "list"]);
    assert!(list.contains("stash@{0}"));
    assert!(list.contains("stash@{1}"));
    assert!(list.contains("stash@{2}"));

    // Pop in LIFO order
    repo.run_ok(&["stash", "pop"]);
    assert_eq!(repo.read_file("f.txt"), "change C");

    repo.run_ok(&["stash", "pop"]);
    assert_eq!(repo.read_file("f.txt"), "change B");

    repo.run_ok(&["stash", "pop"]);
    assert_eq!(repo.read_file("f.txt"), "change A");

    // Stack should be empty
    let list = repo.run_ok(&["stash", "list"]);
    assert!(list.trim().is_empty());
}

#[test]
fn stash_with_staged_changes() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.write_file("f.txt", "staged change");
    repo.run_ok(&["add", "f.txt"]);

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("Changes to be committed:"));

    repo.run_ok(&["stash"]);

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("nothing to commit"));

    repo.run_ok(&["stash", "pop"]);
    assert_eq!(repo.read_file("f.txt"), "staged change");
}

#[test]
fn stash_with_new_file() {
    let repo = TestRepo::new();
    repo.write_file("existing.txt", "base");
    repo.commit_all("base");

    repo.write_file("new.txt", "new file content");
    repo.run_ok(&["add", "new.txt"]);
    repo.run_ok(&["stash"]);

    assert!(!repo.file_exists("new.txt"), "new file should be removed by stash");

    repo.run_ok(&["stash", "pop"]);
    assert!(repo.file_exists("new.txt"), "new file should be restored by pop");
    assert_eq!(repo.read_file("new.txt"), "new file content");
}
