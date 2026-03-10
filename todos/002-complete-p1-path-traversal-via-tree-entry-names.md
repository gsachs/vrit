---
status: pending
priority: p1
issue_id: "002"
tags: [code-review, security]
dependencies: []
---

# Path Traversal via Tree Entry Names in Checkout/Merge/Stash

## Problem Statement

When checking out, merging, or popping a stash, tree entry `name` fields are joined to the repository root to write files. Names from parsed tree objects are not validated against path traversal. A malicious tree entry named `../../../etc/cron.d/malicious` would write files outside the repository.

**Files:**
- `src/commands/checkout.rs:132-141`
- `src/commands/merge.rs:458-474`
- `src/commands/stash.rs:119-127, 177-185`

## Findings

- Security-sentinel: HIGH severity - arbitrary file write
- Same unvalidated `.join(path)` pattern in 4 locations
- Symlinks at target paths could also redirect writes (LOW-2)

## Proposed Solutions

### Option A: Validate paths after construction (recommended)
Canonicalize `file_path` and verify it starts with `repo_root`. Also validate tree entry names during `parse_tree` to reject `..`, absolute paths, null bytes.
- Pros: Defense in depth at both parse and write
- Cons: `canonicalize` requires path to exist for full resolution
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] `parse_tree` rejects entry names containing `..` or starting with `/`
- [ ] All file-write locations validate path is under repo_root
- [ ] Check for and remove symlinks before writing in checkout/merge/stash
