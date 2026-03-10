---
status: pending
priority: p1
issue_id: "003"
tags: [code-review, architecture]
dependencies: []
---

# Extract Shared Infrastructure from Command Modules

## Problem Statement

Core operations are duplicated across 6-9 command modules, creating fragile cross-command dependencies and maintenance risk where a fix in one copy doesn't propagate.

**Duplicated functions:**
- `update_current_ref`: 3 identical copies (commit.rs, merge.rs, reset.rs)
- `commit_tree_entries` pattern: 6 copies with different names (status.rs, checkout.rs, merge.rs, reset.rs, stash.rs, diff_cmd.rs)
- `flatten_tree`: lives in status.rs but imported by 6 other commands (layering violation)
- `resolve_head`: lives in commit.rs but imported by 9 other commands
- `collect_branches`/`collect_tags`: identical functions in branch.rs and tag.rs
- `write_blob_to_working_tree`: 1 extracted + 3 inline copies

**Total duplicated code:** ~130-150 lines

## Findings

- Architecture-strategist: P0 priority, creates fragile inter-command dependencies
- Pattern-recognition: identified 6 distinct DRY violations
- Code-simplicity: ~130 lines removable, main source of duplication

## Proposed Solutions

### Option A: Extend `repo.rs` with shared ref/tree operations (recommended)
Move `resolve_head`, `update_current_ref`, `current_branch`, `collect_refs`, `flatten_tree`, `commit_tree_entries`, `write_blob_to_working_tree` into `repo.rs`.
- Pros: Uses existing module, no new files
- Cons: `repo.rs` grows from 20 to ~100 lines
- Effort: Medium
- Risk: Low (pure refactor, no behavior change)

### Option B: Create `refs.rs` + `worktree.rs` modules
Split by concern: ref operations in `refs.rs`, tree/file operations in `worktree.rs`.
- Pros: Better separation of concerns
- Cons: More new files
- Effort: Medium
- Risk: Low

## Acceptance Criteria

- [ ] No command module imports from a sibling command module
- [ ] `update_current_ref` exists in exactly one place
- [ ] `flatten_tree` lives in a core module, not in commands/status.rs
- [ ] `collect_branches` and `collect_tags` share a single implementation
- [ ] All tests still pass
