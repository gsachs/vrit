# vrit — A Version Control System

*vrit (वृत्) — from the Sanskrit root meaning "to turn, to revolve." Related to vṛtti (change/activity) and etymologically connected to the Latin vertere, the root of "version."*

## Purpose

vrit is a learning-oriented reimplementation of Git's core in Rust. The goal is to deeply understand how a content-addressable filesystem and DAG-based version control system works by building one from scratch. Git-compatibility is a non-goal, though the architecture closely mirrors Git's to maximize transferable understanding.

**Target scale:** repositories up to ~1,000 files. Performance should be reasonable but need not compete with Git.

## Design Decisions & Tradeoffs

### Object Model

vrit uses the same four object primitives as Git:

- **Blob:** file contents, stored as-is (no metadata in the blob itself)
- **Tree:** ordered list of entries, each with mode, name, and SHA-1 reference to a blob or subtree
- **Commit:** points to a root tree, zero or more parent commits, author/committer identity, timestamp, and message
- **Tag:** annotated tags are full objects with name, tagger, timestamp, message, and a pointer to a commit. Lightweight tags are plain refs (no object created)

> **Tradeoff:** Supporting both tag types adds a code branch in tag creation, but annotated tags teach the object model more thoroughly. Worth the complexity.

### Content Addressing

- **Hash:** SHA-1 (40-character hex digest)
- **Rationale:** Matches Git exactly, allowing cross-verification with `git cat-file`. SHA-1's cryptographic weakness is irrelevant for a local learning tool. Revisit if the project ever targets real-world use.

### Storage

- **Loose objects:** each object stored as a zlib-compressed file at `.vrit/objects/<first-2-hex>/<remaining-38-hex>`
- **Format:** `"<type> <size>\0<content>"` — zlib-compressed before writing to disk
- **Debug support:** `vrit cat-file -p <sha>` decompresses and pretty-prints any object
- **Packfiles:** deferred to a later phase. Loose objects are sufficient for ~1K files

> **Tradeoff:** zlib compression adds a dependency (flate2 crate) but makes objects inspectable with standard tools and teaches the real Git format. A `--raw` flag on cat-file could dump uncompressed content for debugging.

### Index (Staging Area)

- **Format:** custom simple binary — not Git's index format
- **Contents:** sorted list of entries, each containing: file path (length-prefixed), blob SHA-1, file mode, and staging timestamp
- **Location:** `.vrit/index`

> **Tradeoff:** a custom format is easier to implement and debug than Git's index v2 (which has stat caching, extensions, and cache tree data). The cost is no interop with `git status` on the same repo. Acceptable for a learning project.

### References

- **Branch refs:** plain text files at `.vrit/refs/heads/<branch-name>` containing a 40-char SHA-1
- **Tag refs:** plain text files at `.vrit/refs/tags/<tag-name>` (lightweight) or pointing to a tag object SHA (annotated)
- **HEAD:** `.vrit/HEAD` contains either `ref: refs/heads/<branch>` (on a branch) or a raw SHA-1 (detached HEAD)
- **Stash:** stored at `.vrit/refs/stash` as a linked list of hidden commits. The ref points to the most recent stash commit; each stash commit's parent is the previous stash entry. This avoids needing a reflog. Each stash captures staged + unstaged modifications (not untracked files)
- **Merge state:** during a conflicted merge, `.vrit/MERGE_HEAD` holds the other branch's tip SHA and `.vrit/MERGE_MSG` holds the auto-generated merge message. `vrit commit` checks for MERGE_HEAD to create a two-parent merge commit. Cleaned up by `vrit merge --abort`

### Ref Updates & Crash Safety

All ref updates (HEAD, branch pointers) use **atomic rename**: write the new value to a temporary file, then `rename()` it over the target. This guarantees that a ref file is never half-written. Orphaned objects from interrupted commits are harmless and can be garbage-collected later.

### Configuration

