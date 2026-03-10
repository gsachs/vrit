---
status: pending
priority: p2
issue_id: "009"
tags: [code-review, security]
dependencies: []
---

# Missing Resource Limits: Decompression Bomb and Index Allocation

## Problem Statement

1. **Zlib decompression bomb:** `Object::read_from_store` decompresses with no size limit. A small crafted file could decompress to gigabytes. (`src/object.rs:123-127`)

2. **Index allocation:** The index deserializer reads a `u32` count and calls `Vec::with_capacity(count)`. A crafted index could request 4 billion entries. (`src/index.rs:119-121`)

## Proposed Solutions

### Option A: Add size limits (recommended)
- Use `decoder.take(MAX_OBJECT_SIZE)` (e.g., 100MB) before `read_to_end`
- Validate index count against `data.len() / min_entry_size`
- Effort: Small
- Risk: Low

## Acceptance Criteria

- [ ] Zlib decompression has a size cap
- [ ] Index deserialization validates count against available data
