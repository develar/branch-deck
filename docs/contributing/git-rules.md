# Git Rules

**Status:** Active  
**Updated:** 2026-10-29  
**Tags:** [git, rust, bd, safety]

## Summary

The hard rules of the git that the app and `bd` run, each with the test that enforces it where there is one. The
flows and the reasons are in [Git Operations](../design/git-operations.md) (open the section a rule links); how to
add an operation across the layers is [Adding a Git Operation](adding-git-operation.md). [AGENTS.md](../../AGENTS.md)
("Git Operations") keeps one line per rule and links here.

## Git 2.54+

Git 2.54 is the minimum (`MIN_VERSION` in `crates/git-executor/src/git_info.rs`), checked when git is discovered, on every platform.

## Executor

All git runs through `GitCommandExecutor` ([Executor](../design/git-operations.md#executor)). It adds
`-c core.quotePath=false -c color.ui=never -c log.showSignature=false -c i18n.logOutputEncoding=UTF-8`, sets
`GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0` (only the `status` of a lease slot may write its index), strips inherited `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE`/
`GIT_NOTES_REF`, empties `GIT_NOTES_DISPLAY_REF` (`%N` shows only the mapping notes, or an explicit `--notes=<ref>`)
and gives git a null stdin unless the call pipes one; callers don't add `--no-pager`. A git with an environment,
stdin, accepted exit codes, a timeout, streamed stdout, progress or a reply protocol is `git.cmd(&args, repo)` with
`.env`, `.stdin`, `.accept`, `.timeout` and one finisher (`.text`, `.bytes`, `.lines`, `.output`, `.status`,
`.stream`, `.stream_progress`, `.progress`, `.interactive`), all on one spawn core; the only shorthands are `execute_command` (`.text`),
`execute_command_lines` (`.lines`) and `execute_command_with_status` (`.status` as text). The executor knows no git
beyond that: a rev or ref fact (`resolve_tree_id`, `ref_exists`) is a git-ops function. It carries what its operation
read once (`with_fact`): the repository's directories (`git_ops::revs::with_dirs_of`), so a step asks `known_dirs`,
`git_common_dir` or `current_worktree` instead of its own `rev-parse`. Test fixtures run git through
`test_utils::git_test_utils::git_command()`, which removes the repository variables a hook or `rebase --exec`
exports (`git_command_test.rs` fails on a raw `Command::new("git")`).

## Scan tests

The scan tests (`crates/git-executor/src/git_usage_scan_test.rs`, on the sources as `source_scan_test.rs` reads them
for the worktree safety scan too): git starts only in `GitCommandExecutor` (and `git_info.rs`'s discovery); the bd
crates (`crates/bd-*`) and src-tauri start no git of their own; no `--no-pager`, no `credential.helper=`; `update-ref` only in
`ref_transaction.rs`; `cat-file --batch*` only in `cat_file_batch.rs`; `--name-only`, `--name-status` and `--raw`
only with `-z`; paths after `--` only with `--literal-pathspecs`; `push`, `fetch` and `ls-remote` only through
`execute_remote_command*`. An exemption names its file, its reason and its exact number of hits.

## Never write to the working tree

No `checkout`, `checkout-index`, `switch`, `reset`, `rebase`, `stash`, `commit`,
`cherry-pick`, `merge`, `restore`, `clean`, `apply` without `--cached`, `read-tree -u`/`--reset`, `git history`;
`crates/git-executor/src/worktree_safety_test.rs` enforces it. Reading worktree files into a temporary
`GIT_INDEX_FILE` is fine. Besides the apply of `bd-host run --apply` (`crates/git-ops/src/host_client.rs`: the patch of a remote run
the user asked to apply, all or nothing, the index left alone), the single exception is the guarded two-way `read-tree -m -u` in
`crates/git-ops/src/worktree_update.rs` (dry run on an index copy first, then the CAS ref move, rollback on failure),
`pub(crate)`, called only by `update_branch.rs`: for Update Project (the app, `bd update`, and the update of a lease
onto its home branch, detached, after a conflict of `bd lease pick`), its undo, the finish
of a resolve session (`update_resolve.rs`), `bd pick` (`pick.rs`, also the worktree of `--follow`) and its undo, and
the unapply and the reapply of a virtual branch (`unapply.rs`, `reapply.rs`) and their undo
([Invariants](../design/git-operations.md#invariants)). The scan
counts its `-u` argument lists and the files that call the moves (`MOVE_CALLERS`). Don't add another caller, another
argument list, or a public way to build a move. A slot of the lease pool is not the user's working
tree: its `checkout --force`, `reset --hard` and `clean -fd` live in `crates/git-ops/src/lease_slot.rs` only, each
counted by the scan ([Lease](../design/git-operations.md#lease)).

## The user's index

The user's index changes only where HEAD's tree changed (`IndexUpdate` in `crates/git-ops/src/rewrite_guard.rs`):
a drop runs `read-tree -m -i <old> <new>` after a preflight on a copy of the index, an amend stages the amended files.
Staged work on other paths survives; never `reset --mixed`, never ignore an index error.

## Index lock

A write of the user's index (a move the files follow, `IndexUpdate::apply`) goes through
`git_ops::index_lock`: it waits up to 30 s for the `index.lock` of another process (an IDE's `git status`),
removes one that no process owns (a killed git's), and answers `IndexLocked` (`locked`) after the wait; never a
removal of any other lock file of git ([Index lock](../design/git-operations.md#index-lock)).

## Ref moves

One `RefTransaction` (`update-ref --stdin`) with old-value CAS and a `branch-deck: <op>` reflog
message; related refs (unapply rename, temp refs) go into the same transaction. The named exceptions: a notes ref
that `fast-import` writes (`write_notes_to_refs`, `write_commits_with_notes`, `write_notes_on_existing`: `from <ref>^0`, and a tip that does not
contain the ref's value fails the run, so a concurrent writer is never overwritten; the cache notes of a sync and of
the detection and the notes a squash carries to the commits it replays run once more from the new tip, then only
warn, `write_cache_notes`, but an identity git cannot form (`NoIdentity`) fails the run at once (a sync with
`no_identity`: no later run could write them); the mapping notes a rewrite carries
once more, then fail it; the resolve notes move in the
session's transaction); the temporary ref of a sync chain's commit writer (`commit_chain.rs`): one per writer
(`unique_temp_ref`), deleted in its own `fast-import` stream after a `checkpoint` wrote it (`reset` from the null
id), and a sync deletes the stale temporary refs its listing found (`is_stale_temp_ref`); and the refs of the review
fetch (below). A move that the files follow (an update and its undo) updates `HEAD` itself (deref) and commits only if
`symbolic-ref HEAD`, read while git holds the locks after `prepare`, is still the checked branch
(`RefTransaction::commit_while`, else `BranchSwitched`); git refuses `symref-verify HEAD` next to either form of the
update. The finish of a resolve session moves the session ref and the notes ref (`refs/notes/branch-deck/resolve`,
written only there) in that transaction.

## Who writes the refs of Branch Deck

- `refs/branch-deck/issue-groups` and `refs/branch-deck/stack-parents`: two refs, never one tree (a store commit
  replaces its whole tree). Only a regroup writes them, under the repository lock, in the ref transaction that moves
  HEAD, with CAS on every ref it moves; a sync only reads them, except that it drops the stack parent of a branch whose
  archived branch is integrated (one CAS write, best effort, only while the repository lock is free,
  `drop_integrated_stack_parents`)
  ([Regroup](../design/git-operations.md#regroup-rename-restack)).
- `refs/branch-deck/parked` (`parked.rs`, a ref of its own too): park and unpark under the repository lock with CAS,
  a rename in its regroup transaction; a sync only drops the line of a branch whose archived branch is integrated
  (`drop_integrated_parked`, as the stack parents). It decides no group: only the pushes leave a parked branch out
  ([Regroup](../design/git-operations.md#regroup-rename-restack)).
- `{prefix}/unapplied/<name>` (`unapply.rs`, `reapply.rs`): only the unapply, the reapply and their undo, in the
  transaction that moves HEAD, and its delete (`delete_unapplied_branch_core`, CAS); a sync only lists it, and a push
  of it goes under its own name ([Drop and unapply](../design/git-operations.md#drop-and-unapply)).
- `refs/branch-deck/conflict/<sha256 of the branch's full name>` (`conflict_chain.rs`): only a sync, in one CAS
  transaction after the branches, only when one changes; a failed move fails no sync
  ([Virtual branch sync](../design/git-operations.md#virtual-branch-sync)).
- `{prefix}/stack/<root>` (`stack_branch.rs`): only a sync, for a stack that forks, with CAS on the tip it listed and
  only when every member was built; deleted once the stack stops forking, never while it has a remote branch or a
  worktree has it checked out. Only a `Built` one is pushed: by the Safe Push of the whole stack (its restart, the
  creation of its pull request, Request Review) and by Push with Children of the root once it is on the remote or its
  pull request is open. Its build is remembered by the base and the original ids (`StackBuilds`), never by
  `CommitKey`, which an amend keeps ([Stack branch](../design/git-operations.md#stack-branch)).
- `refs/branch-deck/review/<n>/{head,base}`: only `review_fetch.rs` (the intent `FetchReview` of the session),
  through its own fetch refspecs; always `--refmap=` and `--no-prune`, so no remote-tracking ref moves; never under
  the repository lock; no fetch when both commits are local; killed on `cancel_operation`, the close of its window or
  120 s of silence; pruned by the session at each change of its review windows, with one `for-each-ref` and one CAS
  `update-ref --stdin`. Range diffs (`DiffSubject::Range`) never go into the object cache (`RangeFilesCache` in
  memory) ([Review fetch](../design/git-operations.md#review-fetch)).
- `refs/branch-deck/resolve/<worktree id>` (a shared ref: a gc in another worktree prunes a `refs/worktree/` ref):
  the state of a resolve session. The copies an agent edits (`resolve_copies.rs`) live only in `branch-deck/resolve/`
  of the worktree's git dir or in `--dir` (absolute, not a symlink, outside every worktree, empty or marked for this
  git dir, used by no other session; the marker `.branch-deck-resolve` names the owner's git dir); flat names
  `<key>-<basename>`, never tree paths; tree paths validated (no empty, `.`, `..` or `.git` component, nor an
  NTFS/HFS+ alias of `.git`); regular files only, bytes as git stores them without filters, never read, written or
  deleted through a symlink, every write a temporary file plus a rename; delete only the files bd wrote, only in a
  directory whose marker names this git dir ([Resolve session](../design/git-operations.md#resolve-session)).

## Repository lock

Every rewrite of the checked-out branch takes the repository lock in its core, only for the witness and the
write. The regroup (`regroup.rs`: the move, rename, restack, unstack and the conflict fix, also `bd group`,
`bd reword` and the app's reword through `reword.rs`) and its undo, park and unpark, the squash (the app's and
`bd squash`) and its undo,
squash fixups (also `bd fold`), amend, add issue reference, the move of a hunk (`bd move-hunk`), unapply, reapply,
the undo of an update, of a pick, of an unapply and of a reapply read and compute before it, on the oids they read
once (the snapshot or the history, merge-tree, replay, the dry run of the files on a copy of the index, the
`fast-import` of the new objects on temporary refs), and hold it only for the witness of those refs and the CAS
write (`lock_prepared`: HEAD, its branch and the refs they computed on in one `for-each-ref`; a move that the files
follow reads the mark of an interrupted move with them and lets those files follow first, `lock_prepared_move`,
`lock_checked_undo`; a write of the stores alone has the compare-and-swap of its transaction as its witness); a ref
that moved meanwhile, HEAD between two reads of the computation too, computes the rewrite once more under the lock,
where another HEAD than `expect_head` is `HeadMoved`. All through their options (`RewriteOptions` or `RegroupOptions`: `held_lock`, `lock_wait`, `expect_head`
checked by `ensure_expected_head`, `repository_lock.rs`; a move, the conflict fix and an undo take a `LockWait`
beside the check their request carries); then `bd push` and
`bd safe-push` (only their sync, then again the witness of their refs and the freeze of their push in the engine,
`operations::land_execute`; the reads of the remote, the trial, the retargets, the cancel, the network push and the
robot start without it; a ref that moved meanwhile is `plan_changed`), the app's pushes (`execute` of the intent
`Push`, `operations::push::execute_push`: of a branch alone, of a stack by `domain_policy::push::push_plan`, up to a
commit, of an unapplied branch, of the stack branch of a whole stack; admitted per ref in the registry of the
session, the user's robots before the lock, the refusals of parked, landed, held and not built branches in its plan,
the witness and the freeze of its refs under the lock, the network push of those commits after it, side by side
with the app's other pushes; only a push of the same remote ref waits, in the registry),
`bd move-hunk`, `bd undo`,
Update Project (the app and `bd update`: after the fetch and the check, for the witness, the local changes and
the move; a conflict that starts a resolve session computes its first round before the lock too and holds it for the
witness, the state with the session ref and the copies of the round; the landed report after the release), `bd pick` (after its check:
the witness and the moves of both worktrees under one hold, its landing report after it), `bd unapply`, `bd reapply` and their undo, and `bd update --continue` (not its `--dry-run`) and `--abort` hold
`$GIT_COMMON_DIR/branch-deck.lock`
(`bd`: `LockWait::Bounded`, then `locked`; the app's rewrites, pushes and Update Project: `LockWait::WhileHeld`, as
long as the holder record names a live process: an operation of the engine, every rewrite of `operations::rewrite`,
Update Project of `operations::update` and a push, says whom for in its step `WaitingForLock { holder }`, stopped by
`cancel_operation`), because
the app and `bd` processes rewrite the same branch. Before the file lock, the operations of one process wait for
each other in its line (a fair queue per lock file, `repository_lock.rs`, without a bound), so `locked` names only
another process; `try_lock_repository` waits for nobody, `lock_as_another_process` is for tests and the E2E server. A sync never waits for it:
neither the app's, nor `bd sync`, nor the `--sync` of a `bd` rewrite, which syncs after the release; only its drop of
store lines takes it, while it is free (`drop_store_lines`), and skips the drop when it is held. A fetch never runs under it ([Regroup](../design/git-operations.md#regroup-rename-restack)).

## Operations registry

The operations registry (`$GIT_COMMON_DIR/branch-deck/operations.json` and its `operations.lock`): only
`operations::registry_file` reads or writes them (`registry_scan_test.rs`); the lock is held for one
read-modify-write, never across git, HTTP, an `await` or the actor of a session; an operation (every one of the
app's, `bd push`, `bd safe-push`, `bd request-review`) is recorded before its session admits it, a rewrite, an
update or a pick of `bd` before it takes any lock (`bd_core::operation_record`), and an operation of another process
that holds what it needs makes it `busy` (exit 3), never one of its own process, HEAD never (the repository lock
orders the rewrites of the processes); no clean rule deletes the lock. A window's subscription watches the
directory (`operations::registry_watch`, `notify`, no git): the operations of other processes show in its session
and decide no admission there; `bd` touches
`$GIT_COMMON_DIR/branch-deck/stamp` after every command that succeeded and is no read, in one place
(`bd-cli::execute`), which syncs the watching sessions (`SyncReason::Stamp`). A start records what it sends before
its request goes out; an answer lost after it went out is `EffectStatus::Unknown`, never sent again: its record stays
`unknown` and holds for every process until a read of Patronus settles it (`operations::reconcile`, at an admission
and after a list of the robots) or the user forgets it (Forget This Start, `bd operation forget`)
([Operations registry](../design/git-operations.md#operations-registry)). The journal beside it
(`journal.jsonl`) has one writer, `operations::journal`, called by the registry under `operations.lock` as a record
leaves: one entry per operation that ended, of the app and of every `bd`, with what it cost from its admission
(`sync_utils::operation_tally`), its id the `operation_id` of its log lines and of the JSON of `bd`; the same module
writes the journal of the page in the app data directory (`record_page`, under `journal.lock`) and empties both for
Clear Log (`clear`); Activity reads them, no window keeps a log ([Journal](../design/git-operations.md#journal)).

## Guards

The guards (`History::check_rewrite`, `check_rewrite_range`, `check_rewrite_on`) before rewriting the checked-out
branch, with the rest of the computation before the repository lock, whose witness checks what they read: no
rebase/merge/cherry-pick/revert in progress, HEAD not detached, no merges in the range (refuse, no rebase fallback).
A sync (the app's and `bd sync`) is refused under the first two as `SyncDeferred` (`check_sync_guards`): a half-done
history would archive the branches it lacks.

## Metadata

Message-only rewrites (reword, move) keep author and committer byte-for-byte (`commit_writer`:
`cat-file --batch` + `fast-import`); content rewrites (amend, drop via `git replay`) keep the author, committer =
current user. The one named exception: `bd move-hunk` (`move_hunk.rs`) keeps author and committer byte-for-byte for
every commit whose diff stays the same (`patch-id --stable`), so their virtual commits stay; `from`, `to` and the
commits whose diff of the file changed get committer = current user and no mapping note. Signatures are stripped.

## Batch

One process per phase, not per item (`--stdin`, `cat-file --batch`, `fast-import` for commits and notes,
`update-ref --stdin`). A chain whose next step needs the answer of the one before keeps one interactive process per
chain: a sync merges a branch in one `merge-tree --stdin` and writes its copies with one `fast-import`
(`commit_chain.rs`), never `commit-tree` per commit; `with_process_stats` counts the processes of an operation. A
check of an update finds the first conflicting commit with a binary search over prefixes, each probe from the longest
clean prefix: ⌈log₂ n⌉ `replay` runs; a resolve session opens its first round there, then one `replay` of the rest
per round. Never parse translated text: exit codes, `--porcelain`, `-z`. The one named
exception: a branch description costs one `git config` per changed branch (`bd describe`, `descriptions` of a
`bd group` plan), because git has no batch config writer; read them with one `--get-regexp`.

## Exit codes

`merge-tree` and `replay` exit with 1 on conflicts; `merge-tree --stdin` exits with 0 even then, and
each merge of its output starts with its status, 1 clean, 0 conflicted (`parse_batch_merge`); `merge-base` exits with
1 for "no" (`--is-ancestor`) or "none" (no merge base, no fork point), and any other non-zero is an error, never a
"no" (`git_ops::revs`).

## Network commands

Network commands (push, fetch) run through `execute_remote_command` (`crates/sync-core/src/remote_command.rs`):
the user's credential helpers stay, prompts are off, stalled HTTP transfers abort, the locale is C (untranslated
`--progress` lines). Never pass `-c credential.helper=`.

## The branch of the baseline on its remote

The branch of the baseline on its remote (`master` of `origin`) takes only the push of a start of a repository
that lands by a push (`sync_core::baseline_push::push_baseline`, run by `land_execute` after the lock): the copy as
the witness froze it, `<tip>:refs/heads/<target>`, no `+`, no lease (the server refuses what is no fast-forward), by
the first push URL of the remote with its `url` and `pushurl` cleared (`-c remote.<r>.url=`), so git takes no
configured remote for the URL and moves no remote-tracking ref: only `fetch_baseline` writes
`refs/remotes/origin/master`. A pre-push hook sees the URL; a second push URL, `receivepack` and `proxy` play no
part ([Push into the baseline](../design/git-operations.md#push-into-the-baseline)).

## Fetch lock

The baseline is fetched only by `sync_core::baseline::fetch_baseline` (the app's Update Project and
Fetch, `bd update`), which holds `$GIT_COMMON_DIR/branch-deck/fetch.lock` for its fetch (`fetch_lock.rs`, its one
taker: `registry_scan_test.rs`), its record naming the tracking ref: a second fetch of any process waits while the
holder lives and takes its result when the ref moved, one transfer; no retry on `cannot lock ref`, no clean rule
([Update project](../design/git-operations.md#update-project), Fetch lock).

## Git config writes

Git config writes are only the `branchdeck.*` keys of Settings → Repository, Diffs & Merge, Landing and Agents
(`repository_config.rs`: one `--get-regexp --show-scope` read, `--replace-all` or `--unset-all`, refused when the
worktree config or `git -c` sets the key), `--local` (`.git/config`), or `--global` only where the request names the
global scope (`set_repository_config` with `scope: "global"`); and the branch descriptions of `bd describe`
([Repository settings](../design/git-operations.md#repository-settings)).

## GitHub credentials

The only git of the GitHub token chain is `git credential fill` (`crates/github/src/token.rs`),
a `GitCommand` with `.timeout` (10 s) and `.status`, with `credential.interactive=false`, an empty `GIT_ASKPASS` and
`GCM_INTERACTIVE=never`; never `credential approve` or `reject`. GitHub itself is asked over HTTP (`GitHubService`),
never by a `gh` process; the app's own sign-ins (GitHub, YouTrack) keep their tokens through
`sync_utils::secret_store`: on macOS a login keychain item written and read by `/usr/bin/security` (the secret on
stdin, as `gh` does; the item trusts `security`, so an update of the unsigned app asks nothing), elsewhere or
while the keychain can't be used a 0600 file of the app data dir; `bd` reads them too
([GitHub credentials](../design/git-operations.md#github-credentials)).

## Symbols

The symbols (`symbol_scan.rs`): source blobs by id from the commit diffs, the declarations the range adds or changes
as the index, never the whole tree; the summaries of trial blobs stay in memory, never in the object cache
([Symbol summaries](../design/git-operations.md#symbol-summaries)).
