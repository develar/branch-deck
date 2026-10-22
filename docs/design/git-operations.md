# Git Operations: Working-Tree Safety, Exact Rewrites and Batching

**Status:** Implemented  
**Updated:** 2026-10-19  
**Tags:** [git, safety, performance, rewrite, replay, fast-import, lease]

## Problem

Branch Deck rewrites the checked-out branch: reword, move to another branch, add an issue reference, amend
uncommitted changes into an older commit, squash commits, unapply (drop) a branch, and update it onto its upstream. The
user is usually in the middle of their own work. The working tree and the index may hold uncommitted and staged changes that Branch Deck has no right to touch.
`git rebase`, `git commit -a` and `git reset --mixed` all write to the working tree or the index. They stage what the
user did not select, discard what the user staged, and depend on the user's `rebase.*` configuration.

A rewrite must also be invisible apart from what it is meant to change. Renaming a commit must not change who
committed it or when. Every commit after it must be recreated with the same author, committer, dates and timezones.

Branch Deck exists for large repositories, so speed is a goal. A process spawn costs milliseconds. An operation that
spawns git once per commit, note or file does not scale to hundreds of commits.

## Solution Overview

Every rewrite is done on objects and refs only:

- Message-only rewrites read the commits as raw objects (`cat-file --batch`) and write the new ones in one
  `git fast-import` stream, copying the `author` and `committer` lines byte for byte.
- Content rewrites use `git replay`, which applies commits in memory and never touches the working tree. Trees are
  combined with `git merge-tree --write-tree`, and working-tree files are read into a temporary index.

The checked-out branch then moves in one `update-ref --stdin` transaction with an old-value check (CAS). The user's
index changes only where HEAD's tree changed, and staged changes on other paths survive.

## Design

### Architecture

```mermaid
flowchart TD
    APP[src-tauri: a Tauri command, run_blocking of a core] --> CORE
    BD[bd-cli: arguments, JSON output, error codes] --> CORE
    E2E[test-server: the commands of the E2E mock] --> CORE
    CORE[sync-core: one core per operation, the expected HEAD, the order of the steps; the sync] --> OPS
    APP --> HR
    BD --> HR
    CORE --> HR[history-rewrite: the rewrites that need no sync, the grouping, the stores, the repository lock] --> OPS
    HR --> BI
    CORE --> BI[branch-integration: the archive step and the detection of integrated branches]
    BI --> OPS[git-ops: one module per operation or shared reader and writer, one git process per phase]
    OPS --> EX[git-executor: GitCommandExecutor, the one place a git process starts]
    OPS --> OC[(object-cache: LMDB keyed by object ids)]
    CORE --> OC
    CORE --> NET[github, patronus: HTTP only]
```

Each layer calls only the layers below it. The adapters (the app, `bd`, the E2E server) parse a request, call a core
(`<operation>_core(params, options)`) and present its result; the app and `bd` run no git of their own and call git-ops
directly only for reads that need no lock or guard, such as the patches of the diff view. A core is where an operation
is made safe against the app and the parallel `bd` processes, once for every caller (the cores of the rewrites that
need no sync, GitHub or Patronus are the crate history-rewrite, the others sync-core): it takes the repository lock,
checks the HEAD the request was made for and orders the git steps. git-ops knows git and no policy: which arguments,
which process, how to read its output, each module one operation or one primitive that operations share; a rewrite there
runs the guards on the history it read and moves the branch with CAS. The `//!` headers of `crates/git-ops/src/lib.rs`,
`crates/history-rewrite/src/lib.rs` and `crates/sync-core/src/lib.rs` map their modules by area; "Adding a git operation" in `AGENTS.md` is the checklist
of a new one, and [Adding a Git Operation](../guidelines/adding-git-operation.md) says where it goes and which helpers
it reuses.

A rewrite of the checked out branch:

```mermaid
flowchart TD
    A[a core: repository lock, expected HEAD] --> G[rewrite_guard: no operation in progress, branch checked out, no merges in range]
    G --> R{What changes?}
    R -->|message only| W[commit_writer: cat-file --batch + fast-import, exact metadata]
    R -->|content| P[merge-tree --write-tree + git replay: amend, drop, squash]
    W --> T[update-ref --stdin: CAS on the branch, temp refs deleted, reflog message]
    P --> F[index preflight on a copy of the index]
    F --> T
    T --> I{HEAD tree changed?}
    I -->|no: reword, move, squash| D[done: index and working tree untouched]
    I -->|drop| X[read-tree -m -i old new]
    I -->|amend| U[update-index the amended files from the working tree]
```

Every path ends at one ref transaction. Only a content rewrite reaches the index step, and the working tree is never
written, except by Update Project and the finish of its resolve session (their own sections below).

### Invariants

1. **Never write to the working tree.** No `checkout`, `checkout-index`, `switch`, `reset`, `rebase`, `stash`,
   `commit`, `cherry-pick`, `merge`, `restore`, `clean`, `apply` without `--cached`, no `read-tree -u` or `--reset`, and
   no `git history`. Reading working-tree files into a temporary index (`GIT_INDEX_FILE`) is allowed.
   `crates/git-executor/src/worktree_safety_test.rs` scans the production sources and fails on such an argument list.

   **The one exception is Update Project** (see "Update project" below), which must bring the upstream's files into
   the working tree, **its undo**, which takes them out again, **the finish of a resolve session** (see "Resolve
   session"), which is the same move to the tip that the session replayed, **`bd pick`** (see "Pick"), which
   moves the branch to the commits of another worktree replayed onto it and, with `--follow`, that worktree onto the
   same tip, and its undo, **and the unapply and the reapply of a virtual branch** (see "Drop and unapply",
   "Reapply"), which take its commits and their files out of HEAD and back, and their undo. All use a single two-way
   `read-tree -m -u <old HEAD tree> <new HEAD tree>`, which writes only the paths whose content differs between the two
   trees; the undo swaps the trees. Three guards come first: a dry run of the same command on a copy of the index, a
   check for ignored files where the move adds a file, and the CAS ref move. Uncommitted and staged changes on the
   other paths survive byte for byte; any overlap refuses the whole move before anything moves. The exemption lives in
   `crates/git-ops/src/worktree_update.rs` only. Its functions are `pub(crate)`, and only `update_branch.rs` calls them:
   for the app's Update Project, `bd update` and `bd undo update:`, and, through its `pub(crate)`
   `move_branch_and_files_with` and `files_follow`, for the finish of a resolve session in `update_resolve.rs`, which
   moves HEAD only to the tip it reads from the session ref, and, through its `pub(crate)` `move_branch_and_files`, for
   `bd pick` in `pick.rs`, which moves HEAD only to the tip its own check replayed (`PickPlan`, private fields), a
   detached HEAD with `option no-deref`, and, through its `pub(crate)` `move_branch_and_files_carrying` and
   `apply_move_undo_with`, for the unapply in `unapply.rs`, which moves HEAD only to the drop its own check replayed
   (`UnapplyPlan`, private fields) and moves the virtual branch in the same transaction, and for the reapply in
   `reapply.rs`, which moves HEAD only to the pick its own check replayed (through `pick.rs`'s `apply_pick_carrying`)
   and deletes the unapplied branch in the same transaction. The safety scan allows `-u`
   there and asserts the number of such argument lists (two: the dry run and the update), so the exemption can't grow
   unnoticed, and it names the files that call those moves, each with its count (`MOVE_CALLERS`).

   **A slot of the lease pool is not the user's working tree** (see "Lease"). The pool owns it while it is free, and a
   get replaces its content: `checkout --force` of a slot that one `status` found clean, and on a return that the
   agent confirmed, `reset --hard HEAD` and `clean -fd`. These argument lists live in `crates/git-ops/src/lease_slot.rs`
   only, and the safety scan asserts each count (`checkout` 2, `reset` 1, `clean` 1).
2. **The user's index changes only where HEAD's tree changes** (see the index policy below). Staged changes on other
   paths survive. A conflict with staged work is detected before any ref moves. If the update fails afterwards, the
   error is returned, never ignored.
3. **Every ref move is one `update-ref --stdin` transaction** (`crates/git-ops/src/ref_transaction.rs`) with an
   old-value CAS and a `branch-deck: <operation>` reflog message. Related ref changes go into the same transaction:
   the unapply's move of the virtual branch together with the move of HEAD, and the deletion of temporary refs. **One named exception:** the
   review fetch writes `refs/branch-deck/review/<n>/{head,base}` through `git fetch` itself (its refspecs, forced, no
   CAS, no reflog message): those refs only keep the fetched commits of a pull request reachable, nothing reads their
   old value, and no other operation writes them. Their pruning is a CAS transaction (see "Review fetch").

   **A second named exception: the refs `fast-import` moves.** Notes are written in the `fast-import` stream of their
   batch, one notes commit per notes ref, `from <notes ref>^0` when the ref exists (`write_notes_to_refs` and
   `write_commits_with_notes` in `crates/git-ops/src/commit_writer.rs`): the mapping and stack notes of a sync, the
   mapping notes of a reword and of a move of a hunk, the detection notes
   (`crates/branch-integration/src/detection_notes.rs`). fast-import moves the notes ref itself when the stream ends,
   with the reflog message `fast-import`: it reads the ref again, refuses a new tip that does not contain that value
   ("not updating … new tip … does not contain …", exit code 1, so the operation fails), and moves the ref with that
   value as the old one. A writer that moved the notes ref meanwhile is therefore never overwritten. The check is a
   fast-forward, not the exact old value of a transaction: a notes ref moved back to an ancestor of the `from` commit
   meanwhile would be moved forward again.

   **The stack branches** (`{prefix}/stack/<root>`, see "Stack branch") follow the rule without an exception: a sync
   moves one with CAS on the tip it listed, only when every branch of its stack was built, and deletes the ones of
   stacks that no longer fork in one CAS transaction, never one with a remote branch or checked out in a worktree. A
   stack branch the sync did not rebuild keeps its tip, and nothing pushes it.
   The notes of a resolve session are not written this way: their commit is written on a temporary ref
   (`StoreNotes`), and the session's transaction moves `refs/notes/branch-deck/resolve` with CAS. fast-import also
   creates the temporary ref its commits are written on (`refs/branch-deck/tmp/<pid>-<time>-<n>`, a name of its own
   per run), which the operation's transaction deletes with CAS on the commit written last on it (`delete_temp_refs`
   of `WrittenCommits` and `WrittenStore`, `delete_temp_ref` of `WrittenEdit`).

   **A third named exception: the ref of a sync chain's commit writer** (`commit_chain.rs`). Each writer gets a
   temporary ref of its own (`unique_temp_ref`), because the branches of a sync build in parallel and the app's sync,
   `bd sync` and the trial builds of `bd check` run at the same time without a common lock. The ref reaches the disk
   only through the `checkpoint` of a resolve by words; then the end of the writer deletes it in its own stream
   (`reset <ref>` `from` the null id), else `reset` without `from` drops it from the memory of `fast-import`. Nothing
   reads its value, so there is no CAS. Up to 0.21 every chain wrote `refs/branch-deck/tmp/chain` and a checkpoint left
   it behind: the next checkpoint of another branch failed with "not updating … does not contain …".

   **The sweep of temporary refs.** A temporary ref left by a killed process keeps its objects alive. A sync lists
   `refs/branch-deck/tmp/` in the `for-each-ref` of its facts (`read_baseline_listing`) and deletes in one CAS
   transaction the ones older than an hour by the time in their name, and any name without a time (the old shared
   `chain`), so the delete costs a process only when there are some (`is_stale_temp_ref`, `delete_stale_temp_refs`).
   A resolve session sweeps the same way at its start (`sweep_stale_temp_refs`).

   **Ref moves that the files follow** (an update and its undo, `move_branch_and_files`) update `HEAD` (deref), not
   `refs/heads/<b>`, so git moves the branch HEAD points at, the one whose files the guarded update writes. That must
   be the branch the check used. git 2.56 refuses `symref-verify HEAD refs/heads/<b>` next to
   `update refs/heads/<b>` ("multiple updates for 'HEAD' (including one via its referent) are not allowed") and next
   to `update HEAD` ("cannot operate with deref mode"). So the transaction runs interactively
   (`RefTransaction::commit_while`): `start`, `update HEAD N H`, `prepare`, which takes the locks of HEAD and of the
   branch and checks the old value, then `symbolic-ref --quiet HEAD` under those locks, then `commit` when it
   is still `<b>`, else `abort` and `BranchSwitched`. While the locks are held, a `git switch` fails on `HEAD.lock`,
   so a switch at any point since the check refuses the move with nothing changed. A read of `symbolic-ref` before
   the transaction only refuses earlier, and the rollback runs the same way. The transactions also pass
   `--create-reflog`, so the undo finds the update in the reflog with `core.logAllRefUpdates=false` too.

   `commit_while` runs `update-ref --stdin` as an interactive git (`GitCommand::interactive`,
   `crates/git-executor/src/interactive_command.rs`): git with piped stdin, stdout and stderr, the config, the
   environment and the per-call `env` of every other call, a reply line awaited with a timeout (60 s; ref locks are
   taken without waiting). A dropped handle closes stdin and waits up to 5 s: `update-ref` then aborts and removes its
   lock files. Then it sends SIGTERM, whose handler in git removes them too, and SIGKILL only as the last resort, which
   would leave `HEAD.lock` and the branch lock behind (on Windows the kill of its job, as for every git). The same interactive git drives the chains of a sync
   (`commit_chain.rs`: one `merge-tree --write-tree --stdin -z` and one `fast-import` per branch, each step waiting for
   the reply of the one before) and the tree check of a trial sync (`crates/sync-core/src/check.rs`:
   `cat-file --batch-check` of the trees the object cache remembers).
4. **Guards before any rewrite of the checked-out branch** (`crates/git-ops/src/rewrite_guard.rs`):
   - no rebase, merge, cherry-pick or revert is in progress;
   - HEAD is not detached;
   - the range from the oldest rewritten commit to HEAD contains no merge; if it does, the rewrite is refused. A
     rewrite of commits it read (see "Rewrite history") takes this from the parents of its history, without a
     process (`History::check_rewrite`); a range it did not read, such as the undo of a squash, asks
     `rev-list --min-parents=2 -n1 HEAD ^<oldest>^@` (`check_rewrite_range`).
5. **Metadata**:
   - A message-only rewrite keeps the author and the committer (name, email, date and timezone) byte for byte, and
     the message bytes of every commit it does not reword.
   - A content rewrite (amend, drop, squash) keeps the author, and the committer becomes the current user and the
     current time, as with `git rebase`.
   - **One named exception:** a move of a hunk (`bd move-hunk`, see "Move a hunk") changes the trees of a range, but
     keeps author and committer byte for byte for every commit whose diff stays the same (`patch-id --stable`): the
     commits between the two whose change only shifted, and every commit above. Their virtual commits keep their ids,
     so the sync leaves their branches as they are. Only the two commits and those between whose diff of the file
     changed get the current user as committer.
   - Signatures are stripped because they would no longer be valid.
6. **Batch**: one process per operation phase, not one per item. Never parse translated text; rely on exit codes and
   on `--porcelain` and `-z` output. **One named exception:** a branch description (`branch.<name>.description`,
   `bd describe` and the `descriptions` of a `bd group` plan) costs one `git config` process per branch, because git
   has no batch writer for config. The current descriptions are still read with one `git config --get-regexp`, and
   only the ones that differ are written.
7. **Git 2.54 or newer**, checked when the executor discovers git, on every platform, so that every command fails
   early with an install or upgrade hint.

### Minimum Git version

| Feature | Since | Used for |
|---|---|---|
| `merge-tree --write-tree` | 2.38 | conflict detection and tree merges without a working tree |
| `merge-tree --merge-base` | 2.40 | cherry-pick semantics in sync, the amended tree |
| `git replay` | 2.44 | replaying commits in memory |
| `fast-import --signed-commits` | 2.52 | stripping signatures that a rewrite invalidates |
| `replay --ref-action=print` | 2.53 | reading the new tip without letting replay move refs |
| `replay --ref`, dropping commits that become empty | 2.54 | segment tips that are commit ids, not branches |

The minimum is therefore **2.54** (`MIN_VERSION` in `crates/git-executor/src/git_info.rs`). On macOS a Homebrew git
is preferred. On other platforms the upgrade hint names the git-core PPA (Ubuntu) and Git for Windows. Every failed
discovery is one summary line, then the hint (`UPGRADE_HINT`), and the app shows the line as the title: "Cannot find
Git: <reason>" for a git that is missing or fails to run (the macOS `/usr/bin/git` stub without the Command Line Tools
exits with an `xcrun: error`, its first line is the reason), "Git 2.39 is too old: Branch Deck requires Git 2.54 or
newer" for an old one. CI checks the runner's git in `.github/workflows/test.yml`.

### Executor

All git runs through `GitCommandExecutor` (`crates/git-executor/src/git_command_executor.rs`):

