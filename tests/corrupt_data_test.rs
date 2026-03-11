// Adversarial tests: corrupt .vrit/ internals and verify graceful error handling
mod helpers;
use helpers::TestRepo;

// --- Index corruption ---

#[test]
fn corrupt_index_truncated() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    repo.write_raw(".vrit/index", &[0x01, 0x00, 0x00]);
    repo.run_err_no_panic(&["status"]);
}

#[test]
fn corrupt_index_wrong_version() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    let mut data = repo.read_raw(".vrit/index");
    data[0] = 255;
    repo.write_raw(".vrit/index", &data);

    let err = repo.run_err_no_panic(&["status"]);
    assert!(err.contains("version"), "expected version error, got: {err}");
}

#[test]
fn corrupt_index_inflated_count() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // Version 1 + count of 999999 but only one entry worth of data
    let mut data = vec![0x01]; // version
    data.extend_from_slice(&999999u32.to_be_bytes());
    // Append minimal entry data (too small for 999999 entries)
    data.extend_from_slice(&[0u8; 27]);
    repo.write_raw(".vrit/index", &data);

    repo.run_err_no_panic(&["status"]);
}

#[test]
fn corrupt_index_zero_length_path() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // Build a valid-looking index entry with path_len=0
    let mut data = vec![0x01]; // version
    data.extend_from_slice(&1u32.to_be_bytes()); // 1 entry
    data.extend_from_slice(&0o100644u32.to_be_bytes()); // mode
    data.extend_from_slice(&[0xaa; 20]); // sha bytes
    data.extend_from_slice(&0u16.to_be_bytes()); // path_len = 0
    repo.write_raw(".vrit/index", &data);

    // Should handle gracefully (empty path) — either error or tolerate
    let output = repo.run(&["status"]);
    // Just verify no panic
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on zero-length path: {combined}"
    );
}

#[test]
fn corrupt_index_non_utf8_path() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    let mut data = vec![0x01]; // version
    data.extend_from_slice(&1u32.to_be_bytes()); // 1 entry
    data.extend_from_slice(&0o100644u32.to_be_bytes()); // mode
    data.extend_from_slice(&[0xaa; 20]); // sha bytes
    let bad_path: &[u8] = &[0xFF, 0xFE, 0x80]; // invalid UTF-8
    data.extend_from_slice(&(bad_path.len() as u16).to_be_bytes());
    data.extend_from_slice(bad_path);
    repo.write_raw(".vrit/index", &data);

    let err = repo.run_err_no_panic(&["status"]);
    assert!(
        err.to_lowercase().contains("utf") || err.contains("invalid"),
        "expected UTF-8 error, got: {err}"
    );
}

#[test]
fn corrupt_index_unsorted_entries() {
    let repo = TestRepo::new();
    repo.write_file("a.txt", "aaa");
    repo.write_file("z.txt", "zzz");
    repo.commit_all("both files");

    // Build index with entries in wrong order (z before a)
    let sha_bytes = [0xbb; 20];
    let mut data = vec![0x01]; // version
    data.extend_from_slice(&2u32.to_be_bytes()); // 2 entries

    // Entry 1: z.txt (should be second)
    data.extend_from_slice(&0o100644u32.to_be_bytes());
    data.extend_from_slice(&sha_bytes);
    let path = b"z.txt";
    data.extend_from_slice(&(path.len() as u16).to_be_bytes());
    data.extend_from_slice(path);

    // Entry 2: a.txt (should be first)
    data.extend_from_slice(&0o100644u32.to_be_bytes());
    data.extend_from_slice(&sha_bytes);
    let path = b"a.txt";
    data.extend_from_slice(&(path.len() as u16).to_be_bytes());
    data.extend_from_slice(path);

    repo.write_raw(".vrit/index", &data);

    // Should not panic — either error or produce output
    let output = repo.run(&["status"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on unsorted index: {combined}"
    );
}

#[test]
fn corrupt_index_duplicate_paths() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // Build index with same path twice
    let sha_bytes = [0xcc; 20];
    let mut data = vec![0x01]; // version
    data.extend_from_slice(&2u32.to_be_bytes()); // 2 entries

    for _ in 0..2 {
        data.extend_from_slice(&0o100644u32.to_be_bytes());
        data.extend_from_slice(&sha_bytes);
        let path = b"f.txt";
        data.extend_from_slice(&(path.len() as u16).to_be_bytes());
        data.extend_from_slice(path);
    }

    repo.write_raw(".vrit/index", &data);

    // Should not panic
    let output = repo.run(&["status"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on duplicate index entries: {combined}"
    );
}

// --- Object corruption ---

#[test]
fn corrupt_object_truncated_zlib() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    let head_sha = repo.read_ref("refs/heads/main");
    let obj_dir = format!(".vrit/objects/{}", &head_sha[..2]);
    let obj_file = format!("{}/{}", obj_dir, &head_sha[2..]);

    // Write truncated zlib data
    repo.write_raw(&obj_file, &[0x78, 0x9c, 0x00]); // zlib header + garbage

    repo.run_err_no_panic(&["cat-file", "-p", &head_sha]);
}

