---
title: "feat: Adversarial Test Suite to Break vrit"
type: feat
status: completed
date: 2026-03-11
origin: docs/brainstorms/2026-03-11-adversarial-tests-brainstorm.md
---

# Adversarial Test Suite to Break vrit

## Overview

Write 70-80 destructive integration tests across four files designed to find real bugs in vrit by corrupting internals, feeding edge-case inputs, violating state machine assumptions, and stressing boundaries. Both black-box (CLI) and gray-box (direct `.vrit/` manipulation) approaches.

## Problem Statement

The existing 57 tests cover happy paths. No test corrupts `.vrit/` internals, crafts malicious tree entries, or exercises operations in wrong states. A real path traversal vulnerability was already found and fixed (see `docs/solutions/security-issues/canonicalize-path-traversal-bypass.md`) — proving adversarial tests catch real bugs.

## Proposed Solution

Four test files matching the brainstorm categories, using the existing `TestRepo` harness. Gray-box tests use `repo.dir` to directly manipulate `.vrit/` filesystem before running commands.

(see brainstorm: docs/brainstorms/2026-03-11-adversarial-tests-brainstorm.md)

## Implementation Phases

### Phase 1: Helper Extensions + Corrupt Data Tests

**File:** `tests/corrupt_data_test.rs` (~22 tests)

**Helper additions needed in `tests/helpers.rs`:**
- `write_raw(path, bytes)` — write arbitrary bytes to a path relative to repo root (for corrupting .vrit/ internals)
- `read_raw(path) -> Vec<u8>` — read raw bytes from a path relative to repo root

**Tests:**

