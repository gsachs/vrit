// Adversarial tests: operations in wrong order or conflicting states
mod helpers;
use helpers::TestRepo;

// --- Unborn branch operations ---

#[test]
fn commit_on_fresh_repo_nothing_staged() {
    let repo = TestRepo::new();
    let err = repo.run_err(&["commit", "-m", "empty"]);
    assert!(
        err.contains("nothing to commit") || err.contains("empty index"),
        "expected empty-index error, got: {err}"
    );
}

#[test]
fn log_on_fresh_repo_no_commits() {
    let repo = TestRepo::new();
    // Log with no commits — should not panic
    let output = repo.run(&["log"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on log with no commits: {combined}"
    );
}

#[test]
fn diff_on_fresh_repo() {
    let repo = TestRepo::new();
    let output = repo.run(&["diff"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on diff with no commits: {combined}"
    );
}

#[test]
fn stash_on_fresh_repo_no_commits() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "dirty");
    let err = repo.run_err(&["stash"]);
    assert!(
        err.contains("no commits") || err.contains("nothing to stash"),
        "expected no-commits error, got: {err}"
    );
}

#[test]
fn stash_pop_with_no_stashes() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["stash", "pop"]);
    assert!(
        err.contains("no stash") || err.contains("entries"),
        "expected no-stash error, got: {err}"
    );
}

// --- Init/setup violations ---

#[test]
fn double_init_same_directory() {
    let repo = TestRepo::new();
    // Second init on already-initialized repo
    let output = repo.run(&["init"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on double init: {combined}"
    );
    // .vrit should still exist and be functional
    assert!(repo.file_exists(".vrit/HEAD"));
}

#[test]
fn command_outside_vrit_repo() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let vrit_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_vrit"));

    let output = std::process::Command::new(&vrit_bin)
        .args(["status"])
        .current_dir(dir.path())
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to execute");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not a vrit repository"),
        "expected not-a-repo error, got: {stderr}"
    );
}

// --- Merge state violations ---

#[test]
fn merge_with_self() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let out = repo.run_ok(&["merge", "main"]);
    assert!(
        out.contains("Already up to date"),
        "merge self should say up-to-date, got: {out}"
    );
}

#[test]
fn merge_during_active_merge() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base\n");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "feature\n");
    repo.commit_all("feature change");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("f.txt", "main\n");
    repo.commit_all("main change");

    // First merge creates conflict
    repo.run_ok(&["merge", "feature"]);
    assert!(repo.file_exists(".vrit/MERGE_HEAD"));

    // Second merge while conflict active — should be refused
    let err = repo.run_err(&["merge", "feature"]);
    assert!(
        err.contains("merge already in progress"),
        "expected merge-in-progress error, got: {err}"
    );
}

#[test]
fn checkout_during_merge_conflict() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base\n");
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["branch", "other"]);

    repo.run_ok(&["checkout", "feature"]);
    repo.write_file("f.txt", "feature\n");
    repo.commit_all("feature");

    repo.run_ok(&["checkout", "main"]);
    repo.write_file("f.txt", "main\n");
    repo.commit_all("main diverge");

    repo.run_ok(&["merge", "feature"]);
    assert!(repo.file_exists(".vrit/MERGE_HEAD"));

    // Checkout should refuse because merge is in progress
    let err = repo.run_err(&["checkout", "other"]);
    assert!(
        err.contains("cannot checkout during a merge"),
        "expected merge-in-progress error on checkout, got: {err}"
    );
}

#[test]
fn stash_during_merge_conflict() {
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

    repo.run_ok(&["merge", "feature"]);

    // Stash during conflict — should not panic
    let output = repo.run(&["stash"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on stash during conflict: {combined}"
    );
}

#[test]
fn reset_during_merge_conflict() {
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

    let head_sha = repo.read_ref("refs/heads/main");
    repo.run_ok(&["merge", "feature"]);

    // Reset during conflict — should not panic
    let output = repo.run(&["reset", &head_sha]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on reset during conflict: {combined}"
    );
}

#[test]
fn merge_abort_when_no_merge() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["merge", "--abort"]);
    assert!(
        err.contains("not currently merging"),
        "expected not-merging error, got: {err}"
    );
}

