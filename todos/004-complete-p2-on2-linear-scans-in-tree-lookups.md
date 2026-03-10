---
status: pending
priority: p2
issue_id: "004"
tags: [code-review, performance]
dependencies: []
---

# O(n^2) Linear Scans on Flattened Tree Vectors

## Problem Statement

Tree entries are stored as `Vec<(String, String)>` and searched with `.iter().find()` in tight loops. In merge, status, checkout, stash, and diff, this produces O(N^2) behavior. A repository with 10K files would require ~100M string comparisons during merge.

**Key locations:**
- `src/commands/merge.rs:443` (`find_sha`) called per-path in 3-way merge
- `src/commands/status.rs:50` linear scan per index entry
- `src/commands/checkout.rs:120, 173` linear scan per entry
- `src/commands/stash.rs:49, 138` linear scan per entry
- `src/commands/diff_cmd.rs:37` linear scan per entry

## Proposed Solutions

### Option A: Convert to HashMap (recommended)
Replace `Vec<(String, String)>` with `HashMap<String, String>` for lookup operations. O(N^2) → O(N).
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] All tree-entry lookup operations use HashMap or BTreeMap
- [ ] Merge, status, checkout, diff are O(N) not O(N^2)
