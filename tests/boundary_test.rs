// Adversarial tests: stress limits, path traversal, and security boundaries
mod helpers;
use helpers::TestRepo;

// --- Path traversal / security ---

#[test]
fn add_parent_directory_traversal() {
    let repo = TestRepo::new();

    // Create a file outside the repo to attempt adding
    let outside_path = repo.dir.parent().unwrap().join("outside.txt");
    std::fs::write(&outside_path, "sensitive data").unwrap();

    let err = repo.run_err(&["add", "../outside.txt"]);
    assert!(
        err.contains("outside") || err.contains("pathspec") || err.contains("did not match"),
        "expected path-outside-repo error, got: {err}"
    );

    // Cleanup
    let _ = std::fs::remove_file(&outside_path);
}

#[test]
fn add_absolute_path_outside_repo() {
    let repo = TestRepo::new();

    // Try to add a file using an absolute path outside repo
    let err = repo.run_err(&["add", "/tmp/nonexistent_vrit_test.txt"]);
    assert!(
        err.contains("outside") || err.contains("did not match") || err.contains("pathspec"),
        "expected outside-repo error, got: {err}"
    );
}

#[test]
fn crafted_tree_with_dotdot_entry_checkout() {
    let repo = TestRepo::new();
    repo.write_file("safe.txt", "original");
    repo.commit_all("base");

    // Craft a tree object with a "../escape" entry name
    // The parse_tree function should reject this with "invalid tree entry name"
    // We write a raw tree object directly to the store
    let blob_content = b"blob 13\0escaped data!";
    let blob_sha = write_raw_object(&repo, blob_content);

    // Build tree with malicious entry: "../escape"
    let tree_body = build_tree_entry("100644", "../escape", &blob_sha);
    let tree_header = format!("tree {}\0", tree_body.len());
    let mut tree_data = tree_header.into_bytes();
    tree_data.extend_from_slice(&tree_body);
    let tree_sha = write_raw_object(&repo, &tree_data);

    // Build commit pointing to this tree
    let commit_body = format!(
        "tree {tree_sha}\nauthor Test <t@t.com> 1 +0000\ncommitter Test <t@t.com> 1 +0000\n\nmalicious\n"
    );
    let commit_header = format!("commit {}\0", commit_body.len());
    let mut commit_data = commit_header.into_bytes();
    commit_data.extend_from_slice(commit_body.as_bytes());
    let commit_sha = write_raw_object(&repo, &commit_data);

    // Write a branch pointing to this commit
    repo.write_raw(
        ".vrit/refs/heads/evil",
        format!("{commit_sha}\n").as_bytes(),
    );

    // Checkout should reject the tree entry with ".."
    let err = repo.run_err_no_panic(&["checkout", "evil"]);
    assert!(
        err.contains("invalid tree entry") || err.contains("refusing") || err.contains("outside"),
        "expected tree traversal rejection, got: {err}"
    );

    // Verify no file was written outside repo
    assert!(
        !repo.dir.parent().unwrap().join("escape").exists(),
        "file was written outside the repository!"
    );
}

#[test]
fn crafted_tree_with_absolute_path_entry() {
    let repo = TestRepo::new();
    repo.write_file("safe.txt", "original");
    repo.commit_all("base");

    let blob_content = b"blob 5\0evil!";
    let blob_sha = write_raw_object(&repo, blob_content);

    // Tree entry with absolute path
    let tree_body = build_tree_entry("100644", "/tmp/pwned", &blob_sha);
    let tree_header = format!("tree {}\0", tree_body.len());
    let mut tree_data = tree_header.into_bytes();
    tree_data.extend_from_slice(&tree_body);
    let tree_sha = write_raw_object(&repo, &tree_data);

    let commit_body = format!(
        "tree {tree_sha}\nauthor Test <t@t.com> 1 +0000\ncommitter Test <t@t.com> 1 +0000\n\nmalicious\n"
    );
    let commit_header = format!("commit {}\0", commit_body.len());
    let mut commit_data = commit_header.into_bytes();
    commit_data.extend_from_slice(commit_body.as_bytes());
    let commit_sha = write_raw_object(&repo, &commit_data);

    repo.write_raw(
        ".vrit/refs/heads/evil2",
        format!("{commit_sha}\n").as_bytes(),
    );

    let err = repo.run_err_no_panic(&["checkout", "evil2"]);
    assert!(
        err.contains("invalid tree entry") || err.contains("refusing"),
        "expected absolute path rejection, got: {err}"
    );
}

