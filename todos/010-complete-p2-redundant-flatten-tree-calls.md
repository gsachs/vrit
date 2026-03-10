---
status: pending
priority: p2
issue_id: "010"
tags: [code-review, performance]
dependencies: ["003"]
---

# Redundant flatten_tree Calls Within Single Operations

## Problem Statement

Several commands call `flatten_tree` multiple times for the same commit:
- `checkout.rs` `switch_to_commit`: calls `target_tree_entries` at line 116, then `check_dirty_files` calls it again at line 160 for the same SHA.
- `stash.rs` `execute_stash`: calls `head_tree_entries` at line 43 and again at line 117.

Each call reads and decompresses every tree object from disk. For 1000 directories, that's 1000+ file reads duplicated.

## Proposed Solutions

### Option A: Pass pre-computed entries as parameters (recommended)
Compute tree entries once and pass the result to functions that need it.
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] No commit's tree is flattened more than once per command invocation
