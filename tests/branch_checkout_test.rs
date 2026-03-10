// Integration tests for vrit branch and vrit checkout
mod helpers;
use helpers::TestRepo;

#[test]
fn branch_create_and_list() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("initial");

    repo.run_ok(&["branch", "feature"]);
    let list = repo.run_ok(&["branch"]);
    assert!(list.contains("* main"));
    assert!(list.contains("feature"));
}

#[test]
fn branch_delete() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("initial");

    repo.run_ok(&["branch", "temp"]);
    repo.run_ok(&["branch", "-d", "temp"]);

    let list = repo.run_ok(&["branch"]);
    assert!(!list.contains("temp"));
}

#[test]
fn branch_refuses_delete_current() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("initial");

    let err = repo.run_err(&["branch", "-d", "main"]);
    assert!(err.contains("current branch"));
}

#[test]
fn branch_refuses_duplicate() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("initial");

    repo.run_ok(&["branch", "feature"]);
    let err = repo.run_err(&["branch", "feature"]);
    assert!(err.contains("already exists"));
}

#[test]
fn checkout_switches_branch() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "main content");
    repo.commit_all("main commit");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);

    let head = repo.read_head();
    assert_eq!(head, "ref: refs/heads/feature");

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("On branch feature"));
}

#[test]
fn checkout_updates_working_tree() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "v2");
    repo.commit_all("feature change");

    repo.run_ok(&["checkout", "main"]);
    assert_eq!(repo.read_file("f.txt"), "v1");

    repo.run_ok(&["checkout", "feature"]);
    assert_eq!(repo.read_file("f.txt"), "v2");
}

#[test]
fn checkout_refuses_on_dirty_working_tree() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "feature version");
    repo.commit_all("feature");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("f.txt", "dirty");

    let err = repo.run_err(&["checkout", "feature"]);
    assert!(err.contains("overwritten") || err.contains("commit or stash"));
}

#[test]
fn checkout_detached_head() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let sha = repo.read_ref("refs/heads/main");
    repo.run_ok(&["checkout", &sha]);

    let head = repo.read_head();
    assert_eq!(head, sha, "HEAD should be raw SHA in detached mode");

    let status = repo.run_ok(&["status"]);
    assert!(status.contains("detached") || status.contains("HEAD detached"));
}

#[test]
fn checkout_restore_file_from_head() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "original");
    repo.commit_all("first");

    repo.write_file("f.txt", "modified");
    assert_eq!(repo.read_file("f.txt"), "modified");

    repo.run_ok(&["checkout", "--", "f.txt"]);
    assert_eq!(repo.read_file("f.txt"), "original");
}

#[test]
fn checkout_removes_files_not_in_target() {
    let repo = TestRepo::new();
    repo.write_file("a.txt", "a");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("b.txt", "b");
    repo.commit_all("add b");

    repo.run_ok(&["checkout", "main"]);
    assert!(!repo.file_exists("b.txt"), "b.txt should be removed when switching to main");

    repo.run_ok(&["checkout", "feature"]);
    assert!(repo.file_exists("b.txt"), "b.txt should reappear on feature");
}
