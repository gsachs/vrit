---
status: pending
priority: p3
issue_id: "011"
tags: [code-review, quality]
dependencies: []
---

# Dead Code: Index::remove_dir

## Problem Statement

`Index::remove_dir` at `src/index.rs:69` is `pub` but never called anywhere. Compiler warns about it.

## Proposed Solutions

Remove it, or wire it into `add.rs` for directory-level removals.

## Acceptance Criteria

- [ ] No dead code warning from compiler
