---
title: "canonicalize() bypass allows path traversal on new files"
date: 2026-03-11
category: security-issues
tags:
  - path-traversal
  - canonicalize
  - checkout
  - write-blob
severity: high
component: src/repo.rs
symptoms:
  - "write_blob_to_working_tree silently writes outside repository root"
  - "Malicious tree entry with '../' in name escapes repo during checkout"
  - "Path traversal check ineffective for files being created (not yet on disk)"
related_fixes:
  - "write_to_store double-hash elimination (src/object.rs)"
  - "Silent no-op for non-blob objects (src/repo.rs)"
  - "Redundant HEAD read in reset (src/commands/reset.rs)"
  - "Vec-to-HashMap boilerplate (src/repo.rs commit_tree_entries_map)"
---

# canonicalize() bypass allows path traversal on new files

## Problem

`write_blob_to_working_tree` in `src/repo.rs` used `std::fs::canonicalize()` on the destination file path to guard against path traversal attacks (e.g., a tree entry named `../../../etc/passwd`). However, `canonicalize()` requires the target path to **already exist on disk**. During checkout, merge, and stash-pop, the files being written are new — they don't exist yet.

The code used `unwrap_or_else` to fall back to the raw, unchecked path when `canonicalize()` failed:

```rust
let canonical = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
```

This meant the path traversal check was **completely ineffective** for the most common case: writing new files during checkout.

## Root Cause

`std::fs::canonicalize()` resolves symlinks and normalizes paths by querying the filesystem. It returns `Err` when the path doesn't exist. The fallback silently used the unresolved path, defeating the security check.

Additionally, `write_blob_to_working_tree` used `if let Object::Blob(content) = obj` which silently returned `Ok(())` when passed a non-blob SHA — hiding bugs at call sites.

## Solution

**Canonicalize the parent directory instead of the file itself.** After `create_dir_all`, the parent directory exists and can be canonicalized. The file name component is then implicitly validated by the parent check.

```rust
let file_path = repo_root.join(path);
if let Some(parent) = file_path.parent() {
    fs::create_dir_all(parent).ok();
    let parent_canonical = parent.canonicalize()
        .map_err(|e| format!("cannot resolve parent directory for '{path}': {e}"))?;
    let root_canonical = repo_root.canonicalize()
        .map_err(|e| format!("cannot resolve repo root: {e}"))?;
    if !parent_canonical.starts_with(&root_canonical) {
        return Err(format!("refusing to write outside repository: {path}"));
    }
}
```

Also changed the non-blob case from silent success to an explicit error:

```rust
let content = match obj {
    Object::Blob(data) => data,
    _ => return Err(format!("expected blob for '{path}', got {}", obj.type_str())),
};
```

## Additional Fixes in Same Session

### 1. `write_to_store` duplicated hash computation (src/object.rs)

`sha()` and `write_to_store` both independently serialized and hashed. Extracted a shared `hash_bytes()` method so `write_to_store` serializes once and reuses the hash.

### 2. Redundant HEAD read in reset (src/commands/reset.rs)

When `commit` is `None`, `resolve_head` was called twice — once to get the default target, once to get `current_sha`. Reordered to read HEAD once and clone the result.

### 3. Vec-to-HashMap boilerplate (src/repo.rs)

The pattern `.into_iter().map(|(p, s, m)| (p, (s, m))).collect()` appeared 5+ times across checkout, merge, and other commands. Added `commit_tree_entries_map()` helper. Removed the now-unused `HashMap` import from merge.rs.

## Prevention

- **Never use `canonicalize()` on paths that may not exist yet.** Canonicalize an ancestor directory that is guaranteed to exist, or use path-component validation instead.
- **Avoid silent fallbacks in security checks.** If `canonicalize()` fails, that's an error — not a reason to skip the check.
- **Prefer `match` over `if let` when all variants should be handled.** Using `if let Object::Blob(content) = obj` made it easy to silently ignore non-blob objects. A `match` with an explicit error arm catches this at the call site.
- **Run `/simplify` after major refactors.** The three-agent review (reuse, quality, efficiency) caught all five issues in this session, including the security bypass.