- **File:** `.vrit/config` — simple `key = value` format (one pair per line)
- **Required keys:** `user.name`, `user.email` (used in commit authorship). `vrit commit` errors with a clear message if either is missing
- **Bootstrapping:** `vrit init` prints instructions to edit `.vrit/config`. No `vrit config` command — manual editing only
- **Default branch:** `main`
- **No global config file.** Per-repo only
- **Re-init:** `vrit init` in an existing repo prints "Reinitialized existing vrit repository" and leaves objects/refs untouched

### Ignore Rules

- **File:** `.vritignore` in the repository root
- **Syntax:** glob patterns, one per line. Lines starting with `#` are comments
- **Supported:** `*` (any chars), `?` (single char), `**` (recursive directory match), trailing `/` (directories only)
- **Not supported:** negation patterns (`!`), nested `.vritignore` files in subdirectories

> **Tradeoff:** simplified ignore rules cover 90% of use cases. Negation and nested ignores add disproportionate complexity. Revisit if actual usage demands it.

### Binary File Handling

- Auto-detect binary files by scanning the first 8KB for null bytes (same heuristic as Git)
- Binary files are tracked as blobs (full snapshots) but excluded from diff and merge operations
- `vrit status` and `vrit diff` mark binary files as "binary" rather than showing content

### Detached HEAD

- **Committing:** allowed. Writes commit SHA directly to HEAD instead of updating a branch ref
- **Warning:** `vrit status` and `vrit checkout <sha>` print a "detached HEAD" warning
- **Risk:** commits made in detached HEAD become unreachable after switching to a branch (no reflog). Acceptable for a learning tool — users are warned

### Symlinks

- **Not supported.** Symlinks in the working directory are silently skipped during `vrit add` and `vrit status`
- **Known limitation.** Document in help output. Revisit if needed.

## Commands

### Core (Phase 1-2)

| Command | Description |
|---|---|
| `vrit init` | Create a new `.vrit` repository in the current directory |
| `vrit add <paths...>` | Stage files for the next commit. Accepts directories (recursive). Auto-detects deleted files and removes from index |
| `vrit rm <path>` | Remove a file from the index (and optionally from working tree). Stages the deletion |
| `vrit commit -m "<msg>"` | Create a commit from staged changes. `-m` flag is required (no editor integration) |
| `vrit status` | Show working tree status: staged, modified, untracked files |
| `vrit log` | Show commit history from HEAD (full DAG traversal with topological sort, follows all parents) |
| `vrit cat-file -p <sha>` | Pretty-print any object (blob, tree, commit, tag) |
| `vrit hash-object <file>` | Compute and optionally store a blob for a file |
| `vrit diff` | Show unstaged changes (working dir vs index). `vrit diff --staged` for index vs HEAD |

### Branching & Merging (Phase 3)

| Command | Description |
|---|---|
| `vrit branch [name]` | List branches or create a new branch |
| `vrit branch -d <name>` | Delete a branch. Refuses to delete the current branch |
| `vrit checkout <branch>` | Switch branches. Refuses if dirty tracked files would be overwritten. `vrit checkout -- <file>` to restore a file from HEAD |
| `vrit checkout <sha>` | Enter detached HEAD state (writes raw SHA to HEAD, prints warning) |
| `vrit merge <branch>` | Merge the given branch into the current branch. Fast-forwards when possible, otherwise three-way merge |
| `vrit merge --abort` | Abort a conflicted merge: clear MERGE_HEAD/MERGE_MSG, reset index and working tree to HEAD |

### Tags (Phase 4)

| Command | Description |
|---|---|
| `vrit tag <name> [commit]` | Create a lightweight tag |
| `vrit tag -a <name> -m "<msg>"` | Create an annotated tag |
| `vrit tag` | List all tags |
| `vrit tag -d <name>` | Delete a tag |

### Reset & Stash (Phase 5)

| Command | Description |
|---|---|
| `vrit reset [commit]` | Move HEAD to `<commit>` and unstage all changes (mixed mode only, no `--hard` or `--soft`) |
| `vrit stash` | Save staged + unstaged changes to the stash stack (errors if clean). Resets working tree to HEAD |
| `vrit stash pop` | Apply the most recent stash and remove from stack. On conflict, keeps the stash entry |
| `vrit stash list` | List stash entries (walks parent chain from refs/stash) |

