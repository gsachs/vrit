---
title: "feat: Build vrit version control system"
type: feat
status: active
date: 2026-03-10
---

# Build vrit — A Git-like VCS in Rust

## Overview

Implement vrit, a learning-oriented Git reimplementation in Rust, following the architecture in `spec.md`. Six phases, each producing a launchable artifact validated before proceeding.

## Spec Gaps Resolved

The following gaps were identified by SpecFlow analysis and resolved here. These decisions should be folded back into `spec.md` before Phase 3.

**File deletion tracking:** `vrit add` auto-detects deleted files and removes them from the index. Additionally, `vrit rm <path>` provides explicit deletion (removes from index and optionally working tree). Added to Phase 2 scope.

**Merge state tracking:** On conflict, vrit creates `.vrit/MERGE_HEAD` (other branch's tip SHA) and `.vrit/MERGE_MSG` (auto-generated message). `vrit commit` checks for MERGE_HEAD to create a two-parent commit. `vrit merge --abort` restores pre-merge state by clearing state files and resetting index/working tree. Added to Phase 3.

**Fast-forward merges:** Default behavior when current branch is an ancestor of the merge target. Just moves the branch pointer — no merge commit created.

**Checkout with dirty working tree:** Refuse if tracked files have uncommitted changes that would be overwritten. Safe default, avoids silent data loss.

**Stash stack storage:** Chain stash commits via parent pointers. `refs/stash` points to the most recent; each stash commit's parent is the previous stash entry, forming a linked list. No reflog needed.

**Config bootstrapping:** `vrit init` prompts user to edit `.vrit/config` (printed instructions). `vrit commit` errors with a clear message if `user.name` or `user.email` are missing.

**Detached HEAD commits:** Allowed — writes commit SHA directly to HEAD. `vrit status` warns "detached HEAD" state. Commits become unreachable on checkout (no reflog). Acceptable for a learning tool.

**`vrit add .` and directories:** Supported. Directory arguments recursively add all non-ignored files.

**Log traversal:** Full DAG traversal with topological sort. Shows all parents, not just first-parent.

**Re-init:** Safe reinit like Git — prints "Reinitialized existing vrit repository", leaves objects/refs untouched.

## Phase 1: Skeleton + Object Store

**Commands:** `vrit init`, `vrit hash-object`, `vrit cat-file`

**Tasks:**
- [x] `Cargo.toml` with deps: `clap` (derive), `sha1`, `flate2`, `colored`
- [x] CLI skeleton with clap subcommands (all phases declared, unimplemented ones return "not yet implemented")
- [x] `.vrit` directory creation: HEAD, config, objects/, refs/heads/, refs/tags/, objects/info/, objects/pack/
- [x] Object model: `enum Object { Blob, Tree, Commit, Tag }` with serialize/deserialize
- [x] Blob creation: `"blob <size>\0<content>"` → SHA-1 → zlib compress → write to `.vrit/objects/<2>/<38>`
- [x] `vrit hash-object [-w] <file>` — compute SHA, optionally write to object store
- [x] `vrit cat-file -p/-t/-s <sha>` — read, decompress, parse, display
- [x] `vrit ls-tree <sha>` — list tree entries
- [x] `.gitignore` for `/target`
- [x] Unit tests: blob roundtrip, SHA matches `git hash-object`, tree serialization

**Validation:** `vrit hash-object <file>` produces identical SHA to `git hash-object <file>`. `vrit cat-file -p` displays the content correctly.

## Phase 2: Index, Add, Commit, Status, Log, Diff

**Commands:** `vrit add`, `vrit rm`, `vrit commit`, `vrit status`, `vrit log`, `vrit diff`, `vrit write-tree`

**Tasks:**
- [x] Index format: binary file with sorted entries (path, blob SHA, mode, timestamp)
- [x] Index read/write with proper serialization
- [x] `vrit add <paths...>` — hash files, write blobs, update index. Support directories (recursive). Detect deleted files and remove from index
- [x] `vrit rm <path>` — remove from index, optionally delete from working tree
- [x] `vrit write-tree` — convert index into tree objects (handle nested directories)
- [x] `vrit commit -m "<msg>"` — create tree from index, create commit object (single parent or root), update branch ref via atomic rename
- [x] `vrit status` — compare HEAD tree vs index (staged), index vs working tree (modified), scan for untracked. Respect `.vritignore`
- [x] `.vritignore` parsing: glob patterns, `#` comments, `**` recursive, trailing `/` for dirs
- [x] Binary file detection: scan first 8KB for null bytes
- [x] `vrit log` — walk commit parents from HEAD, display with colored output (yellow hash, bold author)
- [x] `vrit diff` / `vrit diff --staged` — Myers diff algorithm from scratch, unified output format, colored (green/red/cyan)
- [x] Design commit to support multi-parent (for Phase 3) even though Phase 2 only uses single-parent
- [x] Unit tests: index roundtrip, tree building, Myers diff correctness, ignore pattern matching
- [x] Integration tests: full add/commit/log cycle on temp repos

**Validation:** Create repo, add files, commit, modify, see status/diff, commit again, view log with multiple commits.

## Phase 3: Branching & Merging

**Commands:** `vrit branch`, `vrit checkout`, `vrit merge`

**Tasks:**
- [ ] `vrit branch [name]` — list branches or create new (write ref file)
- [ ] `vrit branch -d <name>` — delete branch ref. Refuse if it's the current branch
- [ ] `vrit checkout <branch>` — update HEAD (to `ref: refs/heads/<branch>`), update index and working tree to match target commit. Refuse if dirty tracked files would be overwritten
- [ ] `vrit checkout -- <file>` — restore file from HEAD, update index
- [ ] Detached HEAD: `vrit checkout <sha>` writes raw SHA to HEAD, prints warning
- [ ] Merge base finder: BFS on parent links to find lowest common ancestor
- [ ] Fast-forward merge: detect ancestor relationship, move branch pointer, done
- [ ] Three-way merge engine: diff base vs each tip, combine changes per-file
- [ ] Handle: file changed one side, both sides same, both sides different (conflict), file added both sides, file deleted one side + modified other
- [ ] Conflict markers: `<<<<<<<`/`=======`/`>>>>>>>` with branch names
- [ ] Merge state: write `.vrit/MERGE_HEAD` and `.vrit/MERGE_MSG` on conflict
- [ ] Auto-commit on clean merge (use generated message, bypass `-m` requirement)
- [ ] `vrit merge --abort` — delete state files, reset index and working tree to HEAD
- [ ] `vrit status` during merge — show "merging" state and conflicted files
- [ ] Unit tests: merge base finding, three-way merge cases (clean, conflict, ff), tree-level operations
- [ ] Integration tests: diverging branches, merge with/without conflicts, merge --abort

**Validation:** Create branch, make diverging commits, merge cleanly. Create conflicts, resolve with add+commit. Fast-forward merge works. Abort restores clean state.

## Phase 4: Tags

**Commands:** `vrit tag`

**Tasks:**
- [ ] `vrit tag <name> [commit]` — write lightweight ref to `.vrit/refs/tags/<name>`
- [ ] `vrit tag -a <name> -m "<msg>" [commit]` — create tag object, write ref pointing to tag object SHA
- [ ] `vrit tag` (no args) — list all tags alphabetically
- [ ] `vrit tag -d <name>` — delete tag ref (and orphan tag object if annotated — gc later)
- [ ] `vrit cat-file -p` support for tag objects
- [ ] Unit tests: tag object serialization, ref creation
- [ ] Integration test: create both types, list, delete, verify cat-file works on annotated

**Validation:** Create lightweight and annotated tags, list them, delete them. `cat-file -p` on annotated tag shows tagger/message.

## Phase 5: Reset & Stash

**Commands:** `vrit reset`, `vrit stash`, `vrit stash pop`, `vrit stash list`

**Tasks:**
- [ ] `vrit reset [commit]` — default to HEAD (unstage all). Move branch pointer if commit differs. Reset index to match target commit. Leave working tree alone. Warn if no reflog means commits become unreachable
- [ ] `vrit stash` — error if clean working tree. Capture staged + unstaged changes as a commit. Write to refs/stash (parent chain for stack). Reset index and working tree to HEAD
- [ ] `vrit stash list` — walk parent chain from refs/stash, display entries with index
- [ ] `vrit stash pop` — apply top stash's changes to working tree. On success, update refs/stash to parent (or delete if last). On conflict, keep stash entry (like Git)
- [ ] Unit tests: stash commit creation, parent chain walk, reset index manipulation
- [ ] Integration tests: stash/pop roundtrip, stash list with multiple entries, reset unstaging

**Validation:** Stash dirty changes, verify clean tree, pop them back. Multiple stashes stack correctly. Reset moves HEAD and unstages.

## Phase 6 (Stretch): Packfiles

Deferred — see spec.md. Only pursue after Phases 1-5 are solid.

## Key Risks

- **Myers diff complexity:** The algorithm is well-documented but tricky to implement correctly, especially hunk grouping for unified output. Budget extra time for Phase 2.
- **Three-way merge edge cases:** File-level merge is straightforward; tree-level operations (add/delete/rename across branches) have many cases. Phase 3 will likely need iteration.
- **Index format design:** Getting the binary format right early matters — changing it later means migration. Keep it simple, add a version byte at the start.

## Sources

- **Primary spec:** `spec.md` — all design decisions, tradeoffs, and non-goals
- **Reference:** AOSA Vol. 2, Git chapter (Susan Potter) — originally in spec.md before refinement
- **Conventions:** `/Users/dev0/.claude/CLAUDE.md` — phase 1 = launchable skeleton, validate before extending, one feature per phase
