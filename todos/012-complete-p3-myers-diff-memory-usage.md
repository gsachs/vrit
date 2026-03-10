---
status: pending
priority: p3
issue_id: "012"
tags: [code-review, performance]
dependencies: []
---

# Myers Diff O(D*(N+M)) Memory Usage

## Problem Statement

The Myers diff implementation clones the full frontier vector (size 2*(N+M)+1) twice per edit-distance iteration. For large diffs (5000 edits on 10K-line files), this allocates ~8GB for trace storage.

**File:** `src/diff.rs:27-28`

## Proposed Solutions

### Option A: Linear-space divide-and-conquer variant
- Effort: High
- Risk: Medium (algorithm complexity)

### Option B: Limit max diff size with early bailout
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] Diffing large files does not exhaust memory