### Plumbing (available from Phase 1)

| Command | Description |
|---|---|
| `vrit cat-file -t <sha>` | Print object type |
| `vrit cat-file -s <sha>` | Print object size |
| `vrit ls-tree <sha>` | List tree contents |
| `vrit write-tree` | Write the current index as a tree object |

## Diff Engine

vrit implements the **Myers diff algorithm** from scratch rather than using a library.

- **Rationale:** The diff algorithm is one of the most educational parts of a VCS. Implementing Myers teaches dynamic programming, edit graphs, and the "shortest edit script" concept
- **Output format:** unified diff (with `---`/`+++` headers, `@@` hunk markers, `+`/`-` line prefixes)
- **Colored output** by default when stdout is a TTY; ANSI green for additions, red for deletions

## Merge Strategy

**Fast-forward merge (default):** when the current branch is a direct ancestor of the merge target, just move the branch pointer. No merge commit created. This is the default behavior, matching Git.

**Three-way merge** (when fast-forward is not possible):

1. Find the **merge base** — the common ancestor of the two branch tips (using BFS on parent links)
2. Diff the base against each branch tip
3. For each file:
   - Changed in only one side → take that side's version
   - Changed in both sides with identical result → take either (they agree)
   - Changed in both sides with different results → **conflict**
   - Added on both sides with different content → **conflict**
   - Deleted on one side, modified on other → **conflict**
4. Conflicts are marked with `<<<<<<<`, `=======`, `>>>>>>>` markers in the file (with branch names)
5. On clean merge: auto-commit with generated message (e.g., "Merge branch 'feature' into main"). This is the one exception to the `-m` requirement
6. On conflict: write `.vrit/MERGE_HEAD` and `.vrit/MERGE_MSG`. Conflicted files are left unstaged. User resolves manually, then `vrit add` + `vrit commit` (which reads MERGE_HEAD to create a two-parent commit)
7. `vrit merge --abort` clears state files and resets index/working tree to pre-merge HEAD

**Dirty working tree:** vrit refuses to merge if tracked files have uncommitted changes. Clear error message: "Please commit or stash your changes before merging."

**Merge with self:** `vrit merge <current-branch>` is a no-op with a message "Already up to date."

**Merge in detached HEAD:** allowed — updates HEAD directly.

> **Tradeoff:** this handles the common case but not criss-cross merges (where the merge base is itself a merge). Git's recursive strategy handles this by merging the merge bases first. Deferred — criss-cross merges are rare in small repos.

## Terminal Output

- **Colored output** when stdout is a TTY (using ANSI escape codes)
- `vrit status`: green for staged, red for modified/untracked
- `vrit diff`: green for `+` lines, red for `-` lines, cyan for `@@` hunk headers
- `vrit log`: yellow for commit hash, bold for author
- **No `--no-color` flag initially.** Detect TTY only. Add flag later if needed.

## CLI Framework

- **clap** crate with derive macros for subcommand definitions
- Auto-generated `--help` for every subcommand

## Repository Layout

```
.vrit/
├── HEAD              # ref: refs/heads/main (or detached SHA)
├── MERGE_HEAD        # (temporary) other branch's tip SHA during conflicted merge
├── MERGE_MSG         # (temporary) auto-generated merge commit message
├── config            # user.name, user.email
├── index             # binary staging area
├── objects/
│   ├── <xx>/         # first 2 hex chars of SHA
│   │   └── <rest>    # remaining 38 hex chars, zlib-compressed
│   ├── info/
│   └── pack/         # empty initially, reserved for future packfile support
└── refs/
    ├── heads/        # branch refs
    ├── tags/         # tag refs
    └── stash         # stash ref (created on first stash, parent-chain for stack)
```

## Testing Strategy

