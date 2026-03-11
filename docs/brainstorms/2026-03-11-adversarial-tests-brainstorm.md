# Brainstorm: Adversarial Test Suite for vrit

**Date:** 2026-03-11
**Status:** Draft

## What We're Building

A comprehensive adversarial test suite (50-80 tests) designed to break vrit by targeting four categories: corrupt data handling, edge-case inputs, state machine violations, and boundary/overflow conditions. Tests use both black-box (CLI-only) and gray-box (direct .vrit/ corruption) approaches, organized into four test files by category.

## Why This Approach

vrit's existing 57 tests cover happy paths and basic error cases. Adversarial tests specifically target the gaps: what happens when data is malformed, operations run out of order, inputs hit extremes, or attackers manipulate internals. Gray-box tests are essential because real-world corruption (disk errors, interrupted writes, manual tampering) bypasses the CLI entirely.

## Key Decisions

- **Both black-box and gray-box**: CLI tests for state violations, filesystem corruption tests for data resilience
- **Comprehensive scale (50-80 tests)**: Cover all 14 commands across all four categories
- **Four files by category**: `corrupt_data_test.rs`, `edge_case_inputs_test.rs`, `state_violations_test.rs`, `boundary_test.rs`
- **Goal is to find real bugs**: Tests should assert graceful failure (error message, non-zero exit), not panics or data loss

## Test Categories & Targets

### 1. Corrupt Data Handling (~15-20 tests) — `corrupt_data_test.rs`

Gray-box: directly corrupt `.vrit/` internals, then run commands.

| Target | Test Idea | Expected |
|--------|-----------|----------|
| Index: truncated | Write partial bytes to `.vrit/index` | Graceful error on any index-reading command |
| Index: wrong version | Set version byte to 255 | Error: invalid index version |
| Index: inflated count | Set entry count to 999999 with only 1 entry | Error, not OOM |
| Index: zero-length paths | Entry with path_len=0 | Error, not panic |
| Index: non-UTF8 paths | Write raw 0xFF bytes as path | Error about invalid path |
| Object: truncated zlib | Write half a compressed blob | Error on cat-file, not panic |
| Object: valid zlib, wrong header | Compress "garbage" without "blob N\0" header | Error on parse |
| Object: SHA mismatch | Write valid object under wrong SHA filename | Detect or silently use? |
| Object: zero-byte file | Empty file in objects/ | Error on read |
| HEAD: empty file | Truncate `.vrit/HEAD` to 0 bytes | Error, not panic |
| HEAD: garbage content | Write random bytes to HEAD | Error about invalid ref |
| HEAD: points to nonexistent branch | `ref: refs/heads/ghost` | Handle as unborn branch |
| Ref: contains non-hex | Write "ZZZZZZ..." to a branch ref | Error on resolve |
| Ref: wrong length SHA | Write 39-char or 41-char hex to ref | Error on resolve |
| MERGE_HEAD: corrupt during merge | Corrupt MERGE_HEAD mid-conflict | Error on commit, clean abort possible |
| Config: malformed INI | Write `[user\nname = ` (unterminated section) | Error on commit, not panic |
| Config: missing file entirely | Delete `.vrit/config` | Error requiring config, not crash |
| Stash ref: circular parents | Stash commit pointing to itself as parent | Log/pop don't infinite loop |

### 2. Edge-Case Inputs (~15-20 tests) — `edge_case_inputs_test.rs`

Black-box: unusual but valid inputs through CLI.

