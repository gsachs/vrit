// Integration tests for vrit tag
mod helpers;
use helpers::TestRepo;

#[test]
fn tag_lightweight_create_and_list() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["tag", "v1.0"]);
    let list = repo.run_ok(&["tag"]);
    assert!(list.contains("v1.0"));
}

#[test]
fn tag_annotated_creates_tag_object() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["tag", "-a", "v2.0", "-m", "Release 2.0"]);

    let list = repo.run_ok(&["tag"]);
    assert!(list.contains("v2.0"));

    // The tag ref should point to a tag object, not directly to a commit
    let tag_sha = repo.read_ref("refs/tags/v2.0");
    let content = repo.run_ok(&["cat-file", "-t", &tag_sha]);
    assert_eq!(content.trim(), "tag");

    let detail = repo.run_ok(&["cat-file", "-p", &tag_sha]);
    assert!(detail.contains("Release 2.0"));
    assert!(detail.contains("tag v2.0"));
}

#[test]
fn tag_lightweight_points_to_commit() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["tag", "v1.0"]);

    let tag_sha = repo.read_ref("refs/tags/v1.0");
    let head_sha = repo.read_ref("refs/heads/main");
    assert_eq!(tag_sha, head_sha);
}

#[test]
fn tag_delete() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["tag", "temp"]);
    repo.run_ok(&["tag", "-d", "temp"]);

    let list = repo.run_ok(&["tag"]);
    assert!(!list.contains("temp"));
}

#[test]
fn tag_refuses_duplicate() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["tag", "v1.0"]);
    let err = repo.run_err(&["tag", "v1.0"]);
    assert!(err.contains("already exists"));
}

#[test]
fn tag_annotated_requires_message() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    let err = repo.run_err(&["tag", "-a", "v1.0"]);
    assert!(err.contains("-m") || err.contains("message"));
}

#[test]
fn tag_multiple_listed_sorted() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "v1");
    repo.commit_all("first");

    repo.run_ok(&["tag", "beta"]);
    repo.run_ok(&["tag", "alpha"]);
    repo.run_ok(&["tag", "gamma"]);

    let list = repo.run_ok(&["tag"]);
    let tags: Vec<&str> = list.lines().collect();
    assert_eq!(tags, vec!["alpha", "beta", "gamma"]);
}