- **Unit tests:** Rust `#[test]` modules colocated with source. Cover object serialization/deserialization, SHA computation, tree construction, diff algorithm, merge logic, index read/write
- **Integration tests:** in `tests/` directory. Spawn `vrit` as a subprocess, perform operations on temp directories, assert on repo state (object existence, ref values, working tree contents)
- **Cross-verification:** selected integration tests also run the equivalent `git` commands and compare object SHAs where formats align (blob and tree objects should produce identical SHAs since the format matches Git's)

## Implementation Phases

### Phase 1: Skeleton + Object Store
`vrit init`, `vrit hash-object`, `vrit cat-file`. Create the `.vrit` directory structure, implement blob/tree/commit object serialization with zlib compression, SHA-1 computation.

**Validation:** `vrit init` creates a valid `.vrit` directory. `vrit hash-object <file>` produces the same SHA as `git hash-object <file>`. `vrit cat-file -p <sha>` displays the object.

### Phase 2: Index, Add, Commit, Status, Log, Diff
Implement the staging area, `vrit add` (with directory support and deletion detection), `vrit rm`, `vrit commit`, `vrit status`, `vrit log` (full DAG traversal), and `vrit diff` (with Myers algorithm). Linear history on a single branch. `.vritignore` support. Design commit to support multi-parent (for Phase 3) even though Phase 2 only uses single-parent.

**Validation:** can create a repo, add files, commit, modify, see status/diff, commit again, view log. Deleted files tracked correctly. Ignored files excluded.

### Phase 3: Branching & Merging
`vrit branch`, `vrit checkout`, `vrit merge` (including `--abort`). Implement branch creation/deletion, switching (refuse on dirty tracked files), detached HEAD, fast-forward merge, three-way merge with conflict markers and MERGE_HEAD/MERGE_MSG state tracking.

**Validation:** create a branch, make diverging commits, merge cleanly (fast-forward and three-way). Create conflicts, resolve, abort. Checkout refuses on dirty tree.

### Phase 4: Tags
Lightweight and annotated tags. `vrit tag` commands.

**Validation:** create both tag types, verify annotated tags create tag objects, list and delete tags.

### Phase 5: Reset & Stash
`vrit reset` (mixed mode — default to HEAD for unstaging, or move branch pointer for other commits; warn about unreachable commits). `vrit stash` (staged + unstaged, error on clean tree) / `vrit stash pop` (keep stash on conflict) / `vrit stash list` (walk parent chain).

**Validation:** reset moves HEAD and unstages. Stash saves and restores working directory changes. Multiple stashes stack correctly. Pop preserves stash on conflict.

### Phase 6 (stretch): Packfiles
Implement pack file creation (delta compression) and reading. Add `vrit gc` to pack loose objects.

**Validation:** `vrit gc` reduces object count. All commands still work on packed objects.

## Key Rust Crates

| Crate | Purpose |
|---|---|
| `clap` (derive) | CLI argument parsing and subcommand routing |
| `sha1` | SHA-1 hash computation |
| `flate2` | zlib compression/decompression for object storage |
| `colored` or `termcolor` | ANSI terminal colors (with TTY detection) |

> **No diff/merge library.** Myers diff is implemented from scratch as a learning goal.

## Non-Goals

- **No remote operations** (clone, fetch, push, pull). vrit is local-only.
- **No interactive rebase.** Deferred indefinitely — depends on editor integration.
- **No submodules.** Out of scope.
- **No hooks.** Lifecycle scripts add complexity without teaching VCS fundamentals.
- **No `git-compatible` index/pack formats.** Custom formats where simpler.
- **No Windows support.** Unix-only (macOS/Linux). File modes, symlink detection, and atomic rename assume POSIX.

## Reference Material

The original Git architecture overview from *The Architecture of Open Source Applications, Volume 2* (Susan Potter) was used as the primary reference for understanding Git's design. Key concepts adopted:

- DAG-based content storage (snapshot model, not deltas)
- Four object types (blob, tree, commit, tag) with SHA-1 identity
- Plumbing/porcelain separation in command design
- Three-area model (working directory, index, repository)
- Immutable objects with mutable refs
