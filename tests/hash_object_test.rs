// Integration tests for plumbing commands: hash-object, cat-file, ls-tree, write-tree
mod helpers;
use helpers::TestRepo;

#[test]
fn hash_object_produces_deterministic_sha() {
    let repo = TestRepo::new();
    repo.write_file("hello.txt", "hello world\n");

    let sha1 = repo.run_ok(&["hash-object", "hello.txt"]);
    let sha2 = repo.run_ok(&["hash-object", "hello.txt"]);
    assert_eq!(sha1.trim(), sha2.trim());
    assert_eq!(sha1.trim().len(), 40);
}

#[test]
fn hash_object_matches_git_known_sha() {
    let repo = TestRepo::new();
    repo.write_file("hello.txt", "hello world\n");

    // git hash-object produces 3b18e512dba79e4c8300dd08aeb37f8e728b8dad for "hello world\n"
    let sha = repo.run_ok(&["hash-object", "hello.txt"]);
    assert_eq!(sha.trim(), "3b18e512dba79e4c8300dd08aeb37f8e728b8dad");
}

#[test]
fn hash_object_write_stores_and_cat_file_reads() {
    let repo = TestRepo::new();
    repo.write_file("data.txt", "some data\n");

    let sha = repo.run_ok(&["hash-object", "-w", "data.txt"]);
    let sha = sha.trim();

    // cat-file -p should retrieve the content
    let content = repo.run_ok(&["cat-file", "-p", sha]);
    assert_eq!(content, "some data\n");

    // cat-file -t should show type
    let obj_type = repo.run_ok(&["cat-file", "-t", sha]);
    assert_eq!(obj_type.trim(), "blob");

    // cat-file -s should show size
    let size = repo.run_ok(&["cat-file", "-s", sha]);
    assert_eq!(size.trim(), "10"); // "some data\n" = 10 bytes
}

#[test]
fn write_tree_and_ls_tree_roundtrip() {
    let repo = TestRepo::new();
    repo.write_file("a.txt", "aaa");
    repo.write_file("b.txt", "bbb");
    repo.run_ok(&["add", "."]);

    let tree_sha = repo.run_ok(&["write-tree"]);
    let tree_sha = tree_sha.trim();
    assert_eq!(tree_sha.len(), 40);

    let listing = repo.run_ok(&["ls-tree", tree_sha]);
    assert!(listing.contains("a.txt"));
    assert!(listing.contains("b.txt"));
}

#[test]
fn cat_file_shows_commit_object() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    let commit_out = repo.commit_all("first commit");

    // Extract SHA from commit output like "[abc1234] first commit"
    let sha_short = commit_out
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.split(']').next())
        .expect("expected [sha] in commit output");

    // Read the ref to get full SHA
    let full_sha = repo.read_ref("refs/heads/main");

    let content = repo.run_ok(&["cat-file", "-p", &full_sha]);
    assert!(content.contains("tree "));
    assert!(content.contains("author Test User"));
    assert!(content.contains("first commit"));
}
