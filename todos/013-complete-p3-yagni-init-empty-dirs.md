---
status: pending
priority: p3
issue_id: "013"
tags: [code-review, quality]
dependencies: []
---

# YAGNI: init Creates Unused Directories

## Problem Statement

`init.rs` creates `.vrit/objects/info` and `.vrit/objects/pack` directories that are never used. Packfiles are a "stretch" goal in the spec.

**File:** `src/commands/init.rs:24-25`

## Proposed Solutions

Remove the two lines. Add them when/if packfile support is built.

## Acceptance Criteria

- [ ] `init` only creates directories that are actually used
