---
status: pending
priority: p3
issue_id: "014"
tags: [code-review, performance]
dependencies: []
---

# Minor Performance Optimizations

## Problem Statement

Several small inefficiencies:
1. `bytes_to_hex` allocates a new String per byte via `format!` (`src/object.rs:343`)
2. `write_tree_from_index` sort comparator allocates Strings per comparison (`src/commands/write_tree.rs:59-72`)
3. `find_untracked` sorts at every recursive call instead of once at top (`src/commands/status.rs:213`)
4. `diff_unstaged` clones working file content unnecessarily (`src/commands/diff_cmd.rs:72`)

## Proposed Solutions

1. Pre-allocate String of capacity 40, use lookup table or `write!`
2. Compare bytes directly, append `/` only conceptually
3. Sort only at the top-level call
4. Compute SHA without `Object::Blob` wrapper

## Acceptance Criteria

- [ ] No per-byte String allocation in hex conversion
- [ ] No String allocation in sort comparators