| Target | Test Idea | Expected |
|--------|-----------|----------|
| Empty file | Add + commit a 0-byte file | Works correctly |
| File with only newlines | Add file containing "\n\n\n" | Correct blob, clean diff |
| Binary file (null bytes) | Add a file with embedded \0 | Stored correctly, diff says "binary" |
| Unicode filename | File named `café.txt` or `日本語.rs` | Add/commit/status work |
| Filename with spaces | `"my file.txt"` | Proper quoting/handling |
| Filename with special chars | `file&name;$(cmd).txt` | No injection, proper storage |
| Very long filename | 255-char filename | Works or clear error |
| Dotfile | `.hidden` file | Added normally (not ignored unless in .vritignore) |
| File named `-flag` | File literally named `--help` or `-m` | Not interpreted as CLI flag |
| Empty commit message | `commit -m ""` | Error or empty message handled |
| Multiline commit message | Message with \n embedded | Stored and displayed correctly |
| Commit message with special chars | Message with `<`, `>`, `\0` | No format corruption |
| Branch name edge cases | `a/b`, `a..b`, `-branch`, `HEAD` | Rejected or handled per validation rules |
| Add nonexistent file | `vrit add ghost.txt` | Clear error |
| Add file outside repo | `vrit add /etc/passwd` | Rejected (path traversal) |
| Diff of identical files | Modify then revert, check diff | Empty diff |
| Status with no commits | Fresh repo, add file, status | Shows staged, no crash |
| Tag with same name as branch | Create tag "main" when branch "main" exists | Handled or clear error |
| Nested .vritignore patterns | `**/*.log` with deep nesting | All levels matched |
| Ignore pattern: negation-like | Pattern starting with `!` (not supported?) | Doesn't crash |

### 3. State Machine Violations (~15-20 tests) — `state_violations_test.rs`

Black-box + gray-box: run operations in wrong order or conflicting states.

| Target | Test Idea | Expected |
|--------|-----------|----------|
| Commit with empty index | `init` then `commit` with nothing staged | Error: nothing to commit |
| Commit on unborn branch, no files | Fresh repo, no add, commit | Error |
| Double init | `init` in already-initialized repo | Idempotent or clear error |
| Merge with self | `merge main` while on main | Error or no-op |
| Merge during active merge | Start merge, get conflict, try another merge | Error: merge in progress |
| Checkout during merge conflict | Get conflict, try checkout | Refused (dirty tree) |
| Stash during merge conflict | Get conflict, try stash | What happens? |
| Stash pop during merge conflict | Active conflict, stash pop | Refused or chaos? |
| Reset during merge conflict | Active conflict, reset | Should clean merge state |
| Commit after merge abort | Abort merge, then commit | Normal commit (no merge parents) |
| Branch delete current branch | `branch -d main` while on main | Error |
| Checkout deleted branch | Delete branch, then checkout it | Error: branch not found |
| Add after rm | `rm file`, then `add file` (file still on disk) | Re-added |
| Rm file not in index | `rm nonexistent.txt` | Clear error |
| Checkout file not in any commit | `checkout -- ghost.txt` | Error |
| Tag already-tagged name | `tag v1` twice | Error: tag exists |
| Log on unborn branch | Fresh repo, `log` | Empty or "no commits" message |
| Diff on unborn branch | Fresh repo, `diff` | Empty or "no commits" |
| Stash on unborn branch | Fresh repo, `stash` | Error |
| Stash pop with no stashes | `stash pop` with empty stash | Error |
| Rapid add/commit cycles | 20 quick add+commit cycles | All objects valid |

### 4. Boundary & Overflow (~10-15 tests) — `boundary_test.rs`

Mixed: stress-test limits and security boundaries.

| Target | Test Idea | Expected |
|--------|-----------|----------|
| Path traversal: `../` in add | `vrit add ../outside.txt` | Rejected |
| Path traversal: absolute path | `vrit add /tmp/evil.txt` | Rejected |
| Symlink following | Symlink to file outside repo, `vrit add link` | Skipped (not followed) |
| Symlink to directory | Symlink dir, `vrit add dir/` | Skipped or safe |
| Many files (1000+) | Generate 1000 files, add all, commit | Works within target scale |
| Deeply nested directory | 50-level deep `a/b/c/.../file.txt` | Works or clear error |
| Large file | 10MB file | Stored correctly, no OOM |
| Many branches | Create 100 branches | List/manage works |
| Many commits | 100 sequential commits | Log traverses correctly |
| Large diff | Change every line in a 5000-line file | Myers cap kicks in gracefully |
| Index with many entries | 1000+ index entries | Serialize/deserialize works |
| Long branch name | 200-char branch name | Works or clear error |
| Merge with many conflicts | 20 files all conflicting | All get markers, no partial state |
| Stash stack depth | 20 stashes | List/pop all work (LIFO) |

## Open Questions

None — all key decisions are resolved.

## Next Steps

Run `/ce:plan` to design the implementation, then write the four test files.