// --- Symlinks ---

#[test]
fn symlink_to_file_outside_repo_skipped() {
    let repo = TestRepo::new();
    repo.write_file("real.txt", "real content");

    // Create symlink pointing outside repo
    let outside_file = repo.dir.parent().unwrap().join("symlink_target.txt");
    std::fs::write(&outside_file, "outside data").unwrap();

    let link_path = repo.dir.join("link.txt");
    std::os::unix::fs::symlink(&outside_file, &link_path).unwrap();

    // add . should skip symlinks silently and add the real file
    repo.run_ok(&["add", "."]);
    let status = repo.run_ok(&["status"]);

    // link.txt should not appear as a staged file
    assert!(
        !status.contains("new file:   link.txt"),
        "symlink should not be staged: {status}"
    );
    // real.txt should be staged
    assert!(
        status.contains("real.txt"),
        "real file should be staged: {status}"
    );

    // Cleanup
    let _ = std::fs::remove_file(&outside_file);
}

#[test]
fn symlink_to_directory_skipped() {
    let repo = TestRepo::new();
    repo.write_file("real.txt", "content");

    let outside_dir = repo.dir.parent().unwrap().join("symlink_dir_target");
    std::fs::create_dir_all(&outside_dir).unwrap();
    std::fs::write(outside_dir.join("secret.txt"), "sensitive").unwrap();

    let link_path = repo.dir.join("linked_dir");
    std::os::unix::fs::symlink(&outside_dir, &link_path).unwrap();

    // add . should skip symlinks silently
    repo.run_ok(&["add", "."]);

    // Verify no symlinked content was staged (it may appear as untracked)
    let status = repo.run_ok(&["status"]);
    assert!(
        !status.contains("new file:   linked_dir"),
        "symlinked dir files should not be staged: {status}"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&outside_dir);
}

// --- Scale tests ---

#[test]
fn thousand_files() {
    let repo = TestRepo::new();
    for i in 0..1000 {
        repo.write_file(&format!("file_{i:04}.txt"), &format!("content {i}"));
    }
    repo.commit_all("1000 files");

    let status = repo.run_ok(&["status"]);
    // All files should be tracked clean
    assert!(
        !status.contains("modified") && !status.contains("untracked"),
        "all 1000 files should be clean"
    );
}

#[test]
fn deeply_nested_directory() {
    let repo = TestRepo::new();
    let mut path = String::new();
    for i in 0..50 {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(&format!("d{i}"));
    }
    path.push_str("/deep_file.txt");
    repo.write_file(&path, "deep content");
    repo.commit_all("deep nesting");

    assert!(repo.file_exists(&path));
}

#[test]
fn hundred_sequential_commits() {
    let repo = TestRepo::new();
    for i in 0..100 {
        repo.write_file("counter.txt", &format!("{i}"));
        repo.commit_all(&format!("commit {i}"));
    }

    let log = repo.run_ok(&["log"]);
    // Should contain all commits (at least first and last)
    assert!(log.contains("commit 0"), "first commit missing from log");
    assert!(log.contains("commit 99"), "last commit missing from log");

    // Count commit entries in log
    let commit_count = log.lines().filter(|l| l.starts_with("commit ")).count();
    assert_eq!(commit_count, 100, "log should show all 100 commits");
}

