---
status: pending
priority: p2
issue_id: "008"
tags: [code-review, quality]
dependencies: []
---

# Silent Error Suppression on fs::read with unwrap_or_default

## Problem Statement

Multiple dirty-check paths use `fs::read(&path).unwrap_or_default()`, silently treating permission errors or I/O failures as "empty file." This could cause false dirty-detection and data loss — e.g., a file that can't be read appears changed, leading to incorrect merge/checkout decisions.

**Locations:**
- `src/commands/checkout.rs:169`
- `src/commands/merge.rs:381`
- `src/commands/stash.rs:33`
- `src/commands/diff_cmd.rs:71`
- `src/commands/status.rs:73`

## Proposed Solutions

### Option A: Propagate errors (recommended)
Replace `unwrap_or_default()` with `?` or explicit error handling. If a file can't be read during a dirty check, that's an error the user should see.
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] No `fs::read().unwrap_or_default()` in dirty-check paths
- [ ] Permission/I/O errors surface to user