- Every call gets `-c core.quotePath=false -c color.ui=never -c log.showSignature=false
  -c i18n.logOutputEncoding=UTF-8` and the environment variables `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0`
  (one exception: the `status` of a slot of the lease pool may write the slot's index, [Lease](#lease)).
  The encoding keeps `log` and `show` messages in UTF-8 for a user who reads logs in another one: the commit list drops
  a record that is not UTF-8.
- Inherited `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and `GIT_NOTES_REF` are removed. A temporary index is always
  passed explicitly, in the per-call environment, which is applied after the removal.
- The executor knows no git beyond running it: a rev or ref fact (`resolve_tree_id`, `ref_exists`) is a function of
  `git_ops::revs`.
- stdin is null unless the call pipes it: git never inherits the stdin of the app or of `bd`. On Windows git queries
  its stdin handle at startup, which waits while a read of the same pipe is pending in our process (`bd check --plan -`
  reads a plan from stdin).
- `GIT_NOTES_DISPLAY_REF` is set to an empty value. It replaces an inherited one and turns off `notes.displayRef`. So
  `%N` shows only the notes of `core.notesRef`, the mapping notes, or those of an explicit `--notes=<ref>`. With
  `notes.displayRef=refs/notes/*`, `%N` would otherwise print the notes of every ref, and the mapping readers would take
  a wrong id. The mapping readers also parse only the first line of a note.
- stdout is a pipe, so git never starts a pager and callers do not need `--no-pager`.
- Paths after `--` are names: an argument list that puts any there starts with `--literal-pathspecs` (git's global
  option, the same as `GIT_LITERAL_PATHSPECS=1`), so `a*.txt` matches no `ab.txt` and `:name` is no pathspec magic.
  No call needs a glob or magic; `git_usage_scan_test.rs` checks the lists.
- Special exit codes still apply: `merge-tree` and `replay` exit with 1 on conflicts. `merge-tree --stdin` exits with
  0 even then: each merge of its output starts with its status, 1 clean and 0 conflicted
  (`merge_tree_output::parse_batch_merge`, the one reader of a merge of a `--stdin` run, from its whole output or
  from the running process of the sync's `CommitChain`).
- An operation can count its processes: `with_process_stats` gives an executor whose clones count every process they
  start, interactive ones included, by subcommand, with its wall time. The summary is logged at info level when the
  last clone is dropped (`git processes`, e.g. `merge-tree 2×40ms, log 5×31ms`). The sync and every `bd` command count
  theirs; `bd -v` prints the summary. Tests assert on `process_stats().by_command()`; the budget of every operation is
  pinned, process by process, in `crates/sync-core/src/process_budget_test.rs` (the cores) and
  `crates/bd-cli/src/process_budget_test.rs` (what a `bd` command adds), so a change that saves a process lowers a
  number there.

**One builder, one spawn core.** A git invocation is `git.cmd(&args, repository_path)` (`GitCommand`), configured with
`.env(&[(k, v)])`, `.stdin(&bytes)`, `.accept(&[1])` (non-zero exit codes that are still success) and
`.timeout(duration)`, and run by one finisher:

| Finisher | Returns | A non-zero exit code |
|---|---|---|
| `.text()` | stdout as text, trimmed | an error with git's stderr, unless accepted |
| `.bytes()` | stdout untouched (`-z`, `cat-file --batch`) | as `.text()` |
| `.lines()` | the non-empty lines of stdout, trimmed | as `.text()` |
| `.output()` | `GitOutput { stdout, stderr, code }`, when the code is the answer | as `.text()` |
| `.status()` | `GitOutput`, whatever the code | not an error, logged at debug level only |
| `.stream(handler)` | stdout to `handler` in chunks, on the calling thread | as `.text()`; `ControlFlow::Break` kills git and ends with `Ok`, without a look at the code |
| `.progress(on_line, stop)` | stdout as text; each stderr line to `on_line` (`fetch --progress`) | an error with the stderr messages, not the `\r` updates |
| `.interactive(delimiter)` | an `InteractiveCommand` for a reply protocol | `read_record` or `finish` fails with git's stderr once git exits |

`GitCommand::spawn` is the only place a process starts, and every finisher but `interactive` runs on
`GitCommand::run`: stdin is written from its own thread, so a large input cannot deadlock against a full stdout pipe;
stdout and stderr are read by threads that send their events to the calling thread (a bounded queue, so a slow
`stream` handler holds git back instead of filling the memory); a timeout or a `ProgressStop` (an idle timeout, a
cancel) makes the run poll and kill git, the error a `StoppedCommand`, and after git exits the rest of its pipes is
awaited for 500 ms only, because a helper of git (ssh, a credential helper) may hold them open. On Windows a kill ends
the job object that git was put into when it started (`crates/git-executor/src/process_tree.rs`): `git` on `PATH` is
mostly the launcher of Git for Windows (`Git\cmd\git.exe`), which starts the real git as its child, and a kill of the
launcher alone would leave that git running with its ssh or helper, its lock files and our inherited pipe handles. A
git that exits by itself ends nothing of its job. Each run is one tracing span (`git`, with the command, the exit code
and the time), and a guard kills a git that still runs and counts the process however the run ends: a handler error, a
stop or a `Break`. Text is decoded with `String::from_utf8`, lossily only when it is not UTF-8; stderr is read as bytes
and decoded lossily, the interactive one too. Three shorthands stay for the common one-liners: `execute_command` is
`.text()`, `execute_command_lines` is `.lines()` and `execute_command_with_status` is `.status()` as text (trimmed
stdout on success, trimmed stderr otherwise, and the code); every other call is the builder. Besides its construction (`new`, `with_info`, `get_info`) and the process
counts (`with_process_stats`, `process_stats`), that is all the executor offers: it knows no git command, and what a
command means (a commit or a tree by name, whether a ref exists) is a function of `git_ops::revs`.

### Operations

#### Rewrite history

Every rewrite of the checked out branch reads the first-parent history of HEAD it rewrites once, newest first, with one
`git log --first-parent --format=%m%H%x1f%T%x1f%P[%x1f%N]%x1e` (`crates/git-ops/src/history.rs`): each commit's id,
tree and parents, and its mapping note where the rewrite carries them. It gives the positions of the commits the plan
names, the first parent of each (`parent(pos)`) and its tree, and the merges of the guard (invariant 4). It is never
the whole history, which has 10⁵–10⁶ commits in a monorepo. There are two bounds:

- **Down to the oldest target** (`History::read_to`): amend, drop, unapply, squash and squash fixups, whose requests
  name no baseline. The log is streamed and stopped (the process killed) at the first commit after the last target
  it saw, the parent of the oldest one, which the plan needs too. A commit that is not on the first-parent line of HEAD
  makes the walk read everything; the rewrite is then refused with "Commit … is not on the current branch, sync and
  try again".
- **Ahead of the baseline** (`History::read_ahead_of`): reword, add issue reference and the move of a hunk, which must
  not rewrite a commit of the baseline. `log --first-parent --boundary HEAD ^<baseline>` (no exclusion on the local
  baseline branch itself, as the commit list does) also prints the boundary, the first parent of the oldest commit,
  and the second parent of every merge among them; the one that is the oldest commit's first parent is kept.

#### Reword, move, add issue reference

Only messages change, so every tree stays the same and the index and the working tree are already correct. The app's
edit of a message and `bd reword` are a regroup (below); this is how it writes, as add issue reference does.

1. Read the commits ahead of the baseline and their mapping notes (see "Rewrite history"). Every reworded commit must
   be among them; the oldest one starts the range.
2. Read the commits of the range as raw objects with one `cat-file --batch`.
3. Write the new commits in one `fast-import --date-format=raw --signed-commits=strip` stream:
   - each commit copies its `author` and `committer` lines and its `encoding`;
   - an unchanged message keeps its bytes; a new one ends with a newline, like `commit-tree -m`;
   - the tree is set with `M 040000 <tree> ""`;
   - commits are chained by marks on a temporary ref `refs/branch-deck/tmp/<pid>-<time>-<n>`;
   - `get-mark` prints the new ids.
4. Run one transaction that moves the branch with CAS and deletes the temporary ref.

`fast-import` objects become readable only when its stream ends, so it suits batches whose commits are all known up
front. Code: `crates/git-ops/src/reword_commits.rs`, `crates/git-ops/src/commit_writer.rs`.

#### Regroup, rename, restack

A move, a regroup plan of the agent CLI (`{branches: {path: [ids]}, unassign: [ids]}`), a branch rename, a restack,
`bd reword` and the app's edit of a message all change the messages of commits, so they are one reword. `regroup_commits_core` decides the messages before it
writes anything:

1. Take the repository lock (below), refuse while an operation is in progress or HEAD is detached, and read HEAD, the
   stack setting, the issue groups and the stack parents (one `cat-file --batch`) and the commits ahead of the
   baseline (one `log`): the `Snapshot` (`crates/history-rewrite/src/snapshot.rs`, which the check reads too; the reads
   that need less take its parts, `CommitsAhead` and `read_grouping_inputs`). An
   `expect_head` that isn't HEAD is `head_moved`.
2. Resolve the ids of the plan (abbreviations allowed); unknown ids, ids that aren't ahead of the baseline,
   duplicates and ids both in a branch and in `unassign` are refused.
3. Move every fixup of a moved target along (`plan_autosquash`); a plan that separates a fixup from its target is
   refused. So is a rewrite after which a fixup folds into another target or none, a `fixup!`, `squash!` or `amend!`
   marker is added or removed, a target that a fixup names by its id (`fixup! <hash>`) gets a new id, or a target
   whose `amend!` waits gets a new text (`fixup_targets.rs`): each would fold the fixup elsewhere, or replace the new
   message at the next fold. The autosquash plan of the new messages decides, on the old ids, so the ids are checked
   apart. The refusal is `RegroupError::FixupsStranded` with its `FixupRefusal`, a reason that names no action: `bd`
   adds "fold them first with bd fold", the app "squash the fixups into their target first".
   A commit without a prefix that goes to the branch of its own issue key keeps its message: it is grouped there
   already. A commit with a prefix keeps its message only when the plan path is exactly that prefix. With stacked
   branches on, a path `auth/login` writes the stack parent `login<TAB>auth` and gives a commit that needs a prefix the
   bare `(login)`; a commit outside the plan whose prefix declares another parent for a child the plan stacks or
   unstacks loses the levels above the child (the levels above it become stack parents).
4. Group every subject ahead of the baseline with the new prefixes, the issue groups and the stack parents after the
   plan in memory
   (`CommitGrouper::new_with`) and compare the stack forest with the current one: a problem the plan introduces (a
   cycle, conflicting parents, a self parent, a leaf collision) refuses the plan, named by its reason without a fix
   (the plan changed nothing yet); a problem that already existed is reported, and its fixes run.
5. Reword the changed commits in one `fast-import` (`reword_commits_batch_at`): the history must still start at the
   HEAD of step 1, and the ref transaction's CAS uses it too. A change of the issue groups or of the stack parents
   joins that transaction (below).

A move of the app (`move_commits_to_branch`) computes its plan from the snapshot of step 1, under the same lock
(`regroup_planned` and `plan_move` in `move_plan.rs`): a whole issue (every commit of its key without a prefix) goes into
`issue_groups`, also into a stacked target, a part of one and a commit without a key into `branches` (a path `P/X`
also writes the stack parent of `X`, with an empty list when the issue groups move every commit), and a prefixed commit
that its key puts into the target anyway into `unassign`. So combining issues rewrites nothing, and a mixed move is one
reword and one ref transaction. A dry run of it is the preview of the "Move to…" form.

`bd reword` (`crates/history-rewrite/src/reword.rs`) gives the regroup whole new messages by id (`Change::messages`,
a `--subject` keeps the body it reads from the snapshot, under the lock), cleaned up as `git commit` does, before the
plan's prefixes apply; a message equal to the old one after the cleanup changes nothing. A new `(prefix)` regroups the
commit as a plan would, with step 4's check, and the fixups of a target whose prefix changes follow it. The mapping
note of a commit whose text (not only its prefix) changed is not carried: its copy must be rebuilt with the message.

The app's edit of a message (the Tauri command `reword`, `reword_request_core` in `reword.rs`, which the E2E server
routes too) is the same reword: whole messages by full id, `RegroupOptions::default()` (the lock and its 10 s wait,
no expected HEAD), and the answer carries the new ids, `previousHead`, `newHead` and the undo token. Its refusals are
told for the app: an id that is no longer ahead of the baseline is "Commit <id> is no longer on the current branch,
sync and try again" (the list was read before a terminal reworded or dropped it).

A rename replaces the old level in every prefix path (children included), renames the issue groups of the old name,
names an issue branch in the issue groups, and renames the old name in the stack parents (as a child, and as the parent
of a child with commits on this HEAD), without a rewrite; a restack writes the stack parent and rewrites only the
commits whose prefix declares another parent, an unstack removes it and the parent levels. The undo token is
`H1..H2` for a rewrite:
`restore_branch_head` moves the branch back with CAS, which is valid because a message rewrite keeps HEAD's tree. The
app undoes a move, a rename, a restack and a reword with that token too (`undo_regroup`), never by rewording the
messages back.

**Issue groups.** The map of issue key to branch name lives in `refs/branch-deck/issue-groups`
([Issue Groups](../../spec/branch/issue-groups.spec.md)), outside `refs/heads`, so the default refspecs neither push nor fetch it.
- A write makes a commit with the one file `issue-groups` on the previous map commit (`StoreCommit::file` and
  `StoreUpdate` in `commit_writer.rs`). With a rewrite, it is one more commit of the reword's `fast-import` stream
  (`reword_commits_batch_at`), so a mixed plan costs one `git var` and one `fast-import`. Without a rewrite,
  `write_store_updates` writes it alone. `fast-import` objects are readable only after the stream ends, which suits
  this write.
- Its `update-ref` line is `update <ref> <new> <old>`, or `create` for the first write. It goes into the transaction
  that moves HEAD and deletes the temporary ref, so both refs move or neither does. A plan that changes only the map,
  and a reword whose messages are equal once cleaned up, add `verify refs/heads/<branch> <head>` instead of a HEAD
  move.
- On a failed transaction the temporary ref is deleted, and a read of HEAD and of the map ref names the one that moved:
  `head_moved` or `groups_moved`.
- The undo token is `groups:G1..G2` for a change of the map only and `H1..H2+groups:G1..G2` for both. The ids are
  full; a `G1` of zeros is a map that did not exist, which the undo deletes. One `for-each-ref` reads the map ref and
  the parent of its commit: `G2` must be the map now (else `groups_moved`), and its parent must be `G1` (else the
  token names no write of the map). `undo_regroup_core` moves both refs back in one transaction
  (`restore_branch_head_with`), with CAS on each; a ref that moves meanwhile is `head_moved` or `groups_moved` too.
- Only a regroup writes the map; the sync, `bd status`, `bd check` and `bd suggest` read it. The ref is shared by
  every branch and worktree (`refs/branch-deck/*` is not per worktree), so a write changes only the keys the plan
  names: a key that groups no commit on this HEAD may group the commits of another branch, and only `ungroup_issues`
  removes it. The lines that the parser can't use are written back as they are, unless the write sets or ungroups
  their key. git keeps no reflog outside `refs/heads`, so the parent chain of the map commits is its history.

**Stack parents.** The map of branch to parent lives in `refs/branch-deck/stack-parents`
([Stack Parents](../../spec/branch/stack-parents.spec.md)), a ref of its own: a store commit replaces its whole tree
(`deleteall`), so a file next to `issue-groups` would be lost by every write of a version that does not know it.
- It is written, read and undone as the issue groups, by the same code: each map is a `LineStore` (`store_lines.rs`:
  its ref, its file, its commit message, the shape and the validation of a line; `ISSUE_GROUPS`, `STACK_PARENTS`).
  One file `stack-parents` of sorted `child<TAB>parent` lines, read in the same `cat-file --batch`, one more store commit of the same `fast-import` run, each
  store on a temporary ref of its own (`{temp}-store-<n>`, so no store commit replaces another tip before the
  transaction), one more line of the same ref transaction with CAS (`write_store_updates` without a rewrite).
- A failed transaction names the ref that moved in the order HEAD, issue groups, stack parents: `head_moved`,
  `groups_moved`, `stacks_moved`. The undo token joins `H1..H2`, `groups:G1..G2` and `stacks:S1..S2`, the parts of the
  refs that moved, with `+`, in this order; `S2` must be a write on `S1`.
- The one write outside a regroup: after the detection of the archived branches, a sync drops the lines of the
  children whose newest archived branch is integrated (`drop_integrated_stack_parents`), with CAS on the commit it
  read and only when a line goes, with a message of its own. It holds no repository lock (the app's sync never does, see "Repository lock"); a
  write that loses the race is skipped and tried at the next sync. A newer archive that is not integrated, such as a
  branch of another worktree that this sync archived, keeps the line. So that such a drop does not break the undo of
  a regroup whose toast is still open, an undo whose `S2` is followed only by drops (one `log`) writes the map of `S1`
  without the dropped children on top of them (`undo_over_drops`, for any `LineStore` with its drop message), in the
  undo's transaction.

**Parked branches.** The branches the user keeps local live in `refs/branch-deck/parked`
([Parked Branches](../../spec/branch/parked-branches.spec.md)), the third `LineStore` (`PARKED`, `parked.rs`): one file
`parked` of sorted `branch<TAB>note` lines, the note one line, possibly empty. It decides no group: a sync builds a
parked branch as any other, and only `bd push`, `bd safe-push` and the app's push surfaces leave it out, and a branch
built on it, which carries its commits.
- It is read in the same `cat-file --batch` as the other two maps (`GroupingInputs.parked`), so the stack setting and
  the three maps still cost two processes, and a sync's budget is unchanged. The sync sends the whole map in
  `BranchesGrouped` (`parked`), the lines of branches without commits here too.
- Park and unpark change no commit: `park_core` takes the repository lock, reads the grouping of the commits ahead
  (each branch must group one), then writes the store with one `fast-import` and one `update-ref --stdin` with CAS on
  the commit it read (`write_store_updates`); a ref that moved is `parked_moved`. `unpark_core` reads only the store
  (one `cat-file --batch`) and drops a line also of a branch without commits here. Both take several branches (the
  app's drag of a selection, the undo of one) and write all their lines in one store commit, or none when one of them
  is refused; the processes are the same for one branch or many. Neither has an undo token: the other one is the undo.
- A rename carries the line: the parked `old` becomes the parked `new` with its note, in the transaction of the rename
  (a third store of `regroup_with`; a parked `new`, a merge, keeps its own note). The undo token gets the part
  `parked:P1..P2` after `stacks:`; a failed transaction names the ref that moved in the order HEAD, issue groups,
  stack parents, parked branches (`parked_moved`). A merge into another branch by a move keeps the line of the
  branch moved from: it may park the branch of another worktree.
- A sync drops the line of a parked branch without commits here whose newest archived branch is integrated, as the
  stack parents (`drop_integrated_parked`, its own drop message, best effort, no lock), and an undo of a rename passes
  over those drops. Every other idle line stays (`idle_parked` of `bd status`): an issue key that groups again is
  parked again.

**Repository lock.** History rewrites take an exclusive `flock` on `$GIT_COMMON_DIR/branch-deck.lock` (all worktrees
share it), waiting up to 10 s for another holder and then failing with `locked`. The in-process mutexes don't cover
the app and parallel `bd` processes. A dry run reads only and takes no lock. Every rewrite of the checked out branch
takes it in its core, before it reads anything: the regroup (also the app's reword) and the move of a hunk (their
options), and the app's squash and its undo, squash fixups (also `bd fold`), amend, unapply and add issue reference, each a
`*_core(…, &RewriteOptions)` (`crates/history-rewrite/src/repository_lock.rs`): `lock_timeout` (10 s by default),
`held_lock` for a caller that holds the lock already, so the core takes none, and `expect_head`, the HEAD the request
was made for, abbreviated ids allowed, else `HeadMoved` (`ensure_expected_head`, one check for every core). The app
shows a lock held for the whole wait as the error toast of the action, "Cannot squash the fixups" with "Another
Branch Deck operation holds … ; retry when it is done". `bd sync`, `bd push`, `bd safe-push` (until its push; with
`--check` only its sync),
`bd move-hunk`, `bd pick` (the moves of both worktrees under one hold) and `bd undo` take it, and so does Update Project (the app's and `bd update`, which also starts a
resolve session under it) after its fetch, and `bd update --continue` and `--abort`; `bd update --status` takes
none. A holder writes who it is next to the lock (`branch-deck.lock.holder`: the command line, the pid, the start of
the process and the time, through a temporary file and a rename) once it has the lock, and deletes it before it
releases it, so the error of a wait names the holder; the lock file itself can't carry it, Windows refuses to read a
locked file. A record whose process is gone (`process_scan::is_running`) names nobody. A core finds the file with one
`rev-parse --git-common-dir`; `bd`'s own lock (`lock_repository_in`) takes the
directory its context resolved, and a wait for it makes `bd` read the config again (`RepoContext::forget_config`). The app's sync
never takes it: a sync of a large repository would make every `bd` writer fail with `locked`. Within a window one sync
runs at a time (`syncBranches`): a request while one runs asks for one follow-up after it, the events of a superseded
run are dropped, and a focus during a sync checks the inputs once after it. A `bd` write during an app sync changes
the inputs (the fingerprint is taken when the sync starts), so the next focus syncs again and corrects what the sync
wrote from the HEAD it read. A fetch never runs under the lock: on a monorepo it can take minutes, and every other writer would fail with
`locked`.

**One call: rewrite, descriptions, sync.** `bd group` (and `rename`, `restack`, `unstack`) takes the lock itself and
passes it to the regroup (`RegroupOptions.held_lock`), so the regroup takes no lock of its own and the lock has no gap:

1. The current descriptions, when the plan has `descriptions`: those of the command's config (`RepoContext::config`,
   one `git config --get-regexp` read once per command and again after a wait for the lock).
2. The regroup above. Before it writes, `regroup_commits_accepted` lets bd refuse the result: a description without a
   title of one line, or of a branch that has no commits after the plan, is `invalid_plan`.
3. After the ref transaction: one `git config` per changed description (the exception of the batch rule).
4. With `--sync`: the sync of `bd sync` (`sync_locked`).

A failure in step 3 or 4 leaves the rewrite in place: the output keeps the undo token and has the error as `error`.
A failed description write stops the later ones and skips step 4. The undo moves HEAD and the issue groups back, not
the descriptions or the virtual branches; the output lists the texts the descriptions replaced.

Code: `crates/history-rewrite/src/regroup.rs`, `crates/history-rewrite/src/snapshot.rs`, `crates/history-rewrite/src/move_commits.rs`,
`crates/history-rewrite/src/issue_groups.rs`, `crates/history-rewrite/src/parked.rs`, `crates/history-rewrite/src/repository_lock.rs`.

#### Amend

The user selects files in the uncommitted changes. They go into commit X, which may be HEAD, an older commit or the
root commit.

1. Read the first-parent history down to X and its parent (see "Rewrite history"). Build `T_wt` in a temporary
   index: HEAD's tree plus only the selected files from the working tree.
2. Compute the new tree of X:
   - if X is HEAD, it is `T_wt`;
   - otherwise it is `merge-tree --write-tree --merge-base=HEAD^{tree} X^{tree} T_wt`, which applies only the delta
     between HEAD and the working tree. A later commit that also changed the file does not leak into X. A conflict
     refuses the amend.
3. Write X' with `fast-import`: X's author and message, the current user as committer.
4. Unless X is HEAD, run `git replay --ref-action=print --onto=X' --ref=refs/branch-deck/replay X..HEAD`. `--ref`
   only names the tip in the printed update; with `print`, replay writes no ref.
5. Check that the new tip's tree equals `T_wt` before any ref moves, so the amended files match the working tree
   afterwards and nothing else changed.
6. Move the branch, then stage the amended files from the working tree (see the index policy).

There is no `commit -a` and no `rebase --autosquash` fallback. A range with merges is refused (invariant 4).

Code: `crates/git-ops/src/amend_operations.rs`, with the shared replay and merge helpers in
`crates/git-ops/src/replay.rs`.

#### Drop and unapply

The dropped commits are ordered oldest to newest on the first-parent line, read down to the oldest of them (see
"Rewrite history"); a commit that is not on it refuses the drop. Each run of kept commits between two dropped ones,
and after the last one, is replayed onto the result of the previous run:

```
git replay --ref-action=print --onto=<base> --ref=refs/branch-deck/replay <d_i>..<parent of d_i+1>
```

The last run is `<d_k>..HEAD`. That is at most k processes for k dropped commits, whatever the length of the range.
A conflict makes `replay` exit with 1, without output, and nothing has changed. A drop is a rewrite plan
(`prepare_plan`) whose steps are all `Drop`.

Unapply ([Unapply](../../spec/branch/unapply.spec.md)) takes the commits of a virtual branch out of HEAD with their
files and keeps the branch in the unapplied namespace, which no sync archives or deletes. Under the repository lock,
after the mark of an interrupted move is finished (`finish_pending_update`):

1. The commits of the branch as they group now: the grouping inputs (one `cat-file --batch`) and the commits ahead
   (one `log`). The ids the app's request names must be them, else `BranchChanged`.
2. The build: one `for-each-ref` of the virtual branch (its tip, the worktree that has it checked out, which refuses),
   the start of its own commits (`own_commits_start`: its stack note, else the merge base with the baseline) and one
   `rev-list` of them, which must be the copies the mapping notes of the commits name, in order. A branch that a
   conflict or a missing sync left without a copy of a commit is refused (`NotBuilt`): the commit would leave HEAD
   without one.
3. The drop above, then the dry run of the files: `read-tree -m -u -n` of the old and the new HEAD tree on a copy of
   the index, and the check for an ignored file where a file comes back (`blocking_paths`). A local change on a path
   the drop changes, staged or not, refuses the unapply (`MoveBlocked`); work on other paths stays as it is.
4. Under `REF_RENAME_MUTEX`, one `for-each-ref` picks a free name, and the guarded move of "Update project" moves HEAD
   with the files (`move_branch_and_files_carrying`, `branch-deck: unapply`). Its transaction also creates the branch
   in the unapplied namespace, deletes the virtual branch (checked against its tip) and the temporary refs of the drop;
   the rollback of files that changed since the dry run moves both back.

A failed unapply therefore never leaves a renamed branch behind, and a crash between the move and the files leaves the
mark that the next move of the files finishes.

The app lists them in the Unapplied card; a push of one (`StackPushBranch::unapplied`, `bd push --unapplied <name>`)
goes to `origin` under its own name, leased as any push: the remote keeps a copy, and the remote branch of its virtual
branch, a pull request's head, stays as it was last pushed. `bd push` and `bd safe-push` refuse its name as
`unapplied`: it lands nothing. Its delete is the one of an archived branch (`delete_unapplied_branch_core`: one
`for-each-ref`, one CAS transaction, refused while a worktree has it). `bd status` lists them from the listing it read
(`unapplied`, no process).

The undo (`unapply:<previous head>..<new head>:<tip>:<virtual branch>:<unapplied branch>`, the toast's Undo and
`bd undo`) is the check of an undo of "Update project" with the trees swapped (`check_unapply_undo`: the last entry of
the branch's reflog must be `branch-deck: unapply` from the previous head, else `NotAnUpdate`; HEAD elsewhere is
`HeadMoved`), then the same guarded move back (`branch-deck: undo unapply`), whose transaction deletes the unapplied
branch (checked against the tip) and creates the virtual branch at the tip again. The originals come back with their
ids and their mapping notes, so the next sync reuses the copies.

Code: `crates/git-ops/src/rewrite_plan.rs`, `crates/git-ops/src/unapply.rs`, `crates/sync-core/src/unapply_branch.rs`,
`crates/branch-integration/src/archive.rs`, `crates/bd-update/src/unapply_command.rs`.

#### Squash

The user selects commits; they are squashed into the oldest one, which keeps its place and its author. The newer ones
leave the history, as fixups do with `rebase --autosquash`, and the commits in between stay. A squash is a rewrite
plan with one `Replace` step per group (`SquashGroup`: a target, the folded commits and a `SquashMessage`, either a new
text or the target's own message and encoding):

1. Read the first-parent history down to the oldest target (see "Rewrite history") and the positions of the commits;
   each must be on it, the folded ones newer than the target. The history has the trees of the commits and of their
   parents; one `cat-file --batch` reads the targets, for their authors and messages (a kept message is taken from
   there, not read twice).
2. Replay the kept commits before the target, if an earlier step changed the history.
3. Compute the squashed tree, starting from the target's tree, then applying each folded commit oldest first as a
   cherry-pick (merge base: its parent's tree). All merges of the plan, of every group, go to one interactive
   `merge-tree --write-tree --stdin -z` (`CommitChain::merge`), each line answered before the next, as the next
   merge starts from its tree. A conflict refuses the squash, as does a result without changes.
4. Write the commit with `fast-import`: the target's author and the current user as committer.
5. Replay the rest up to HEAD (`git replay`, as for a drop).
6. Check, before any ref moves:
   - the new HEAD tree equals the old one, so the index and the working tree are already right and a dirty tree is
     fine (one `rev-parse` of the replayed tip; the old tree is the history's);
   - the commit count matches, because `replay` silently drops a commit that becomes empty.
7. Move the branch in one transaction (`branch-deck: squash N commits`) that also deletes the temporary refs.

**Undo** moves the branch back to the old HEAD with a CAS on the new one (`branch-deck: undo squash`). It is refused if
the branch moved since, or if the two trees differ; equal trees mean the index and the working tree need no change.

Code: `crates/git-ops/src/squash_commits.rs`, `crates/git-ops/src/rewrite_plan.rs`, `crates/history-rewrite/src/squash.rs`.

#### Squash fixups

`fixup!`, `squash!` and `amend!` commits are squashed into their targets as `git rebase --autosquash` would, but as
one squash rewrite with a group per target. The plan (`plan_autosquash`) is pure, over the commits ahead of the
baseline, oldest first, and follows git's `todo_list_rearrange_squash`:

1. A subject is a fixup if it starts with `fixup! `, `squash! ` or `amend! `; the space is required. All leading
   markers are stripped (`fixup! squash! X` targets `X`); the first one gives the kind. The grouper strips them the same
   way, so `fixup! (auth) X` stays in `auth`.
2. The target is the oldest older commit that matches, in this order: the exact subject; a hash prefix of 4 or more hex
   digits that matches one commit of the range; the subject without the `(prefix)`, the prefixes agreeing if the fixup
   has one (Branch Deck's addition: `fixup! X` finds `(auth) X`); a subject, or a subject without the `(prefix)`,
   starting with the text. Fixups are never matched by subject.
3. A fixup of a fixup (by hash) goes to the final target. Anything unmatched, a newer target included, is an orphan:
   reported, not squashed.

Messages: a group of fixups only keeps the target's message and encoding (`SquashMessage::Target`). A `squash!` appends
its body, an `amend!` replaces the message with its body, as UTF-8 text; if an involved commit has another encoding
(one `cat-file --batch` for those), the squash is refused.

`squash_fixup_commits_core` plans again and takes the groups of the scope: the target's branch (grouper rules), the
unassigned targets, or one fixup. It refuses with `HeadMoved` unless HEAD, the newest commit of the planned range and
the history the squash replays all are the confirmed head; these are three reads, and a commit between them would
otherwise give a plan for one head and a rewrite of another. The history is read last, down to the oldest target of
the scope (see "Rewrite history"), once the plan names the targets. The branch moves with `branch-deck: squash fixups` and a
CAS on that head; Undo is the squash undo. The core takes the repository lock first; `bd fold` passes its
`--expect-head` and lock timeout to it, with the head it planned for.

Code: `crates/history-rewrite/src/autosquash.rs`, `crates/history-rewrite/src/commit_grouper.rs` (`parse_subject`,
`GroupingInputs::branch_of`).

#### Move a hunk

`bd move-hunk <from> <to> <path> [--block <id>]…` moves the change of one file, or some of its blocks, from commit
`from` into commit `to`, both ahead of the baseline on the checked out branch: the fix for a change that landed in the
wrong commit. HEAD's tree never changes, by construction, so the index and the working tree need nothing:

- forward (`to` older than `from`): the trees of `[to, from)` get the change Δ added; `from` keeps its tree, which now
  holds only the rest of its change;
- backward (`to` newer than `from`): the trees of `[from, to)` lose Δ; `to` keeps its tree;
- the commits above the newer of the two get new parents only.

The steps, a fixed number of processes whatever the range (`crates/git-ops/src/move_hunk.rs`):

1. Under the repository lock (none for `--dry-run`): the guards, the commits ahead of the baseline, `--expect-head`
   (prefix match), the files of `from` from one `log --raw` (`read_commit_changes`), and the blocks of the path from
   `FileDiffs::patches_of` (one `cat-file --batch` of its two blobs; the blocks of the diff views, so an id is the one
   of `bd commits --patch`). A rename of the path is refused, as are `--block` of a file without blocks, an id that
   does not parse, and an id whose lines or hash match no block now (`block_not_found`, with the ids of now); all
   blocks named is the whole file.
2. One `log --first-parent --boundary` of the history with trees and mapping notes, the rewrite guard
   (`check_rewrite` from the older of the two), one `cat-file --batch` of the commits, one
   `diff-tree --raw --no-renames` of the path in `from`: a mode change, a type change, a symlink or a submodule is
   refused.
3. "Theirs", the merge base with Δ: forward `from^`'s tree with the path as the selected blocks applied to
   `from^:<path>`, backward `from`'s tree with them taken out of `from:<path>`. The blocks apply by their line ranges
   to the blobs (one `cat-file --batch -Z`, `text_diff::apply::apply_blocks`), byte for byte: a line ends with `\n`,
   `\r\n` or a lone `\r`, as the diff views split it, and keeps its own line end; the whole file moves its blob, its mode or its
   deletion as they are, also for a binary file. `write_edited_commit` writes the tree with one `fast-import` (a
   commit on a temporary ref).
4. One `merge-tree --write-tree --stdin -z --no-messages -X no-renames`, a line `<base> -- <commit tree> <theirs>` per
   commit of the range: the base is `from^`'s tree forward and `from`'s tree backward. Theirs differs from the base
   only in the path, so every other path keeps the commit's version, and the path gets Δ applied to the commit's own
   version of it: a commit that only shifts the lines takes it, one that changes the same lines, or lacks the file,
   conflicts (`hunk_conflict`, naming the commit and the path, nothing written but objects). Without renames, a file
   that doesn't exist in a commit is a conflict, not a guess. A conflict here is never resolved by words (see "Resolve
   by words"): HEAD's tree is the same by construction, so a conflict says that the move splits a change of the
   user's own commits wrongly, which a resolve would hide.
5. Checks in memory: no commit of the range or the one above it becomes empty (moving all of `from`'s only change
   is "squash it instead"), and the last tree is HEAD's.
6. One `diff-tree --stdin -p -U0 -- <path>` of the old and the new tree pair of each commit between the two, one
   `patch-id --stable`: line numbers and whitespace don't count, so a change that only shifted keeps its id.
7. One `fast-import` writes the commits and their notes (`write_commits_with_notes`, with the committer of step 3, so
   one `git var` serves both). Metadata, the named exception of
   invariant 5: `from`, `to` and the commits between whose diff changed keep the author and get the current user as
   committer; every other commit keeps author and committer byte for byte. Notes: a commit between with the same diff
   gets its note with the same virtual commit and the new trees as its source (a note without a source is copied as
   it is, for the sync to check); the commits above get their mapping lines copied; the changed ones get none, so the
   sync copies them again, and the virtual commits of the other branches stay.
8. One `RefTransaction` (`branch-deck: move hunk`) moves the branch with CAS on the HEAD it read and deletes the
   temporary refs. The undo token `<previous>..<new>` works with `bd undo`: the trees are equal.

`--dry-run` stops after step 6 and deletes the temporary ref at once: no ref moves and no note is written, only the
objects of theirs and the merges. `--sync` syncs under the same lock.

A trial (`MoveHunkMode::Trial`, no lock) also runs step 7 without the notes and then deletes the temporary refs instead
of step 8: the rewritten commits exist as objects only, for a trial build of the history after the move.
`CheckSession::with_rewrite` puts them in place of the old ones in its snapshot (their parents follow, without
mapping notes, so their branches are built anew) and `rewritten_plan` renames the plan's ids; `bd check` proves a
`move_hunk` fix that way before it prints the command (see "Dangling references (check)").

Code: `crates/git-ops/src/move_hunk.rs`, `crates/sync-core/src/move_hunk.rs`, `crates/bd-rewrite/src/move_hunk_command.rs`.

#### Update project

Update Project (`⌘T`) brings the checked out branch onto its upstream, as `git pull --rebase` does, but it works with
a dirty working tree when that is safe and never leaves a half-done rebase. The local commits are replayed in memory,
and the branch and the files move only after every check passed. The same check, without the network and without the
final steps, runs inside every sync, so the header can say whether the update would apply cleanly.

| Step | Command | On failure |
|---|---|---|
| 0. Guards | `rev-parse --git-path` for the operation markers, `symbolic-ref HEAD` | error: an operation in progress, a detached HEAD |
| 1. Upstream | `remote`, `for-each-ref --format=%(upstream)%00%(upstream:remotename)%00%(upstream:remoteref) refs/heads/<b>` | a local baseline: nothing to update from |
| 2. Fetch | `fetch --porcelain --no-tags --no-write-fetch-head --no-recurse-submodules --no-auto-maintenance <remote> +<remoteref>:<tracking>`; none for `bd update --no-fetch` | error (`FetchFailed`), nothing changed |
| 2b. Lock | `$GIT_COMMON_DIR/branch-deck.lock`, held to the end; the wait is the step `waiting` | `locked` after 10 s, nothing changed |
| 2c. Pending files | `rev-parse --verify refs/branch-deck/update-pending/<worktree id>`; with one, the files of the interrupted move follow first (below) | `finish_pending`: the branch moved since, nothing changed |
| 3. Classify | `rev-parse HEAD U`, `merge-base HEAD U` | `B == U`: up to date; `B == H`: fast-forward, `N = U`; exit 1: no common history, any other exit (a history git can't read) an error with git's message |
| 4. Previous tips | the `+ <old> <new> <tracking>` line of the forced fetch, `merge-base --fork-point <tracking> HEAD` | none (exit 1): the merge base suffices; any other exit (the ref is gone) an error |
| 5. Linear | `rev-list --min-parents=2 -n1 HEAD ^U ^<previous tips>` | error: merges are not replayed |
| 6. Replay | `replay --ref-action=print --onto=U --ref=refs/branch-deck/replay HEAD ^U ^<previous tips>` | exit 1: `Conflict`, see below |
| 7. Dry run | on a copy of the index: `update-index -q --refresh`, `read-tree -m -u -n --no-recurse-submodules H^{tree} N^{tree}` | `Blocked{paths}` |
| 8. Ignored files | `diff-tree -r -z --no-renames --name-status H N`; an added path that exists on disk | `Blocked{paths}` |
| 9. Move | `symbolic-ref --quiet HEAD` (still `refs/heads/<b>`), then `update-ref --create-reflog --stdin`: `start`, `update HEAD N H`, `create refs/branch-deck/update-pending/<worktree id> H`, `prepare`, `symbolic-ref` under the locks, `commit`; `branch-deck: update project` | `BranchSwitched` or `HeadMoved`: HEAD switched or the branch moved meanwhile; `locked` with the path when `HEAD.lock` or the branch's lock file exists; nothing changed |
| 10. Files | on the real index: `update-index -q --refresh`, `read-tree -m -u --no-recurse-submodules H^{tree} N^{tree}`, `update-index -q --refresh`, then `delete refs/branch-deck/update-pending/<worktree id> H` | roll back: `update HEAD H N` and the mark deleted, `branch-deck: update project rolled back` |

Details:

- **Lock.** The fetch runs before the repository lock, so a slow network never blocks another writer; the check and
  the move run under it, so the app's Update Project, `bd update` and every rewrite of the app or of bd never
  interleave. The check runs again under the lock, so an update never applies a check made before another writer
  moved the branch. `bd update --dry-run` is the check inside sync (below): no fetch, no lock, no move.
- **Fetch scope.** Only the baseline's remote branch is fetched, into its tracking ref. The other tracking refs stay
  where the last push saw them, so the `--force-with-lease` of the virtual branches keeps protecting work pushed from
  elsewhere. `--porcelain` prints `<flag> <old> <new> <ref>` per updated ref (` ` fast-forward, `+` forced, `*` new)
  and nothing for a ref that did not move. The fetch runs through `execute_remote_command_with_progress` (Key
  Decision 8).
- **Fetch without update.** The Fetch of a merged robot's toast (`crates/sync-core/src/baseline_fetch.rs`) is step 2
  alone, then a sync: the check inside it compares HEAD with the fetched tracking ref. A forced update needs no
  record: the next check excludes the old tip by the fork point of the tracking ref's reflog.
- **Transfer progress.** Git has no machine-readable progress: trace2 reports a stage only when it ends. So the fetch
  and the app's push run with `--progress`, as IDEA's do: `execute_remote_command_with_progress` adds it after the
  subcommand, and `GitCommand::progress` passes each stderr line as it arrives (split on `\r`, with which git
  overwrites a progress line, and `\n`). `TransferProgressAnalyzer` (`crates/sync-core/src/transfer_progress.rs`)
  ports IDEA's `GitStandardProgressAnalyzer`: a share per stage, earlier stages done. A fetch receives (or unpacks)
  where a push writes, both are the transfer stage; the `remote: ` prefix doesn't matter, since a fetch counts on the
  server and a push resolves the deltas there. `TransferProgressReporter` throttles to one progress per 100 ms but
  always sends the end of a stage, a stage at 100 %, which also covers the server's `Resolving deltas: 100% (2/2),
  completed with 2 local objects.` that has no `, done.`. It is the one place that reads human output, and only for a
  progress: an unknown line changes nothing. The error of a failed command keeps the `\n` lines only, without the `\r`
  updates.
- **Steps of an update.** The update reports its step on the progress channel (`UpdatePhase`): `fetching` with the
  transfer progress, `waiting` once when the lock is held by another operation (`lock_repository_with` calls its
  `on_wait` on the first `WouldBlock`; the poll loop and its `rev-parse` stay one), `checking` once the lock is taken,
  `updating` before the move. The page turns the steps into one progress, the share of each its usual time in the
  repository (`src/lib/update-progress.ts`). The file write itself reports nothing: `read-tree -v` prints "Updating
  files" only after git's 1 s progress delay, translated, and reading it would put a progress reader with its idle
  kill around the one guarded working-tree write.
- **Force-pushed upstream.** After an upstream force-push, `HEAD ^U` also contains the commits the upstream dropped,
  and replaying them conflicts or brings them back. As `pull --rebase` does, they are excluded with the old tip from
  the forced fetch line and with the fork point from the tracking ref's reflog; the fork point also covers a forced
  fetch made in a terminal before the check inside a sync.
- **Conflict.** `replay` reports a conflict only by its exit code. The files come from `merge-tree --write-tree
  --name-only -z --no-messages U H` (`<tree>\0<file>\0…`, exit 1). If only the replay of single commits conflicts, they
  are the paths both `diff-tree B U` and `diff-tree B H` change. The commits are `log --reverse HEAD ^U ^<previous
  tips>`. Two more `--literal-pathspecs log --reverse --no-merges --no-renames --name-only -z -- <files>` runs tell
  which commits change the files: one over the local range, one over `B..U` with
  `--max-count=50` (a monorepo upstream has thousands). At most 50 files go on the command line, which Windows limits
  to 32K characters. The result names the files, the commits and the upstream commits with their authors and
  committers and both times (`%an %at %cn %ct`: the committer time of an upstream commit is about when it landed), with
  H, U and B, for the dialog and the agent prompt; nothing has changed. In `bd update` (not `--dry-run`), a conflict
  starts a resolve session under the same lock instead ("Resolve session" below). Before the conflict, the check
  resolves the conflicts of git by words when `branchdeck.resolveByWords` is on ("Resolve by words" below).
- **Dirty working tree.** Two-way `read-tree -m -u` updates a path only where `H` and `N` differ, and only if the index
  and the working tree still match `H` there. Uncommitted or staged changes on any other path stay byte for byte. A
  dirty or staged path that the update changes, an untracked file where it adds one, or an untracked file inside a
  directory it replaces makes git refuse; the dry run on the index copy finds that before anything moves, and
  `Blocked` lists those paths (`status --porcelain=v1 -z --no-renames --untracked-files=all` ∩ the changed paths). The
  copy is refreshed first, so a file that was only touched (same content, new stat data) does not block.
- **Ignored files.** `read-tree -u` overwrites an ignored file where the update adds one, silently, as `checkout`
  does. Step 8 refuses that instead: an added path that exists on disk (or has a file in place of one of its
  directories) blocks the update.
- **Paths as bytes.** The paths of `diff-tree -z`, `diff-index -z`, `status -z` and `merge-tree -z` stay bytes as git
  stores them, untrimmed: they name the index entries (`:<path>` of `cat-file -Z`), the files on disk and the input of
  `hash-object --stdin-paths`, so a path that is not UTF-8 (Latin-1 on Linux) still names its entry. Only the
  reported paths are text, with U+FFFD for the bytes that are not UTF-8; such a likely conflicting file is listed
  without commits, because a pathspec argument must be UTF-8.
- **Order and rollback.** The branch moves before the files, with a CAS on H: if a commit landed meanwhile, the move
  fails and nothing was written. `read-tree` checks every path again before it writes any file, so a file changed
  between the dry run and step 10 fails it with nothing written; the branch then moves back with a second CAS
  transaction and the update reports "Files changed during the update; nothing was changed".
- **Already upstream.** `replay` drops a local commit that becomes empty, e.g. one merged upstream from a pull request.
  The result reports it: `rev-list --count N ^U` replayed against `rev-list --count HEAD ^U ^<previous tips>` local.
  **Landed commits left out.** When the replay conflicts, the check asks sync-core for sets of landed local commits
  (`LandedCandidates`): those with a counterpart upstream with their fixups, then those alone. Each set is left out as a
  drop does, one `replay` per run of kept commits, the first run `<d_1>^ ^U ^<previous tips>` onto U. The result counts
  only if `merge-tree --merge-base=<base> <result> H` gives the result's tree: merging every local change into it
  changes nothing, so nothing local is lost (an amend after the push, a fixup made after it). A robot that autosquashes
  pushes the target with its fixups folded in; the target alone then adds other content than the upstream has, and a
  plain replay conflicts. Tried only after a conflict, so a clean update costs nothing more.

  Which commits landed, and where, takes three read-only `log` processes after the check (the preview's are cached
  with its replay): the local commits (`HEAD ^U ^<previous tips>`), the ones that stay (`N ^U`) and the upstream
  commits since the merge base by the authors of the local commits (`--fixed-strings --author=<email>`, so a monorepo
  upstream stays small). None when the upstream has nothing since the merge base (up to date): no upstream commit can
  carry a local one. See "Landed commits" in [Integration Detection](integration-detection.md); the report is
  `branch-deck/landed.json` in the worktree's git directory, written through a temporary file and a rename, never git
  state.
- **Metadata.** The replayed commits keep their author; the committer is the current user, as with any content rewrite.
- **The check inside sync** (`CheckPurpose::Preview`) runs steps 0, 1 and 3 to 6 against the last fetched tracking
  ref, and a cheaper form of steps 7 and 8. It starts as soon as the sync knows the baseline, beside the commit
  grouping and the branch rebuilds, and sends `SyncEvent::BaselineStatus` whenever it is done; its duration is logged.
  A failed check (a rebase in progress, a detached HEAD) is reported in the event, never as a sync error. `fetched_at`
  is the time git last wrote `FETCH_HEAD` (a fetch elsewhere); the app keeps the time of its own fetch.

  The preview differs from the check before an update so that a sync on every window focus stays cheap in a monorepo
  with a huge index; its cost grows with the paths the update changes, not with the index:
  - **No index copy, no full refresh, no dry run.** The paths come from the raw `diff-tree -r -z --no-renames H N`
    (mode and blob in H). An added path is blocked by anything on disk there (step 8). A changed or deleted path is
    blocked if its index entry differs from H (staged: one `cat-file --batch-check=%(objectname) -Z` of `:<path>`, a
    binary search per path) or if its file differs from H's blob (`hash-object --stdin-paths`, with the clean
    filters; a symlink by the id of its target, `blob <length>\0<target>` hashed in-process with the algorithm of the
    repository, `commit_utils::blob_id`). Comparing content, a file that was only touched does not block, and the
    index is never written.
    `status`, `diff-files` and `diff-index` take no `--pathspec-from-file`, and as arguments thousands of pathspecs
    are matched against every index entry: `status` with 2,105 pathspecs took 5–12 s on a 50k-file index.
  - **Cache.** Per repository, in memory: the replay outcome (conflict, or the new tip and the changed paths) keyed by
    H, U and the excluded previous tips, and a probe of the local state. The probe keeps the index blobs for one
    index file (git writes a new file, a new inode, on every change) and, per hashed file, its stat data, when it was
    hashed and its blob, trusted like git's racy check only when the file was modified at least 2 s before it was
    hashed. An unchanged state therefore costs no replay, no `cat-file` and no `hash-object`, only an `lstat` per
    changed path; after a fetch only the newly changed paths are read.
  - Its replay sets `GIT_COMMITTER_DATE=@0 +0000`, so a replay after a cache miss writes the same objects again; such
    a plan can't be applied.
  - **A conflict is shared with the update.** Whether the replay conflicts, and its files and commits, don't depend on
    the committer date. So the exact check takes a cached conflict of the same H, U and previous tips instead of
    replaying, and writes its own conflict to the cache: the click on Update after the sync that found the conflict,
    and the sync after a conflicting click, cost no replay (in a monorepo, hundreds of local commits replay for
    seconds). A clean outcome is never shared: its commits must carry the real committer date.
  - **Bounded.** A conflict reports the first 50 likely conflicting files (51 rather than one left out, as every cut
    list: `list_cut::kept`), the ones its commits are attributed to
    (each a pathspec, and Windows limits a command line to 32K characters), and counts all (`file_count`); a block
    reports the first 100 paths and counts all (`path_count`). The event of every sync carries the state, so a conflict
    in thousands of files costs the app and `bd` a list of 50, not thousands of paths and file lists per commit.
  - It leans to blocked where the two can differ: a sparse checkout (a missing file then doesn't block), an
    executable bit with `core.fileMode=false`, a symlink, a path with a newline. A directory that the update replaces
    by a file, or the reverse, is checked by the update's dry run only. The update always runs the exact check
    (`CheckPurpose::Apply`) after its fetch.

**Undo of an update** (`bd undo update:<H>..<N>`, `undo_update_core`). The move back from N to H, with the files
following, under the repository lock:

1. The guards of step 0, and HEAD must be N (`HeadMoved` otherwise, `undo_stale` in bd: a commit on top, another
   update or a switch can't be undone this way, and a retry can't change that).
2. The last entry of the branch's reflog must be `branch-deck: update project` to N
   (`log -g -n2 --format=%H%x00%gs refs/heads/<b>`), with the value before it at H (`rev-parse refs/heads/<b>@{1}`,
   which also reads the old value of a reflog the update started). So a token can't move the branch to any other
   commit, and only the last update can be undone. When HEAD is at H and the last two entries are
   `branch-deck: undo update` to H and the update to N, the undo is done already: `already_undone`, no error, so a
   repeated undo (a timed-out call that did succeed) is harmless.
3. Steps 7 and 8 with the trees swapped (`check_update_undo`, the same `blocking_paths`): the dry run of
   `read-tree -m -u N^{tree} H^{tree}` on a copy of the index refuses every path changed since the update, and an
   ignored file where the undo brings a file back (one that the update deleted) blocks it too. A block is
   `UndoBlocked{paths}`, `uncommitted_overlap` in bd.
4. Steps 9 and 10 with `update HEAD H N`, `branch-deck: undo update`, and the same rollback. Only
   `check_update_undo` builds the `UpdateUndo` that `apply_update_undo` applies: its fields are private and the
   reflog message is fixed, so no other crate can move HEAD to another commit through the exemption.
5. The landed report goes back to the one from before the update: `record_update` keeps it as `previous` in
   `branch-deck/landed.json`, and a sync drops it when it finds the branch elsewhere.

**Residual risk.** `read-tree -u` validates every path before it writes the first file, but writing can still fail
midway, e.g. when a smudge filter (Git LFS) fails or the disk fills up. The branch then moves back to H, and the files
already written show up as uncommitted changes that bring in the upstream's version; `git status` and the reflog
(`branch-deck: update project`) show what happened. Submodules are not updated (`--no-recurse-submodules`); a
changed gitlink shows as a modified submodule until the user updates it.

**Interrupted files.** A process killed between the move (step 9) and the files (step 10), e.g. by an agent harness at
its timeout on a monorepo index, leaves HEAD at N and the index and the files at H; no rollback runs. The move creates
the mark `refs/branch-deck/update-pending/<worktree id>` at H in its transaction (the undo: at N), and step 10 deletes
it, so the state is never silent. Every later update, undo and the app's Update Project starts with the mark, under the
lock (step 2c). When the last reflog entry of the branch is that move from the mark's commit to HEAD, the files follow
through the same guarded update, or only the mark goes when the index already has the new side. A local change in the
way refuses it, and the branch moves back with the mark deleted. When the branch moved since, the result is
`finish_pending`, which names `git update-ref -d <mark> <commit>` and changes nothing. `bd update --status` prints the
mark as `pending_update`.

Code: `crates/git-ops/src/update_branch.rs` (check, apply, the undo check, and the moves of a resolve finish),
`crates/git-ops/src/paths.rs` (the changed paths of two trees, the commits that change the likely conflicting files),
`crates/git-ops/src/worktree_update.rs` (the exempt `read-tree -u`), `crates/sync-core/src/update_project.rs` (upstream, fetch, lock, undo, the check inside
sync), `crates/bd-update/src/update_command.rs` (`bd update`, `bd undo update:`).

#### Resolve session

An update that conflicts starts a resolve session in `bd update` (spec: [bd update](../../spec/cli/update.spec.md)).
The app's Update Project starts none. The session replays the local commits outside the working tree, one round per
conflicting commit. The agent resolves a round in copies of the files, and `bd update --continue` commits the round.
Only the finish moves the branch, and the files follow through the guarded update of "Update project". Code:
`crates/git-ops/src/update_resolve.rs` (the state and the engine), `crates/git-ops/src/resolve_check.rs` (the checks),
`crates/git-ops/src/merge_tree_output.rs` (the `-z` parser, shared with the sync's cherry-pick),
`crates/sync-core/src/resolve_session.rs` (the lock and the output), `crates/sync-core/src/resolve_copies.rs` (the
copies), `crates/bd-update/src/update_command.rs`.

**State in git.** All state of a session is in git; only the copies are files.

- The session ref is `refs/branch-deck/resolve/<worktree id>`: `main` for the main worktree, else the name of the
  linked worktree. It is a shared ref, not a `refs/worktree/` ref: a probe showed that a `gc --prune=now` in another
  worktree prunes the objects of a `refs/worktree/` ref, because the gc of one worktree does not see the per-worktree
  refs of another.
- The ref points at a state commit. Its tree holds `state.json` (version 2 since a round can resolve itself by words;
  the only file that readers parse, its `version` first, so a bd of another version refuses it by that number),
  `round/merge` (a `040000` entry of the round's merge tree, which keeps the `edit` copies rebuildable) and
  `round/{before,onto,commit}/<path>` (the stage blobs, by id). Its parents are the previous state, the replayed tip
  and HEAD as the session knows it. The tip keeps the replayed commits reachable, and HEAD keeps the original local
  commits reachable, which `--status --materialize` needs. Reachability goes through commit parents only, never a
  gitlink, so every git command, a gc included, follows it.
- `state.json` records the worktree's git dir, the branch, the upstream, the merge base, the upstream tip, the
  excluded upstream tips, the local commits, how many are replayed, the tip, the round, the status, the open round
  (the commit, the tip it replays onto, the merge tree, the conflicting files with their stages), the resolutions so
  far, and the fixed committer. State commits have that committer too, so `var GIT_COMMITTER_IDENT` runs only at the
  start.
- Every step moves the ref with compare-and-swap. `--continue --round <n>` refuses another round (`round_moved`, exit
  2, not retryable: `bd update` prints the open round).
- The states are `resolving` (a round is open), `ready` (every commit is replayed, the finish is next), `applying`
  (the finish started, the branch has not moved) and `applied` (the branch moved, the files may not have followed).

**Binding.** A session belongs to one worktree and one branch, and keeps the upstream tip it started with; it never
fetches. There is one session per worktree, so no id is needed, and `bd update` with a session open prints the open
round again (no fetch, nothing changed) and writes its missing copies. Before every step:

- the git dir of the worktree must be the one in `state.json`; otherwise the session is `session_stale`, with
  `bd update --abort --worktree <id>` as the way out. In another worktree, `--continue` finds no session of its own
  (`no_session`);
- the guards of step 0 run, and HEAD must be on the session's branch;
- HEAD must contain the session's HEAD. Peers' commits on top of it (`merge-base --is-ancestor`, no merges) join the
  local commits and are replayed as more rounds, usually clean. A rewritten HEAD (exit 1 of `--is-ancestor`) is
  `session_stale` (exit 4, not retryable); the session stays for `--abort`. Any other exit (an object git can't read)
  is an error with git's message, never taken for a rewrite; so is the check of the crash recovery that the branch
  still contains the tip.

`--status` lists every session of the repository with one `for-each-ref` and one `cat-file --batch`; a session whose
git dir is gone is `orphaned`, and `--abort --worktree <id>` removes it. The session of a worktree that still exists
needs `--force` too; an id is letters, digits, `.`, `_` and `-` only.

**The engine.**

| Step | Command |
|---|---|
| Start | the check of "Update project" conflicted under the lock, and the session reuses its landed candidates; `for-each-ref` sweeps stale temporary refs (also at `--abort` and in a recovery), `var GIT_COMMITTER_IDENT` (the fixed committer), `rev-list --reverse HEAD ^U ^<previous tips>` (the local commits), `rev-list --cherry-mark`, `diff-tree --stdin -p` and `patch-id --stable` (the landed ones, left out) |
| Find the conflict | a binary search over the prefixes `<last> ^<first>^` with `replay --onto=<tip>`: `replay` stops at the first conflict, so ⌈log₂ n⌉ replays find it |
| Resolve by words | with `branchdeck.resolveByWords` on, the merge of the round (below) resolved by words: one `check-attr`, one `cat-file --batch` and one `fast-import`; a resolved commit is a round that resolves itself, and the search goes on after it ("Resolve by words") |
| Open a round | `-c merge.conflictStyle=zdiff3 merge-tree --write-tree -z --merge-base <c>^ <tip> <c>`, parsed as bytes: the stages and the messages; one `cat-file --batch -Z` of the commit, the merge result per path (`<merge tree>:<path>`) and the stage blobs, which also writes the copies |
| The copies | one `log -1 --patch-with-stat` for `commit.patch`, one `log` of the upstream commits of the files, and the check of every sync for `blocked_paths` (two `diff-tree`, `rev-parse --git-path index`, `cat-file --batch-check -Z`, `hash-object --stdin-paths`) |
| The state | one `fast-import` of the state commit on a temporary ref, one `update-ref --stdin` (create or CAS, the temporary ref deleted) |
| `--continue` | one `cat-file --batch` of HEAD and the session, the guards (2); one `cat-file --batch -Z` of the round's objects for the checks; one `fast-import` writes the resolved commit (`M 040000 <merge tree> ""`, then `M`/`D` per path, author and message of `c`, the fixed committer) and `ls`s its tree and its parent's; one `replay` of the rest, a binary search only when it exits with 1; then the next round as above, or the finish |

Measured on a middle round of a fixture with three local commits, `--continue` runs 20 git processes after the 3 of
bd's own start: 2 replays, 5 for `blocked_paths`, 1 `merge-tree`, 2 `cat-file -Z`, 2 `fast-import`, 1 `update-ref`, 2
`log`, 2 guards, the lock and the session read. The plan's estimate of about 9 plus the replays is not reached; the
check for `blocked_paths` is the largest share.

- **Fixed committer.** Every commit the session writes, by `replay` (`GIT_COMMITTER_NAME`, `_EMAIL`, `_DATE`) and by
  `fast-import`, has the committer and the date of the session start. So the same input gives the same ids: a rerun
  after a failed step writes the same objects, and `--continue --dry-run` reports the id that `--continue` writes.
  This is a stated deviation from invariant 5, where a content rewrite gets the current time.
- **An empty result.** When the resolved tree equals its parent's (the `ls` of the same `fast-import`), the round is
  `empty_resolution` (exit 2), and nothing moves. `--allow-empty` keeps the empty commit; `--drop-empty <commit>`, with
  the round's commit, drops it: no `resolved` id and no note. The finish counts a dropped commit as
  `dropped_by_resolution` and keeps it out of `dropped_already_upstream` and of the landed report, because the
  upstream does not have it.
- **Supported types.** A round supports `CONFLICT (contents)` of text (also `CONFLICT (add/add)`, a content conflict
  without `before`), `CONFLICT (modify/delete)` and `CONFLICT (rename/delete)` (the stages are at the new path; the
  message gives the old one, so `--take` and `--delete` take either), on regular files (`100644`, `100755`). The mode is merged as git
  does: the side that changed it wins, and only two different modes on both sides are `distinct modes`. Any other
  conflict type of the `-z` messages (binary, file/directory, rename/*, distinct modes), a symlink (`120000`) or a
  gitlink (`160000`) stage, an invalid path, and a content file whose merge result has no block of bd's labels refuse
  the whole round with `unsupported_conflict`, with the paths. A file/directory conflict names its moved side file
  (`df~<id>`) too. At the start no session is created. Every refusal names the round and its commit (id and
  subject), and a refused `--continue` says that the round stays open and the copies keep the agent's edits.
- **Landed commits.** At the start, one `rev-list --cherry-mark --no-merges U...H` marks the commits of both sides
  that have a counterpart with the same patch, and one `diff-tree --stdin -p` with one `patch-id --stable` pairs them.
  A local commit whose patch id equals that of an upstream commit since the merge base is left out of the replay, as
  `git rebase` leaves it out (`landed_dropped`): the patch id proves that the upstream has its change, also when the
  upstream changed the lines further and a replay would conflict. The other local commits form runs of consecutive
  commits (`run_starts`): each replay covers one run, and the binary search runs inside the first run that conflicts.
  A commit that the landed detection finds only by its author, author time and subject gets the hint "likely landed:
  take onto" and is never left out: that key is no proof.
- **Blocked paths early.** Each round prints `blocked_paths`: the local changes on the paths that the upstream changed
  since the merge base and that differ between HEAD and the upstream, found with the cheaper check of every sync. The
  agent asks the user about them before it resolves; the finish runs the exact check.

**The checks of a round.** Every conflicting path needs an explicit resolution: an edited `edit` copy, `--take
onto:<path>`, `--take commit:<path>` or `--delete <path>`. A take of a side that has no file deletes it. A missing
copy is `missing_resolution`, never a delete, and so is an unchanged modify/delete copy (the version left in the tree
decides nothing).

- Blocks: bd knows the labels it passed to `merge-tree`, so a block starts with a run of 7 or more `<` and the new
  parent's id, has the base section (`|` and `<commit>^`), the separator of `=` of the same length, and ends with `>`
  and the commit's id. With a rename (on either side, or of a directory), merge-ort adds `:<path>` to each label, the
  path as it is, and the block still counts. So the longer markers of `conflict-marker-size` count, and any other
  marker-like line, e.g. the `=======` under a Markdown setext heading, is text.
- Hard: outside the conflict blocks, an edited content file must be byte for byte the `edit` copy that bd wrote (read
  again from the merge tree, not from disk). The plain runs between the blocks are matched in order, leftmost, and the
  regions between them replace the blocks. A changed plain line is `resolution_outside_conflict`; when only the line
  ends differ (CRLF and LF, or the newline at the end), the message says so. A marker line of bd's labels in a
  replaced region is `unresolved_markers`. Both are exit 2, nothing written. Outside the blocks the merge already holds
  every change of both sides; a change that the commit needs there, or in another file, goes into a `fixup! <subject>`
  commit after the update, and the refusal says so.
- The message of a refusal names the absolute `edit` copy of each path. For leftover markers and a missing resolution,
  one `diff --name-only -z HEAD -- <paths>` also names the paths that changed in the working tree, where bd never
  reads: the commonest mistake of an agent.
- Report only: per file the lines of a side's change (`before` to `onto` for `dropped_upstream_lines`, `before` to
  `commit` for `dropped_commit_lines`) that the result lost, at most 20 per side; also for `--take` and `--delete`. A
  line counts as kept when the result has it as it is, or when one line that replaced a conflict block (of an edit;
  the whole result of a take) has every word that the side added to its nearest `before` line (the largest share of
  words in common). So a lost change of one value (`APP_MODE=test`, a version, a flag) is reported, also when its word
  is in another line of the file, and a rewrite that combines both sides in one line is not. A rule by the share of
  words in common hid such losses, and so did a look at the whole file. The agent shows them to the
  user. An earlier gate that merged the result with the upstream again fired on nearly every correct resolution, and
  is cut.

**The copies (the store rule).** The copies are the only files of a session outside git objects, and all of them can
be rebuilt from the session ref.

- They live in `branch-deck/resolve/` of the worktree's git dir (`state.git_dir`), or in the directory of `--dir`.
  Per round `r<n>/edit/`, `before/`, `onto/` and `commit/`, each file under the flat name `<key>-<basename>`, and
  `r<n>/commit.patch`; `.branch-deck-resolve` marks the directory and names the git dir of its owner. A flat name
  can't reach outside the round directory, can't collide with another file (two paths that differ only in case get
  distinct keys), and needs no directories of the tree. A basename longer than 100 bytes is cut on a character
  boundary and keeps its extension, so the name stays below `NAME_MAX` with the key and the temporary file.
- Tree paths are validated per component by `gix_validate::path::component`: no empty, `.` or `..` component, no `.git`
  in any case or a name NTFS or HFS+ takes for it (`GIT~1`, `.git.`, `.g\u{200c}it`), no NUL, and on Windows no `\`,
  no drive prefix, no `:` stream and no device name (`CON`). Characters a file system may refuse are replaced in the flat name only.
- Bytes are written and read in Rust, as git stores them, without filters. A write goes to a new temporary file that a
  rename puts in place, which replaces a symlink at the target instead of writing through it. A read takes only a
  regular file, and the file it opens must be the one `lstat` saw. Every directory from the git dir (or `--dir`) down
  to the copy must be a real directory: bd neither reads nor deletes through a symlinked directory.
- A new round gets a new directory, so a stale edit never applies to the next round, and the copies of the closed
  round are removed. bd deletes only the file names it wrote, then the directories left empty, only in a directory
  whose marker names its own git dir, and removes the session directory only when it created it.
- `--dir <path>` (for a sandbox that keeps `.git` read only) must be absolute, not a symlink, not inside a worktree
  (the git dir aside), empty or marked for this git dir, and used by no other session (`list_sessions`); a refused
  directory that bd created is removed.

**The finish**, when every commit is replayed (`finish` in `update_resolve.rs`), in four compare-and-swap steps:

1. The exact check of steps 7 and 8 of "Update project" from HEAD to the tip (`blocking_paths`). Local changes there
   are `uncommitted_overlap` with the paths; the state is committed as `ready` then, so no resolution is lost, and a
   later `--continue` only finishes. Without a block, no `ready` state is written: one `fast-import` writes the
   `applying` and the `applied` state commits and the notes commit, each on a temporary ref, and one transaction
   moves the session ref to `applying` and deletes the temporary ref of the last resolved commit.
2. `move_branch_and_files_with` moves the branch as step 9 does: `update HEAD <tip> <head>` (deref), committed only
   while git holds `HEAD.lock` and `symbolic-ref HEAD` is still the branch (`RefTransaction::commit_while`). The same
   transaction moves the session ref to `applied`, moves the notes ref with CAS (or creates it) and deletes the
   temporary refs. The reflog message is `branch-deck: update project`, so `bd undo update:<head>..<tip>` takes the
   finish back.
3. The files follow (step 10). On a failure the branch, the session ref (back to `applying`) and the notes ref move
   back together in one transaction, under the same check.
4. The session ref is deleted with CAS on `applied`, the landed report is written as after an update, and the copies
   are removed.

**Crash recovery.** A process killed at a step leaves `applying` or `applied`. The next `--continue` completes it,
except in the last case below:

- `applying` (killed before step 2, or the move refused, e.g. by a `git switch`): nothing moved. The next
  `--continue` runs the finish again, with the peers' commits replayed first.
- `applied`: the index is "left behind" when a path that the update changes still has the old side in the index
  (`cat-file --batch-check -Z` of `:<path>`; no entry for an added path).
  - Not left behind, and the branch still contains the tip (also with peers' commits on top): the files did follow,
    and `--continue` or `--abort` only deletes the session ref.
  - Left behind, and HEAD at the tip: `--continue` runs `files_follow` in `update_branch.rs`, the exact check and the
    guarded update. The local changes it reports are those against the old HEAD (`diff-index`), so bd's own files of
    the old HEAD are not named. On a block or a failure the branch, the session ref and the notes ref move back to
    `applying` together. `--abort` is `finish_pending` here: the branch moved already.
  - Left behind, and HEAD left the tip (a peer committed the old files on top): bd can't finish. `--continue` and
    `--abort` are `finish_pending` (exit 4); the message names `git update-ref -d <session ref> <commit>` and tells
    the agent to look at `git status` with the user.
- Temporary refs that a killed process left (`refs/branch-deck/tmp/<pid>-<nanos>-<n>`) are deleted when they are older
  than an hour, by the time in their name: at the start of a session and in a `--continue` of an `applying` or
  `applied` session.

**The notes.** Each resolved commit gets a note in `refs/notes/branch-deck/resolve`: the session (the worktree and the
start time), the round, the original commit and an outcome per path (`edited`, `onto`, `commit`, `deleted`). The
content is deterministic. The resolution records stay in `state.json` until the finish; only the finish transaction
writes the notes (on a temporary ref, moved with the branch), so `--abort` leaves none. User decision, 2026-10-15
(option a): no bd rewrite carries these notes, so a note is an audit record of one session, valid until the next
rewrite of that commit. A record that survives rewrites needs a store keyed by the landed key (author email, author
time, subject without the prefix), as the issue groups are; that is a follow-up, only if the app shows "resolved by an
agent".

**Residual risk.**

- A later round with an unsupported conflict stops the session: nothing is saved for it, every rerun gives the same
  refusal, and `--abort` drops the earlier rounds too.
- `--continue --dry-run` writes git objects and a temporary ref, which it deletes with compare-and-swap on the
  resolved commit. A step whose transaction fails, or whose replay of the rest fails (a later round that can't be
  resolved), deletes its temporary refs (the state's and the resolved commit's) in one more `update-ref --stdin`
  (`discard_temp_refs`); a killed one leaves them to the sweep.
- A peer that commits after the finish moved the branch but before the files followed commits on the index of the old
  HEAD; bd then refuses with `finish_pending`, and the user decides with `git status`.
- `blocked_paths` is the cheaper check of every sync; it can miss a block that the exact check of the finish finds.

#### Reapply

Reapply ([Unapply and Reapply](../../spec/branch/unapply.spec.md)) puts the commits of an unapplied branch back on top
of the checked out branch, the inverse of "Drop and unapply". Under the repository lock (a `--dry-run` takes none),
after the mark of an interrupted move is finished:

1. One `for-each-ref` of the unapplied branch (its tip, the worktree that has it, which refuses), the baseline and its
   tracking ref, and the start of its own commits (`own_commits_start`: its stack note, else the merge base).
2. The grouping prefix back (`restore_chain_messages`): a virtual commit keeps none, so each commit whose subject
   groups elsewhere by the grouping inputs (one `cat-file --batch`) gets `(<name>) `; one `rev-list`, one
   `cat-file --batch` and, when a message changes, one `fast-import` of the chain on its parents, trees, authors and
   committers byte for byte, on a temporary ref.
3. The check of "Pick" on `<start>..<restored tip>` (`check_pick`, `PickSource::Revisions`): a commit HEAD has by its
   patch is left out, a conflict names its commit and files (`ReapplyConflict`), local changes in the way refuse it
   (`MoveBlocked`), and nothing to pick refuses it (`AlreadyInHead`).
4. The move (`apply_reapply`, through `apply_pick_carrying`, `branch-deck: reapply`): its transaction also deletes the
   unapplied branch (checked against its tip) and the temporary ref; the rollback of files that changed since the dry
   run creates the unapplied branch again.

The copies keep their authors; the committer is the current user, the date as "Pick" sets it. The next sync builds the
virtual branch from them. The undo (`reapply:<previous head>..<new head>:<tip>:<unapplied branch>`) is the undo of a
move (`check_reapply_undo`, `branch-deck: undo reapply`), whose transaction creates the unapplied branch at its tip again.

Code: `crates/git-ops/src/reapply.rs`, `crates/git-ops/src/pick.rs`, `crates/sync-core/src/reapply_branch.rs`,
`crates/bd-update/src/unapply_command.rs`.

#### Pick

`bd pick` (`pick_core` in `crates/sync-core/src/pick.rs`, `check_pick` and `apply_pick` in
`crates/git-ops/src/pick.rs`) lands the commits of another worktree on the branch checked out here: the way agent
sessions that commit in worktrees of their own (a `bd lease` worktree, detached HEAD, shared git dir) land their work on
the shared branch without `git cherry-pick`, which writes the working tree, leaves `CHERRY_PICK_HEAD` behind on an empty
pick, and interleaves with a fold or an update of the owner.

1. The lock (one hold for both worktrees), then the interrupted moves: `finish_pending_update` in the target and, with
   `--follow`, in the source (a detached HEAD by the reflog of `HEAD`).
2. Guards: `check_update_guards` in the target; with `--follow`, no rebase, merge, cherry-pick or revert in progress
   in the source (the state files are per worktree). The source must be another worktree of the same repository
   (`repository_dirs`: the same common dir, another git dir).
3. The range: `<source HEAD> ^<target HEAD> ^<upstream> ^<base>`, read as `log --cherry-mark --right-only <target
   HEAD>...<source HEAD> ^<base> ^<upstream>` with `%m`: one process gives the range and, by `=`, the commits that a
   target commit above the base and the upstream has the patch id of. The base is `merge-base --fork-point
   refs/heads/<target> <source HEAD>`: the reflog of the target holds the commit the worktree was cut from, also after
   a fold, a regroup or new dates rewrote the target, so the target's old commits never come back. A source on a newer
   baseline (its merge base with the upstream is not in the target) is refused; without a fork point, a local shared
   history is refused too, and `<a>..<b>` names the commits. No merge in the range (`ensure_no_merges`).
4. A range whose oldest commit's parent is the target's HEAD is a fast-forward: the commits land as they are, no
   replay, and the source of `--follow` stays. Otherwise one `git replay --onto <target HEAD>` of the range, objects
   only, with `GIT_COMMITTER_DATE` set to the newest committer date of the range and of the target's HEAD (both read
   with what the check reads anyway: HEAD by one `log -1 --date=raw`, the range's `%P %cd` in its `log`), unless the
   user sets it. This is a stated deviation from invariant 5, as the fixed committer of the resolve session is: the
   committer is still the current user, but the same pick writes the same commits (the dry run reports the tip the
   pick writes, a pick a second later too), a copy is never older than its parent, and the worktree's dates carry
   over. A commit marked `=` is left out before the replay (`already_present`, `by: patch_id`): the target has it in a
   form that may no longer apply. The rest replays in segments between the left out commits, each `--onto` the tip
   the one before printed (`<last> ^<target HEAD> ^<base> ^<upstream> ^<the left out commit before it>`): `git replay
   --cherry-pick` is no fit, it replays the child of a filtered commit onto `--onto`, and the chain forks without a
   word. One `replay` without a left out commit, one per segment otherwise. A commit whose change the target has in
   another form becomes empty and drops: the copies are matched back to the kept commits by author, author time and
   subject, and the dropped ones are `already_present` with `by: empty`. Only when something is present, each gets the
   target's commit of the same patch id (`rev-list --cherry-mark --left-only` of the target's side, then
   `pair_by_patch_id`, shared with the resolve session). A conflict: the first conflicting commit by a binary search
   over the prefixes of the segment that conflicts, onto the tip before it (`replay::first_conflict`, shared with the
   resolve session), its files from one `merge-tree --write-tree --name-only -z` onto the longest clean prefix;
   nothing moves. The upstream bounds both sides: a commit that reached the upstream in another form is not found by
   patch id, it drops as empty or conflicts.
5. Both dry runs before either move: `blocking_paths` of the target (HEAD → new tip) and of the source (its HEAD →
   new tip), each on its own index copy and working tree.
6. How the picked commits land, from one read before the move (`landing` in `crates/sync-core/src/pick.rs`): the
   grouping inputs (`git config`, one `cat-file --batch`) and the commits ahead of the baseline (one `log`), grouped
   without and with the copies on top, give the `group` of each copy and `reparented`, a branch the target had whose
   declared parent a `(parent/leaf)` prefix of a copy changed. Only when a prefix names a branch the target has, the
   diffs of the commits ahead and of the copies (`read_commit_diffs_cached`, by id) give the edges by lines
   (`compute_dependencies`, no symbols): a copy whose branch is not built on each branch it needs is in
   `dependencies`, with the move into the branch of the stack that holds them all (`stack::stack_top`, shared with
   the conflict fix) or, when it needs its own branch too, the restack of its branch. Offered, never applied: the
   pick stays one guarded move, and `bd group` and `bd restack` have their own lock and undo.
7. The target moves through `move_branch_and_files` (`branch-deck: pick`, the mark, CAS, rollback), then the source
   through the same move, a detached HEAD with `option no-deref` and a check under the locks that HEAD is still
   detached (`HeadAttached` otherwise), so a `git switch` to a branch at the same commit can't move that branch. A
   source that moved or switched after the check stays where it is while the target moved: the outcome says so, and
   the same pick again finds every commit present and moves only the source.

**Undo** (`bd undo pick:<H>..<N>[+follow:<worktree id>:<S>..<N>]`, `undo_pick_core`): under the lock, both moves are
checked first as the undo of an update is (`check_pick_undo`, the last reflog entry of the branch, of `HEAD` when
detached, must be `branch-deck: pick` to N with the value before it at H or S; the dry run with the trees swapped),
then both move back (`branch-deck: undo pick`). The source moves back too: otherwise its commits would no longer be in
the range, since the fork point of N is N. The worktree path comes from `$GIT_COMMON_DIR/worktrees/<id>/gitdir`.

Code: `crates/git-ops/src/pick.rs`, `crates/git-ops/src/update_branch.rs` (the moves, `HeadRef`, the undo check by
operation), `crates/sync-core/src/pick.rs` (lock, baseline, grouping, dependencies, the undo token), `crates/bd-update/src/pick_command.rs`.

#### Lease

`bd lease` (`crates/sync-core/src/lease.rs`, the git of a slot in `crates/git-ops/src/lease_slot.rs`) keeps a pool of
linked worktrees, one agent session per slot. It replaces the Treehouse pool for the IntelliJ monorepo (1.4M tracked
files). There an acquire took 20 minutes or more, for two reasons measured on 2026-10-19:

- Treehouse reuses a free slot only when its HEAD is an ancestor of `master`, and parks a returned slot at the local
  `master` tip. `bd pick`, `bd update` and rebases rewrite that `master` often, so a parked commit soon becomes
  unreachable, every free slot looks like unlanded work, and almost every acquire adds a full checkout.
- It runs `git status --untracked-files=all` and three `git ls-tree -r` in every free slot on every acquire, and
  `treehouse status` runs `git status` in every slot: about 7 minutes for 10 slots, 33 s for one.

The pool state is the authority instead: `$GIT_COMMON_DIR/branch-deck/leases.json`, written as a temporary file and a
rename. A free slot is pool property, and a get replaces its content, so no ancestry is checked. The pool lock is
`leases.lock` next to it, not the repository lock: a get never waits for an update, and the lock is held only while
the state is read and written, never across a git that writes a slot.

1. Get: under the pool lock, a `preparing` lease whose process is gone (its pid and start time, `process_scan`) is
   freed, and a free slot whose directory is gone is dropped. The first free slot that is a linked worktree of the same
   common dir (its `.git` file and admin directory, no git process) and has no process in it is reserved, clean slots
   before the ones marked dirty, and among them the one returned last first (`returned`, the number of its return in
   the pool), then the ones never returned in the order they joined. The slot returned last has the warmest index,
   fsmonitor daemon and build caches and the commit closest to the source; by the order they joined alone, the first
   slot served every lease and the others sat idle for days, cold and thousands of files behind. The lease is recorded as `preparing`, with the pid and its start time, and the lock
   is released.
2. Without the lock: one `status --porcelain=v1 -z` of that slot, in the untracked mode below, which may write the
   slot's index (below). A dirty slot is freed again,
   marked dirty and skipped, and the next one is reserved. A clean one gets `checkout --force --detach <source HEAD>`
   (or `-b <branch>`), which writes only the paths that differ. `--force` is safe after the clean status: on a
   case-insensitive file system `status` does not see a case-only rename, and a plain checkout then refuses.
3. No free slot: a new slot at `<pool>/<n>/<repository name>`. The leaf keeps the name of the repository, which tools
   derive identity from. On macOS it is an APFS clone of a template (below); elsewhere, or when the clone fails,
   `worktree prune`, best effort, then `worktree add --detach <path> <source HEAD>`, a full checkout. The output says
   which: `created_by` `clone` or `checkout`.

Both checkouts run with `-c checkout.workers=0`, one worker per core, unless the repository config sets
`checkout.workers` (one `config --get-regexp` reads it with `branchdeck.leasePool`): git's default of one worker
checked out 43,891 files of idea-1 in 12.2 s, 18 workers in 4.2 s. Every `status` of the pool (the get, the check of
a clone, the return, an adopt) lists untracked files with `--untracked-files=all` when `status.showUntrackedFiles` is
`all` (the same `config` read), else with `normal`, never `no`, which would hide them. git's untracked cache keeps the
mode it was built for, is used only in that mode and is built only in the configured one; idea-1 configures `all`,
and there the `status` with `normal` walked every directory: 41 s against 2 s in the same slot. The return reads the
config for it, one `config` more.

That `status` is the one git of Branch Deck that may write an index the executor would leave alone: it runs with
`GIT_OPTIONAL_LOCKS=1`, so the refreshed stat data, the fsmonitor token and the untracked cache stay in the slot's
index. With the executor's `0` nothing stayed, and a slot of idea-1 idle for days walked its whole tree on every
`status`, 2-5 min each, and its checkout without changes lstat'ed every file (4-10 s); after one write a `status` took
2 s and the checkout 1-3 s. The index is the slot's, and no other process works in the slot during a get, a return or
an adopt (the process scan). git still leaves a directory changed in the second of the write out of the cache.
4. Seed: the ignored paths that `.worktreeinclude` at the source root names are copied into the slot. A pattern is a
   literal path or a glob in its last component, read from that one directory, never a walk of the tree; one
   `check-ignore -z --stdin` in the source keeps only the ignored ones.
5. Under the lock again the lease becomes `ready`, with the seeded paths. A failure after step 1 gives the slot back
   (free, or dropped when it was never added), and the error says so.

A new slot by a clone (`add_cloned_slot`, `crates/sync-core/src/clone_dir.rs`). Measured on idea-1 (1,387,212 index
entries): one `clonefile(2)` of a whole slot took 40 s, the refresh of the copied index 14 s, and a checkout of the
same tree takes several minutes.

1. The template is a free slot of this repository that the get passed over (dirty or busy, one without a process
   first), marked `templating` under the pool lock, so no get takes it during the clone. Without one it is the source
   checkout. A free slot is the better template: an ignored file that the template writes during the clone can be
   copied half written, and the source is where the user works.
2. `clonefile(2)` of the template's top level to `<path>.cloning`, then the marker is released. The destination must
   not exist and must be on the same volume; any error (`EXDEV`, `ENOTSUP`, `EACCES` of a file it can't read) removes
   what is left and falls back to `worktree add`.
3. `worktree add --no-checkout --detach <path> <source HEAD>` writes only `<path>/.git` and the admin directory. That
   `.git` file replaces the cloned `.git` (the whole git directory of a main checkout: 0.2 s for the 2,079 files of
   idea-1's), the empty `<path>` goes, and `<path>.cloning` becomes `<path>`.
4. The template's `index` is copied into the admin directory and refreshed with `update-index -q --refresh` under
   `core.checkStat=minimal`, `core.trustctime=false` and `core.fsmonitor=false`. A clone keeps the mtime and the size
   of every file, but its inodes and ctimes are new: the default check rehashes every file (288 s on idea-1, against 14
   s). The refresh writes the new stat data, so every later git runs with the default check.
5. `checkout --force --detach <source HEAD>` (parallel, as above) writes the diff from the template's index, `clean -fd`
   removes the untracked files of the template, and one `status` must find nothing. The template's changes, staged
   files and untracked files never reach the lease; its ignored files stay as the warm cache. The seed runs as for
   any get, so the return removes the seeded files also when the source template carried them.

Return: one process scan, then `log HEAD --not --branches --tags --remotes <leased commit>` (not `--all`, which holds
the HEAD of every worktree; the leased commit keeps out what the slot started with, which a rewrite of master may have
left unreachable), then one `status`. When `log` finds commits, the question is whether the branch the source had
checked out at the get (recorded with the source in the lease) has every change of the slot. After `bd pick
--follow` and a rewrite of that branch (`bd update`, a fold), the slot's HEAD is reachable from nothing, yet every
commit is there by content. So a dry run of the pick decides (`commits_to_pick` in `crates/git-ops/src/pick.rs`): the
fork point of the slot in the reflog of the branch, `git replay` onto its tip, and only the commits that do not drop
are unlanded. It runs on git objects only: `check_pick` needs a worktree on the branch, its index for the dry run of
the files, and the guards, none of which a return may take from the shared checkout. Reachability stays the fast
path, and decides alone without a recorded branch, when the branch is gone, or when the pick can't tell
(`range_unknown`, a merge in the range); the output names the check (`landed_check`).

git's background maintenance keeps no lease either. A commit in a slot starts `git maintenance run --auto --detach`;
it and its git children (`git repack -d -l --geometric=2 --write-midx`, `git multi-pack-index write`) keep the slot
as their working directory for about 30 s in idea-1, and write only the shared object store. The return waits up to
60 s for them (a `git maintenance` or a `git gc --auto` by argv, and every git below it in the parent chain), then
goes on; one still there is `slot_busy`. A get does not skip a free slot for them.

A Bazel server of the slot (its argv has `--workspace_directory=` the slot or a directory in it, `community/` in
idea-1) keeps no lease: Bazel derives its output base from the workspace path, so it serves only that slot. Once
nothing else refuses, the return sends it SIGTERM, waits up to 30 s and names it in `stopped_servers`; the output
base stays, and the next lease of the slot finds its disk cache. No other process is signalled, and any other process
in the slot refuses the return before a server is. A get never reuses a slot under a live server: a `--watchfs`
server can miss the events of a large checkout. Unlanded commits, local changes and a process in the slot refuse it; with
`--confirm-preserved` the changes go with `reset --hard HEAD` and `clean -fd`, and the ignored build outputs stay
for the next lease. The seeded paths are removed, and a branch HEAD is on is detached with a `RefTransaction` (`option
no-deref`, `branch-deck: lease return`). The slot stays at its commit: the next get moves it.

The clean rules (`crates/sync-core/src/lease_clean.rs`). Cargo never deletes a unit whose hash went out of use, so the
free slots of Branch Deck held 679 GB of stale units, which a prune script nobody ran left alone. The return now cleans
its slot by the `[lease] clean` rules of the committed `.branchdeck.toml`, read once from the source the lease
recorded (the slot's own when the source is gone), after every check passed and before the slot is freed. A rule is
`cargo` (the stale units of the profile dirs of a target dir: the build-dir layout of Branch Deck, `build/<pkg>/<hash>`,
and the classic one of idea-1, a unit `<name>-<16 hex>` across `deps/`, `.fingerprint/` and `build/`, whose files go
together; a dry run on the idea-1 checkout found 27.9 of the 33 GB of `community/tools/vm/target` stale) or
`delete` (a path); no command, as a return must not run arbitrary code. A path is normalized and relative, stays in the
slot, names no `.git`, crosses no symlink, and git tracks nothing under it (one `ls-files -z` of all paths): a rule that
breaks one is reported and skipped. A rule error never fails the return. `bd lease clean` applies the rules on
demand, skipping a slot with a lease or a process. The rules are configured on purpose, so both delete by default (the
user's decision): `--dry-run` only reports, `bd lease return --no-clean` applies none, and `--keep` and `--age` set the
parameters of every cargo rule for one call.

Status reads the state file only: one `rev-parse` and one `config` (`branchdeck.leasePool`), no git in any slot.
Adopt registers existing linked worktrees (old Treehouse slots) where they are, after one `status` of each. With
`--move` each goes to the next slot path by `git worktree move`, which fixes both links of the worktree, under the
pool lock (a rename on one volume, so the lock is held for a moment; another volume, by `st_dev`, refuses, since git
would copy every file). The empty parent of the old directory goes; the Treehouse state is not touched. Bazel derives
the output base from the workspace path, so a moved slot starts with a cold output base.

The process scan (`crates/sync-utils/src/process_scan.rs`) reads the working directory and the argv of every process of
the user: on macOS through libproc (`proc_listallpids`, `proc_pidinfo` with `PROC_PIDVNODEPATHINFO`) and
`sysctl(KERN_PROCARGS2)`, on Linux from `/proc/<pid>/cwd` and `/proc/<pid>/cmdline`. It leaves out `bd` and its ancestors, and compares canonical paths by components. Windows has no
scan: the outputs say `processes_checked: false`, and a lease never waits for it.

Code: `crates/git-ops/src/lease_slot.rs`, `crates/sync-core/src/lease.rs`, `crates/sync-utils/src/process_scan.rs`,
`crates/bd-lease/src/lease_command.rs`.

#### Virtual branch sync

Virtual branches never touch HEAD, the index or the working tree.

A sync starts with the guards of a rewrite without the merge check (`check_sync_guards`, one `rev-parse --git-path`
for the operation markers and one `symbolic-ref HEAD`): while git is in the middle of a rebase, merge, cherry-pick or
revert, or HEAD is detached, the history of HEAD is not the branch the user works on, and a sync would archive the
virtual branches of the groups the half-done history lacks. It is refused before any event as `SyncDeferred`: the app
names it by `CommandErrorKind` (`operationInProgress`, `detachedHead`) and shows an `info` toast for a sync the user
asked for, nothing for one it started itself; `bd sync` answers `operation_in_progress` or `detached_head` (exit 4).

**What a sync reads once** (`RepoFacts`, `crates/sync-core/src/repo_facts.rs`), after the guards, for every step that
needs it: `remote`; one `for-each-ref --sort=-committerdate` of the checked out branch (its upstream: `%(upstream)`,
`%(upstream:remotename)`, `%(upstream:remoteref)`, `%(upstream:short)`), the baseline candidates (`<first
remote>/master`, `/main`, or the local ones) and the branches under the prefix, local (`%(worktreepath)`,
`%(contents:subject)`) and on `origin`; and one `rev-parse --git-path FETCH_HEAD --git-path branch-deck/landed.json
HEAD^{commit} <baseline>^{commit}` (the landed state of the worktree, which the update check records, is no process of
its own).
The baseline is found by the rules of `detect_baseline_branch` (the upstream when it exists, else the first candidate
that does; an upstream that is no candidate costs one `rev-parse --verify` more), its upstream by those of
`resolve_baseline_upstream`. The update check (`check_known_baseline_update`, through `check_update_of` without its
guards and its `rev-parse`), the archive step (`archive_and_clean_up_listed`: only the detection notes are read), the
placement of stacked branches (`empty_parent_states_listed`: only the notes) and the integration detection take these:
one baseline commit for the whole sync, also when a fetch moves the baseline meanwhile, so the remote statuses, the
detection and the update check compare with the same commit.

**The unapplied branches** (`{prefix}/unapplied/<name>`, "Drop and unapply") are in the same listing. A sync never
writes them; after the stack branches it sends them once, also an empty list, in `UnappliedBranchesFound`: the name,
the tip, its subject, the worktree that has it, and the remote status of one whose remote branch exists, from the same
`RemoteStatusMemo` (the push times include them); one without a remote branch costs no process.

**What a sync returns** (`SyncReport`): the baseline, its commit, HEAD and the checked out branch as it read them, the
grouping inputs (the stack setting, the issue groups, the stack parents), the grouped branches as `BranchesGrouped`
sent them (with their placements), the stack forest, the remote tips on `origin`, the descriptions, the tip of
each grouped branch's virtual branch after it (the head it wrote or kept, another process's tip after
a lost CAS, the old tip of a branch it could not rebuild), and the stack branches as their `StackBranchUpdate` events
sent them (see "Stack branch"). The app takes the events; `bd sync` takes the baseline, the
branches and the tips from the report (`SyncOutput::report`), so a command that goes on after the sync (`bd push`,
`bd safe-push`) need not read them again.

1. Each group's commits are copied onto the baseline one after another. A commit whose parent tree did not change
   reuses its tree. Otherwise it is merged, and a conflict resolves by words (see "Resolve by words") or produces the
   conflict view.
2. **One merge and one commit process per branch** (`crates/git-ops/src/commit_chain.rs`). A cherry-pick needs only
   trees: the commit's parent tree, the commit's tree and the tree of the copy below it. So the merges of a branch go
   to one `merge-tree --write-tree --stdin -z --no-messages`, a line `<base tree> -- <target tree> <commit tree>` per
   commit, each answered before the next is sent. The created commits go to one `fast-import --date-format=raw`, which
   answers each id at once (`get-mark`), so the reuse checks and the next commit have it. The grouping prefix is
   stripped from the subject (a prefix that is an issue key stays as the key: `(IJPL-1) docs: x` is copied as
   `docs: IJPL-1 x`, see [Virtual Branches](virtual-branches.md#grouping-rules)) and `branch-deck` is the committer. The commit object has the bytes `commit-tree -m`
   wrote before: the message completed with a newline, name and email without git's crud, a date without a timezone
   in UTC, no `encoding` header and no signature (with `commit.gpgSign`, `commit-tree` signed the copies, and a
   signature carries its time, so a copy made again got another id). A commit copied again therefore has the id it had, and the virtual commits of a repository stay as
   they are across the change. The writer's commits are readable by other processes only after it ends: it ends
   before a conflict is analysed (the conflict is merged once more with the commits, for the conflict view, unless the
   object cache remembers the view: see `conflict_info` in "Object cache") and when
   the branch is built, before its head goes to the branches built on it. Its ref is reset before `done`, so the
   commits are on no ref until the sync's transaction moves the branch, as with `commit-tree`; a chain dropped on an
   error still ends `fast-import` with `done`, which would otherwise leave a crash report in the git directory.

   **A conflicting branch keeps its copies before the conflict** (`crates/sync-core/src/conflict_chain.rs`). When
   commit k of a branch conflicts, the branch keeps its last good tip (an older build: the report names the branch in
   `not_built`, nothing pushes it, and the repository view takes no tip of it, `BranchTable::tip_of`), and the copies of commits 1..k-1 are on no
   branch, yet the conflict's merge, and its view, are onto the copy of commit k-1. The sync keeps that copy, and with
   it the copies below it, on `refs/branch-deck/conflict/<sha256 of the branch's full name>` (a hash, so the refs of
   a branch `a` and a branch `a/b` never clash and a rename is a delete and a create): `git gc` doesn't prune them,
   and the view of the conflict opens from them while the branch conflicts, with or without the object cache. No ref
   is kept when the conflict is at the first commit or the copy of commit k-1 is on the virtual branch already, both
   on a ref. The object cache remembers the chain (`conflict_chain`), and the next sync of the unchanged branch takes
   the copies (`RememberedCopy` of `copy_commit`, their trees into the tree cache, the status and the note as a copy
   written again would get) instead of merging and writing them: its chain starts no process, and the remembered
   view of the conflict needs no merge. Only while the copies are kept: the ref is at the last copy, or the last copy
   is on the virtual branch. Only a sync: a check rewrites messages under the same commit ids.

   The refs are listed by the sync's one `for-each-ref` and moved by one transaction per sync, after the branches,
   with CAS on the listed tips, only when one changes: created or moved for a branch that conflicts after commit 1,
   deleted for a branch that built, conflicts at its first commit or has the copy on its branch, and for the refs of
   the branches no longer grouped; the ref of a branch not built for another reason (blocked, a stack problem, an
   error) stays. A failed transaction fails no sync: the refs are listed again, and a ref another sync already moved
   where this one wants it (the copies are deterministic) counts as moved; the rest waits for the next sync. The
   fingerprint of the automatic sync doesn't include `refs/branch-deck/`, so the move starts no sync; the chains show
   in `git log --all`.
3. Known trees are seeded into the tree cache (`%T` of the listed commits, the tree of each new commit), and the
   cherry-pick takes the parent from the commit list instead of asking git.
4. A branch is moved with an `update-ref --stdin` transaction, and its `original → new` mapping notes are written as
   one notes commit by one `fast-import` (`N inline <oid>` on the default notes ref, `from <ref>^0`). The transaction
   is a CAS on the tip the sync listed (`update <ref> <head> <listed tip>`, `create` for a new branch): the app's sync
   and `bd sync` don't share the repository lock. When it fails, one `rev-parse --verify` reads the ref again: at the
   head the sync built, another sync wrote the same deterministic copy, and the branch counts as written; at another
   tip, another process moved it, and the branch keeps that tip and reports `Error` without notes, for the next sync;
   at the listed tip, the failure is not about the old value (a ref of the name in the way) and fails the branch's
   task as before. One transaction per branch, in its finish task, so its status and its remote status don't wait for
   the other branches.

**The report of a conflict** (`build_merge_conflict_info`, `crates/git-ops/src/merge_conflict.rs`): the commits, each
conflicting file with its number of conflicts (the `<<<<<<<` lines of the merged file, which the report counts and
does not keep), and the missing commits, the older commits of the source that change a conflicting file and are not
in the target (`MergeConflictInfo`). The views read the merges and the diffs from the engine (`get_file_merges`,
`get_file_diffs`, [Diff View](diff-view.md)), not from the report. A view of
a conflict in big files is megabytes (a 5.6 MiB file: 28 MiB of JSON), and the sync sends each conflict twice (the
error of the commit and the status of the branch) at every run; without the contents an event is about 1.3 KiB
(`a_sync_reports_a_conflict_without_the_contents_of_its_files`). After the merge with markers (`merge_with_markers`,
one `merge-tree`), four processes, whatever the number of files and missing commits:
1. One `rev-list --left-right --boundary <parent>...<target>` (`find_divergence`) counts the commits of each side, and
   its boundary, the commits outside both sides that a commit of a side has as a parent, holds every merge base (the
   child of a merge base on the way to a side is on that side), so a boundary of one commit is the merge base. Any
   other boundary (criss-cross merges, unrelated histories) asks `merge-base` once more; without a merge base there is
   no analysis.
2. One `log --no-walk --stdin` reads the parent, the target, the commit and the merge base.
3. One `log -z --name-only --no-renames --no-merges <parent> ^<target>` lists the missing commits with their whole
   messages and the conflicting paths they change, NUL-separated and never quoted (`%B` is the
   last field, so a body line is never read as a path).
4. One `cat-file --batch` reads the conflicting files in the merge tree, as texts, to count their markers: a binary
   file and one over 8 MiB (`MAX_BLOB_BYTES`, its bytes skipped) count none.

The conflicting paths go to the log of the missing commits as names: `--literal-pathspecs`, so `a*.txt` matches no
`ab.txt` and `:name` is no pathspec magic. `the_report_of_a_conflict_costs_four_processes` pins the count.

Before the copies, a sync with branches takes their descriptions (`bd describe`) from its `ConfigSnapshot`, the one
`git config` of the sync that also gives the stack setting (see "Repository settings"; `bd` reads them with
`crates/git-ops/src/branch_description.rs`), and sends the title of each with the grouped branches.

Code: `crates/git-ops/src/copy_commit.rs`, `crates/git-ops/src/cherry_pick.rs`, `crates/git-ops/src/notes.rs`,
`crates/sync-core/src/branch_processor.rs`. The integration-detection cache writes its notes in one `fast-import` as
well (`crates/branch-integration/src/detection_notes.rs`).

#### Resolve by words

A conflict of git whose every conflicting file the engine merges without a conflict that its words leave resolves, as
IntelliJ's merge viewer resolves the simple conflicts (`resolve_by_words`, `crates/git-ops/src/word_resolve.rs`). All
or nothing: one file that does not resolve, or one conflict that is not of the contents of a text, and the
cherry-pick stays the conflict of git.

- **Which conflicts.** A path with the stages 1, 2 and 3, regular files whose modes merge as git merges them (a side
  that changed the mode wins; two other changes stay a conflict), only `CONFLICT (contents)` messages, the `merge`
  attribute unspecified, set or `text` (as git finds it for a merge: `check-attr` without `--source`), and UTF-8 text
  without NUL on every side. An add/add (no stage 1), a modify/delete, a rename/rename, a rename/delete, a
  file/directory and a binary file (`CONFLICT (binary)`, also by `merge=binary`) stay conflicts.
- **The merge.** `merge_texts` of the stages (left: stage 2, ours, the target; base: stage 1; right: stage 3, theirs,
  the commit) under `Default`, without a deadline, so that every machine resolves alike; the size limits of the
  engine (`MAX_INNER_LINES`, `MAX_INNER_BYTES`) still hold, and a file over them stays a conflict. Every block must
  resolve: a side that changed alone, the same change of both, or a conflict of both that the words resolve
  (`MergeResolveUtil.tryResolve`, the simple resolve; the greedy one stays off).
- **The result.** `compose_merge` of the three blobs, which must keep every line end (`exact`, see [Text Diff
  Engine](text-diff.md), "Merge"). It replaces the file with the markers in the tree of the merge, with the merged
  mode; the rest of the tree is git's, renames and clean files as git placed them.
- **The tree id** is computed without a process: the trees on the paths of the resolved files, read with the stages,
  each entry replaced where it is, so the order stays git's, and hashed bottom-up (`object_id_of`, SHA-1 or SHA-256 as
  the repository). The blobs and the trees exist only once the commit that holds them is written.
- **The count.** The conflicts of a file are those of git: the `<<<<<<<` lines of the file with the markers.
- **Processes.** One `check-attr` and one `cat-file --batch` (the stages, the files with the markers, the trees),
  none when a path fails before; the merges stop at the first file that does not resolve.
- **`RESOLVE_VERSION`** names the way of the resolve: bump it when the merge, the resolve or the composition can give
  another result.

In the sync (`perform_fast_cherry_pick_with_context`, `crates/git-ops/src/cherry_pick.rs`), with
`branchdeck.resolveByWords` on (the default; `ResolveMode` of `ConfigSnapshot::resolve_mode`):

- **Where.** A conflict of the chain's `merge-tree --stdin` is merged again with the commits and its messages
  (`merge_with_markers`, as before). The resolve runs on that output, before the analysis; a resolved commit has no
  analysis (four processes fewer) and costs one `check-attr` and one `cat-file`. One that does not resolve goes on to
  the analysis as before.
- **The commit.** The author, the committer, the date and the message are those of any copy; only the tree differs.
  The chain's `fast-import` writes the resolved files inline onto the tree of the merge (`M <mode> inline <path>`),
  answers the tree it built (`ls :<mark> ""`), which must be the computed one, and makes it readable for the next merge
  of the chain with a `checkpoint`, whose end a `progress` line answers. The commits above it merge onto it as onto
  any copy.
- **What is told.** `CommitSynced.resolved` (`WordResolutionSummary`): the conflicts of git and the files, each with
  its conflicts; a copy reused by its note has the counts only. The mapping note keeps the counts (see "Virtual commit
  reuse"); the commit message says nothing of it.
- **The same verdict everywhere.** The trial builds of `bd check`, of the conflict fix and of the stack branches build
  through the same cherry-pick. The memo of a check remembers each resolve (`word_resolution`), a resolved tree only
  while it exists, as a clean merge. The keys of the conflict views, of the chains of a conflicting branch and of the
  stack builds hold the resolve mode, and the chain keeps the resolve of each copy for its note.
- **The same commit on every machine.** No deadline, the size limits, the committer date of the inputs: the same
  originals on the same base give the same copy.

In Update Project and `bd update`, with the same setting (`crates/git-ops/src/word_replay.rs`):

- **The check.** When the replay of the local commits conflicts, also without the landed candidates, the check
  replays them again commit by commit (`replay_resolving_words`). Each run that applies is one `replay`; a binary
  search finds the commit that conflicts; its `merge-tree --write-tree -z --merge-base <c>^ <tip> <c>` is resolved,
  and one `fast-import` writes the commit with the author and the message of `c` and the committer of the replay
  (a preview has the fixed date). A commit that the words leave without a change is dropped, as `replay` drops one. The
  first commit that does not resolve makes the check a conflict, as before. All or nothing: the check is `Clean` only
  when every conflicting commit resolves.
- **What is told.** `UpdateCheck.resolved_by_words` lists the resolved commits with their subjects, conflicts and
  files. The replay key of the preview cache holds the mode, so a preview of the other mode replays again. The app says
  it in the tooltip of Update ("Rebases onto origin/master, resolving 1 conflict by words"), in the toast and in
  Activity; `bd update` in `resolved`.
- **The apply** moves the branch as any clean update, so the undo of Update Project restores it. Then it writes a note
  per resolved commit on `refs/notes/branch-deck/resolve` (`branch-deck resolve`, `commit: <c>`, `resolved-by-words:
  <version> <conflicts>`, a `words\t<path>` line per file); a failure only leaves the notes out.
- **The rounds of a session.** In a resolve session (below), a conflicting commit that its words resolve is a round
  that resolves itself: it counts in `round`, its resolution has `words` and the outcome `words` per path, and the
  replay goes on. The agent sees only the rounds that do not resolve. `start_session` gets the mode with the check,
  `continue_session` from the config at each step.
- **The cost.** A check that conflicts in a commit whose words do not resolve replays the prefixes once more: one
  binary search, then the session searches again. The trial without the words is unchanged.

#### Stack branch

With stacked branches on, a stack that forks (a branch of it has two children or more) also builds as one more virtual
branch, `{prefix}/stack/<root>`: every commit of its branches, in the order of the checked out branch's history, on
the base of the sync, as a flat virtual branch of those commits would be built. A Safe Push of the whole stack lands
it in one robot ([Whole Stack](../../spec/branch/whole-stack.spec.md), [Landing](landing.md)); a straight chain has
none, its leaf is the whole stack. `crates/sync-core/src/stack_branch.rs`.

- **Which branches.** The tree of a stack follows `effective_base` (a branch built past a parent without commits is in
  the stack of the branch it is built on), the parent that blocks a blocked branch, and the declared parent of a branch
  with a stack problem: every member's virtual branch has its ancestors' commits, so the stack branch has every piece.
  A branch whose parents lead into a cycle is in no stack. The plan (`plan_stack_branches`) is a function of the
  grouping and the order of the commit list: no git.
- **The history order**, not the main line first: a new commit on a side branch would turn the stack branch over and
  rewrite all of it, and a build in another order than the local history can conflict where nothing conflicts locally.
  The commits of the main line keep no ids by being first: Patronus rebases the source branch with
  `REBASE_AUTOSQUASH` anyway.
- **One chain, no notes, no branch events.** The build runs `create_or_update_commit` for each commit in one
  `CommitChain` (one `merge-tree --stdin`, one `fast-import`), with the deterministic committer dates, so a copy is the
  commit a flat build of the same commits on the same parent writes. Its mapping notes are dropped: the one note of an
  original names the copy of its own branch. It sends no event of a branch row (`CommitSynced`, `BranchStatusUpdate`, …),
  only `StackBranchUpdate`. A conflict is reported by its commit and files (`MergeConflictInfo::light`, a tree cache
  with the `LightConflicts` memo), without the analysis of a conflict view.
- **Built beside the branches.** The builds run as tasks of the sync's `ProcessPermits` beside the builds of the virtual
  branches, from the same tree cache (the trees of the commit list, the date of the base). Once the branches are built,
  one blocking task moves each stack branch (`move_branch`, CAS on the listed tip), computes its remote status on the
  sync's `RemoteStatusMemo` (its remote tip is read with the others) and sends the updates.
- **Unchanged without a process.** The build is a function of the base and the ids of the original commits in their
  order (a commit id covers its tree, parents, author, committer and message), so its outcome is remembered by a hash
  of them (`StackBuilds`): in the memory of the process, else in the object cache (`stack_branch`). A sync whose stack
  has the same commits on the same base takes the tip from it and runs no git for the stack branch
  (`an_unchanged_sync_of_a_forking_stack`); without the object cache a new process builds it again, to the same commit,
  and the ref does not move. Not by the identities of the commits (`CommitKey`): an amend keeps the author, the time
  and the subject, and the stack branch would keep the old content.
- **States.** `Built`: the ref is the build of the stack's commits now. `Conflict`: a commit conflicts with the commits
  of the stack before it. `Blocked`: a member has a stack problem, is blocked, conflicts or failed (the first in stack
  order, a failure before a block, `StackMemberBlock`) while the stack branch's own build is clean; that build is
  remembered but not written. The stack branch's own conflict comes first: it is a function of its commits, so
  `bd status` knows it without a sync, and the member's row says its own. `Stale`: the ref was not rebuilt for another
  reason (another process moved it, the build failed, its stack no longer forks). In every state but `Built` the ref
  keeps its tip, and no push may send it: only a `Built` tip is the stack's commits as they are.
- **Deletion.** A stack branch whose stack no longer forks (its root has no branch with two children, the root is gone,
  stacks are off) is deleted in one transaction with CAS on the listed tip (`branch-deck: delete stack branch`), best
  effort: a failure leaves it for the next sync. One with a remote branch stays, `Stale` without members, for the user
  to decide on (its pull request may be open); so does one a worktree has checked out. `BranchesGrouped` names the
  roots of the stack branches the sync reports (`stack_branches`), so the app drops the others at once.
- **Not a branch of the archive.** The listing of the sync has the stack branches (`read_baseline_listing` lists every
  namespace under the prefix), but the archive step and the integration detection leave them out (`branch_data_of`):
  a stack branch is never archived, and its tip must not name the detection note of a branch that is.
- **`bd status`** reads the state without a sync (`read_stack_branches`): blocked by the placement, else the
  remembered build of the current commits when the ref is at it, else stale; no git.

#### Sync inputs check

Auto-sync on Focus syncs only when what a sync reads changed since the last sync (`sync_inputs_fingerprint`,
`crates/sync-core/src/sync_inputs.rs`), so it is on by default: a focus of an unchanged repository costs two git
processes and three file stats, no object read (three processes when HEAD's branch tracks another remote branch than
a `master` or `main`).

1. `rev-parse --path-format=absolute --git-dir --git-common-dir`: where `config` and `config.worktree` are.
2. One `for-each-ref --format='%(HEAD)%(objectname) %(refname)%(if)%(HEAD)%(then) %(upstream)%(end)'` over the local
   branches without the virtual branches of the prefix (`--exclude=refs/heads/<prefix>/`, which the sync writes
   itself), the `master` and `main` of every remote (the baseline a fetch moves), the remote virtual branches of the
   prefix (`refs/remotes/*/<prefix>/virtual/**`, a push from elsewhere; `**`, as a glob matches the whole name and `*`
   no slash) and the issue groups, the stack parents and the parked branches (not the rest of `refs/branch-deck/`: a
   review fetch writes there). `%(HEAD)` marks the branch HEAD is on, with its upstream, which `@{u}` makes the baseline.
3. Only when that upstream is not in the listing (`origin/develop`): `for-each-ref --format=%(objectname) <upstream>`,
   so a fetch of it is a change too.
4. The size and the modification time of `config`, `config.worktree` (descriptions, `branchdeck.stacks`, the
   upstream configuration) and `.idea/vcs.xml` (issue links).

The hash of these and the prefix is the fingerprint. The page reads it before each sync, so a commit made while the
sync runs is a change, and keeps it with the result; a focus compares it. Not in it: the notes the sync writes, the
working tree (the paths that block an update are read by the next sync).

#### Virtual commit reuse

A sync reuses a virtual commit when the original's mapping note points to it; otherwise it copies the original again.
- **Reuse checks the parent.** The mapped commit must be in the existing branch with the new parent as its first
  parent (`log --format=%H %P %T %an %ae %at %B <branch> ^<base>`, one call per branch). A commit added below, or a new
  base, copies the rest again. A branch whose commits are all reused still moves if its old tip had more commits.
- **Reuse checks the source.** A note is `v-commit-v1:<copy>` and `v-commit-source-v1:<parent tree> <tree>`, the trees
  of the original it was copied from; the lines are read by their prefixes (`notes.rewriteMode=cat_sort_uniq` sorts
  them), and older binaries read the first. `git notes copy`, `git rebase` and `git commit --amend` with
  `notes.rewriteRef` carry a note to a commit whose trees or message changed: a source that is not the commit's trees,
  or a copy whose message (the subject without the prefix and the body) or author (name, email, date) is not the
  commit's, makes the note stale, and the commit is copied again (`warn!` "mapping note is stale"). The trees come from
  the commit list (`%T` of every commit ahead of the baseline seeds the tree cache) and the listing of the branch, so
  the check runs no git.
- **A resolve by words is in the note.** A copy whose conflict of git resolved by words has a third line,
  `v-commit-resolved-v1:<resolve version> <conflicts> <files>` (`mapped_resolution`); the reuse gives the commit row
  its counts, and `mapping_lines` copies it with the other two. With `branchdeck.resolveByWords` off, such a note is
  stale, so the commit is copied again and gets the conflict of git. A note of another resolve version is taken as it
  is while its source holds, also with `verify_mapping_notes`: the copy changes only with its inputs, not with an
  upgrade of the engine.
- **Older notes are checked by the tree.** A note without the source line, written before it, or any note with
  `SyncOptions::verify_mapping_notes` (`bd --no-cache`), is checked against the copy itself: the copy's tree must be
  the tree a copy would get (the original's tree on the same parent tree, else the merge the copy runs). A match keeps
  the copy and writes the note with its source, so an older copy (another committer date, from before the
  deterministic dates) is not replaced and its branch doesn't move; the cost is one merge per commit, once. A re-copy
  that writes the commit the note names is `unchanged` too.
- **Deterministic copies.** The committer date is the newest committer date of the original, the originals below it
  and the base (the base's date is read with its tree, once per sync before the builds start, not by each build that
  starts on it). The same originals on the same base give the same ids, even
  after the notes are lost. Both dates keep the original's timezone (`%aI`, `%cI` of the commit list), so the copies
  don't depend on the machine's timezone either. Copies written before these dates (89960d7, 2026-10-14) have other
  committer dates; the check by the tree above keeps them.
- **Rewords carry the notes.** A reword or move recreates every commit after the oldest reworded one, and its
  `fast-import` stream also writes the old commits' mapping notes onto the new ones (`N inline :<mark>`), both lines:
  the trees stay, so the source still holds. A reworded
  commit keeps its note only if its virtual commit stays the same: the subject without the grouping prefix and the
  body are unchanged, as in a move. Without this, each move would rebuild every virtual branch after it.
- **Bounded walk.** A reword reads only the first-parent history ahead of the baseline, notes included, with one
  `log --first-parent --boundary HEAD ^<baseline>` (the whole history on the local baseline branch, as the commit list
  does), and refuses a commit outside it; amend, drop and squash read down to their oldest target (see "Rewrite
  history").
- **One listing, one notes write.** A sync lists the branches under the prefix once, with its other refs
  (`RepoFacts`): the virtual tips decide reuse, the unapplied and archived names give the state of a stack parent
  without commits, and the archive step and the detection take the same listing. The branches return their mapping
  notes, and the sync writes them in one notes commit at the end; a note lost to a failed sync only costs a copy.
- **Archive before the rebuild.** Before any virtual branch is rebuilt, one transaction deletes the old integrated
  archives (`branch-deck: clean up archived branches`) and one moves the virtual branches no group has into
  `<prefix>/archived/<today>/` (`branch-deck: archive`: the create and the delete checked against the tip read), so a
  rebuilt `foo/bar` never meets the `foo` of a renamed group. Both are best effort: a failure leaves the refs as they
  are for the next sync. A branch checked out in a worktree (`%(worktreepath)` of the same `for-each-ref`) is left
  alone, because `update-ref` deletes a checked-out branch without asking. See
  [Integration Detection](integration-detection.md), decision 9.
- **Delete an archived branch.** "Delete Archived Branch…" reads the tip and `%(worktreepath)` with one
  `for-each-ref`, refuses a branch checked out in any worktree, as `git branch -D` would, naming the worktree, and
  deletes it in one transaction with a CAS on that tip (`branch-deck: delete <branch>`), not with `branch -D`
  (`crates/sync-core/src/delete_archived_branch.rs`).
- **Stacks.** A stacked branch is built on its parent's new head (see [Stacked Branches](../../spec/branch/stacked-branches.spec.md)).
  Each branch is a build task, which publishes its head once its commit writer has ended, and a finish task for the ref,
  the status and the remote status. A child is spawned only once its parent's head is known, so no task holds a slot
  of the process limit while it waits; ready branches start longest chain first. The tasks take their slots from one
  `ProcessPermits` (`crates/git-executor/src/concurrency.rs`), which the integration detection running beside them
  shares, so the sync as a whole runs at most `max_parallel_git_processes()` of them at a time. A rebuilt child's tip gets a note on
  `refs/notes/branch-deck/stack` naming its base, in the same `fast-import` run as the mapping notes (one notes commit
  per ref), for the integration detection of its archived branch.

Code: `crates/git-ops/src/copy_commit.rs`, `crates/sync-core/src/branch_processor.rs`,
`crates/git-ops/src/reword_commits.rs`, `crates/history-rewrite/src/reword.rs`.

#### Object cache

`bd commits` and `bd status --stats` read the changes of each commit (files, statuses, line counts, the line ranges
of the blocks and the blob ids before and after) from a disk cache before they run git. Only for the commits it misses
they run one `log --no-walk=unsorted --stdin -z --raw --no-abbrev` (`--no-abbrev`, not `--abbrev=40`, so a SHA-256 id
stays whole) and one `cat-file --batch` of the blobs that the diff caches lack. The blocks are those of the diff views
under `Default` (`file_diffs::blocks`).
- **Safe without invalidation** (but `review_answer`, `patronus_robots` and `build_result`, below). Keys are object ids (`remote_status`, `stack_branch`: a hash of them), and an object never changes: the changes of a commit against
  its first parent with fixed diff options (`-M`, `--no-ext-diff`, `--no-textconv`; the blocks come from the diff
  engine, so `diff.algorithm` changes no dependency edge) are the same forever. Entries are only evicted, never stale. Each
  store versions its values in its database name (`commit_diff.v4`, `Store::version`), bumped when their type, the
  options or the engine that compute them change; opening drops the older versions of a store, and a bump of one store keeps the
  others, the costly `merge_result` above all.
- **Shared by processes.** The cache is four LMDB environments (heed) in `$GIT_COMMON_DIR/branch-deck/cache/`, shared
  by every worktree and by the app and parallel `bd` calls: writers are serialized by LMDB's lock file, readers never
  wait. This is why LMDB: redb, fjall and sled lock their file for one process, and SQLite would add a query and a copy
  to every read of immutable, id-keyed values. Within a process there is one environment per directory, as LMDB
  requires, and it closes with the last handle (`EnvHandle`; the app keeps one per repository while it runs): on macOS
  LMDB locks an environment with a System V semaphore set that only its close removes, so the environments of deleted
  test directories would use up the system's sets (`kern.sysv.semmns`) until every open fails with `ENOSPC`.
- **One write transaction per phase.** Each commit of a write transaction syncs the file (`F_FULLFSYNC` on macOS,
  ~4 ms) and holds the writer lock that parallel `bd` calls wait for, so callers write a batch, never one entry per
  transaction in a loop. The sync stays: without it (`MDB_NOSYNC`) a power loss can leave a wrong but readable tree
  id in `merge_result`, and a check would build a branch from it.
- **Never in the way.** The map size caps each environment; a full map clears the stores of that environment and the
  write is retried. What is bulky and cheap to read again never evicts what is costly: the diffs and merges of
  `file_diff` and `file_merge` in `cache/patches/` (512 MiB); the merges of `merge_result`, the trees of `commit_tree`
  and the resolves of `word_resolution` in `cache/merges/` (256 MiB: about 250 and 130 bytes an entry, the merges and
  trees of some 700,000 commit copies); the conflict views
  of `conflict_info` and the chains of `conflict_chain` in `cache/conflicts/` (512 MiB; a view is a few kilobytes,
  one above 64 MiB encoded is not kept), so the diffs, lines and summaries of the
  main environment (`cache/`, 1 GiB) and the conflict views never clear the merges a check costs most to learn again. A
  store moved to another environment is dropped from the old one on open, and a retired store (`RETIRED_STORES`:
  `commit_patch`, the patches of the old diff views) from every environment. Any error is a miss,
  never a failed command, and `--no-cache` bypasses it. `--no-cache` also makes a sync and a check verify every mapping
  note against the tree of its copy (see "Virtual commit reuse"). The cache holds only derived data: no ref, index or working
  tree is read or written through it.
- `merge_result` ((base, commit, target) trees → result tree or conflict files) and `commit_tree` (commit → tree) are
  read and written by `bd check` through the `MergeMemo` of the tree cache (`crates/git-ops/src/merge_memo.rs`). A
  sync remembers no merge: its merges already run in the branch's one `merge-tree --stdin` process. The memo of a check
  (`CacheMemo`, `crates/sync-core/src/cache_memo.rs`) keeps what it learns in memory and writes it once after the
  branches are built, before the dangling references: in the monorepo a cold check wrote 52 transactions (~200 ms),
  now one (~10–25 ms).
- **A remembered tree may be gone.** The trees of trial builds are on no ref, so `gc` prunes them while their
  `merge_result` entries stay, and a copy written onto a pruned tree fails. A remembered clean merge counts only after
  one `cat-file --batch-check=%(objecttype)` per check, started at the first such hit, answers `tree` for it; a
  missing tree, or a failure to ask, is a miss, and the merge runs again and overwrites the entry. A conflict names
  only paths and needs no check.
- `word_resolution` ((resolve version, base, commit, target) trees → the tree with the resolved files and git's
  conflicts in each, or that the cherry-pick does not resolve; see "Resolve by words") holds what the resolve by words
  gave. `merge_result` keeps what git gives, whatever the setting, and only a sync or a check with the resolve on reads
  this store. `RESOLVE_VERSION` (`word_resolve.rs`) is part of the key, and its first write clears the store.
- `conflict_info` ((view version, resolve mode, commit, its parent, the commit it is applied onto), full ids → the conflict view,
  `MergeConflictInfo`) is written and read by the sync of the app and of `bd` through the `ConflictMemo` of its tree
  cache (`crates/sync-core/src/cache_memo.rs`, `SyncOptions::object_cache`). The view of a conflict reads only those
  commits and what they reach, so it is the same at every sync of the unchanged branch, whose copies are
  deterministic: the cherry-pick returns the remembered view before it merges (`MergeMemo::conflict`), and the
  branch runs neither its own `merge-tree` nor the analysis (`an_unchanged_sync_of_a_conflicting_branch`; 7 processes
  per conflicting branch: an unchanged `bd sync` of the monorepo clone with 16 of them ran 150 processes, now 41). The
  view is remembered before the sync names the branches of its missing commits, which it does again on each read; a
  view whose analysis fell back (a failed divergence walk, list of missing commits or read of the merged files) is not
  remembered. The views wait in memory and are written in one transaction when the sync's tree cache drops. `CONFLICT_INFO_VERSION`
  (`merge_memo.rs`) is part of the key: a change of `MergeConflictInfo` or of `build_merge_conflict_info` bumps it,
  and its first write clears the store. So is the resolve mode of the sync (`ResolveMode::key`: 0 off, else
  `RESOLVE_VERSION`): a conflict that a sync with the resolve off remembers may resolve with it on, and the cherry-pick
  takes a remembered view before it merges. A check reports light conflicts and reads none; `--no-cache` analyses every
  conflict.
- `conflict_chain` ((chain version, a hash of the branch's full name, its base and the resolve mode) → the originals before the conflict
  and their copies with their trees) is written and read by the sync through `ChainMemo`
  (`crates/sync-core/src/conflict_chain.rs`), in the conflict environment beside the views: a conflicting branch whose
  originals still start with those commits takes the copies instead of merging and writing them again (see "A
  conflicting branch keeps its copies" in "Virtual branch sync"; the copies are on `refs/branch-deck/conflict/`, which
  keeps them from `gc`). Not in the key, as for `conflict_info`: the merge config and the version of git.
  `CONFLICT_CHAIN_VERSION` (`merge_memo.rs`) is part of the key: a change of how `copy_commit` copies a commit bumps it,
  and its first write clears the store. The new chains are written in one transaction when the memo drops.
- `commit_files` (commit → files with statuses and line counts, no hunks) is read and written by the app for the
  commit rows (see "Commit files" below) and by the review pre-check for the paths of the commits (see "Patronus
  review pre-check"). It is a store of its own: an entry without hunks in `commit_diff` would read as "no hunks" to
  `bd`.
- `remote_status` ((status version, a hash of the branch's tip, its remote tip, the baseline commit, a stacked
  branch's parent head, where the parent's commits came from and the user's email) → the remote status without its
  push time) is read and written by the sync of the app and of `bd`, and by `bd push --remote`, through
  `RemoteStatusMemo` (`crates/sync-core/src/remote_status.rs`), after the process's own memory of the statuses. The
  walks of a status read only what those commits reach above the baseline, so an unchanged `bd sync` walks no branch
  (`an_unchanged_sync_of_pushed_branches_in_a_new_process`); a status with a ref name among its commits is not kept.
  The push time is not a function of the commits (push X, fetch Y, push X again), so it is not kept: the first status
  that needs one reads the push times of all branches of the operation with one `reflog`. The computed statuses wait
  in memory and are written in one transaction when the memo drops; a remote branch at the branch's tip ran no walk
  and is not written. The version is part of the key (it covers `DROPPED_COMMITS_LIMIT`), and its first write clears
  the store.
- `stack_branch` ((build version, a hash of the base and the ids of the original commits of a stack branch, in their
  order) → what their build gave: the tip, or the commit that conflicts and its files) is read and written by the sync
  of the app and of `bd` through `StackBuilds` (`crates/sync-core/src/stack_branch.rs`), after the memory of the
  process, and read by `bd status`. The copies are deterministic, so the build is a function of those commits; the new
  builds are written in one transaction when the sync drops the memo. `STACK_BUILD_VERSION` is part of the key: a
  change of the copies bumps it, and its first write clears the store.
- `review_answer` (a hash of what the Patronus pre-check decides on → its answer and when it was fetched) is the first
  store not keyed by object ids or a hash of them: the ownership rules of the target can outdate an answer, so the reader decides how
  long it holds (`sync_core::review_readiness`): an answer that evaluated the pull request a day, one without the
  evaluation 2 minutes while approvals are missing, 10 without. Errors are never written; `bd safe-push` always asks
  again and writes the answer for the app.
- `patronus_robots` (a hash of the Patronus server and token, the Space repository and the branch prefix → the user's
  robots as last listed and polled, when, and a rejected token or a backoff) keeps what `bd status` would ask Patronus
  on every call (`crates/bd-remote/src/robot_list.rs`): the reader decides how long it holds, a minute while a robot of it
  runs, five while none does, then a list of the robots started since and a poll of the running ones are merged into
  it ("Load on Patronus" in [Patronus](../../spec/branch/patronus.spec.md#load-on-patronus)). `bd push`, `bd
  safe-push` and `bd robot` list fresh and write it; the app keeps its robots in memory.
- `build_result` ((key version, a hash of the command, `BD_BRANCH` and the sorted `(path, object id)` pairs of what the
  build extracts: one `("", tree id)` without `--build-paths`) → it passed) is the second: `bd check --build` reads it
  in one transaction before the builds and writes the new passes in one after them, so a build that passed on a tree
  is not run again. Only a pass is kept: a failure may be by chance (a flaky test, a full disk), and runs again. Not
  `BD_COMMIT`: a regroup writes new commits of the same trees, and a key with the commit would miss after each of
  them. The key knows only the tree: a build that reads something else is outdated by it without a miss, the
  environment (a JDK, a toolchain) and the commit itself, which `export-subst` of `git archive` writes into the files
  of a path with that attribute. `--rebuild` runs the command anyway and writes its pass again; `--no-cache` skips the
  whole cache, the merges included. `BUILD_KEY_VERSION` (`check_command.rs`) is part of the key: a change of what the
  build sees bumps it, and its first write clears the store.

Code: `crates/object-cache/src/cache.rs`, `crates/object-cache/src/build_result.rs`, `crates/sync-core/src/diff_scan.rs`.

#### Symbol summaries

The dependencies by symbols ([Symbol Dependencies](symbol-dependencies.md)) read the source files the commits change,
before and after each commit, by blob id; nothing reads the working tree or an index of the whole tree.
- **Blob ids from the same log.** The `log --raw` of the commit diffs runs with `--no-abbrev` (not `--abbrev=40`,
  which would cut a SHA-256 id), so every changed file carries the ids of its two blobs; a side without the file, a
  symlink (`120000`) and a submodule (`160000`) have none.
- **Two processes per scan at most.** One `cat-file --batch -Z` of the directories above the files at HEAD
  (`HEAD:<dir>`, on stdin: thousands of directories don't fit a command line on Windows) gives the module roots, the
  crates' `Cargo.toml` and the `tsconfig*.json` files; one `cat-file --batch -Z` reads the blobs the object cache
  misses together with those manifests (`read_blob_bytes`).
- **Cached by blob id.** The `blob_summary` store keys a summary by the extractor version (the rules and the node
  types of the grammars), the language and the blob id; the first write of another version clears the store once. A
  summary the parse budget cut is not cached. The parse runs on one thread per core, each with its own parsers.
- **The index is the range.** Only the declarations the commits ahead of the baseline add or change are looked up (a
  changed member brings the overloads its file had before along); a use of anything else is no edge.

Code: `crates/sync-core/src/symbol_scan.rs`, `crates/sync-core/src/module_roots.rs`,
`crates/sync-core/src/symbol_dependencies.rs`, `crates/object-cache/src/blob_summary.rs`, `crates/code-symbols`.

#### Commit files

The commit rows show "3 files · +42 −7", and the commit preview lists the files with their diffs
([Commit Files](../../spec/commit/commit-files.spec.md)). Both are read-only:
- **One process per list.** `get_commits_files` reads the commits of an expanded branch (or the unassigned commits)
  with one `diff-tree --stdin --raw --numstat -z -r -M --root --diff-merges=first-parent`, the ids on stdin. The output
  is NUL-separated: each commit id, its `--raw` entries (status letter, paths verbatim, the blob ids of both sides), then
  its `--numstat` entries in the same order (`-` counts for a binary file). A zero id is a missing side. The id of a
  submodule (`submodule: true`) is the commit it points to. `get_commit_files` is the same for one commit, when a
  preview shows a commit no list loaded. The ids must be full object ids, so none reads as an option or a revision.
- **Cached by commit id** in the `commit_files` store of the object cache: the commits the output lists are written
  back, so a commit seen once costs no process again, also in a later session. A commit the output doesn't list (no
  changes, or not fetched yet) is not cached. The app resolves `--git-common-dir` once per repository
  (`RepositoryStateCache`) to open the cache; outside a repository the cache is disabled and the read still runs.
- **No warm.** With the files of a list in the store, the first view of a commit needs no `diff-tree`: `get_file_diffs`
  reads its blobs with one `cat-file` and compares them ([Diff View](diff-view.md)). Measured on the last 100 commits
  with the list cached and an empty `file_diff` store, the first content of a view came after 16 ms (p50) and 39 ms
  (max) in Branch Deck, 8 ms and 29 ms in the monorepo, so nothing reads the diffs of the listed commits ahead. The old
  patch warm saved a `diff-tree -p`, 146 ms (p50) in the monorepo.
- **One reader of `cat-file --batch`.** `for_each_object` and `read_objects` (`crates/git-ops/src/cat_file_batch.rs`)
  write the names NUL-terminated (`--buffer -Z`, so a `<rev>:<path>` may hold a space or a newline) and parse the
  output as it streams: each header gives the size, so the content of an object over the caller's limit is skipped
  as it arrives, never buffered, and reported as too large with its size; `<name> missing` and `ambiguous` are
  missing. Memory holds one object within the limit, not the output, and each object's bytes are handed over without
  a copy. The blobs of the diff view, the texts of the conflict view (`MAX_BLOB_BYTES`) and of the dangling-reference
  check, the blobs and trees of the symbol scan (`object_texts.rs`, which also holds the one binary test,
  `is_binary`), the raw commits of a rewrite (`read_raw_commits`), the line maps (`store_lines.rs`), the blobs of a
  hunk move (`move_hunk.rs`), the `OWNERSHIP` files (`ownership.rs`) and the session and round objects of a resolve
  session (`update_resolve.rs`) read through it; no other parser of its output exists.
- The frontend keeps the files by commit id in memory (`src/stores/commit-files.ts`) and asks only for the ids it
  doesn't have; a second request for an id waits for the one in flight.
- **A range** (`DiffSubject::Range`, the review of a pull request or of a branch, see "Review fetch"): its files come
  from one `diff-tree --raw --numstat -z --no-commit-id` with `-l1000` and `--end-of-options <base> <head>`, never
  `--stdin` (`read_diff_files`). `get_range_files` keeps them in memory only (`RangeFilesCache`,
  `src-tauri/src/range_files.rs`: 16 MiB weighed by the paths and 64 bytes a file), and the readers of one range at
  the same time run one read. It passes `-c core.bigFileThreshold=8m` (= `MAX_BLOB_BYTES`): a range may change
  thousands of files, and a file the diff view can't show is counted as binary instead of read. A commit keeps exactly its arguments. Both ids must be full object
  ids, as for a commit. The returned `commitId` is `"<base>..<head>"` (`DiffSubject::id`), which the frontend keys
  by. A range is never written to the object cache: a `commit_files` entry is keyed by one commit, and a range on the
  head's key would answer the commit's files with the range's.

Code: `crates/git-ops/src/commit_files.rs`, `crates/git-ops/src/cat_file_batch.rs`,
`crates/object-cache/src/commit_files.rs`, `src-tauri/src/commands/commit_files.rs`, `src-tauri/src/range_files.rs`,
`src-tauri/src/repository_state.rs`.

#### File diffs

The views get the diff of each file as typed JSON, never as patch text ([Text Diff Engine](text-diff.md), "Model").
`FileDiffs` (`crates/file-diffs`) reads a subject and compares each file with `text-diff`. It is a crate of its own
above `git-ops`, because it joins the git reads with the comparison and the declarations of `code-symbols`, which the
git plumbing does not depend on.

- **The file list.** A commit and a range take the files of "Commit files": the raw entries name the blob ids of both
  sides. A commit that the `commit_files` store has costs no process. The working tree is one
  `status --porcelain=v2 -z --untracked-files=all`: each tracked entry names its `HEAD` blob id, and the new side is
  the file on the disk. Its id is the blob id of its bytes, as `git hash-object --no-filters` gives it. A symlink is
  its target. A file with a conflict compares its `ours` stage with the working tree.
- **The stream.** `get_file_diffs` sends events through a Tauri `Channel`: `files` once (the headers in the order of
  git), then a `content` for each file, then a `scopes` for a text that the caches did not have. The files in view
  (`priority_paths`) come first, then the others in the order of the tree, for the cache reads, the blob reads and
  the work; the events go out as the results come. `FileDiffs::read` collects the same events, for the tests and the
  command line.
- **The blobs and the workers.** One `cat-file --batch` reads the blobs that no cache has, at most `MAX_BLOB_BYTES`
  (8 MiB) each. A file goes to a worker as soon as both its sides are read: the cores but one run the comparisons and
  the parses (`std::thread::scope`), while the thread of the caller reads the blobs and sends the events. A file holds
  its texts only until its worker has it, so memory holds the files in flight, not the subject. A diff of a commit
  costs two processes, and none when the caches have every diff.
- **The content.** A side that git counts no lines for, or a side with a NUL in its first 8000 bytes, gives `Binary`. A
  side over 8 MiB gives `TooLarge` with the sizes. A submodule gives `Submodule` with the commit of each side, as git
  writes `Subproject commit <id>`; in the working tree its new side is the commit in the index. A blob that git cannot
  read (a pruned object, a partial clone) gives `Missing`, which no cache keeps. Else the diff is a `TextDiff`; a text
  that is not UTF-8 is compared as decoded, with U+FFFD for each bad byte.
- **Budgets.** Each file has 100 ms for its inner ranges; after that, it is compared by line (`line_level_only`). Its
  line comparison has 1 s; after that, it is one block. These budgets catch a pathological file only. No budget spans
  the subject, so the result does not depend on the load of the machine or the order of the files.
- **Scopes.** The types, functions and modules of each side are its scopes, with the label `Outer › inner`, for the
  hunk headers of a view. They come from the summary of the blob in the `blob_summary` store, which the sync of the
  branches made already for most commits. A side without one is parsed (`code_symbols::extract::summarize`), and its
  summary goes to the store. The parse costs more than the comparison (8.1 s against 0.9 s over 300 commits of Branch
  Deck, one thread), so the `content` of a text goes out before its `scopes`.
- **Caches.** A diff is keyed by its blob pair, the whitespace policy and the languages of the scopes. Blobs never
  change, so one entry serves every commit, range and window that shows the pair. Memory first (64 MiB), then the
  `file_diff` store (`MODEL_VERSION`). The store never keeps a diff that the time cut short; memory keeps it for
  the run. The blobs that a diff read stay in a memory cache (128 MiB), so the texts that a view asks for next
  (`blob_texts`) cost no process. A text of the working tree is read again by its path, and only when its bytes still
  have the id.
- **Go to Declaration** ([Symbol Navigation](../../spec/commit/symbol-navigation.spec.md)). A declaration of the
  review comes from the texts in memory and the summaries of the store: no process. A name the review does not declare
  asks the index of accio (`symbol --format json --limit 20 --project <root> --ref <the commit of the side> -- <name>`,
  a null stdin, killed after 5 s; an index that fails or does not answer takes nothing from the answer of the review, which says why), only when the index directory of the repository exists, because a query without
  an index builds one. For the hits outside the review, one `cat-file --batch-check` names the blobs of
  `<commit>:<path>` and one `cat-file --batch` reads them (`object_ids_and_types`, `blob_texts`); the texts stay in
  the memory cache, so the view of the file reads nothing again. For uncommitted changes the hits are files of the
  working tree, read from the disk. `declarations_test.rs` pins the two processes. Find Usages runs no process for the
  review (the texts in memory) and, for the usages outside it, `accio references --format json --limit 100 --project
  <root> --ref <commit> -- <name>` and the same two `cat-file` (`usages_test.rs`).

Code: `crates/file-diffs/src/service.rs`, `crates/file-diffs/src/worktree.rs`, `crates/file-diffs/src/declarations.rs`,
`crates/file-diffs/src/usages.rs`, `crates/file-diffs/src/accio.rs`, `crates/object-cache/src/file_diff.rs`.

#### Review fetch

The review of a pull request someone asked the user for ([Review Requests](../../spec/repository/review-requests.spec.md))
needs its commits. `fetch_review_head` gets them without the working tree, the index or the repository lock: it
writes only `refs/branch-deck/review/<n>/head` and `…/base`, which no other operation reads or writes, so it runs
beside a sync or an update.

| Step | Command | Note |
|------|---------|------|
| 0. Validate | none | both ids full object ids, both branch names safe for a refspec (no `:`, `*`, `..`, leading `-`): they come from the frontend |
| 1. Local check | `cat-file --batch-check=%(objecttype)` of the head id, the base id, `refs/remotes/origin/<base>` and `HEAD` | both commits local: no network, no ref (a reopened review, a pruned ref whose objects gc kept) |
| 2. Source | `resolve_github_repository_source` (1–3 processes), `config --get-regexp ^remote\..*\.url$` | the configured URLs, so `insteadOf` doesn't hide which repository a remote is |
| 3. Fetch | `fetch --progress --porcelain --verbose …` (below) `<source> +<src>:refs/branch-deck/review/<n>/head` and, while the base id is missing, `+refs/heads/<base>:refs/branch-deck/review/<n>/base` | the new tips from the porcelain lines, no `rev-parse` |
| 4. Head state | `cat-file --batch-check` and `merge-base --is-ancestor <head id> <tip>`, only when the tip is not the head id | `Same`, `SourceAhead`, `Diverged`, or `Missing` (the head id still not local: the review shows the tip) |
| 5. Range | `merge-base <head> <base id>`, then `log -z --format=%H%x00%s%x00%an%x00%at%x00%B --reverse --max-count=1000 <merge base>..<head>` (the full messages for the commit preview, NUL-framed) | exit 1 of `merge-base`: `NoCommonHistory`, "The pull request has no common history with <base>" |

- **Sources.** The source follows from where `resolve_github_repository_source` found the GitHub repository and from
  origin's configured fetch URL:
  - origin is that GitHub repository: origin `refs/pull/<n>/head`;
  - otherwise (`branchdeck.githubRepository` or the Space mirror: origin is Space) a pull request of the same
    repository: origin `refs/heads/<head branch>`, which Space has first. If the head id is still missing (the mirror
    is behind, or the branch was force-pushed), a second fetch asks GitHub; when that fails, the review shows origin's
    tip (`Missing`);
  - a fork, or a push URL on GitHub with another fetch URL: GitHub `refs/pull/<n>/head`, through a configured remote
    whose URL is that repository (its credentials and `insteadOf` apply), else `https://github.com/<o>/<n>.git`.
- **The flags**, on top of the remote policy (Key Decision 8):
  - `--porcelain --verbose`: machine-readable ref lines, also `=` for a ref that is up to date (the tip of an earlier
    review fetch);
  - `--no-tags --no-write-fetch-head --no-recurse-submodules --no-auto-maintenance`, as for Update Project;
  - `--refmap=`: no opportunistic update of `refs/remotes/origin/<branch>` for a fetched `refs/heads/<branch>`; the
    push lease and the outgoing status read those refs, and a review must not move them;
  - `--no-prune`: the user's `fetch.prune` would otherwise prune `refs/remotes/origin/*`;
  - `--no-show-forced-updates`: no ancestry walk per updated ref;
  - `--no-write-commit-graph`: a large repository may set `fetch.writeCommitGraph`, and writing the graph of tens of
    millions of objects after every review fetch costs more than the fetch;
  - `--negotiation-tip=refs/remotes/origin/<base>` (else `HEAD`): the haves are the base branch's history, not every
    ref of a repository with thousands, which keeps the negotiation short and the pack the size of the pull request.
- **The base.** When the base id GitHub reports is not local, the same fetch brings `refs/heads/<base>`: diffing
  against a stale `origin/master` would bring master's newer changes into the range. The range's base is the merge
  base of the head and the base id (milliseconds with the commit-graph), as GitHub's "Files changed".
- **Timeout and cancel.** The fetch runs through `execute_remote_command_with_progress_until`
  (`GitCommand::progress` with a `ProgressStop`): git is killed when stderr is silent for 120 s (SSH has
  no `http.lowSpeedLimit`), or when the app's latest-request counter moved on (`cancel_review_fetch`, or a newer
  `fetch_review_head`, `src-tauri/src/commands/review_requests.rs`), within 20 ms. The error is then a
  `StoppedCommand`. A grandchild (ssh, a remote helper) that keeps stderr open does not keep the result waiting.
- **Credentials.** A refused authentication when fetching from GitHub (git's C-locale words: `could not read
  Username`, `terminal prompts disabled`, `Permission denied (publickey)`) is `GitHubCredentials`, "GitHub asked for
  credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)". The card can show the request
  with a token git does not have (`GH_TOKEN`, `gh` without its git helper), so the fetch may still need git's
  credential helper for github.com.
- **Pruning.** `prune_review_refs(keep)` is one `for-each-ref --format=%(refname) %(objectname)
  refs/branch-deck/review/` and one `update-ref --stdin` deleting every review ref of a number not in `keep` with
  its old value (the `sweep_stale_temp_refs` pattern). A ref that moved meanwhile (a review fetch of another window)
  fails the transaction, which is only logged: the next prune deletes it. It is best effort across app instances;
  the diffs are read by object id, and gc keeps unreachable objects for about two weeks.
- **Never** the repository lock: the fetch moves no branch, and a fetch never runs under the lock (Update Project
  fetches before it takes it).

Code: `crates/sync-core/src/review_fetch.rs`, `crates/sync-core/src/remote_command.rs`,
`crates/git-executor/src/git_command_executor.rs`, `src-tauri/src/commands/review_requests.rs`.

#### Trial sync (check)

`bd check` answers "would these branches build?" for a plan without touching anything a sync updates:

1. The plan is applied in memory and validated as a regroup dry run, and the branches are placed as a stacked sync
   places them (`place_branches`).
2. Each branch is built with the sync's `build_branch`, a child on its parent's new head, in parallel as a sync builds
   them (`run_builds`, up to `max_parallel_git_processes` threads): a root at once on the base, a child as soon as its
   parent has its head, a branch on a failed parent blocked without a build; the outcomes are those of a check one by
   one, in stack order. The base's date is read once before the builds, as in a sync. On the monorepo pin a cold check
   ran its 18 chains one after another (about 2.7 s of git at a low load) and now overlaps them, the same 49 processes
   and output (a cold `bd check` 7.0 → 5.4 s at the best of four runs, on a loaded machine; the rest is the symbols
   of the commits, in process). It writes
   commit objects (the chain's `merge-tree` and `fast-import`) and nothing else: no ref transaction, no notes, no remote status, no
   repository lock. The commits of the existing copies a build may reuse are read as a sync reads them, by one `log`
   for all branches (`ExistingCommits`), not one per branch, and once per session, not once per check.
3. The tree cache carries a memo backed by the object cache: a merge of known trees returns the remembered tree, while
   `gc` has not pruned it (see "Object cache"), or conflict, and a conflict is reported light (its files), without the
   conflict analysis.

A `CheckSession` reads the snapshot (the guards, HEAD, the stack setting, the baseline and one `log`) and the branch
tips once; `check_plan` is one session with one check. `bd suggest` checks up to 6 plans with one session, one per
repair round, and the conflict fix up to 24, so the checks of a session share what a plan doesn't change: the states
of the declared parents without commits (`empty_parent_states`: the detection notes, the baseline and the detection
of a parent whose virtual branch lost its commits) are read once for each set of such parents, since a plan changes
which parents have commits but never the tips; the tree and committer date of the commit the branches start on, the
commits of the existing copies (`ExistingCommits`) and the baseline commit (the detection and the dangling-reference
checks compare with it) once, also for the sessions of `with_rewrite`. Each check still runs the git processes of
building each branch the merge memo misses: a second check of the same plan runs none
(`the_second_check_of_a_session`).

`stack_independence` asks whether a stacked branch builds without its parent: the plan `unstack: [branch]`, and only
that branch is built, as a root on the baseline. Its first conflict names the commit and the files. Only then the
changes of every commit ahead of the baseline are read (the object cache, one `log --raw` and one `cat-file` for the misses) for the line
dependencies (`dependencies.rs`), lock files included: they name the commits of the ancestors the branch needs. The
changes and the line dependencies with the fixup units (`CheckSession::line_graph`) are read once per session, and the
conflict fix, the symbol edges and `bd check` share them.
The app (`get_stack_independence`) and `bd safe-push --check` (`alone`, in the session of its dangling references)
ask it; the sync and a start of `bd safe-push` never do. It takes no lock.

`bd check --build` runs a command in the tree of each branch that builds, extracted by `git archive` into a temporary
directory outside every working tree (`git_ops::tree_archive::write_tree_archive`, streamed into the file). With
`--build-paths` only the paths a tree has are extracted (`git archive` fails on a pathspec that matches nothing), and
`--literal-pathspecs` makes `git archive` take each as the same name, never a pattern. One `cat-file --batch-check -Z`
of every `<head>:<path>` (without `--build-paths`, `<head>^{tree}`) asks for all branches which paths the tree has and
their object ids (`git_ops::cat_file_batch::object_ids_and_types`), the key of the passed builds in the object cache
(`build_result`, see "Object cache"): a build that passed runs no `archive` and no command again.
bd itself runs no git process: every one goes through a git-ops or sync-core function.

Code: `crates/sync-core/src/check.rs`, `crates/sync-core/src/cache_memo.rs`, `crates/git-ops/src/cherry_pick.rs`,
`crates/git-ops/src/tree_id_cache.rs`, `crates/git-ops/src/tree_archive.rs`, `crates/bd-rewrite/src/check_command.rs`.

#### Dangling references (check)

A branch that removes a declaration its own tree still uses merges cleanly and fails only when it is compiled: the
commit that changed the use is in another branch (the Safe Push of 2026-10-16 that motivated it). After the builds of
a check, `find_dangling_references` looks for such uses without a compiler:
1. The changed lines of every commit ahead of the baseline, in the files of the known languages, come from the object
   cache (`commit_lines`), read once per `CheckSession`. For the misses, one `log --no-walk --stdin -z --raw
   --no-renames` gives the files, and `FileDiffs::changed_lines_of` gives the lines of their blocks (one `cat-file
   --batch` of the blobs): the same blocks as the diff views, the dependency edges and `bd commits --patch`, so the
   user's diff config changes no line. Without renames, a moved file removes its lines from the old path and adds them
   at the new one. The paths are those of the raw entries, NUL-separated and verbatim. A side without text (a symlink)
   is empty. The `commit_lines` store is at version 4: the entries of version 1 lack such files, those of version 2
   were read with the user's hunk options, those of version 3 hold git's hunks.
2. A removed line of a branch's commit is a declaration when it declares a name other files can use (a top-level type
   or function, a static member), by per-language patterns. A member is searched as `<file stem>.<name>`, a top-level
   name as a whole word, a private one only in its file; a top-level JVM declaration moved to another package as
   `<old package>.<name>`. A name still declared in its file in the branch's tree (an overload, a change in place) is
   not searched, nor a member moved with its class (a file of the same stem), nor a private declaration the branch
   declares elsewhere (its file imports it from there). Only a private declaration or a member is searched unqualified
   in its own file. On the 200 commits and 35 branches of the IntelliJ checkout these rules took the check from nine
   false references to none, in 8 s, the trial builds included. A Rust name after a single `.` is a field or a method
   (`.collect()`), never an item; `..` is a range. With `branchdeck.symbols on`, whose module roots know the crate of
   every path of the commits (`ModuleRoots::read_crate_of`, the names of `SymbolInput::crates`), a `pub(crate)`,
   `pub(super)` or `pub(in …)` item is searched in its own crate only (`pub(self)` as a private one), and a file of
   another crate uses a `pub` one only through its crate: a path that starts with the crate's name or with a name a
   `use` of the crate binds, or a bare name where such a `use` names it or a glob. Without the symbols, or for a path
   of no known crate (Bazel without a `Cargo.toml`), every file of the language is searched.
3. A use the branch's tree has and HEAD's tree lacks can only be in a path where the two differ: one `diff-tree
   --stdin -r -z --raw` with a line `<virtual head> <HEAD>` per built branch (and `<baseline tip> <base>` for the uses
   the baseline got after the base) lists them, one `cat-file --batch -Z` reads their blobs, the branches' versions
   of the removing files and the removing files before the commit (`<removed_by>^:<file>`, for the line of the
   declaration, `declared_in`), and the search runs in Rust. Paths where the trees differ are the other branches' changes,
   a few hundred even in a monorepo.
4. The syntax trees subtract (`code_symbols`, Kotlin, Java, Rust, TypeScript and JavaScript): a candidate line counts
   only where the tree of its blob has a reference to the name, so a word of a comment or a string, a local of the same
   name, a key and the name of an `override` member (it declares one) are none, nor, for a Kotlin top-level function
   that is no extension, a call on an expression (`client.name()`, an empty qualifier); a name an import still binds in the
   removing file (a Rust `pub use`, a Kotlin alias) is not removed. The lines found the candidates and the trees only
   take some away, so a use the regular expression finds (a Kotlin extension call) stays. Both sides of a path go by the
   same method: a side the parser can't read well keeps the lines on both. The summaries of these blobs, new in every
   trial, stay in the session (`SyntaxMemo`), never in the object cache; `branchdeck.symbols off` leaves the lines
   alone.
5. A path dangles when the branch's tree has more lines that use the name than HEAD's, so a reformatted use is no
   reference. `fixed_by` is the oldest commit outside the branch and its ancestors whose patch of the path removes more
   such lines than it adds; `mutual` when its added lines name a declaration the branch adds. Without `fixed_by`, and
   for the baseline, the reference is only possible: a warning.

6. With `branchdeck.symbols on` and a commit that changes a source file of a grammar, the same `diff-tree` and
   `cat-file` also serve the checks by the declarations (`declaration_checks.rs`, decision 15 of
   [Symbol Dependencies](symbol-dependencies.md)): an instance or static member a branch's commit removes, a function,
   Kotlin or Java function, property or constant whose header, parameters or reach it changes, a type whose
   constructors it changes or a key of which it removes an overload, and a JVM type or Kotlin top-level function it
   adds while the tree has the key in another file of its package and module. The declarations come from the
   summaries of the commits' files (`CheckSession::symbol_input`, read once per session and shared with the sessions
   of `with_rewrite`, or handed over by `bd suggest`), only those on the changed lines. A use counts where it resolves
   to the declaration as an edge would and, for a change, fits the old form but no form in the branch's tree; a change
   the summary can't judge the use by (a return type, a suspend call in the lambda of an unknown function) makes the
   reference only possible, a warning. `fixed_by` is the commit outside the tree whose change of the path removes such
   uses (or the other declaration). A blob is read
   only where its summary names a search or is unknown (a merge of a trial), and the summary of a blob a commit has
   comes from the session, so only those of new blobs are parsed. The `kind` of the reference says which:
   `removed_declaration`, `changed_declaration`, `duplicate_declaration`.

`bd check` reports a proven one as `dangling_reference` (`--allow-dangling` keeps it and exits 0), `bd suggest` turns it into a repair edge (the removing commit
needs `fixed_by`) unless it is mutual, and `bd safe-push` refuses it (`--allow-dangling`). The sync doesn't run it.

The fix of a mutual one moves the change of the use into the branch: `fixed_by` itself when it changes only that file,
else its change of the file into the removing commit (`bd move-hunk`). `bd check` and the refusal of `bd safe-push`
offer it only when a trial build proves it (`DanglingFixProver` in `crates/sync-core/src/dangling_fix.rs`): the plan
with the commit moved, or the history after a trial move of the hunk (see "Move a hunk"), builds the branch without a
proven dangling reference and breaks no branch that built. A hunk move costs the rewrite of the commits above the older of the
two plus a check of every branch, so at most two are tried per check; `bd suggest` only names the move.

Code: `crates/sync-core/src/dangling_references.rs`, `crates/sync-core/src/declaration_checks.rs`,
`crates/sync-core/src/dangling_fix.rs`, `crates/git-ops/src/object_texts.rs`, `crates/object-cache/src/commit_lines.rs`.

#### File merges (the conflict view)

The conflict view gets the merge of each conflicting file as typed JSON (`FileDiffs::stream_merges`,
`crates/file-diffs/src/merge.rs`, the command `get_file_merges`), never as text with markers.

- **Git decides which files conflict.** One `merge-tree --write-tree -z --merge-base <parent> <target> <commit>` (the
  merge of the sync) gives the conflicting paths with the blob ids of their stages: 1 the base, 2 ours (left, the
  target), 3 theirs (right, the picked commit). It also gives the conflict type of each path: content, add/add,
  modify/delete, rename/delete, rename/rename or binary. Git types an add/add as contents without a base stage, so the
  stages name it. A file that git merges cleanly is never merged again. Memory keeps the stages of a merge.
- **The engine merges each file again**, as IntelliJ's merge view does: `merge_texts` on the three blobs
  ([Text Diff Engine](text-diff.md), "Merge"). So it can find that a conflict of git resolves by its words (the
  `text` resolution), or that it covers fewer lines than the hunk of git.
- **A resolved copy.** A commit whose conflict the sync resolved by words (see "Resolve by words") opens the same
  merge with `resolved`, its copy: each file then names its blob in the copy (`resolved_id`, one `cat-file
  --batch-check` of `<copy>:<path>`), which the view shows as the result, what the sync committed.
- **The content.** A `TextMerge`. `OneSided` for a modify/delete or a rename/delete: the two-way diff of the base to
  the side that changed the file. `Binary`, `TooLarge`, `Submodule` (the commit of each stage), or `Missing`, which no
  cache keeps.
- **Processes.** One `merge-tree`, and one `cat-file --batch` for the blobs. A merge that memory has costs none. The
  blobs are read first, because a conflict holds a few files; then the cores but one merge the files. The events are
  those of `get_file_diffs`: `files` with the stage ids, a `content` for each file (the files in view first), and the
  `scopes` of a text merge.
- **Caches.** A merge is keyed by its three blob ids, the policy and the languages of the scopes: memory first, then
  the `file_merge` store (`MERGE_MODEL_VERSION`). A merge that the time cut short stays in memory only. The blobs stay
  in the blob cache of `get_blob_texts`.
- **Measured** on cherry-picks of the last commits onto an older commit, under load (load average 15 to 20). In Branch
  Deck, 141 merges and 886 files: p50 47 ms, of which 26 ms are the `merge-tree`. In the monorepo, 62 merges and 392
  files: p50 77 ms, of which 57 ms are the `merge-tree`. 142 of the 709 text merges of Branch Deck and 38 of the 328
  of the monorepo are conflicts of git that their words resolve.

Code: `crates/file-diffs/src/merge.rs`, `crates/text-diff/src/merge_model.rs`, `crates/object-cache/src/file_merge.rs`.

#### Conflict fix (check)

The fix of a sync conflict (`conflict_fix.rs`, [Conflict Fix](../../spec/branch/conflict-fix.spec.md)) is a search over
trial builds of one `CheckSession`: it writes commit objects and the object cache, no ref, and takes no lock.
1. One check of the current grouping finds the problems: the conflicts, then the proven dangling references, each in
   build order (`bd check --fix` reuses the check it just printed). The changes of every commit ahead of the baseline
   (the object cache, one `log` for the misses) give the line dependencies, with the fixup units, read once.
2. The direct answer, one trial by the line dependencies alone: every conflicting branch whose commits all depend on the
   conflicting one is restacked on the branch that holds what it needs, or on a chain of those branches. Only when
   problems are left are the edges by symbols read.
3. For each problem left the candidates are plans of `plan_move` (a whole issue by the issue groups, a part by a
   prefix): move the conflicting commit with what depends on it to the branch whose stack has what it needs, pull what
   it needs in, restack the branch. A move also takes a commit of the branch that uses a name the moved ones declare: by
   the edges by symbols with `branchdeck.symbols on` (`CheckSession::symbol_graph`, read once per session), by the
   declaration patterns of the dangling references (`uses_added_declarations`) for the languages without a grammar
   and with the symbols not on. The same edges keep a commit from leaving a branch whose staying commits use it.
4. Each candidate is one more check of the plan built so far (`check_until`). A trial of a conflict looks for no
   dangling reference; once the conflicts are done, the dangling references of the plan so far are read from its
   report (no build again), and a trial of a dangling reference looks at the branches it can alter only (the others
   keep theirs). A dangling reference or a new conflict in a branch the candidate fills extends it. The best candidate
   that breaks no clean branch and leaves fewer failing ones is kept.
5. The steps form components by the branches they touch; with several, each component's plan is checked alone, when
   the budget has room for it.

The budget (`FixBudget`, `FixRun`) counts the trials and the time from the start of the command, the first check
included: the app's 24 trials and 60 s (Cancel stops it), `bd check --fix`'s 12 and 180 s (--max-trials,
--time-budget). The time and a cancel are checked before every branch build of a trial; a trial cut short is thrown
away, its merges stay in the memo. Each trial is a line of progress, which `bd check --fix` prints on stderr.

Merges are memoized (`merge_result`), so the trials repeat no `merge-tree` of a tree pair; on the IntelliJ checkout a
trial took 0.5–2.5 s and the search 9 trials. Apply (`apply_conflict_fix`) is a regroup (`regroup_planned`): under the
repository lock the snapshot's fingerprint (HEAD, the issue groups and stack parents commits, the oldest commit ahead
of the baseline) must be the one the fix was found for, else the fix is stale and is looked for again; then one
message-only rewrite and the ref transaction of a move, with its undo token.

Code: `crates/sync-core/src/conflict_fix.rs`, `src-tauri/src/commands/conflict_fix.rs`.

#### Agent push (`bd push`)

`bd push` syncs and pushes under the repository lock, so the refs it pushes are the ones it just built. It reads
nothing the sync read again: the stack (`GroupedBranchInfo::stack`, `StackForest::order`), the tips the sync left,
the remote tips of `origin`, the remotes and the descriptions come from its `SyncReport`. Besides the push it reads
only the push URL of `origin`, once per command (`RepoContext::origin_url`: the Space repository of the holds, the
GitHub repository and the `--repo` of `gh pr create`), and the committed Space settings when neither
`branchdeck.githubRepository` nor that URL names the GitHub repository (one `cat-file`, once per command,
`RepoContext::github_repository`):

1. It refuses (`push_refused`) before talking to the remote: a branch that didn't sync (error, conflict, blocked; the
   `not_built` of its report, the rule of the app's push too), and a
   branch whose remote branch has commits it lacks unless `--force`. Branches go parents first. A stacked branch takes
   the branches it is built on (the effective base chain) along when the sync counted commits of theirs the remote
   lacks; `--alone` and `--expect-remote` don't, and the branch carries their commits, as its pull request into the
   baseline does. `--with-children` adds the branches stacked on the named ones, directly or further down
   (by the effective base, or the declared parent of a blocked one), so a blocked child refuses the push as any
   branch that didn't sync; a named root of a stack that forks adds its stack branch after the branches, from the
   `StackBranchUpdate` of the report (its tip, its remote status and so its lease: no process), as the app's Push
   with Children does: once it is on the remote or the pull request of the whole stack is open, only when the sync
   built it and the remote lacks it, and only to `origin`, which its remote status compares with ("Stack branch"). A
   stack branch the remote lacks costs one GitHub search under the lock (`list_pull_requests`), whose listing the
   retargets after the push reuse: the search moves, none is added.
   On Space it then reads the user's robots once (best effort: off Space, without a token or
   an answer, nothing is held) and refuses (`robot_running`) a named branch whose Safe Push runs on it, or one of a
   branch built on it or of the stack branch of its stack whose commit has commits of it ("Robot span": no process for
   a robot at the tip the sync left), naming the branch the push moves or the commits the robot lands; an ancestor such
   a robot lands stays out of the push, and so does the stack branch of `--with-children` while the Safe Push of the
   whole stack runs on it. A branch a merged Safe Push landed whole is refused as `landed`; with `--with-children` it is only left out,
   and so is the stack branch of a stack every branch of which landed, the push refused only when nothing else is
   left. A parked branch, or one built on a parked branch (`sync_core::parked::parked_state` on the report's grouping),
   is refused as `parked` when named; with `--all` or `--with-children` it is only left out (`skipped`, `code:
   parked`), and so is the stack branch of a stack with a parked member. `--restart-running` cancels the
   robot of a named branch over HTTP before the push and restarts it after it, as the app's restart that pushes first does.
2. One `git push --porcelain --atomic --force-with-lease=refs/heads/<b>:<lease>… <remote> <tip>:refs/heads/<b>…`
   through `sync_core::push::push_leased`, the one leased push of the app's push too (below; `push_lease::push_refs`
   underneath: exit 1, a rejected ref, still prints the porcelain report), the tips those the sync left
   (`leased_push_refs`, no `for-each-ref`). Atomic: a stack is pushed whole or not at all.
3. The lease comes from `push_lease`, which the app's push button uses as well (below): the remote head the sync of
   the same run compared with (`remote.head`), a view as fresh as the app's row, or with `--expect-remote <commit>`
   that commit. Step 1 decides whether the push may drop commits; the lease only makes sure it drops nothing the sync
   didn't see. With `--remote` other than `origin`, which the sync compares with, `bd push` computes the remote status
   of the branches it pushes against that remote's remote-tracking refs: one `for-each-ref` of all of them, then
   `RemoteStatusMemo` on the tips the sync left and its baseline commit (0 to 5 processes per branch, none for commits
   the object cache has a status of, and one `reflog` for the push times of all of them),
   so steps 1, 3 and 4 work the same for every remote. A `--remote` that is not one of the remotes the sync listed
   (`git remote`) is `no_remote`: a URL has no remote-tracking refs.
4. A rejection fetches every branch of the push (`fetch_remote_branches`, in `push_leased`), computes their remote
   statuses (`compute_remote_statuses`) and lists the ones whose fetched head is not their lease in `lease_rejected`
   (`head`, `behind`, `dropped_commits`); a branch refused as behind is listed the same way. The agent shows what a
   push drops and, once the user approves, pushes with `--force --expect-remote <head>`. When every lease held, the
   remote refused for another reason (a hook, a protected branch): `push_refused` with what git said. A dry run that
   the remote would reject fetches nothing, so it can't tell which lease failed.
5. After the push, with the lock released, the open pull requests of the pushed branches whose base is a virtual
   branch (`{prefix}/virtual/…`) are retargeted to the baseline over HTTP (`pull_request_retarget.rs`: one search of
   the pull requests, one `PATCH pulls/<n>` each), as the app's push does; no git process.

Code: `crates/bd-remote/src/push_command.rs`, `crates/sync-core/src/push.rs`, `crates/sync-core/src/push_lease.rs`,
`crates/sync-core/src/remote_command.rs`.

#### Agent Safe Push (`bd safe-push`)

`bd safe-push` pushes the branch exactly as `bd push <branch>` does (`push_locked`, the same sync, refusals and
lease), so its git side adds only reads before the push, all under the repository lock with the sync. `--check` writes
nothing after its sync and releases the lock then: its trial builds and the questions to Patronus and GitHub keep no
other `bd` writer waiting. A stacked
branch lands with its ancestors in one robot ([Landing](landing.md)): the push takes the ancestors with work for the
remote along, and the robot starts on the branch's virtual branch, which carries their commits (`lands`). A parked
branch, one built on a parked branch and a whole stack with a parked member are refused as `parked` right after the
sync, before anything is asked of Patronus.

0. Before the lock, the fetch of the target for the target conflict check (below, "Target conflict check"): a fetch
   never runs under the lock. `--create-pull-request` starts no robot and fetches nothing.
1. The Space repository from the push URL of `origin` (`RepoContext::origin_url`, the one read the push and the
   GitHub repository share), the target branch from the upstream of the baseline the sync read (`SyncReport::upstream`,
   `target_branch_of`: no process), the stack and the tips from the sync's report, as for `bd push`.
2. For the code-owner pre-check of a Safe Push, one `git log -z ^<baseline> <tip>` for the commits, their paths from
   the object cache (one `diff-tree --stdin` for the misses), and, for the groups Patronus names without paths, one
   `cat-file --batch` of the `OWNERSHIP` files of the baseline (`sync_core::review_readiness`, shared with the app's
   start of a Safe Push).
   The dangling references of the branch and its ancestors come from a trial check on the sync's report
   (`session_of_sync`, `CheckSession::of_sync`): no guards, HEAD, config, stores or baseline are read again, only the
   commits ahead of the baseline with the mapping notes the sync wrote (one `log`, so the trial reuses the virtual
   commits) and the branch tips under the prefix (one `for-each-ref`). Only the branch and the branches it is built on
   are built (`CheckSession::check_landing`), every branch only for a whole stack that forks; the landing gate (below,
   "Landing gate") reads the missing declarations in the same session. With `--check`, the
   trial build of a stacked
   branch alone (`alone`, `stack_independence`) runs in the same session: its build and, for the names it uses, the
   changes of the commits; a start runs no trial.
   The target conflict check replays the tip of the branch (or of the stack branch) onto the baseline commit that the
   sync read after the fetch (`check_target`, one `replay` when it applies).
   A Safe Push that pushes new commits into the pull request the robot starts from, which someone approved, asks
   GitHub for the rules of the target: one `GET rules/branches/<target>` (`sync_core::branch_rules::read_branch_rules`,
   the function of the app's `get_branch_rules`), HTTP, no process. Rules that dismiss stale reviews on push refuse
   the start where the approvals count (`push_dismisses_approvals`), and rules GitHub doesn't answer refuse it as
   `rules_unknown` (`--allow-unknown-rules`), never "keeps the approvals" (`rules_check`).
3. Then the push; the lock is released before the robot starts, as a start from a pull request comment waits up to a
   minute for the answer of Patronus.
   With `--whole-stack` the robot starts from the stack branch of the root (a straight chain: from its leaf, as
   above). Everything about it comes from the sync's report: its state, tip and remote status (`StackBranchUpdate`),
   the robots of the stack and of its branches, the pull request text from the descriptions the sync read
   (`stack_pull_request.rs`). The push is one `push_leased` of the stack branch alone, leased on the remote head of
   its remote status; the branches of the stack are not pushed. The pre-check reads the stack branch's commits as a
   branch's (`log`, `diff-tree`), and the dangling references take the members as the branches built.
4. `--create-pull-request` opens the pull request with the text of the draft `bd push` prints
   (`sync_core::pull_request_text`): the `bd describe` text, else the text of the branch's commits and of the branches
   it is built on, from the sync's report, no git. The app's "Create pull request" dialog writes it before its push,
   from the local branch the push sends: one `config`, and without a description one
   `git log --reverse --format=%D%x1f%s%x1f%b%x1e --decorate-refs=refs/heads/<ancestor>… refs/remotes/origin/<base>..refs/heads/<branch>`,
   whose decorations, the tips of the branches it is built on, split their commits from the branch's own
   (`read_new_pull_request`). The summaries of the issues come from YouTrack over HTTP, only for a key whose link of
   `.idea/vcs.xml` is an issue page of the YouTrack signed in to, within 3 s for all keys: the app renews its token as
   for the popovers, `bd` only reads it (`YouTrackService::without_renewal`), because a renewal rotates the refresh
   token the app keeps. GitHub is asked over HTTP, never by git: `GET /user`, `POST pulls`, then the assignee and
   the reviewers; right after a push (`wait_for_mirror`) a `POST pulls` refused for a head or a base GitHub doesn't
   have yet (a Space mirror copies a pushed branch a moment later) is asked again every 2 s for 30 s. The app's dialog
   of the whole stack ("Create Pull Request of the Stack…") takes its text from `read_new_stack_pull_request`: one
   `config` for the descriptions of the stack branch and its members, the subjects as the rows show them, no `log`
   (`stack_pull_request_text`); its Create pushes the stack branch alone (below, "App stack push"), then creates the
   pull request of its head.

Code: `crates/bd-remote/src/safe_push_command.rs`, `crates/sync-core/src/review_readiness.rs`,
`crates/sync-core/src/branch_rules.rs`, `crates/sync-core/src/patronus_review.rs`, `crates/sync-core/src/patronus_start.rs`,
`crates/sync-core/src/stack_pull_request.rs`.

#### Target conflict check

A sync builds each virtual branch on the base of the checked out branch: the parent of the oldest commit ahead of the
baseline, not the tip of the baseline. A fetch of the baseline (by Branch Deck or by any `git fetch`) moves the
baseline, but the branches stay on the old base until Update Project moves the checked out branch. Patronus rebases
the branch onto the newest target before any check (`REBASE_AUTOSQUASH`), and a conflict fails the robot at once:
"Rebase … onto master is impossible". The target of a monorepo moves every minute, so the check runs at each start,
of `bd safe-push` and of the app (`sync_core::target_conflict`, one core for both).

1. The fetch of the target's remote branch only (`fetch_target`, through `fetch_baseline_core`, as Update Project
   fetches it): `remote`, `for-each-ref`, `symbolic-ref` and `rev-parse` for the baseline and its upstream, then one
   `fetch`. Never under the repository lock. A failed fetch is no error: the check uses the last fetched tip, and the
   result says why the fetch failed (`fetch_error`).
2. The replay of what the robot lands onto the target tip, as a plain `git rebase` replays it, with no resolution by
   words, because Patronus can't resolve by words (`git_ops::trial_rebase`): one `git replay --onto=<target>
   <tip> ^<target>`, objects only, never a ref. The app resolves both tips first (one `rev-parse`); `bd safe-push`
   has them from the sync.
3. Only on a conflict: the commits of the range (one `log`), the first conflicting commit by a binary search over the
   prefixes (⌈log₂ n⌉ `replay`, `first_conflict`), its files from one `merge-tree --write-tree --name-only -z` onto
   the longest clean prefix, the base (one `merge-base`), and the commits of the target after the base that change
   the files (one `log --literal-pathspecs … -- <files>`, the newest 10).

A conflict refuses the start: `target_conflict` (exit 4) in `bd`, the error toast "Cannot start the Safe Push" with
Update Project in the app; nothing is pushed or started. Update Project resolves the conflict locally (a resolve
session), and the next start replays cleanly. A replay that fails for another reason holds nothing back (`NotRun`).
The E2E server checks without the fetch: most templates name the repository itself as `origin`. The check replays a
`fixup!` commit as a commit of its own, but `REBASE_AUTOSQUASH` folds it into its target first. In a rare case the fold
removes the conflict (the fixup undoes the conflicting change); then the check refuses a start that Patronus would
take. `bd fold` (Squash Fixups in the app) before the start removes the difference.

Code: `crates/sync-core/src/target_conflict.rs`, `crates/git-ops/src/trial_rebase.rs`, `src/lib/target-conflict.ts`.

#### Landing gate

git merges a branch that uses a name only another branch adds, and the robot's build fails, as the robots of
2026-10-17 (an override of an overload of another branch) and 2026-10-21 (`StillEncoding`, `Stills` and
`Manifest.stills` of `stills-codec`, used by `lane-run`) failed. So every start of a Safe Push or a Dry Run, of
`bd safe-push` and of the app, asks one core first (`sync_core::landing_gate`, `check_landing_gate` of a
`domain_types::landing::Subject`: a branch, or the whole stack of a root): a trial check of what lands, the branch and
the branches it is built on (`CheckSession::check_landing`), every branch for a whole stack, then

- the dangling references a commit proves (`dangling_references`), and
- with `branchdeck.symbols on`, the missing declarations (`missing_declarations`): the edges by symbols from a commit
  of what lands to a commit outside it, which no merged Safe Push landed (the commits `landedBy` names). An edge to a
  branch of the landing stack is none, a sibling's of a whole stack that forks too: one robot lands both.

The result is typed (`LandingRefusal`, with `LandingFix`: the restack of the root of what lands onto the branch that
adds the name, else, with stacked branches off, an unassigned commit or a restack that would close a cycle, the move of
the commit), and the overrides are the user's (`LandingAllowances`: `--allow-dangling`, `--allow-missing`, Start
Anyway). It writes objects only, never a ref: the symbol graph costs one `log --raw` of the commit changes and one
`cat-file --batch` of the source blobs whose summaries the cache lacks, once per session. `bd safe-push` runs it in the
session of its dangling references (`session_of_sync`, no second `log` of the commits); the app opens a session of the
current grouping (`CheckSession::open`) before the push (`checkLandingBeforeStart`), and the start command checks again
before it asks Patronus (`start_patronus_robot`, `ensure_landing_starts`), so no surface starts past it. The E2E server
runs the same core without the object cache.

The same rule answers before any start, for every branch (`landing_needs`, the landing needs of the
[repository view](#repository-view)): one trial check of every branch (`CheckSession::check`, the dangling references
of all of them), one search of the missing declarations, and per branch the refusals its start would get, filtered by
the one predicate of the gate (`refuses`: a use by a commit of what lands of a name a commit outside it adds, which
no merged Safe Push landed). The types are of `domain_types::landing` (`LandingRefusal`, `LandingFix`, `StackCommit`); a
dangling reference carries its fix too (the restack onto the branch of the commit that changes the use, or its move),
and a missing declaration the running Safe Push that lands the branch adding the name (`landing_in`,
`name_landing_robots`, from the holds and spans of the view). The app computes it in the background after a sync
(`landing_needs_of`, a session of the current grouping through the object cache), keyed by its inputs: the baseline,
the tips of the table and the landed commits; while they stay, nothing is computed again. On the synced
`util`/`flow`/`flow/ui` fixture it costs what a `bd check` of the synced branches costs, no `merge-tree` and no
`fast-import` (the copies of the sync are reused): `cat-file` 4, `config` 2, `for-each-ref` 2, `log` 5, `remote` 1,
`rev-parse` 4; with the object cache it filled `cat-file` 2 and `log` 3 (`the_landing_needs_of_every_branch`).
`bd status` runs it on Space in its own session.

Code: `crates/sync-core/src/landing_gate.rs`, `crates/domain-types/src/landing.rs`,
`crates/sync-core/src/missing_declarations.rs`, `crates/bd-remote/src/safe_push_command.rs`,
`src-tauri/src/commands/patronus.rs`, `src/lib/landing-gate.ts`; the landing needs `crates/repository-view/src/views.rs`, `crates/bd-remote/src/status.rs`.

#### Agent robot status (`bd robot`)

Read-only, without the repository lock and without a sync: `bd robot` reads the branches as the last sync left them.

1. The grouping of the commits ahead of the baseline (`read_grouping`, as `bd status`): the baseline and the parent of
   a stacked branch (`placements[..].effective_base`), and from the same listing the local and the pushed tips of the
   branches under the prefix (`BranchGrouping::listed`, `remote_tips`), which a robot of a temporary ref (the IDE, the
   Safe Push CLI) is matched by (`find_branch_robot`).
2. The Space repository comes from the push URL of `origin` (`RepoContext::origin_url`).
3. For a failed or failing robot, the [robot commit map](#robot-commit-map) and one `symbolic-ref --quiet HEAD`
   (`robot_failure_report`, the same as the app's `get_robot_failure`).

Code: `crates/bd-remote/src/robot_command.rs`.

#### GitHub credentials

The GitHub API is asked with the first token of a chain (`crates/github/src/token.rs`): `GH_TOKEN`/`GITHUB_TOKEN`,
`gh auth token`, then the password git stores for github.com. The last one is the only git process of the chain:

- `git -c credential.interactive=false credential fill` with `protocol=https`, `host=github.com` and
  `path=<owner>/<name>.git` on stdin, in the repository (its local `credential.helper` and `useHttpPath` apply);
- environment: `GIT_ASKPASS=` (empty: git runs an askpass program, `core.askPass` or `SSH_ASKPASS`, before it
  looks at `GIT_TERMINAL_PROMPT`, and an empty value stops the lookup), `GCM_INTERACTIVE=never`,
  `GCM_GUI_PROMPT=false`, plus the executor's `GIT_TERMINAL_PROMPT=0`;
- `git.cmd(..).stdin(..).timeout(..).status()`: killed after 10 s (a helper may wait for a keychain unlock),
  a non-zero exit is no credential and is not logged as an error (128 without a stored credential); stdout is read
  for `password=` only, other keys (`password_expiry_utc`, `oauth_refresh_token`) are skipped;
- never `credential approve` or `reject`: these are the user's credentials, and a token GitHub rejects is only
  skipped by the app (`TokenChain::reject`), not erased from the helper.

It reads nothing of the working tree and writes nothing; the worktree safety scan allows it. GitHub itself is asked
over HTTP (`GitHubService`, `crates/github/src/service.rs`); the only `gh` process is `gh auth token` of the chain.

The token of the app's own sign-in (and of the YouTrack sign-in) is a secret of `sync_utils::secret_store`. On macOS,
once the app or `bd` called `enable_keychain` at its start, it is a generic password of the login keychain, service
the app data directory's name, account the file's stem (`github-token`, `youtrack-token`), written and read by
`/usr/bin/security` as the `gh` CLI's go-keyring does: `security -i` with `add-generic-password -U … -w <hex>` on
stdin (never in the arguments a process listing shows; hex, because `-w` prints a secret with other bytes as hex),
`find-generic-password … -w`, `delete-generic-password` (exit 44: no item). The item trusts `security`, which Apple
signs and an update of Branch Deck never changes, so an unsigned build asks nothing after an update. A `security`
that fails or waits 10 s (a locked keychain) leaves the secret in the 0600 file; a file the keychain lacks is copied
into it on the first read and stays for an installed earlier version until the next write or sign-out removes it.
The tests never call `enable_keychain`: a test that needs the keychain runs a fake `security`
(`secret_store_test.rs`).

#### Repository settings

Settings → Repository shows and writes four keys of the repository's git config, Settings → Diffs & Merge the
fifth, `branchdeck.resolveByWords`, and Settings → Agents the two rules of coding agents
(`crates/sync-core/src/repository_config.rs`):
`branchdeck.stacks`, `branchdeck.symbols`, `branchdeck.githubRepository`, `branchdeck.branchPrefix`,
`branchdeck.resolveByWords` (a git boolean, on when unset: whether a sync resolves a conflict of git by words, see
"Resolve by words"), `branchdeck.agentLease` (a git boolean, on when unset: an agent works in a lease of the pool and
lands each commit with `bd pick`) and `branchdeck.agentSubjectBranch` (a git boolean, off when unset: an agent starts
each subject with its `(branch)`). `bd` reads the same keys, so the app keeps nothing of them in its own settings; the
two agent keys only `bd workspace` reads ([bd workspace](../../spec/cli/workspace.spec.md)): no sync, no operation.

- **Read**: one `git config -z --show-scope --get-regexp '^branchdeck\.(stacks|symbols|githubrepository|branchprefix|resolvebywords|agentlease|agentsubjectbranch)$'`
  (exit 1: none is set), `--global` in `.` without a repository. `-z` separates the entries with NUL, and the key from
  its value with a newline; a key without `=` (an implicit `true`) has no newline. Git prints the scopes in its order
  (system, global, local, worktree, command), so the last entry of a key is the effective one; `local_set` says whether
  the local config has one, the setting's `invalid` why its value is not taken (a git boolean as `git config
  --type=bool` reads it: `true`/`yes`/`on`, `false`/`no`/`off` in any case and without white space around, the empty
  text is false, else an integer of `git_parse_int`: `strtoimax` with base 0 as C17 has it (no C23 `0b`, which only
  the git of glibc 2.38+ reads; the libc of the user's git is unknown), an optional `k`/`m`/`g`, within a C
  `int`; `parse_git_bool`, tested against git itself). The GitHub repository found without
  `branchdeck.githubRepository` (`detect_github_repository_source`: the push URL of `origin`, then the Space mirror)
  costs one or two more processes, only for this read. Each setting also carries what applies once the local value is
  unset (`fallback_value`, `fallback_scope`: the last system or global entry, else the default), for "Use Default" or
  "Use Global".
- **Stacks off**: with stacked branches on, the read groups the commits ahead of the baseline as a sync would with them
  off (`flat_name_clashes`: baseline detection, one `cat-file --batch` for the issue groups, one `log`; stacked
  branches off, nothing of this runs). A `(auth/login)` is then the flat branch `auth/login`, whose ref git cannot keep
  beside the `auth` of `(auth)` (a directory/file conflict of refs), and the sync would fail
  (`turning_stacks_off_fails_the_sync_on_the_flat_child`); the page refuses the switch with those names
  (`stacks_off_clashes`). A failed grouping (no baseline yet) is no clash: the sync then says what fails.
- **One read per operation** (`ConfigSnapshot`, `crates/sync-core/src/config_snapshot.rs`): one
  `git config -z --show-scope --get-regexp` of the five keys an operation reads (not the agent keys) and every `branch.<name>.description` (the branch name
  keeps its case, which `--get-regexp` keeps for a subsection), with the accessors `stacks`, `symbol_mode`,
  `resolve_mode`, `branch_prefix`, `github_repository` and `descriptions`. A sync reads it once for the stack setting and the
  description titles, or takes the one its caller read (`SyncOptions::config`): `bd` reads it once per command
  (`RepoContext::config`), for the branch prefix and then for its sync, `bd push` and `bd request-review` (the report
  carries it), and again after a wait for the repository lock, when another process may have changed it.
  `read_symbol_mode` and the configured GitHub repository of the pull requests are one snapshot each, for a caller that
  needs one key; `stacks_enabled` (`history_rewrite::stack`, which knows no `ConfigSnapshot`) is one
  `git config -z --get-regexp` of `branchdeck.stacks` alone, the last entry read as `parse_git_bool` reads it
  (`sync_utils::git_bool`), so it says what `ConfigSnapshot::stacks` says. `get_branch_prefix_from_git_config_sync` keeps its own read for the app, with its
  errors (a repository that is not accessible).
- **Write**: the local config (`.git/config`, shared by every linked worktree): `git config --local
  --replace-all <key> <value>`, or `--unset-all` (exit 5, nothing to unset, is a success). A boolean is written as
  `true` or `false`, never left to an absent key, so a local `false` wins over a global `true`. The read before the
  write refuses a key that the worktree config or the command line (`git -c`, `GIT_CONFIG_*`) sets: they win over
  `.git/config`, so the write would change nothing. A branch prefix is normalized and validated as the app's field does
  (`domain_types::branch_names::is_valid_branch_prefix`), a GitHub repository is `owner/name` without whitespace; an empty
  text unsets. The answer is the settings read again, and the main window gets `repository_config_changed`
  (`{repositoryPath, key}`) to sync; the agent keys sync nothing.
- **Global write**: `set_repository_config` with `scope: "global"` (`WriteScope`, local when omitted) writes the same
  key to the user's global config instead: `git config --global --replace-all`, or `--unset-all`, with the same
  validation. No scope refuses it, because it is the value of every repository; a repository whose local, worktree or
  command value sets the key keeps that value, and the answer, the settings read again, shows that scope. Without a
  repository the answer is the global config alone. The test server gives each test repository a global config of its
  own (`GIT_CONFIG_GLOBAL`, `AppState::global_config_of`), which its config reads and writes use, so that no test
  reads or changes the config of the developer, and two tests that run at once share none.
- **Branch prefix**: the backend's order (`resolve_branch_prefix`, used by `bd`) puts the specific prefix first: the
  repository's own (local, worktree or command scope), then the app's `globalUserBranchPrefix`, then the global or system one.
  `get_branch_prefix_from_git_config_sync` reads the value with its scope in one `git config -z --show-scope --get`.

No other git config is written by the app, except the branch descriptions of `bd describe` and the global write above.
`bd install` reads the agent keys of the global config alone (one `git config --global`), for the `agents` of its
output and the app's toast.

#### Push lease

The app's push (`push_branch`) and `bd push` share one lease (`crates/sync-core/src/push_lease.rs`), so a push from
either one leaves the next push of the other valid. A push replaces exactly the remote state the user saw:

- The remote status of a branch names the commit it was computed against (`remote_tip`) and lists the commits a push
  drops ([Remote Status](remote-status.md)); older versions of commits amended or restacked here, or of commits the
  baseline has above the remote branch (landed: one more `rev-list --fixed-strings --author=…` of the baseline, only
  while commits are left dropped), the remote copies of reworded commits and the commits of the branches a stacked
  branch is built on that its remote branch lacks are counted apart (`commits_amended`, `commits_restacked`,
  `commits_landed`, `commits_reworded`, `ancestor_commits_ahead`), a push replaces them without asking. The push button sends that commit back
  (`expected_remote`, null when the row showed no remote branch), and `prepare_push_expecting` leases on it. A Force
  Push drops what the row listed and nothing else, also when Branch Deck's last push is older than someone else's
  fetched commit; a plain push after the user brought that commit into the branch goes through as well.
- The sync `bd push` runs first gives it the same view, so `bd push` leases on the sync's `remote_tip` too.
- A background fetch that moves the remote-tracking ref can't defeat the lease the way a bare
  `--force-with-lease=<ref>` can: the status computed after it lists the fetched commits as dropped (`behind`), and
  a push replaces them only once the user saw them (a Force Push, `bd push --force`).
- The push names the tip it read (`<tip>:refs/heads/<branch>`), so it pushes exactly that commit even if the branch
  moves meanwhile.
- Push Up to Here (`StackPushBranch::tip`, [Stacked Branches](../../spec/branch/stacked-branches.spec.md#push-up-to-here))
  pushes a prefix of the branch: the refspec names the chosen commit instead of the tip, with the same lease, and the
  commits after it stay unpushed. `check_push_tip` checks it with one `rev-list --first-parent <branch> ^<base>` (the
  parent's branch, or the baseline): a virtual branch is a line of cherry-picked commits, so the order of that list
  says that the commit is one of the branch's own and that the remote head is an older one of them, a fast-forward. A
  commit outside the branch, one the remote already has, or a remote head that is no commit of the line (the branch was
  rewritten since its push) refuses the push before anything goes out. The ancestors go whole in the same atomic push
  (`pushUpTo`, the order of a Push Stack, "App stack push" below); `bd push` has no cut.
- Both push through `push_leased` (`crates/sync-core/src/push.rs`) and `push_refs` under it (`push --porcelain` with
  the leases and the refspecs, `execute_remote_command_*`, `parse_push_porcelain`); the app's push passes a progress
  callback (`--progress`, reported to the push button, "Transfer progress" under Update project), `bd push` none (no
  one to show it to). A push of several refs is `--atomic` in both (below); one ref goes without it, which a remote
  may not support.
- A rejected ref is `!` in `push --porcelain` (exit 1); the summary is only logged. `fetch_remote_branches` then
  fetches just the rejected branches, `+refs/heads/<b>:refs/remotes/<remote>/<b>` with the flags of the baseline fetch
  (`--porcelain --no-tags --no-write-fetch-head --no-recurse-submodules --no-auto-maintenance`), so the caller shows
  what the remote has now. A branch deleted on the remote fails the fetch: `ls-remote --exit-code` (2: no such ref)
  tells it from a failed connection, and its remote-tracking ref is deleted.
- `push_branch` returns `PushOutcome::RemoteMoved` with the new remote status when the fetched head differs from the
  expected one: the row turns into the push the remote now needs, and a toast offers it (a push that drops the fetched
  commits confirms in the row's push button first, as any such push does). When the fetched head is the
  expected one, the lease held and the remote refused for another reason (a hook, a protected branch): an error.
  `push_branch` is the stack push of one branch (`push_branches`), so both answer alike.

#### App stack push

Push Stack, Push with Children and Push Up to Here push several branches of a stack, parents first. They go in one
atomic push (`push_branches`, the Tauri command of `src/lib/git/stack-push.ts`), as `bd push` pushes, never one push
per branch: a push that stopped at a failure left the stack half pushed, a child on the remote on a parent the remote
lacks, and its pull request showing the parent's commits.

1. The page waits for a running sync and for a push of one of the branches from another row, then computes the order
   on the branches as they are (`stackPushOrder`, `stackChildrenPushOrder`, the steps of Push Up to Here). The holds
   are checked per branch (`checkPushHold`; a held one refuses the whole push, the robot a Safe Push start replaces is
   canceled once), and an open pull request that needs a retarget is retargeted, as before a push of one branch.
2. `push_branches` first refuses a branch the last sync did not build (`not_built_refusal`, by `SyncReport::not_built`,
   which the Tauri commands take from the table of `RepositoryViews` and `bd` from its own sync): a conflict, an error
   or a ref another process moved leaves the virtual branch at an older build, of commits the branch may no longer
   have, and a push pressed while that sync ran would push it. Then it reads the tips of all branches with one
   `for-each-ref` (`prepare_push_expecting`), each leased on
   the remote head its row showed, checks a branch pushed up to a commit (`check_push_tip`, one `rev-list`), and
   pushes them with one `git push --porcelain --atomic --progress` (`push_leased`). The progress of the one push is
   the progress of each of its branches: every row shows it, the row it started from names it ("Pushing 7 branches
   45%").
3. Pushed: one `for-each-ref` reads the local and the remote tips of every branch, one `reflog` their push times
   (`compute_remote_statuses`); no walk, each remote branch is at its tip. The page updates every row, records an
   Activity entry per branch, refreshes the pull requests once and shows one toast.
4. Refused: nothing was pushed. Every branch is fetched, their statuses computed the same way, and the first branch
   (parents first) whose fetched head is not its lease is named (`PushBranchesOutcome::RemoteMoved`): every row shows
   what its remote has now. When that remote has no commits its branch lacks (`pushesOverMovedRemote`), the page pushes
   the stack again at once, leased on the fetched heads, once; else the toast of that branch offers its push over it.
   When every lease held, the remote refused for another reason: an error, as for one branch.
5. The stack branch of the whole stack (`{prefix}/stack/<root>`) goes the same way, as a branch of `push_branches`
   with `wholeStack` (no parent, no tip), leased on the remote head of its remote status, only while the sync built it:
   alone by the start of the Safe Push of the whole stack and the restart of its robot, the creation of its pull
   request and Request Review on it (`pushWholeStack`), the branches of the stack left as they are; and after the
   branches by Push with Children of the root (`getStackBranchAlong`), once it is on the remote or the pull request of
   the whole stack is open, so that pull request follows its branches, as `bd push <root> --with-children` does. A
   refusal names it apart from its root's virtual branch (`PushBranchesOutcome::RemoteMoved::whole_stack`). One push
   of it runs at a time: a second request while one runs joins it (`runningStackBranchPushes`) and then pushes only
   what the remote still lacks, so no `push_branches` goes for a stack branch its remote already has.

Code: `crates/sync-core/src/push.rs`, `src-tauri/src/commands/push.rs`, `src/lib/git/stack-push.ts`.

#### Patronus review pre-check

Read-only. Before a Safe Push, the code-owner review check of Patronus needs the commits of the virtual branches with
the paths they change ([Patronus](../../spec/branch/patronus.spec.md#code-owner-review)). Patronus is the only judge of
whose approvals count: the app and `bd` ask it with the pull request the robot would start from and render its answer
(`patronus::review_readiness`); nothing re-implements its rules (bots, eligibility, self-approval, the latest review).

- One `git log -z --topo-order --max-count=<501 per tip> --format=… ^<baseline> <tip>…` for all branches asked
  (`read_review_commits` in `crates/sync-core/src/patronus.rs`): hash, parents, author, committer, the message. Each
  branch gets the commits its tip reaches through the parents in that log (a stacked branch shares those of its
  parents); `--topo-order`, since equal commit times of several tips leave the date order to chance. A log cut by its
  limit can't tell which tips it covers whole: then one `git log` per tip.
- The paths from the `commit_files` store of the object cache, with one `diff-tree --stdin` for the commits it misses
  (`fill_changed_paths`, `read_commit_files_cached`), as `git log --no-renames --name-only` lists them: a rename is a
  delete and an add, whose owners both review it; a deleted path stays in the list; a merge has none.
- 501 commits of a branch mean "too many": its check is skipped, as the IDE skips it at 500.
- The answer is kept in the `review_answer` store of the object cache (below), keyed by a hash of what Patronus decides
  on: the server and the token, the Space repository, the target, the sorted paths, and with a pull request its
  number, author and reviews. A rebase onto a new baseline keeps the key, a new review changes it. A kept answer
  costs the log and no request; a pull request the app shows sends its reviews along, so no backend reads GitHub for it.
- Starting a robot pushes nothing itself: the branch is pushed by `push_branch` first, then Patronus is asked over HTTP.
- When Patronus names a reviewer group without its paths, the union of the paths of all branches asked is given to
  the code owners (below) in one read, and a group gets the paths of its branch whose owner has its name.

#### Code owners (OWNERSHIP)

Read-only, generic: a repository without `OWNERSHIP` files has no owners. `read_ownership` in
`crates/git-ops/src/ownership.rs` reads, for a set of paths, the `OWNERSHIP` file of the root and of every ancestor
directory in the tree of the baseline in one `cat-file --batch --buffer -Z` of `<baseline>:<dir>/OWNERSHIP` names (a
missing one answers `missing`): only those files can match the paths, their rules are anchored at their directory.

- A file is sections of `/pattern` lines closed by `owner 'Name'` (none is `/**`), `#` comments, blank lines; a file
  that doesn't parse is skipped with a warning, its parents still decide.
- The rules are ordered by the depth of their file, then its path, then their order; the last match wins, as
  Patronus resolves owners (`OwnershipMappingBuilder`, `OwnershipMapping.getMatch`). The matcher is jgit's
  `FastIgnoreRule.isMatch(path, false)`: a rule matches the path or a parent directory of it, `**` is any number of
  segments (a trailing one at least one), a trailing `/` a directory.
- Callers: the Patronus pre-check (above) and `get_file_owners` of the branch review window, once per path.

#### Branch review

Read-only. The review window of a branch ([Branch Review](../../spec/branch/branch-review.spec.md)) diffs parts of its
stack: `get_branch_review` (`crates/sync-core/src/branch_review.rs`) runs one `for-each-ref` of the virtual branches
of the chain (root first) for their tips, then one `git log --first-parent --max-count=2001 ^<baseline> <last tip>`
and cuts it at the tips: a child is built on the head of its parent (`release_children` of the sync), so the log
passes through every parent's tip. The base of the root is the first parent of the oldest commit, the range the
pre-check reads. A tip missing from the log is a child built on an older head of its parent: an error that asks for a
sync. The files of a part are `DiffSubject::Range` (`get_range_files`, kept in memory, see "Commit files"), and its
diffs come from `get_file_diffs`. The whole stack (`whole_stack`, the chain its root alone) is the same two processes on the stack
branch `{prefix}/stack/<root>`: one part of every commit of the stack.
- **Read ahead.** `prefetch_branch_review` reads what the window asks for first in the background: the review (through
  the memo) and the files of the part it shows (the branch on its parent, or the whole stack), so the
  window finds them in memory. The main window asks once per state of the branch (the chain and its heads): when the
  pointer rests on its row for 200 ms or the row gets the focus (`intent` of `usePreviewHotkeys`), and on the open,
  beside the start of the window. The test server answers it with nothing: the E2E read git on every read.
- **Memoised by what it reads.** The `for-each-ref` also lists the refs the baseline's name may stand for
  (`refs/<name>`, `refs/tags/`, `refs/heads/`, `refs/remotes/`), so one process gives the tips and the baseline's ids.
  The app's `BranchReviewMemo` (32 reviews) keys a review by the params, the tips and those ids, and runs the `log` only
  for a key it hasn't seen: a moved tip (a sync) or baseline (a fetch) is another key, and the commits are objects. A
  baseline that names no ref is never memoised. In the monorepo the `log` took about 10 ms at a load of 18.
- The ownership read of the window (`get_file_owners`, one `cat-file --batch` of the `OWNERSHIP` files) is timed in
  the debug log and not memoised: it took 15–28 ms in the monorepo, about the cost of the `rev-parse` a key by the
  baseline's commit id would need.

#### Robot commit map

Read-only. The prompt for a failed robot ([Safe Push Failure Prompt](../../spec/branch/safe-push-failure-prompt.spec.md))
says which commits the robot checked, by the hashes of the checked-out branch: the robot ran on `source_revision`, the
tip of the virtual branch when it started, and since then the branch may have been amended, extended, rebuilt on a
newer baseline, or a commit of it may have landed. `robot_commit_map` (`crates/sync-core/src/robot_commits.rs`), the
same for the app and `bd robot`, runs one process per phase:

1. One `cat-file --batch-check` of the source revision and `refs/heads/<prefix>/virtual/<name>`. A source revision the
   repository lacks (the robot started on another machine, a gc pruned it) lists the commits of the checked-out branch
   as `unknown`; no walk runs.
2. The commits of the branch and of its stack ancestors on HEAD with their mapping notes: the stack setting and one
   `cat-file --batch` of the issue groups and stack parents, one `log` of `<baseline>..HEAD` as the sync reads it,
   grouped by `branch_of`. The robot of a stacked branch builds the whole stack, so the map covers the ancestors'
   commits too, each with its branch: a failure may come from a parent's commit.
3. One `rev-list ^<baseline> <virtual branch>` and one
   `rev-list --left-right --cherry-mark ^<baseline> <source>...<virtual branch>`, both the walk of the remote status
   (`walk_commits`, `crates/sync-core/src/commit_walk.rs`: the mark, the id, the author and the subject). A commit both reach (the symmetric
   difference leaves it out, so the first walk is needed) or a patch-equivalent copy (`=`) is `tested`.
4. A new commit (`>`) and a robot commit (`<`) with one identity (`CommitKey`) are compared by `same_changes` of the
   remote status: one `diff-tree --stdin -p -U0` and one `patch-id --stable`, none without such pairs. The same
   changes are `rebased`, others `changed`. The patch ids of `--cherry-mark` include three lines of context, which a
   rebuild on a newer baseline moves when the baseline changed lines near the change; without context only the
   changed lines count (a patch id ignores the line numbers).
5. The other robot commits are `landed` when a baseline commit since the source revision has its identity, else
   `removed`: one `rev-list --fixed-strings --author=<email>… <baseline> ^<source>`, run only
   when such commits remain. Identity, not patch id, as for [landed commits](integration-detection.md): Patronus
   rebases the commit onto a newer target, which can move its context. The other new commits are `added`.

A virtual commit is shown by the HEAD commit whose mapping note names it (a note copied onto a commit with another
tree, `notes.rewriteRef` on an amend, doesn't count), else by the HEAD commit with its identity. A HEAD commit no note
maps into the virtual branch was made or changed after the last sync: `virtual_behind_head`, and a commit shown by it
is `changed`, not `tested`. A missing virtual branch (archived) leaves every robot commit `landed` or `removed`.

The robot of the whole stack ran on the stack branch `{prefix}/stack/<root>`: `stack_robot_commit_map` runs the same
phases on it, the members of the stack (root first) as the branches of phase 2. The stack branch has no mapping notes,
so every commit is shown by the HEAD commit of its identity; a HEAD commit of a member that no commit of the stack
branch shows is `virtual_behind_head`. A HEAD commit amended since the last sync keeps its identity, so it is not told
as changed: the next sync rebuilds the stack branch.

The app also asks the map for a running robot once the branch it tests changed since it started (`get_robot_commit_map`,
`robot_subject_commit_map`): the commit rows mark the commits it does not cover ([Patronus](../../spec/branch/patronus.spec.md#commits-a-running-robot-covers)).
The same phases run once per tip of that branch, never during a sync.

#### Robot landing

Read-only. What a merged robot landed of the branches as the table has them ([Landing](landing.md), "Landed by
content"), asked by the repository view (below) for the robots the hashes don't answer: `robot_landings_of_grouped`
(`crates/sync-core/src/robot_landing.rs`), the phases of the robot commit map for many robots at once.

1. The commits of the branches as the sync groups them: the caller passes the grouped branches of its table, so this
   phase runs no process.
2. One `cat-file --batch-check` of every robot's source revision and of the tip it ran on: the tip the caller's table
   has (`RobotLandingRequest::tip`), not the ref, which a sync may move meanwhile, so the answer is of that source and
   that tip each time; a source the repository lacks lands nothing, a tip at the source needs no walk. At the source
   every commit the sync built into the ref landed (its mapping note names its copy); one without a note was made
   since.
3. Per other robot the two walks of phase 3 above (`SourceWalk`), and one `same_changes` (one `diff-tree --stdin -p -U0`
   and one `patch-id --stable`) of the pairs of every robot, none without such pairs.
4. No process: each HEAD commit takes the state of the commit of the ref with its identity (`CommitKey`), and a branch
   landed when its oldest commits are `tested` or `rebased` and the rest only `added` (or not in the ref yet): those
   are after it, the work left. A `changed` commit, or an added one below a tested one, leaves the branch not landed.

The repository view asks only for a robot whose hashes don't tell, once per state of the branches (`ContentMemo`).

#### Robot span

Read-only. What a running robot lands of the branches as they are now, so that a push is held only for the commits
the robot's commit has ([Patronus](../../spec/branch/patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)):
`robot_spans_of_grouped` (`crates/sync-core/src/robot_landing.rs`), asked by the repository view (below). The same
requests and phases 1 to 3 as the robot landing. Phase 4 reduces otherwise: a branch is in the span while one of its
commits is `tested`, `rebased` or `changed` (the robot has an earlier version: an amend below its commit), with those
commits and the amended ones among them. A parent a restack placed below the robot's branch after the start has only
`added` commits: it is not in the span, and its push goes on. A request the repository can't answer (a missing source,
a gone ref) has no span (`branches: null`): the stack as it is now holds. Not looked for: a commit moved out of the
robot's branch into a branch that is no ancestor of it, as the robot landing doesn't look for it either.

### Repository view

Read-only. What Rust derives of the branch table for the page and for `bd`, so both read one rule: the crate
`repository-view`. `BranchTable` (`branch_table.rs`) is the table a sync left: the grouped branches in the order of the
table, their virtual tips, the copies the sync wrote (`SyncReport::copies`, recorded from its `CommitSynced` events: a
mapping note read at the sync's start names the copy of before), the stack branches and the parked branches; `bd`
builds it from the report of its sync (`bd status` from a grouping). The app keeps the table of the last sync of each
repository and branch prefix (`RepositoryViews` in `views.rs`, written by `sync_branches` from the report it no longer
drops; a failed sync keeps the one before) and answers `get_repository_view` from it, with the robots, their matches
and the remote facts the page sends: a push moves a remote branch without a sync, the page knows it, the table doesn't.
The E2E server keeps it the same way from its own sync.

The Patronus section (`patronus_view.rs`): what the robots land, landed and hold, and why a start waits (the rule in
[Patronus](../../spec/branch/patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs)). Its inputs besides the table
are the robots, the robot of each branch and whole stack (`match_robots`, which the page does itself) and what the asker
knows of the remote branches (`RemoteFacts`: the remote tip, and whether the remote has the branch by content). No
process when the hashes tell:
- the robot took its subject as it is: its tip, or its remote branch while that is up to date by content (a remote
  rebuilt on an older baseline included);
- a merged or running robot of a branch took a commit of it the branch built new ones on: its copy is a copy of a commit
  of the branch now (`BranchTable::copy_of`), and every commit below it has one;

else the robot landing and the robot span ask the content, one call each for every robot the memo lacks
(`ContentMemo`, by the robot's commit, the tip of its subject and the commits of the candidate branches with their
copies): a view of the same state asks no git again. A git error is an answer of nothing by content: a merged robot
landed nothing, a running one lands what the stack as it is now says. `patronus_view_content_test.rs` pins the
processes: none for a robot at its tip or under new commits, one `cat-file --batch-check` and two `rev-list` after a
restack, none for the same state again.

The page asks (`askRepositoryView` in `src/stores/patronus.ts`) only when an input changed: the robots (their record,
which a poll that changed nothing keeps), the matches, the remote facts, or what the last sync read (the fingerprint of
its inputs, `syncedInputs`: a sync of the same inputs left the same table, so it asks nothing). During a sync only a
change of the robots asks, from the table of the sync before. An answer equal to the view the page has changes no
store, so no row reads it again.

The landing needs of each branch (`BranchPatronus.landingNeeds`, `{state: unknown} | {state: known, refusals}`) are a
fact with its inputs: the baseline, the tips of the table and the commits merged Safe Pushes landed (`landedBy` of the
same view). A view whose inputs have no answer says `unknown` for every branch and starts their computation in the
background (`RepositoryViews::landing_needs`, a thread of the view's span, one trial session for every branch,
[Landing gate](#landing-gate)); a view asked meanwhile with other inputs is computed next, by the same thread; a
failure leaves them unknown until the inputs change, never an empty list. `landingGeneration` counts the finished
computations: while a branch's needs are unknown the page waits for the next one (`wait_landing_needs`, which returns
once a computation finished after the view's, or none runs; at most 120 s) and asks the view again. A view of the
same inputs answers from the last computation: no process. `views_test.rs` pins both: unknown before, known after
one computation, none again.

### Index policy

When a rewrite changes HEAD's tree, the index has to follow. Otherwise `git status` shows the rewrite reversed as
staged changes. `IndexUpdate` in `crates/git-ops/src/rewrite_guard.rs` has two forms.

- **Drop: two-way merge.** `read-tree -m -i <old> <new>`, without `-u`:
  - entries that matched the old HEAD take the new HEAD's entry;
  - staged changes on untouched paths and intent-to-add entries are kept;
  - the working tree is never read or written.

  Before any ref moves, the same command runs on a copy of the index (`GIT_INDEX_FILE`). If a staged path is also
  changed by the drop, git fails ("Entry 'x' would be overwritten by merge"), and the drop is refused. The exit code
  decides; the message is not parsed, since a quote in a path would cut it. Only then two `-z` diffs name the paths:
  `diff-index --cached --raw -z <old>` (staged unlike the old HEAD) and `diff-tree -r --raw -z <old> <new>` (changed
  by the drop), the paths of both whose staged mode and blob are not the new HEAD's. When none is left (an unmerged
  index), the refusal says so without paths.
- **Amend: stage the amended files.** HEAD now holds the working-tree versions of the selected files, so exactly
  those are staged (`update-index --add --remove -z --stdin`), like `git commit -- <paths>` does. Other staged
  changes stay staged.

Both end with `update-index -q --refresh`, which restores the stat data so that `git status` does not rehash the
files. A failure after the ref moved needs a concurrent `git add` and is rare. It is returned as an error with a
recovery hint (`git reset -q`).

### Key Decisions

1. **Decision**: `git replay` for content rewrites (amend, drop).
   **Rationale**: it applies commits in memory, never touches the working tree, and needs one process per run of
   commits. The cost: the committer of replayed commits is the current user, as with `git rebase`.

2. **Decision**: our own `fast-import` writer for reword and move.
   **Rationale**:
   - `replay` cannot change messages;
   - `commit-tree` costs one process per commit and gets the date only as unix seconds, losing the timezone;
   - `fast-import` writes exact `author`/`committer` lines and `data <len>` messages.

3. **Decision**: refuse a range that contains merges.
   **Rationale**: `replay` dies with "replaying merge commits is not supported yet!", and the writer recreates a
   single parent. The previous `rebase --autosquash` fallback touched the working tree and flattened the merges.

4. **Decision**: Update Project writes the working tree with a guarded two-way `read-tree -m -u`, the only exception to
   invariant 1. Its undo runs the same command and guards in the other direction (N to H), not a second mechanism.
   **Rationale**:
   - `pull --rebase`, `rebase --autostash` and `stash` + `checkout` write every file of the rebase, stop halfway on a
     conflict and leave the user in a rebase, or a stash pop conflict, that the app can't finish;
   - replaying in memory first turns a conflict into a result with nothing changed, and the dry run on an index copy
     proves the files can follow before the branch moves;
   - two-way `read-tree` touches only the paths that differ between the old and the new HEAD, so unrelated local
     changes survive without being stashed;
   - two-way `read-tree` is symmetric: from N back to H, the dry run refuses every path the user edited after the
     update, so the undo is as safe as the update. Only the ignored-files check must run on the swapped trees, because
     files that the update deleted come back.

5. **Decision**: do not use `git history` (2.54+, experimental).
   **Rationale**:
   - `drop` updates the index and the working tree;
   - `reword` opens an editor and changes one commit per run;
   - it does not support merges.

6. **Decision**: our own transaction, rather than `replay --ref-action=update` or `fast-import` updating the branch.
   **Rationale**:
   - only our transaction checks the old value of the checked-out branch;
   - it writes a reflog message;
   - it combines related refs atomically;
   - it deletes the temporary ref in the same step.

7. **Decision**: push with `--force-with-lease=refs/heads/<branch>:<expected>`, where `<expected>` is the remote head
   of the status the user saw (the app's row, the sync `bd push` runs first, the status `bd push --remote` computes,
   `--expect-remote`) ("Push lease"). A rejected push fetches the branch and reports what the remote has now.
   **Rationale**: a virtual branch is rebuilt on every sync, so the push must force. The lease refuses to overwrite a
   remote branch that changed since the user looked, and a background fetch shows up in the status as commits a push
   drops, which a bare lease on the remote-tracking ref would replace silently. Leasing on the commit Branch Deck last
   pushed instead (an earlier design, `refs/branch-deck/pushed/`) made a branch stuck: once someone else's commit was
   fetched, every push was rejected, even a Force Push the user confirmed and even after the commit was brought into
   the branch.

8. **Decision**: commands that talk to a remote (push, the fetch for Update Project, and the review fetch) go through
   `execute_remote_command` in `crates/sync-core/src/remote_command.rs`. It adds
   `-c credential.interactive=false -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=30`, `GCM_INTERACTIVE=never` and
   `LC_ALL=C LANGUAGE=` on top of the executor's `GIT_TERMINAL_PROMPT=0`.
   **Rationale**:
   - the user's credential helpers (osxkeychain, Git Credential Manager, `gh auth git-credential`) keep working, so
     stored HTTPS credentials are used. The earlier `-c credential.helper=` disabled them, and an HTTPS remote that
     needed credentials failed;
   - nothing may ask for new credentials: the app has no terminal, and a hidden prompt would block forever;
   - a transfer that stalls below 1000 bytes/s for 30 s is aborted instead of hanging on a dead connection;
   - the C locale keeps the `--progress` lines untranslated, so the fetch and push progress can be read (IDEA runs
     every git command in an English locale for the same reason). An unknown locale falls back to C as well.

   SSH is left as configured. The app started from Finder has no controlling terminal, so ssh fails instead of
   prompting. Forcing `GIT_SSH_COMMAND="ssh -o BatchMode=yes"` would override `core.sshCommand` and break
   `SSH_ASKPASS` prompts.

   SSH has no low-speed abort, so a fetch the user waits for and may abandon (the review fetch) runs through
   `execute_remote_command_with_progress_until`: killed after 120 s without a stderr line, or on cancel. The review
   fetch also passes `--refmap=` and `--no-prune`, so fetching a pull request never moves or prunes a
   remote-tracking ref of origin.

9. **Decision**: a conflicting update is resolved in a session of objects and refs, one round per commit, not in a
   rebase of the working tree.
   **Rationale**:
   - in a shared working tree, `git rebase` and `git stash` move files under the other sessions and take their
     changes; a session writes the working tree only at the finish, through the guarded update;
   - all state in git (a ref, a state commit, notes) survives a context compaction of the agent and a crash, and every
     step is one compare-and-swap; the copies are rebuildable;
   - the fixed committer makes every step repeatable, at the cost of invariant 5's current time.

## Performance

| Operation | Before | After |
|---|---|---|
| Reword or move of n commits | n `commit-tree` + n `merge-base --is-ancestor` | 1 `log` + 1 `cat-file` + 1 `fast-import` + 1 `update-ref`, plus the guard |
| Drop k commits from a range of n | up to n `merge-tree` + n `commit-tree` + `reset` | at most k `replay` + 1 `update-ref` + 1 `read-tree` |
| Amend into a non-HEAD commit | n `merge-tree` + n `commit-tree`, or a rebase | 1 `merge-tree` + 1 `fast-import` + 1 `replay` + 1 `update-ref` |
| Squash the fixups of a range | a terminal `rebase -i --autosquash` | 1 `log` + 1 `rev-parse` for the plan, then the squash, with one `cat-file` more if messages are combined |
| Move a hunk across n commits | `commit-tree` plumbing by hand, per commit | 1 `log` + the guard + 1 `cat-file` + 1 `diff-tree` (+ 1 `cat-file` of two blobs) + 1 `fast-import` + 1 `merge-tree --stdin` + 1 `diff-tree --stdin` + 1 `patch-id` + 1 `fast-import` + 1 `update-ref` |
| Squash k commits into one, n commits after it | a terminal `rebase -i` | 1 `log` + 1 `cat-file` + 1 `merge-tree --stdin` (k merges) + 1 `fast-import` + 1 `replay` + 1 `rev-list` + 1 `update-ref` |
| Sync of a branch whose n commits are copied, m of them merged | m `merge-tree` + n `commit-tree` | 1 `merge-tree --stdin` + 1 `fast-import` (a conflict: 1 `merge-tree` more and the conflict analysis, 1 `update-ref` per sync for the refs that keep the copies before the conflicts; none of these for a conflicting branch that did not change, whose copies and view the object cache remembers) |
| Sync notes for n created commits | n `notes add` | 1 `fast-import` per sync |
| Existing virtual branches of a sync | 1 `show-ref` per branch | 1 `for-each-ref` per sync |
| Update check inside a sync, p paths changed by the update | an index copy, a full `update-index --refresh` and a dry run: grows with the index | unchanged state: 2 `merge-base` and an `lstat` per path, the guards, the upstream and the commits from the sync's `RepoFacts`; new state: + `rev-list`, `replay`, `diff-tree`, `cat-file`, `hash-object` of the paths not seen yet |
| The repository facts of a sync | the guards, the current branch, `remote`, the upstream and the baseline commit read again by the update check, the archive step and the detection, 15–17 processes | the guards (2) + `remote` + 1 `for-each-ref` + 1 `rev-parse`; 1 `rev-parse` more for an upstream that is no `master` or `main` |
| Conflict view | `ls-tree` + `cat-file` per revision, 1 `show` per missing commit | 1 `merge-tree` + 5: 1 `rev-list` (merge base and divergence), 2 `log`, 1 `cat-file` for all contents, 1 `diff-tree --stdin -p` for all patches; none, not even the merge, for a view the object cache remembers (`conflict_info`) |
| Remote status of a branch in a sync | 1 `rev-parse` + 1–6 walks and checks | none when the branch, its remote and the baseline did not move since a status of this process; else none from the object cache for the same commits (`bd sync`), or 2 walks (the fast-forward from the walk of the remote commits), none when the remote branch is at the branch's tip, the remote tips read with the branches; 2 more to tell an amend from a rebased copy; 1 `reflog` per sync for the push times of all branches, read only when a status needs one |
| A round of `bd update --continue`, n commits left | a terminal `rebase --continue` per commit | about 18 processes (guards, reads, 2 `fast-import`, 1 `update-ref`, 5 for `blocked_paths`; the planned 9 not reached) + 1 `replay`, ⌈log₂ n⌉ more only for a new conflict |
| File counts of an expanded branch of n commits | not shown | 1 `diff-tree --stdin` for the commits the object cache misses, none for a branch seen before |
| Patches of a list of n commits, before their review | 1 `diff-tree -p` per commit at its first diff | 1 background `diff-tree --stdin -p` for the commits the patch store misses, none for a list seen before |
| Review pre-check of k branches | 1 `git log --name-only` and 1 request per branch, cached in memory by the commits | 1 `log` for all, 1 `diff-tree --stdin` for the commits the object cache misses, 1 request per branch whose kept answer (by paths and reviews) doesn't hold |
| Review of a pull request | the browser | both commits local: 3 processes (`cat-file --batch-check`, `merge-base`, `log`); else 2–4 more to find the source, 1 `fetch` (2 for a mirror behind GitHub), at most 2 for the head state; then 1 `diff-tree` for the files and 1 for the patch |

### Process budgets

Every git process of an operation is pinned, by subcommand, in `crates/sync-core/src/process_budget_test.rs` (a core)
and `crates/bd-cli/src/process_budget_test.rs` (a whole `bd` command, its context and its config included), one test
per operation on a small fixture (two branches of three commits, unless the test says otherwise). A change that saves a
process lowers a number there, so the diff of the assert names the process that went away; a change that adds one must
raise it. The totals, when each test was first pinned (the audit of the git layer, 2026-10-17; the tests marked * later
in it; † with the remote statuses or the conflict chains in the object cache, 2026-10-18, first pinned is the code
before it; ‡ with the stack branches, 2026-10-18; § with the landing of a stacked branch by `bd safe-push`, 2026-10-18,
first pinned is the refusal before it, whose trial build alone opened a second trial session; ¶ with the Safe Push of
the whole stack, 2026-10-18; ** with the whole stack in the app, 2026-10-18) and now:

| Operation | Test | First pinned | Now |
|---|---|---|---|
| First sync | `a_first_sync` | 37 | 22 |
| Unchanged sync | `an_unchanged_sync` | 25 | 12 |
| Unchanged sync of a conflicting branch, without / with the object cache | `an_unchanged_sync_of_a_conflicting_branch` * | 20 / 13 | 19 / 12 |
| A branch that conflicts after a clean commit, with the object cache: its first sync, each next one | `an_unchanged_sync_of_a_conflict_after_a_clean_commit` † | 34, 14 | 35, 12 |
| Unchanged sync of a pushed and a fetched branch in a new process (`bd sync`): the first, each next one | `an_unchanged_sync_of_pushed_branches_in_a_new_process` † | 16, 16 | 15, 13 |
| First sync of a stack that forks (three branches and the stack branch) | `a_first_sync_of_a_forking_stack` ‡ | 30 | 30 |
| Unchanged sync of a stack that forks, the same process / a new one with the object cache | `an_unchanged_sync_of_a_forking_stack` ‡ | 12 / 12 | 12 / 12 |
| Trial sync (the core of `bd check`), empty / filled object cache | `a_check` | 18 / 16 | 18 / 16 |
| Trial sync after a sync | `a_check_of_synced_branches` * | 16 | 15 |
| The checks of a session after its open: the first, each next one | `the_second_check_of_a_session` * | 4, 0 | 4, 0 |
| Amend into an older commit | `an_amend` | 19 | 17 |
| Unapply a branch, its files following (the build checked, the dry run of the files; 2026-10-21) | `a_drop` | 19 | 35 |
| Undo an unapply (2026-10-21) | `an_undo_of_an_unapply` | 20 | 20 |
| Reapply an unapplied branch, its prefixes restored (2026-10-21) | `a_reapply` | 39 | 39 |
| Undo a reapply (2026-10-21) | `an_undo_of_a_reapply` | 20 | 20 |
| Squash two commits | `a_squash` | 18 | 14 |
| Squash fixups | `a_squash_of_fixups` | 31 | 24 |
| Update check, Update Project | `an_update_check`, `an_update` | 20, 38 | 20, 38 |
| App stack push of two branches (`push_branches`; first a push per branch) | `an_atomic_push_of_a_stack` * | 8 | 4 |
| The text of the app's pull request of the whole stack (`read_new_stack_pull_request`) | `the_text_of_a_new_pull_request_of_the_whole_stack` ** | 1 | 1 |
| What a merged robot landed after a change below its commit (`robot_landings`, 2026-10-18) | `the_landing_of_a_merged_robot_after_a_change_below_its_commit` | 8 | 8 |
| The target conflict check of a Safe Push start without its fetch, clean / one conflicting commit (2026-10-19) | `a_target_conflict_check` | 2 / 6 | 2 / 6 |
| `bd check`, with `--build` of two paths | `a_check`, `a_check_with_a_build` | 20, 26 | 20, 23 |
| `bd push --dry-run` | `a_push` | 54 | 25 |
| `bd safe-push --check` (the fetch and the replay of the target conflict check since 2026-10-19: 8 more) | `a_safe_push_check` * | 57 | 40 |
| `bd safe-push --check` of a stacked branch, with its trial build alone | `a_safe_push_check_of_a_stacked_branch` § | 40 | 43 |
| `bd safe-push --whole-stack --check` of a stack that forks (three branches and the stack branch) | `a_safe_push_check_of_the_whole_stack` ¶ | 39 | 47 |
| `bd fold` | `a_fold` | 36 | 30 |
| `bd pick --from --follow` (2026-10-18; the grouping before the move, 2026-10-19) | `a_pick` | 54 | 55 |
| `bd pick --dry-run` of a fast-forward into a branch the target has: the dependency check (2026-10-19) | `a_pick_into_a_branch_the_target_has` | 28 | 28 |
| `bd lease get` of a free slot, then `bd lease return` (2026-10-19) | `a_lease_get_and_return` | 5, 4 | 5, 4 |
| `bd status` (the push URL of `origin` for `landed_by` since 2026-10-18) | `a_status` * | 10 | 8 |
| `bd status` of a stack that forks | `a_status_of_a_forking_stack` ‡ | 7 | 8 |
| `bd robot`, `bd request-review --check`, `bd group --dry-run` | `a_robot`, `a_request_review_check`, `a_group_dry_run_with_a_description` * | 8, 11, 11 | 8, 11, 11 |

`bd sync` runs the processes of its core and one `rev-parse` for its context (`a_sync_with_the_prefix_of_the_config`).
On a clone of a monorepo (217 commits ahead of the baseline in 38 stacked groups, 16 of them conflicting), a release
`bd` before the audit and after it, the same outputs, the processes `bd -v` counts and the median wall time of three
runs on a loaded machine (noisy):

| Command | Before | After |
|---|---|---|
| `bd sync`, every branch built | 254 processes, 3.5 s | 168, 2.1 s |
| `bd sync`, nothing changed | 228, 2.3 s | 36, 0.45 s |
| `bd status` | 11 | 7 |
| `bd commits`, warm object cache | 13 | 11 |
| `bd check`, cold / warm object cache | 72 / 53 | 50 / 31 |
| `bd push --dry-run` of one stacked branch | 245 | 39, 0.58 s (148 right after the object cache was emptied) |
| `bd push --all --dry-run` | 238 | 37 |

An unchanged sync reads every fact once (`RepoFacts`, `ConfigSnapshot`) and asks git nothing for a clean branch whose
commits, remote and baseline did not move. Its 36 were about 13 fixed reads, the remote status walks of the 5 branches
whose remote tip differs from the local one (12 `rev-list`, 5 `reflog`), and the chains of the conflicting branches,
copied again (3 `merge-tree --stdin`, 3 `fast-import`; see "Follow-ups"). Since the remote statuses are kept in the
object cache (`remote_status`), a `bd sync` of commits that did not move walks no branch: on the same clone with the
remote branches of 2026-10-18 (14 of the 38 branches on the remote), an unchanged `bd sync` ran 32 processes before
(10 `rev-list`, 3 `reflog`; 0.47–0.92 s) and 20 after (the 13 fixed reads, one `reflog` for the push times of all
branches and the chains of the conflicting branches; 0.14 s), the same output. Since the conflicting branches take
the copies before their conflicts from the object cache (`conflict_chain`), measured with `pnpm perf:monorepo` on that
pin: an unchanged `bd sync` 20 → 14 (no `merge-tree`, no `fast-import`: the 13 fixed reads and one `reflog`),
`bd push --dry-run` of one branch after it 23 → 17, `--all` 21 → 15; the full sync 162 → 163 (one `update-ref` for the
refs that keep the copies), every output the same.

The stack branches (2026-10-18), on the same clone: its one stack that forks (11 branches, 63 commits) conflicts at
its first commit, as its root does, so a full `bd sync` ran 162 processes before and 163 after (the stack branch's
`merge-tree --stdin`, one merge), an unchanged one 20 and 20, `bd status` 7 and 7; the outputs are the same but for
`stacks`. Under a load of 64–117 the wall times of alternating runs overlap (a full sync 1.3–9.0 s before, 1.2–6.3 s
after; an unchanged one 0.16–2.8 s and 0.14–0.61 s). That was measured on the code before the conflict chains above;
with both, a full `bd sync` runs 164 (the stack branch's merge and the conflict refs' `update-ref`), an unchanged one
14 and `bd status` 7, the outputs those of each alone. A stack branch that builds runs its chain beside the branches:
about one merge per commit whose parent tree differs, in the first sync after a change only.

### Measuring on a large repository

The process budgets pin a small fixture; what a change saves on a real history is measured with `pnpm perf:monorepo`
(`scripts/perf/monorepo.mjs`, its header lists the commands) on a clone of a large repository pinned at two commits,
so two builds of `bd` see the same state:

```bash
pnpm perf:monorepo pin --source <repo> --head <commit> --base <commit> --prefix <prefix> --stacks --dir <work>
pnpm perf:monorepo measure --dir <work> --bd <release bd> --out <work>/after --push <branch>
pnpm perf:monorepo measure --dir <work> --bd <release bd> --out <work>/after-count --push <branch> --count
pnpm perf:monorepo summarize <work>/after <work>/after-count
pnpm perf:monorepo compare <work>/before <work>/after
```

- `pin` makes a shared clone without a working tree (`master` at the head, `origin/master` at the base, the stack
  parents, issue groups and pushed marks of the source, a push remote whose branches are the clone's remote branches)
  and keeps its refs and config; `reset` and every full step restore them and delete the object cache.
- `measure` runs the steps in order, each on the state the one before left: a full sync, an unchanged sync, `status`,
  `commits` and `check` with a cold and a warm object cache, `push --dry-run` of one branch and of all. Three runs each;
  the processes are the "git processes" line of `bd -v`, the count of the executor by subcommand, and the wall time is
  noisy on a loaded machine (`load.txt` keeps the load of each step).
- `--count` runs each step once with `BD_GIT` set to `scripts/perf/git-count.sh`, which records every git process `bd`
  starts: it finds the `git version` of the discovery outside the executor's count, and nothing else.
- `compare` checks that the first run of each step printed the same in two measurements, the path of the binary in the
  push hints aside: a change of the git layer must not change what `bd` says.
- Use a release build (`cargo build --release -p bd-cli --bin bd`); build the base commit's `bd` in another checkout
  or a `bd lease` slot.

## Follow-ups

- **Replay conflicts**: `replay` reports a conflict only by its exit code, so the error does not name the commit or
  the files yet.
- **Mapping notes** live on the default notes ref (`refs/notes/commits`), so they show up in the user's `git log`.
  Moving them to a private ref needs a migration that reads both refs.
- **Resolve notes across rewrites**: a store keyed by the landed key, if the app shows "resolved by an agent".
- **SSH stalls**: the low-speed limits apply to HTTP only. A dead SSH connection is bounded only by the user's
  `ServerAliveInterval`.

Left out of the audit of the git layer on purpose, the gain smaller than the noise or the risk:

- **Typed ids**: an `ObjectId` newtype, binary keys of the object cache and a shared `Arc<Commit>` would touch nearly
  every signature, and new cache keys would make every user's cache cold once. Ids stay full hex `String`s, checked
  where they come from outside (`is_object_id`).
- **sync-core in directories**: grouping its modules into directories changes every `sync_core::` import of the
  workspace (re-exports are not allowed), while `lib.rs` changes with almost every feature. Its `//!` index groups the
  modules by area instead.
- **`update_resolve.rs` and `update_branch.rs` split**: both change often; the import cycle between them is gone since
  `current_worktree` moved to `git_ops::revs`.
- **One walk for the remote status**: its walks differ in their ranges on purpose (`^baseline`), and the walk saved
  would be a cheap one.
- **The landing check reads `HEAD ^base` again** (`read_local` in `crates/sync-core/src/landed.rs`): one `log` per sync
  of a fresh process while an update is pending, though the commit list of the sync has those commits.

## References

- Implementation: `crates/git-executor/src/git_info.rs`, `crates/git-executor/src/git_command_executor.rs`,
  `crates/sync-core/src/remote_command.rs`, `crates/sync-core/src/push.rs`, `crates/sync-core/src/push_lease.rs`,
  `crates/git-ops/src/rewrite_guard.rs`, `crates/git-ops/src/ref_transaction.rs`,
  `crates/git-ops/src/commit_writer.rs`, `crates/git-ops/src/reword_commits.rs`,
  `crates/git-ops/src/rewrite_plan.rs`, `crates/git-ops/src/replay.rs`, `crates/git-ops/src/amend_operations.rs`,
  `crates/git-ops/src/squash_commits.rs`,
  `crates/history-rewrite/src/autosquash.rs`, `crates/git-ops/src/update_branch.rs`, `crates/git-ops/src/worktree_update.rs`,
  `crates/sync-core/src/update_project.rs`, `crates/git-ops/src/update_resolve.rs`, `crates/git-ops/src/resolve_check.rs`,
  `crates/git-ops/src/merge_tree_output.rs`, `crates/sync-core/src/resolve_session.rs`,
  `crates/sync-core/src/resolve_copies.rs`, `crates/history-rewrite/src/regroup.rs`, `crates/history-rewrite/src/repository_lock.rs`,
  `crates/object-cache/src/cache.rs`, `crates/git-ops/src/commit_files.rs`; the module maps in
  `crates/git-ops/src/lib.rs` and `crates/sync-core/src/lib.rs`
- Tests: `crates/git-executor/src/worktree_safety_test.rs` and `git_usage_scan_test.rs` (the scans of the git argument
  lists),
  `crates/sync-core/src/process_budget_test.rs` and `crates/bd-cli/src/process_budget_test.rs` (the process budgets)
- Specs: [Commit Row](../../spec/commit/commit-row.spec.md), [Commit Files](../../spec/commit/commit-files.spec.md),
  [Move Commits](../../spec/branch/move-commits.spec.md),
  [Amend](../../spec/commit/amend.spec.md), [Squash](../../spec/commit/squash.spec.md),
  [Squash Fixups](../../spec/commit/fixups.spec.md),
  [Unapply](../../spec/branch/unapply.spec.md), [Update Project](../../spec/repository/update-project.spec.md),
  [Agent CLI](../../spec/cli/agent-cli.spec.md), [bd update](../../spec/cli/update.spec.md)
- Related docs: [Virtual Branches](virtual-branches.md), [Integration Detection](integration-detection.md),
  [Async Patterns](async-patterns.md)
- Git manuals: `git help replay`, `git help fast-import`, `git help merge-tree`, `git help read-tree` (two-way merge,
  `-u`, `-n`), `git help fetch` (`--porcelain`), `git help merge-base` (`--fork-point`)