#[test]
fn hundred_branches() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    for i in 0..100 {
        repo.run_ok(&["branch", &format!("branch-{i:03}")]);
    }

    let branches = repo.run_ok(&["branch"]);
    let branch_count = branches.lines().count();
    // 100 created + 1 main = 101
    assert_eq!(branch_count, 101, "should list all 101 branches");
}

#[test]
fn large_diff_many_lines_changed() {
    let repo = TestRepo::new();
    let original: String = (0..2000).map(|i| format!("line {i}\n")).collect();
    repo.write_file("big.txt", &original);
    repo.commit_all("original");

    let modified: String = (0..2000).map(|i| format!("changed line {i}\n")).collect();
    repo.write_file("big.txt", &modified);

    let diff = repo.run_ok(&["diff"]);
    assert!(
        diff.contains("---") && diff.contains("+++"),
        "diff should show changes for large file"
    );
}

// --- Stash depth ---

#[test]
fn twenty_stashes_lifo() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base");
    repo.commit_all("base");

    for i in 0..20 {
        repo.write_file("f.txt", &format!("stash {i}"));
        repo.run_ok(&["stash"]);
    }

    let list = repo.run_ok(&["stash", "list"]);
    let stash_count = list.lines().count();
    assert_eq!(stash_count, 20, "should list all 20 stashes");

    // Pop all and verify LIFO order
    for i in (0..20).rev() {
        repo.run_ok(&["stash", "pop"]);
        let content = repo.read_file("f.txt");
        assert_eq!(
            content,
            format!("stash {i}"),
            "stash pop #{} should restore 'stash {i}'",
            19 - i
        );
    }
}

// --- Long names ---

#[test]
fn long_branch_name() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let long_name = "b".repeat(200);
    let output = repo.run(&["branch", &long_name]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on long branch name: {combined}"
    );
}

#[test]
fn merge_with_many_conflicting_files() {
    let repo = TestRepo::new();

    // Create 10 files
    for i in 0..10 {
        repo.write_file(&format!("f{i}.txt"), "base content\n");
    }
    repo.commit_all("base");

    repo.run_ok(&["branch", "feature"]);
    repo.run_ok(&["checkout", "feature"]);
    for i in 0..10 {
        repo.write_file(&format!("f{i}.txt"), "feature content\n");
    }
    repo.commit_all("feature changes");

    repo.run_ok(&["checkout", "main"]);
    for i in 0..10 {
        repo.write_file(&format!("f{i}.txt"), "main content\n");
    }
    repo.commit_all("main changes");

    let out = repo.run_ok(&["merge", "feature"]);

    // All 10 files should have conflict markers
    let conflict_count = out.lines().filter(|l| l.contains("CONFLICT")).count();
    assert_eq!(
        conflict_count, 10,
        "all 10 files should have conflicts, got {conflict_count}"
    );

    // Verify each file has markers
    for i in 0..10 {
        let content = repo.read_file(&format!("f{i}.txt"));
        assert!(
            content.contains("<<<<<<<") && content.contains(">>>>>>>"),
            "f{i}.txt missing conflict markers"
        );
    }
}

// --- Helper functions for crafting raw objects ---

fn write_raw_object(repo: &TestRepo, data: &[u8]) -> String {
    use sha1::{Digest, Sha1};

    let sha = format!("{:x}", Sha1::new_with_prefix(data).finalize());
    let dir = format!(".vrit/objects/{}", &sha[..2]);
    let file = format!("{}/{}", dir, &sha[2..]);

    // Compress with zlib
    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, data).unwrap();
    let compressed = encoder.finish().unwrap();

    repo.write_raw(&file, &compressed);
    sha
}

fn build_tree_entry(mode: &str, name: &str, sha_hex: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(mode.as_bytes());
    buf.push(b' ');
    buf.extend_from_slice(name.as_bytes());
    buf.push(0);
    // Convert hex SHA to 20 bytes
    for i in (0..40).step_by(2) {
        buf.push(u8::from_str_radix(&sha_hex[i..i + 2], 16).unwrap());
    }
    buf
}
