// Adversarial tests: unusual but valid inputs through the CLI
mod helpers;
use helpers::TestRepo;

// --- File content edge cases ---

#[test]
fn add_commit_empty_file() {
    let repo = TestRepo::new();
    repo.write_file("empty.txt", "");
    repo.commit_all("empty file");
    let status = repo.run_ok(&["status"]);
    assert!(
        !status.contains("empty.txt"),
        "empty file should be tracked clean"
    );
}

#[test]
fn add_commit_file_with_only_newlines() {
    let repo = TestRepo::new();
    repo.write_file("newlines.txt", "\n\n\n");
    repo.commit_all("newline file");
    assert_eq!(repo.read_file("newlines.txt"), "\n\n\n");
}

#[test]
fn binary_file_with_null_bytes_diff() {
    let repo = TestRepo::new();
    repo.write_raw("binary.bin", &[0x00, 0x01, 0x02, 0xFF, 0x00, 0xFE]);
    repo.commit_all("add binary");

    // Modify the binary file
    repo.write_raw("binary.bin", &[0xFF, 0x00, 0x01]);

    let diff_output = repo.run_ok(&["diff"]);
    assert!(
        diff_output.contains("Binary") || diff_output.contains("binary") || diff_output.is_empty(),
        "expected binary diff message or empty, got: {diff_output}"
    );
}

#[test]
fn large_file_content() {
    let repo = TestRepo::new();
    let large_content = "x".repeat(1024 * 1024); // 1MB
    repo.write_file("large.txt", &large_content);
    repo.commit_all("large file");

    let stored = repo.read_file("large.txt");
    assert_eq!(stored.len(), 1024 * 1024);
}

// --- Filename edge cases ---

#[test]
fn unicode_filename() {
    let repo = TestRepo::new();
    repo.write_file("café.txt", "latte");
    repo.commit_all("unicode name");

    let status = repo.run_ok(&["status"]);
    assert!(
        !status.contains("café.txt") || status.contains("nothing"),
        "unicode file should be tracked"
    );
}

#[test]
fn filename_with_spaces() {
    let repo = TestRepo::new();
    repo.write_file("my file.txt", "content");
    repo.commit_all("spaced name");

    let status = repo.run_ok(&["status"]);
    assert!(
        !status.contains("my file.txt"),
        "spaced filename should be tracked clean"
    );
}

#[test]
fn filename_with_special_chars() {
    let repo = TestRepo::new();
    // Shell special chars in filename (no actual injection, just storage)
    repo.write_file("file&name.txt", "safe");
    repo.commit_all("special chars");

    assert!(repo.file_exists("file&name.txt"));
    assert_eq!(repo.read_file("file&name.txt"), "safe");
}

#[test]
fn very_long_filename() {
    let repo = TestRepo::new();
    let long_name = format!("{}.txt", "a".repeat(251)); // 255 chars total
    repo.write_file(&long_name, "content");

    let output = repo.run(&["add", "."]);
    // Should either succeed or give a clear error — not panic
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on long filename: {combined}"
    );
}

#[test]
fn dotfile_is_tracked() {
    let repo = TestRepo::new();
    repo.write_file(".hidden", "secret");
    repo.commit_all("dotfile");

    let status = repo.run_ok(&["status"]);
    assert!(
        !status.contains(".hidden"),
        "dotfile should be tracked clean after commit"
    );
}

#[test]
fn file_named_like_flag_double_dash() {
    let repo = TestRepo::new();
    // Create a file literally named "--help"
    let file_path = repo.dir.join("--help");
    std::fs::write(&file_path, "not a flag").unwrap();

    // Use -- to disambiguate: add should not interpret this as a flag
    // Note: depending on clap config, this may or may not work
    let output = repo.run(&["add", "--", "--help"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on flag-like filename: {combined}"
    );
}

#[test]
fn file_named_dash_m() {
    let repo = TestRepo::new();
    let file_path = repo.dir.join("-m");
    std::fs::write(&file_path, "not a flag").unwrap();

    // Try to add it — should handle gracefully
    let output = repo.run(&["add", "."]); // add via directory to avoid arg parsing issues
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on -m filename: {combined}"
    );
}

// --- Commit message edge cases ---

#[test]
fn empty_commit_message() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.run_ok(&["add", "."]);

    // Empty message — may be accepted or rejected
    let output = repo.run(&["commit", "-m", ""]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on empty commit message: {combined}"
    );
}

#[test]
fn multiline_commit_message() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.run_ok(&["add", "."]);
    repo.run_ok(&["commit", "-m", "line one\nline two\nline three"]);

    let log = repo.run_ok(&["log"]);
    assert!(log.contains("line one"), "first line should appear in log");
}

#[test]
fn commit_message_with_special_chars() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.run_ok(&["add", "."]);
    repo.run_ok(&["commit", "-m", "<script>alert('xss')</script> & \"quotes\""]);

    let log = repo.run_ok(&["log"]);
    assert!(log.contains("<script>"), "special chars should be preserved");
}

// --- Branch/tag name edge cases ---

#[test]
fn branch_name_with_slash() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["branch", "feature/foo"]);
    let branches = repo.run_ok(&["branch"]);
    assert!(branches.contains("feature/foo"));
}

#[test]
fn branch_name_with_double_dots_rejected() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["branch", "a..b"]);
    assert!(
        err.contains("invalid"),
        "double-dot branch should be rejected, got: {err}"
    );
}

#[test]
fn branch_name_starting_with_dash_rejected() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    // Leading dash should be rejected by validate_ref_name
    let err = repo.run_err(&["branch", "-badname"]);
    assert!(
        err.contains("invalid") || err.contains("error"),
        "dash-prefixed branch should be rejected, got: {err}"
    );
}

#[test]
fn branch_name_head() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    // Creating a branch named "HEAD" — may be allowed or rejected
    let output = repo.run(&["branch", "HEAD"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on branch named HEAD: {combined}"
    );
}

#[test]
fn tag_with_same_name_as_branch() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    // Branch "main" already exists — create a tag also named "main"
    let output = repo.run(&["tag", "main"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on tag sharing branch name: {combined}"
    );
}

#[test]
fn tag_name_with_special_chars() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    // Tag name with characters that might cause filesystem issues
    let output = repo.run(&["tag", "v1.0-rc.1"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on tag with dots/dashes: {combined}"
    );
}

// --- Plumbing command edge cases ---

#[test]
fn cat_file_nonexistent_sha() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let fake_sha = "abcdef1234567890abcdef1234567890abcdef12";
    let err = repo.run_err(&["cat-file", "-p", fake_sha]);
    assert!(
        err.contains("not found") || err.contains("object"),
        "expected object-not-found error, got: {err}"
    );
}

#[test]
fn ls_tree_with_blob_sha() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    // Get the blob SHA for f.txt via cat-file on the tree
    let head_sha = repo.read_ref("refs/heads/main");
    let commit_out = repo.run_ok(&["cat-file", "-p", &head_sha]);
    // Parse tree SHA from commit output
    let tree_sha = commit_out
        .lines()
        .find(|l| l.starts_with("tree "))
        .unwrap()
        .strip_prefix("tree ")
        .unwrap()
        .trim();

    let tree_out = repo.run_ok(&["ls-tree", tree_sha]);
    // Extract a blob SHA from the tree listing
    let blob_sha = tree_out
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();

    // ls-tree on a blob SHA should error
    let err = repo.run_err(&["ls-tree", blob_sha]);
    assert!(
        err.contains("not a tree") || err.contains("error"),
        "expected not-a-tree error, got: {err}"
    );
}