#[test]
fn forged_merge_head_then_commit() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let head_sha = repo.read_ref("refs/heads/main");

    // Manually inject MERGE_HEAD with a valid SHA
    repo.write_raw(".vrit/MERGE_HEAD", format!("{head_sha}\n").as_bytes());

    // Modify file so commit has something to do
    repo.write_file("f.txt", "v2");
    repo.run_ok(&["add", "."]);

    // Commit will read MERGE_HEAD and create a merge commit
    let output = repo.run(&["commit", "-m", "forged merge"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on forged MERGE_HEAD commit: {combined}"
    );

    // MERGE_HEAD should be cleaned up after commit
    assert!(
        !repo.file_exists(".vrit/MERGE_HEAD"),
        "MERGE_HEAD should be cleaned up"
    );
}

#[test]
fn commit_after_merge_abort_is_normal() {
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

    repo.run_ok(&["merge", "feature"]);
    repo.run_ok(&["merge", "--abort"]);

    // Now make a normal commit — should not have merge parents
    repo.write_file("g.txt", "new file");
    repo.commit_all("normal commit after abort");

    let log = repo.run_ok(&["log"]);
    // The latest commit should NOT have "Merge:" in it
    let lines: Vec<&str> = log.lines().collect();
    let latest_commit_section: String = lines
        .iter()
        .take_while(|l| !l.starts_with("commit ") || lines[0] == **l)
        .map(|l| *l)
        .collect::<Vec<&str>>()
        .join("\n");
    assert!(
        !latest_commit_section.contains("Merge:"),
        "post-abort commit should not be a merge commit"
    );
}

// --- Branch operations ---

#[test]
fn delete_current_branch() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["branch", "-d", "main"]);
    assert!(
        err.contains("current branch"),
        "expected current-branch error, got: {err}"
    );
}

#[test]
fn checkout_deleted_branch() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["branch", "-d", "feature"]);

    let err = repo.run_err(&["checkout", "feature"]);
    assert!(
        err.contains("not match") || err.contains("not found"),
        "expected branch-not-found error, got: {err}"
    );
}

#[test]
fn create_branch_that_already_exists() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature"]);
    let err = repo.run_err(&["branch", "feature"]);
    assert!(
        err.contains("already exists"),
        "expected already-exists error, got: {err}"
    );
}

// --- File operations ---

#[test]
fn add_nonexistent_file() {
    let repo = TestRepo::new();
    let err = repo.run_err(&["add", "ghost.txt"]);
    assert!(
        err.contains("did not match") || err.contains("not found") || err.contains("pathspec"),
        "expected pathspec error, got: {err}"
    );
}

#[test]
fn rm_file_not_in_index() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["rm", "nonexistent.txt"]);
    assert!(
        err.contains("not in index") || err.contains("not found") || err.contains("did not match"),
        "expected not-in-index error, got: {err}"
    );
}

#[test]
fn checkout_restore_nonexistent_file() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["checkout", "--", "ghost.txt"]);
    assert!(
        err.contains("did not match") || err.contains("not found") || err.contains("pathspec"),
        "expected file-not-found error, got: {err}"
    );
}

// --- Detached HEAD ---

#[test]
fn commit_in_detached_head() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let head_sha = repo.read_ref("refs/heads/main");
    repo.run_ok(&["checkout", &head_sha]);

    // Verify detached
    let head = repo.read_head();
    assert!(
        !head.starts_with("ref:"),
        "should be detached HEAD, got: {head}"
    );

    // Commit in detached state
    repo.write_file("f.txt", "v2");
    repo.run_ok(&["add", "."]);
    repo.run_ok(&["commit", "-m", "detached commit"]);

    // HEAD should now point to the new commit (different SHA)
    let new_head = repo.read_head();
    assert_ne!(new_head, head_sha, "HEAD should advance to new commit");
    assert!(
        !new_head.starts_with("ref:"),
        "should still be detached after commit"
    );
}
