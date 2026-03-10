// Integration tests for vrit merge
mod helpers;
use helpers::TestRepo;

#[test]
fn merge_fast_forward() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "v2");
    repo.commit_all("feature commit");

    repo.run_ok(&["checkout", "main"]);
    let out = repo.run_ok(&["merge", "feature"]);
    assert!(out.contains("Fast-forward"));

    assert_eq!(repo.read_file("f.txt"), "v2");

    // main and feature should point to the same commit
    let main_sha = repo.read_ref("refs/heads/main");
    let feature_sha = repo.read_ref("refs/heads/feature");
    assert_eq!(main_sha, feature_sha);
}

#[test]
fn merge_already_up_to_date() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature"]);
    let out = repo.run_ok(&["merge", "feature"]);
    assert!(out.contains("Already up to date"));
}

#[test]
fn merge_three_way_clean() {
    let repo = TestRepo::new();
    repo.write_file("shared.txt", "base");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("feature.txt", "feature only");
    repo.commit_all("feature work");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("main.txt", "main only");
    repo.commit_all("main work");

    let out = repo.run_ok(&["merge", "feature"]);
    assert!(out.contains("Merge made"));

    // Both files should exist
    assert!(repo.file_exists("feature.txt"));
    assert!(repo.file_exists("main.txt"));
    assert_eq!(repo.read_file("shared.txt"), "base");

    // Log should show merge commit
    let log = repo.run_ok(&["log"]);
    assert!(log.contains("Merge"));
}

#[test]
fn merge_conflict_markers() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base content\n");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "feature content\n");
    repo.commit_all("feature change");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("f.txt", "main content\n");
    repo.commit_all("main change");

    let out = repo.run_ok(&["merge", "feature"]);
    assert!(out.contains("CONFLICT"));
    assert!(out.contains("fix conflicts"));

    // File should have conflict markers
    let content = repo.read_file("f.txt");
    assert!(content.contains("<<<<<<<"));
    assert!(content.contains("======="));
    assert!(content.contains(">>>>>>>"));

    // MERGE_HEAD should exist
    assert!(repo.file_exists(".vrit/MERGE_HEAD"));
}

#[test]
fn merge_abort_restores_state() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base\n");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "feature\n");
    repo.commit_all("feature");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("f.txt", "main\n");
    repo.commit_all("main diverge");

    repo.run_ok(&["merge", "feature"]);
    assert!(repo.file_exists(".vrit/MERGE_HEAD"));

    repo.run_ok(&["merge", "--abort"]);
    assert!(!repo.file_exists(".vrit/MERGE_HEAD"));
    assert_eq!(repo.read_file("f.txt"), "main\n");
}

#[test]
fn merge_abort_without_merge_errors() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["merge", "--abort"]);
    assert!(err.contains("not currently merging"));
}

#[test]
fn merge_refuses_dirty_tree() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("new.txt", "feature file");
    repo.commit_all("feature");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("f.txt", "dirty");

    let err = repo.run_err(&["merge", "feature"]);
    assert!(err.contains("commit or stash"));
}

#[test]
fn merge_conflict_then_resolve_and_commit() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base\n");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "feature\n");
    repo.commit_all("feature");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("f.txt", "main\n");
    repo.commit_all("main");

    // Create conflict
    repo.run_ok(&["merge", "feature"]);
    assert!(repo.file_exists(".vrit/MERGE_HEAD"));

    // Resolve by choosing main's version
    repo.write_file("f.txt", "resolved\n");
    repo.run_ok(&["add", "f.txt"]);
    repo.run_ok(&["commit", "-m", "merge resolved"]);

    // MERGE_HEAD should be cleaned up
    assert!(!repo.file_exists(".vrit/MERGE_HEAD"));

    // Log should show merge commit with two parents
    let log = repo.run_ok(&["log"]);
    assert!(log.contains("merge resolved"));
    assert!(log.contains("Merge:"));
}

#[test]
fn merge_with_new_file_on_branch() {
    let repo = TestRepo::new();
    repo.write_file("base.txt", "base");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("feature.txt", "new file");
    repo.commit_all("add feature file");

    repo.run_ok(&["checkout", "main"]);
    assert!(!repo.file_exists("feature.txt"));

    repo.run_ok(&["merge", "feature"]);
    assert!(repo.file_exists("feature.txt"));
    assert_eq!(repo.read_file("feature.txt"), "new file");
}
