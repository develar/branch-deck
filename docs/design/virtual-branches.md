# Virtual Branches Architecture

**Status:** Implemented  
**Updated:** 2026-10-13  
**Tags:** [architecture, git, workflow]

## Problem

Modern development workflows suffer from branch management chaos:
- Developers work on multiple features simultaneously but struggle to organize commits
- Traditional branches require constant switching and rebasing
- Commit history becomes tangled with interleaved changes
- No automatic way to group related commits into logical units

Branch Deck solves this by enabling linear development on main while automatically organizing commits into virtual branches based on commit message patterns.

## Solution Overview

Branch Deck watches your commits for grouping patterns (like `(feature-auth)` prefixes or `IJPL-123` issue keys) and automatically cherry-picks related commits into dedicated virtual branches. This allows developers to work linearly on main while maintaining clean, organized virtual branches for review and integration.

The system handles the entire lifecycle: creating virtual branches, archiving inactive ones, detecting when branches are integrated back into main, and cleaning up completed work.

## Design

### Architecture

```mermaid
flowchart TD
    subgraph "Linear Development on main"
        A[main branch]
        B["(auth) Add login"]
        C["IJPL-123: Fix API"]
        D["(auth) Add logout"]
        E["(db) Migration"]
    end
    
    subgraph "Automatic Organization"
        direction TB
        F[CommitGrouper<br/>Scans & groups by pattern]
        G[Cherry-pick Pipeline<br/>Creates/updates branches]
    end
    
    subgraph "Virtual Branches"
        H[user/virtual/auth<br/>Add login<br/>Add logout]
        I[user/virtual/IJPL-123<br/>Fix API]
        J[user/virtual/db<br/>Migration]
    end
    
    subgraph "Lifecycle"
        K[Archive Inactive<br/>archived/2025-01-23/auth]
        L[Detect Integration<br/>Merged/Rebased/Squashed]
        M[Cache Status<br/>Git notes]
    end
    
    A --> B & C & D & E
    B & C & D & E --> F
    F --> G
    G --> H & I & J
    H & I & J --> K
    K --> L
    L --> M
    
    style B fill:#f9f
    style D fill:#f9f
    style H fill:#f9f
    style C fill:#9ff
    style I fill:#9ff
    style E fill:#ff9
    style J fill:#ff9
```

### Key Decisions

1. **Decision**: Use commit message patterns for grouping
   **Rationale**: Zero friction - developers just commit normally with a prefix, no tool commands needed
   
2. **Decision**: Cherry-pick commits to virtual branches instead of rebasing
   **Rationale**: Preserves original commits on main, allows clean history on virtual branches

3. **Decision**: Archive by date (`archived/YYYY-MM-DD/branch-name`)
   **Rationale**: Natural organization, easy cleanup of old branches, clear timeline

### Grouping Rules

`CommitGrouper` reads the subject line only (`parse_subject` in `crates/sync-core/src/commit_grouper.rs`):

1. A leading autosquash marker (`fixup!`, `squash!`, `amend!`) is skipped, so a fixup stays in the branch of its target.
2. An explicit `(name)` prefix at the very start wins. The name is sanitized into a Git ref name (`mockito 5.19` →
   `mockito-5.19`); the virtual branch commit drops the prefix.
3. Otherwise an issue key at the start (after an optional `[category]` and `type(scope):`) names the branch, e.g.
   `ABC-123: Fix` → `ABC-123`. The commit message is kept as is.
4. Anything else is unassigned.

So `(lexer) ABC-123: Add lexer` belongs to `lexer`, and its pushed message is `ABC-123: Add lexer`. A prefix branch
whose commits all carry one issue key belongs to that issue (`issue_key` of `GroupedBranchInfo`): the branch table
nests it under the issue. `groupFor` in `src/lib/commit.ts` mirrors the rule for the editor's warning.

The explicit prefix is the only grouping state, so every regrouping is a message rewrite:
`move_commits_to_branch` (`crates/sync-core/src/move_commits.rs`) sets or removes the prefix of many commits in one
rewrite. The UI uses it to move commits, split an issue, rename a branch and ungroup one; an undo restores the
previous messages with `reword_commits`. See [Move Commits](../../spec/branch/move-commits.spec.md).

### Implementation Notes

- **Commit Grouping**: `CommitGrouper` scans commits and extracts patterns (see `crates/sync-core/src/commit_grouper.rs`)
- **Branch Materialization**: Cherry-picks grouped commits onto baseline (see `crates/git-ops/src/cherry_pick.rs`)
- **Integration Detection**: Parallel strategies for merge/rebase/squash detection (see `crates/branch-integration/`)
- **Async Boundaries**: Tauri commands use `spawn_blocking` for git operations (see `src-tauri/src/commands/`)

## Alternatives Considered

- **GitButler (Recommended for Most Users)**:
  - GitButler provides a more comprehensive solution for branch management with virtual branches
  - Branch Deck exists specifically as a workaround for GitButler's [performance](https://github.com/gitbutlerapp/gitbutler/issues/3235) [issues](https://github.com/gitbutlerapp/gitbutler/issues/9018) on large repositories
  - Why Branch Deck instead: Only when working with very large codebases where GitButler performance degrades
  - Consider GitButler first unless you have a large repository with performance constraints

## Performance Considerations

- **Constraint**: Must handle repositories with 10,000+ branches
- **Solution**: Batch operations, parallel detection, git notes caching
- **Measurement**: Sync completes in <2s for typical 100-branch repository

Key optimizations:
- Native Git CLI via direct command execution - no libgit2 or reimplementation overhead
- Object-only Git plumbing (cherry-mark, `merge-tree --write-tree`, one `fast-import` stream for the notes of a branch),
  never the working tree: see [Git Operations](git-operations.md) for the rules and the Git 2.54 minimum
- Parallel processing of independent operations across multiple CPU cores
- Efficient caching with git notes to avoid redundant computations
- Result: Can sync hundreds of commits into virtual branches in seconds, even on large monorepos

## References

- Implementation: `crates/sync-core/`, `src-tauri/src/commands/`
- Tests: `crates/integration-detection-tests/`
- Related docs: 
  - [integration-detection.md](integration-detection.md) - Detection strategies
  - [distributed-settings.md](distributed-settings.md) - Settings synchronization
  - [async-patterns.md](async-patterns.md) - Async/sync boundaries