#[test]
fn corrupt_object_valid_zlib_garbage_content() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    let head_sha = repo.read_ref("refs/heads/main");
    let obj_file = format!(
        ".vrit/objects/{}/{}",
        &head_sha[..2],
        &head_sha[2..]
    );

    // Compress garbage that doesn't have "type size\0" header
    let garbage = b"this is not a valid git object";
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, garbage).unwrap();
    let compressed = encoder.finish().unwrap();
    repo.write_raw(&obj_file, &compressed);

    repo.run_err_no_panic(&["cat-file", "-p", &head_sha]);
}

#[test]
fn corrupt_object_zero_byte_file() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    let head_sha = repo.read_ref("refs/heads/main");
    let obj_file = format!(
        ".vrit/objects/{}/{}",
        &head_sha[..2],
        &head_sha[2..]
    );

    repo.write_raw(&obj_file, &[]);
    repo.run_err_no_panic(&["cat-file", "-p", &head_sha]);
}

#[test]
fn corrupt_object_directory_missing() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // Use a SHA that definitely doesn't exist
    let fake_sha = "0000000000000000000000000000000000000000";
    repo.run_err_no_panic(&["cat-file", "-p", fake_sha]);
}

// --- HEAD/ref corruption ---

#[test]
fn corrupt_head_empty_file() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    repo.write_raw(".vrit/HEAD", &[]);
    repo.run_err_no_panic(&["status"]);
}

#[test]
fn corrupt_head_garbage_bytes() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    repo.write_raw(".vrit/HEAD", b"\xff\xfe\x00garbage\n");
    repo.run_err_no_panic(&["status"]);
}

#[test]
fn corrupt_head_points_to_nonexistent_branch() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    repo.write_raw(".vrit/HEAD", b"ref: refs/heads/ghost\n");
    // This should be treated as an unborn branch (None), not a crash
    let output = repo.run(&["status"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !combined.contains("panicked at"),
        "panicked on nonexistent branch ref: {combined}"
    );
}

#[test]
fn corrupt_branch_ref_non_hex() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    repo.write_raw(".vrit/refs/heads/main", b"ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ\n");

    // Trying to use this corrupt ref should error, not panic
    repo.run_err_no_panic(&["log"]);
}

#[test]
fn corrupt_branch_ref_short_sha() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // 39-char hex (one too short)
    repo.write_raw(
        ".vrit/refs/heads/main",
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
    );
    repo.run_err_no_panic(&["log"]);
}

#[test]
fn corrupt_branch_ref_long_sha() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // 41-char hex (one too long)
    repo.write_raw(
        ".vrit/refs/heads/main",
        b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
    );
    repo.run_err_no_panic(&["log"]);
}

// --- Config corruption ---

#[test]
fn corrupt_config_malformed_ini() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.run_ok(&["add", "."]);

    // Overwrite config with malformed content
    repo.write_raw(".vrit/config", b"[user\nname = broken");

    // Commit requires config — should error gracefully
    repo.run_err_no_panic(&["commit", "-m", "test"]);
}

#[test]
fn corrupt_config_missing_file() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.run_ok(&["add", "."]);

    std::fs::remove_file(repo.dir.join(".vrit/config")).expect("remove config");

    let err = repo.run_err_no_panic(&["commit", "-m", "test"]);
    assert!(
        err.contains("user.name") || err.contains("config") || err.contains("not set"),
        "expected config error, got: {err}"
    );
}

// --- Merge/stash state corruption ---

#[test]
fn corrupt_merge_head_non_hex_then_commit() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // Manually create a bogus MERGE_HEAD
    repo.write_raw(".vrit/MERGE_HEAD", b"not-a-valid-sha\n");

    // Modify and stage a change so commit has something to do
    repo.write_file("f.txt", "modified");
    repo.run_ok(&["add", "."]);

    // Commit should reject the corrupt MERGE_HEAD
    let err = repo.run_err_no_panic(&["commit", "-m", "merge commit attempt"]);
    assert!(
        err.contains("corrupt MERGE_HEAD") || err.contains("invalid SHA"),
        "expected corrupt MERGE_HEAD error, got: {err}"
    );
}

#[test]
fn corrupt_stash_ref_then_list() {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "hello");
    repo.commit_all("first");

    // Write a bogus stash ref pointing to a non-existent object
    std::fs::create_dir_all(repo.dir.join(".vrit/refs")).unwrap();
    repo.write_raw(
        ".vrit/refs/stash",
        b"0000000000000000000000000000000000000000\n",
    );

    repo.run_err_no_panic(&["stash", "list"]);
}
