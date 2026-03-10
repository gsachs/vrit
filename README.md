# vrit

*वृत् — from the Sanskrit root meaning "to turn, to revolve." Related to vṛtti (change/activity) and etymologically connected to the Latin vertere, the root of "version."*

A learning-oriented reimplementation of Git's core in Rust (~3,800 lines, 25 tests). Built to deeply understand how a content-addressable filesystem and DAG-based version control system works by building one from scratch.

Git-compatibility is a non-goal, though the object format (blob, tree, commit, tag) matches Git's — `vrit hash-object` produces identical SHAs to `git hash-object`.

## Building

```sh
cargo build
```

## Quick Start

```sh
vrit init
echo 'user.name = Your Name' >> .vrit/config
echo 'user.email = you@example.com' >> .vrit/config

echo "hello" > file.txt
vrit add file.txt
vrit commit -m "Initial commit"
```

## Commands

### Core

| Command | Description |
|---------|-------------|
| `vrit init` | Create a new `.vrit` repository |
| `vrit add <paths...>` | Stage files (supports directories, detects deletions) |
| `vrit rm <path>` | Remove a file from the index and working tree |
| `vrit commit -m "<msg>"` | Create a commit from staged changes |
| `vrit status` | Show staged, modified, and untracked files |
| `vrit log` | Show commit history (full DAG traversal) |
| `vrit diff` | Show unstaged changes (Myers diff) |
| `vrit diff --staged` | Show staged changes (index vs HEAD) |

### Branching & Merging

| Command | Description |
|---------|-------------|
| `vrit branch [name]` | List branches or create a new one |
| `vrit branch -d <name>` | Delete a branch |
| `vrit checkout <branch>` | Switch branches |
| `vrit checkout <sha>` | Enter detached HEAD state |
| `vrit checkout -- <file>` | Restore a file from HEAD |
| `vrit merge <branch>` | Merge (fast-forward or three-way) |
| `vrit merge --abort` | Abort a conflicted merge |

### Tags

| Command | Description |
|---------|-------------|
| `vrit tag` | List all tags |
| `vrit tag <name>` | Create a lightweight tag |
| `vrit tag -a <name> -m "<msg>"` | Create an annotated tag |
| `vrit tag -d <name>` | Delete a tag |

### Reset & Stash

| Command | Description |
|---------|-------------|
| `vrit reset` | Unstage all changes |
| `vrit reset <commit>` | Move HEAD to a commit, reset index |
| `vrit stash` | Save dirty working tree to stash stack |
| `vrit stash pop` | Apply and remove the top stash entry |
| `vrit stash list` | List stash entries |

### Plumbing

| Command | Description |
|---------|-------------|
| `vrit hash-object [-w] <file>` | Compute blob SHA (optionally write to store) |
| `vrit cat-file -p/-t/-s <sha>` | Display object content, type, or size |
| `vrit ls-tree <sha>` | List tree entries |
| `vrit write-tree` | Write current index as a tree object |

## Architecture

```
Working Tree ←→ Index (.vrit/index) ←→ Object Store (.vrit/objects/)
                                              ↑
                                         Refs (.vrit/refs/)
```

All objects are immutable once written. The only mutable state is refs (branch/tag pointers) and the index, both protected by atomic rename. This means zero concurrency primitives — no mutexes, locks, or async — while remaining safe for concurrent reads.

## Key Implementation Details

- **Object store**: zlib-compressed loose objects at `.vrit/objects/<2>/<38>`, self-describing format (`type size\0body`)
- **Diff engine**: Myers diff algorithm (edit graph shortest-path) implemented from scratch
- **Merge**: three-way merge using LCA (BFS) as common ancestor, with conflict markers
- **Index**: binary format with version byte, big-endian fields, sorted entries, allocation bomb guard
- **Refs**: atomic writes via temp file + rename
- **Ignore**: `.vritignore` with glob patterns (`*`, `?`, `**`, trailing `/`)
- **Binary detection**: null-byte scan of first 8KB
- **Security**: path traversal defense (parent-dir canonicalization), decompression bomb limits, tree entry name validation, fail-closed error handling

## Project Structure

```
src/
├── main.rs          # Entry point
├── cli.rs           # Argument parsing and command dispatch
├── object.rs        # Content-addressable object model (blob, tree, commit, tag)
├── index.rs         # Binary staging area (sorted entries, atomic save)
├── diff.rs          # Myers diff algorithm
├── repo.rs          # Shared plumbing (HEAD resolution, tree traversal, ref ops)
├── config.rs        # INI-style config parser
├── ignore.rs        # .vritignore glob matching
└── commands/        # Porcelain commands (one file per command)
```

## Non-Goals

- No remote operations (clone, fetch, push, pull)
- No packfiles (loose objects only)
- No interactive rebase
- No submodules or hooks
- No Windows support (POSIX-only)

## License

MIT License

Copyright (c) 2026 Sachin Siddaveerappa

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
