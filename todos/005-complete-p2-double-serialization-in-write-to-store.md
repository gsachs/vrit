---
status: pending
priority: p2
issue_id: "005"
tags: [code-review, performance]
dependencies: []
---

# Double Serialization in Object::write_to_store

## Problem Statement

`write_to_store` calls `self.sha()` (which calls `serialize()`) then calls `serialize()` again for compression. For blobs, `serialize_body()` clones the entire content. A 100MB blob triggers ~300MB of heap allocation.

**File:** `src/object.rs:83-111`

## Proposed Solutions

### Option A: Serialize once, hash, then compress (recommended)
```rust
let data = self.serialize();
let sha = compute_sha(&data);
let compressed = zlib_compress(&data);
```
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] `write_to_store` calls `serialize()` exactly once
- [ ] SHA is computed from the already-serialized buffer
