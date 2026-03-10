---
status: pending
priority: p2
issue_id: "006"
tags: [code-review, security]
dependencies: []
---

# Branch/Tag Name and HEAD ref: Path Traversal

## Problem Statement

Branch and tag names from CLI arguments are used directly with `.join()` to construct paths under `refs/heads/` or `refs/tags/`. A user could supply `../../HEAD` as a branch name to overwrite repository metadata.

Additionally, HEAD's `ref:` path is trusted without validation — a tampered HEAD file with `ref: ../../some/path` would cause writes outside `.vrit/`.

**Files:**
- `src/commands/branch.rs:70, 95`
- `src/commands/tag.rs:85, 107, 149`
- `src/commands/commit.rs:93-94` (resolve_head ref: path)

## Proposed Solutions

### Option A: Validate ref names (recommended)
Reject names containing `..`, control characters, or that escape the refs directory. Validate `ref:` paths start with `refs/`.
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] Branch/tag creation rejects names with `..` or path separators that escape refs
- [ ] `resolve_head` validates ref path starts with `refs/`