Index corruption (gray-box):
1. Truncated index (write 3 bytes to `.vrit/index`, run `status`)
2. Wrong version byte (set version to 255)
3. Inflated entry count (count=999999 with 1 entry of data)
4. Zero-length path in entry (path_len=0)
5. Non-UTF8 path bytes in entry (0xFF bytes)
6. Unsorted entries (two entries in wrong sort order, verify `status` doesn't silently misbehave)
7. Duplicate paths in index (same file listed twice)

Object corruption (gray-box):
8. Truncated zlib in object file (write half of a compressed blob, run `cat-file -p`)
9. Valid zlib wrapping garbage (no `blob N\0` header)
10. SHA mismatch (write valid object under wrong SHA path)
11. Zero-byte object file (empty file in `objects/xx/`)
12. Object directory missing (delete an `objects/xx/` dir, run `cat-file`)

HEAD/ref corruption (gray-box):
13. Empty HEAD file (truncate to 0 bytes)
14. Garbage bytes in HEAD
15. HEAD pointing to nonexistent branch (`ref: refs/heads/ghost`)
16. Branch ref with non-hex content
17. Branch ref with 39-char SHA (too short)
18. Branch ref with 41-char SHA (too long)

Config corruption (gray-box):
19. Malformed INI (unterminated section header)
20. Missing config file entirely (delete `.vrit/config`, try commit)

Merge/stash state corruption (gray-box):
21. Corrupt MERGE_HEAD (non-hex content), then run `commit`
22. Circular stash parent (stash commit pointing to itself), then run `stash list`

**Validation:** `cargo test corrupt_data` — all 22 tests pass. Every test asserts either a graceful error message (non-zero exit, no panic backtrace in output) or correct handling.

---

### Phase 2: Edge-Case Input Tests

**File:** `tests/edge_case_inputs_test.rs` (~22 tests)

File content edge cases (black-box):
1. Add + commit a 0-byte empty file
2. File with only newlines (`\n\n\n`)
3. Binary file with embedded null bytes — verify `diff` says "Binary files differ"
4. Very large file content (1MB string)

Filename edge cases (black-box):
5. Unicode filename (`café.txt`)
6. Filename with spaces (`my file.txt`)
7. Filename with shell-special chars (`file&name$(cmd).txt`)
8. Very long filename (255 chars)
9. Dotfile (`.hidden`)
10. File named like a flag (`--help` as filename) — verify not interpreted as CLI arg
11. File named `-m` — verify not interpreted as commit flag

Commit message edge cases (black-box):
12. Empty commit message (`commit -m ""`)
13. Multiline commit message with embedded newlines
14. Message with angle brackets and special chars (`<script>`, `\0`)

Branch/tag name edge cases (black-box):
15. Branch name with slash (`feature/foo`)
16. Branch name with double dots (`a..b`) — should be rejected
17. Branch name starting with dash (`-branch`) — should be rejected
18. Branch name `HEAD` — should be rejected or handled
19. Tag with same name as existing branch
20. Tag name with special characters

Plumbing command edge cases (black-box):
21. `cat-file -p` with non-existent SHA
22. `ls-tree` with a blob SHA (not a tree)

**Validation:** `cargo test edge_case` — all 22 tests pass.

---

### Phase 3: State Machine Violation Tests

**File:** `tests/state_violations_test.rs` (~22 tests)

Unborn branch operations (black-box):
1. `commit` on fresh repo with nothing staged
2. `log` on fresh repo (no commits)
3. `diff` on fresh repo
4. `stash` on fresh repo (no commits to stash against)
5. `stash pop` with no stashes

Init/setup violations (black-box):
6. Double `init` in same directory — idempotent or error?
7. Run any command outside a vrit repo (no `.vrit/` found)

Merge state violations (black-box + gray-box):
8. `merge main` while on main (merge with self)
9. Start merge, get conflict, try `merge` again (merge during merge)
10. Checkout during active merge conflict — refused
11. Stash during active merge conflict
12. Reset during active merge conflict
13. `merge --abort` when no merge is in progress
14. Manually create MERGE_HEAD (gray-box), then commit — verify behavior
15. Commit after `merge --abort` — should be normal commit (no merge parents)

Branch operations (black-box):
16. Delete current branch
17. Checkout a deleted branch
18. Create branch that already exists

File operations (black-box):
19. `add` a nonexistent file
20. `rm` a file not in the index
21. `checkout -- ghost.txt` (restore file not in any commit)

Detached HEAD (black-box):
22. Commit in detached HEAD state — verify HEAD updated correctly

**Validation:** `cargo test state_violations` — all 22 tests pass.

---

### Phase 4: Boundary & Overflow Tests

**File:** `tests/boundary_test.rs` (~14 tests)

Path traversal / security (mixed):
1. `add ../outside.txt` — rejected
2. `add /tmp/evil.txt` (absolute path) — rejected
3. Craft tree object with `../escape` entry name (gray-box), checkout that commit — file NOT written outside repo
4. Craft tree object with `/etc/passwd` entry name (gray-box), checkout — rejected

Symlinks (black-box):
5. Symlink to file outside repo — `add` skips it
6. Symlink to directory — `add` skips it

Scale tests (black-box):
7. 1000 files — add all, commit, status works
8. 50-level deep nested directory — add + commit works
9. 100 sequential commits — `log` traverses all correctly
10. 100 branches — create, list, verify all present
11. Large diff (change every line of a 2000-line file) — diff completes

Stash depth (black-box):
12. 20 stashes — `stash list` shows all, pop returns LIFO order

Long names (black-box):
13. 200-char branch name — create and checkout works or clear error
14. Merge with 10 conflicting files — all get markers, no partial state

**Validation:** `cargo test boundary` — all 14 tests pass.

---

## Acceptance Criteria

- [x] `tests/corrupt_data_test.rs` — 21 tests, all passing
- [x] `tests/edge_case_inputs_test.rs` — 22 tests, all passing
- [x] `tests/state_violations_test.rs` — 22 tests, all passing
- [x] `tests/boundary_test.rs` — 14 tests, all passing
- [x] Existing 57 tests still pass (`cargo test`)
- [x] No test relies on timing or sleep — deterministic assertions only
- [x] Gray-box tests use `repo.dir` + helper methods, not hardcoded paths
- [x] Every test asserts either graceful error (non-zero exit) or correct behavior — never accepts panics

## Known Risks

- **Scale tests may be slow.** The 1000-file and 100-commit tests could take seconds each. Acceptable for CI but consider `#[ignore]` if they exceed 10s.
- **Some tests may find real bugs.** Tests that fail because vrit panics or produces wrong output are discoveries, not test failures. Fix the application code or document as known limitations.
- **Gray-box tree object crafting requires understanding the binary format.** The tree serialization format (`mode name\0<20-byte-sha>`) must be constructed correctly in test helpers. Reference `Object::serialize()` in `src/object.rs`.

## Sources

- **Origin brainstorm:** [docs/brainstorms/2026-03-11-adversarial-tests-brainstorm.md](docs/brainstorms/2026-03-11-adversarial-tests-brainstorm.md) — all four categories, black-box + gray-box approach, comprehensive scale
- **Security learning:** [docs/solutions/security-issues/canonicalize-path-traversal-bypass.md](docs/solutions/security-issues/canonicalize-path-traversal-bypass.md) — path traversal via tree entries is the known vulnerability class
- **Test harness:** [tests/helpers.rs](tests/helpers.rs) — TestRepo API (new, run_ok, run_err, write_file, read_file, commit_all, etc.)
- **SpecFlow gaps found:** plumbing command coverage, detached HEAD state, stash-pop data loss, MERGE_HEAD validation, rm path traversal, unsorted index entries
