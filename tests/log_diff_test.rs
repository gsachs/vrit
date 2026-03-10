// Integration tests for vrit log and vrit diff
mod helpers;
use helpers::TestRepo;

#[test]
fn log_shows_commits_in_reverse_order() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");
    repo.write_file("f.txt", "v2");
    repo.commit_all("second");
    repo.write_file("f.txt", "v3");
    repo.commit_all("third");

    let log = repo.run_ok(&["log"]);
    let first_pos = log.find("third").expect("third not found");
    let second_pos = log.find("second").expect("second not found");
    let third_pos = log.find("first").expect("first not found");
    assert!(first_pos < second_pos, "third should appear before second");
    assert!(second_pos < third_pos, "second should appear before first");
}

#[test]
fn log_on_empty_repo_errors() {
    let repo = TestRepo::new();
    let err = repo.run_err(&["log"]);
    assert!(err.contains("no commits"));
}

#[test]
fn diff_shows_unstaged_changes() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "line one\n");
    repo.commit_all("initial");

    repo.write_file("f.txt", "line one\nline two\n");
    let diff = repo.run_ok(&["diff"]);
    assert!(diff.contains("+line two"));
}

#[test]
fn diff_staged_shows_index_vs_head() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "old\n");
    repo.commit_all("initial");

    repo.write_file("f.txt", "new\n");
    repo.run_ok(&["add", "f.txt"]);

    let diff = repo.run_ok(&["diff", "--staged"]);
    assert!(diff.contains("-old"));
    assert!(diff.contains("+new"));
}

#[test]
fn diff_no_changes_produces_empty_output() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "content\n");
    repo.commit_all("initial");

    let diff = repo.run_ok(&["diff"]);
    assert!(diff.is_empty() || diff.trim().is_empty());
}
