---
status: pending
priority: p2
issue_id: "007"
tags: [code-review, quality]
dependencies: ["003"]
---

# Hardcoded 0o100644 Mode Loses Executable Bit

## Problem Statement

When building `IndexEntry` during checkout, merge, reset, and stash, the mode is hardcoded to `0o100644`. Executable files (`0o100755`) lose their permission bit. Only `add.rs` reads the actual file mode.

Also, `TreeEntry.mode` is `String` while `IndexEntry.mode` is `u32` — a type inconsistency that forces `format!("{:o}", ...)` conversions.

**Locations:** checkout.rs:144, merge.rs:99/451, reset.rs:46, stash.rs:72/130/188

## Proposed Solutions

### Option A: Carry mode from tree entries (recommended)
Change `flatten_tree` to return `(path, sha, mode)` triples. Use the mode when constructing IndexEntry.
- Effort: Medium (touches many callsites)
- Risk: Low

## Acceptance Criteria

- [ ] Executable files retain 755 mode through checkout/merge/reset/stash
- [ ] Mode is sourced from tree entries, not hardcoded
