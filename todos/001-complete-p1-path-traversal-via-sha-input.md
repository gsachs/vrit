---
status: pending
priority: p1
issue_id: "001"
tags: [code-review, security]
dependencies: []
---

# Path Traversal via SHA Input to Object Store

## Problem Statement

`Object::read_from_store` and `write_to_store` take user-provided SHA strings and use them directly to construct filesystem paths via `.join()`. The only validation is a minimum length check (`sha.len() < 4`). There is no check that the SHA is valid hex.

A crafted SHA like `aa/../../../etc/passwd` could read arbitrary files. Since SHA values also come from parsed tree/commit objects, a malicious repository could trigger reads outside the object store.

**Files:** `src/object.rs:115-130`

## Findings

- Security-sentinel: HIGH severity - path traversal via `.join()` on unvalidated SHA
- No hex validation anywhere in the codebase
- `hex_to_bytes` silently converts invalid hex to `0x00` (LOW-1 related)

## Proposed Solutions

### Option A: Validate SHA at read/write boundaries
Add `fn is_valid_sha(s: &str) -> bool { s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) }` and check in `read_from_store`/`write_to_store`.
- Pros: Simple, centralized
- Cons: Doesn't catch malicious SHAs parsed from objects
- Effort: Small
- Risk: Low

### Option B: Validate SHA at parse time AND at read/write (recommended)
Also validate SHAs when parsing tree entries (`parse_tree`), commit parents, and tag objects.
- Pros: Defense in depth against malicious repos
- Cons: Slightly more code
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] `read_from_store` rejects non-hex SHA strings
- [ ] `write_to_store` rejects non-hex SHA strings
- [ ] `parse_tree` validates SHA bytes produce valid hex
- [ ] `parse_commit` validates parent/tree SHAs
- [ ] `hex_to_bytes` returns `Result` instead of silently producing zeros
