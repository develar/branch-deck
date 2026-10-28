/// What the reading commands write, for `bd --help` and `bd help agent`; a macro, so that `concat!` takes it
macro_rules! read_only {
  () => {
    "Read only: `bd status`, `bd commits`, `bd suggest`, `bd check`, `bd robot`, `bd robot-stats`, `bd schema`,
`bd version`, `bd describe <branch>` without a setter, `bd update --status` without
--materialize, `bd lease status`, `bd workspace`, and
`bd group --dry-run`, `bd rename --dry-run`, `bd restack --dry-run`, `bd unstack --dry-run`, `bd update --dry-run`
(also with --continue), `bd pick --dry-run`, `bd fold --dry-run`, `bd move-hunk --dry-run` and `bd reword --dry-run`
write no ref, config, index or working tree: at most git objects (a trial sync, a replay, the trees of a move) and the
object cache (none with --no-cache); `bd check --build` runs its command in a temporary copy of each tree.
`bd push --dry-run`, `bd safe-push --check` and `bd request-review --check` sync first."
  };
}

/// The read-only line of `bd --help`
pub const READ_ONLY: &str = read_only!();

/// `bd help agent`: the rules and the workflow of an agent that groups commits into virtual branches
/// (docs/guidelines/agent-cli.md), a core of at most 10 KB: every rule that waits for the user stays here, the fields
/// of an output and the other refusals are in the `--help` of its command (`after_long_help`). The first line names the
/// version (`version::VERSION`). `agent_help_test.rs` checks it against the commands and options
pub const AGENT_HELP: &str = concat!(
  "bd ",
  env!("BD_VERSION"),
  ": Branch Deck for agents. JSON on stdout, one object per run (each has \"schema\": 2 and \"operation_id\"); logs on stderr
(-v for debug). `bd <command> --help` says what a command prints and what each of its refusals asks.

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into branches, one per name: the `(name)`
prefix of a subject, else the issue group of its issue key, else the issue key (`ABC-123 Fix ...`). A sync rebuilds
each branch as the virtual branch `{prefix}/virtual/{name}` on the baseline; the working tree is never touched. With
stacked branches (`git config branchdeck.stacks true`) a branch is built on a parent: a subject declares it with
`(parent/leaf)`, or the stack parents do (`refs/branch-deck/stack-parents`) and the subject keeps the bare `(leaf)`; a
stacked branch is named by its leaf.

Rules
- Ask the user first, and run only what the user approved: a push (`bd push`, also --force and --restart-running), a
  Safe Push or a Patronus Dry Run (`bd safe-push`, also --cancel-running, --without-approvals, --allow-dangling,
  --allow-missing and --create-pull-request), `bd request-review`, a `gh pr create` that bd prints, `bd park`,
  `bd unpark`, `bd unapply`, `bd reapply`, and any change of a commit that another session made, a `bd reword` of its
  message too. Show what bd printed (`lands`, `review`, `dropped_commits`, the plan) when you ask.
- A branch `parked` in `bd status` stays local, and so do a branch built on it (`with` names the parked one) and the
  stack that holds it. `bd park <branch> --note '<why>'` and `bd unpark <branch>` only when the user asks (no rewrite;
  `idle_parked` lists the lines of branches without commits here).
- `bd unapply <branch>` (`--expect-head <id>`) takes a branch and its files out of the checked out branch, kept as
  `{prefix}/unapplied/{name}` (`unapplied` of `bd status`); `bd reapply <name>` (`--dry-run`, `--expect-head <id>`)
  puts it back on top; `bd undo <undo_token>` takes either back. Only when the user asks. `bd push` and `bd safe-push`
  refuse an unapplied name (`unapplied`); `bd push --unapplied <name>` keeps a copy on the remote when the user asks.
  `branch_not_built` (sync first), `branch_checked_out`, `drop_conflict`, `reapply_conflict` and `nothing_to_reapply`
  change nothing: tell the user.
- Never reorder, squash or drop commits yourself (`bd fold` folds fixup!, squash! and amend!; `bd update` drops
  landed ones); move a change only with `bd move-hunk`, never with plumbing. Never stash, reset, check out or commit the
  user's files: on `uncommitted_overlap` (exit 4) show `paths` to the user and ask.
- Only `bd update`, `bd pick`, `bd unapply`, `bd reapply` and their undo write the working tree (a guarded update).
- Branch on `error.code` and the exit code, never on the message. 0 ok. 1 a conflict, never an error object: the
  output says what conflicts (`state: \"conflict\"`, a branch not built, `ok: false`). 2 invalid input: fix the
  arguments or the plan. 3 stale (`head_moved`, `locked`, `busy`, `plan_changed`): read again and retry. 4 refused:
  do what the table below or the `--help` of the command says for the code, else stop and show `message` to the user.
  5 git or network error: show it. Retry only when `retryable` is true, whatever the exit code. A `git_error` \"not
  updating refs/branch-deck/tmp/chain (… does not contain …)\": a ref of Branch Deck 0.21 or older; `git update-ref -d
  refs/branch-deck/tmp/chain`, then retry.
- An error is {\"schema\":2,\"error\":{\"code\",\"message\",\"commits\",\"paths\",\"branches\",\"review\",\"blocks\",\"processes\",
  \"next_steps\",\"retryable\"}}; `paths`, `branches`, `review`, `blocks`, `processes` and `next_steps` only where they
  apply.
- Printed commands: run the `next` of `bd workspace`, `bd update` and `bd pick` as it says. Any other ready command
  waits for the user's approval when it pushes, starts or cancels a robot, or rewrites: the `push` of `bd sync`, the
  `fix` of `landing_needs` (`bd status --help`) and of `bd check`, and each `error.next_steps` entry with
  `needs_approval`.
- `bd fold --expect-head <id>`, as every write that takes it, refuses unless HEAD is still the `head` (or `new_head`)
  you read last: `head_moved` (exit 3). Every write prints `previous_head`, `new_head` and an `undo_token` (full ids:
  pass it as printed). A write whose step after it failed (a sync) keeps its output with `after_error`: retry that
  step, never the write. `bd undo <undo_token>` takes a write back, `undo_stale` (exit 4) once anything moved on.
- Ids are abbreviated per output: compare them by prefix, or pass --full-ids.

Refusals that wait for the user (exit 4); the `--help` of the command has the rest:
- `push_too_large`: show `paths`; never raise `branchdeck.pushSizeLimit` yourself.
- `parked`: tell the user; never push, start or unpark a parked branch on your own.
- `lease_rejected`: show `error.branches[].remote.dropped_commits`; `bd push --force --expect-remote <remote.head>` for
  that one branch only once the user approved.
- `robot_running`: wait until the robot merges; `bd push --restart-running` or `bd safe-push --cancel-running` only once
  the user approved.
- `approvals_missing`: show `error.review`; `--create-pull-request` once the user approved; `--without-approvals` only
  when the user says so: Patronus fails it.
- `dangling_reference`, `missing_declaration`: fix the plan (`bd check --help`); `bd safe-push --allow-dangling` or
  `--allow-missing` only when the user says it is a false alarm. `rules_unknown`: ask the user; `bd safe-push
  --allow-unknown-rules` only when the user says so.
- `low_disk_space`: tell the user; never pass `bd lease get --min-free 0` on your own.

Global options: -C <repo>, --prefix <p> (else `branchdeck.branchPrefix` of the git config or the app setting;
`prefix_source` of `bd status` names which), --full-ids, --no-cache, -v.
",
  read_only!(),
  "
`bd schema <command>` prints the JSON Schema of the outputs of a command (every `state` with its values), `bd schema`
the index of the commands, `bd version` the version of bd and git.

Workflow: the commits into pull requests
1. Look. `bd suggest` prints a plan checked by a trial sync (exit 1: edit the plan): quote the `why` of a cluster to
   the user, review the `weak` ones first; `bd suggest --help` has its fields.
2. Plan and check. A plan is {\"branches\": {\"<path>\": [ids]}, \"unassign\": [ids], \"issue_groups\": {\"<name>\":
   [keys]}, \"ungroup_issues\": [keys], \"unstack\": [branches], \"descriptions\": {\"<branch>\": {\"title\", \"body\"}}};
   a path is a branch, or `parent/leaf` for a stacked one. Run `bd check --plan plan.json` until the `result` of every
   branch is `ok`; `bd check --fix` searches the regroup that fixes the rest. First run `bd check --help`: what each
   `result` asks.
3. Apply. `bd group --expect-head <head of bd suggest> --sync --plan plan.json`: one message-only rewrite, then a sync.
   `bd rename <old> <new>`, `bd restack <branch> <parent>` and `bd unstack <branch>` take the same options;
   `bd describe <branch> --title ... --body ...` sets the pull request text of one branch. `bd group --help`: its
   refusals and its undo.
4. Push the branches, never into master (`bd land`). Show the result to the user; once approved,
   `bd push <branch>...`, and name every branch of `branches`. Each has a ready `gh pr create` into the baseline; pipe
   its `body` into it. `bd push --help`: the refusals and the stack branches.
5. Safe Push (Patronus). First, always, `bd safe-push <branch> --check`; show the user `lands` (a stacked branch lands
   what it is built on, in one robot; `--whole-stack` the whole stack) and `review`. Once approved,
   `bd safe-push <branch>` (`--patronus-dry-run`: a Dry Run). `bd safe-push --help`: the fields of --check and every
   refusal. Any route: `bd land --help`.
6. Review. `bd request-review <branch> --check` names whom it would ask; once approved, the same without --check
   (`bd request-review --help`).
7. Robot. `bd robot <branch>` (`--full` after a failure says why and what to fix; `bd robot --help`).
8. Statistics. `bd robot-stats` (`bd robot-stats --help`).

Other tasks
- Before the first edit of a session: `bd workspace` says whether to work in a lease of your own
  (`branchdeck.agentLease`, on by default) and whether to start each subject with `(branch)`
  (`branchdeck.agentSubjectBranch`, off by default), and `next` what to run; the user's words win over it.
- Update the branch onto its baseline: `bd update` (--no-fetch, --dry-run), the app's Update Project; a conflict opens
  a resolve session (exit 1), which `bd help update` explains.
- Several sessions on one branch: `bd lease get` leases a worktree, `bd lease pick` in it lands its commits where it
  was cut from, `bd lease return` gives it back; `bd pick --from <worktree> --follow` (or `bd pick <rev>`,
  `<a>..<b>`, --remote; --expect-head, --dry-run: `bd pick --help`), never `git cherry-pick` (`bd help worktrees`).
- Squash: `bd fold --help` (fixups), `bd squash --help` (the commits the user asked to squash).
- A message to change: `bd reword <commit> --subject '<subject>'`, its `(prefix)` kept; never an empty `amend!` commit
  or plumbing for it (`bd reword --help`).
- A change in the wrong commit: `bd move-hunk <from> <to> <path>` (`bd move-hunk --help`).
- `bd install` puts `bd` on PATH and the skill where agents find it; `bd help skill` prints the skill.
"
);

/// `bd help update`: the update and its resolve session (spec/cli/update.spec.md); `agent_help_test.rs` checks it with
/// `AGENT_HELP` against the commands and options
pub const UPDATE_HELP: &str = "\
bd update: bring the checked out branch onto its baseline (e.g. origin/master, `upstream` in the output), as the app's
Update Project, and resolve its conflicts outside the working tree. Other sessions may work in the same tree: bd never
stashes, resets or checks out, and only the finish writes the working tree, through the guarded update (only the files
the update changes; local changes on them refuse it, `uncommitted_overlap` with `paths`).

`bd update` fetches the baseline (--no-fetch: the last fetched state; --dry-run: the check only, no fetch, nothing
written but git objects). Exit 0: `state` is up_to_date, fast_forward or rebase, the last two with an `undo_token`.
Exit 1: a commit conflicts, and a resolve session opens, one round per conflicting commit. The session belongs to this worktree
and branch, and keeps the baseline it started with (in a lease, the session of `bd lease pick`: the branch it was cut
from, `upstream`, on its detached HEAD; its finish names `bd lease pick` in `next`); peers' commits on top of HEAD join it as more rounds. Running
`bd update` again prints the open round (and writes its missing copies); `state: pending` (exit 1) means every
commit is resolved and `session.next` finishes. A conflict of git that its words resolve (`branchdeck.resolveByWords`, on by
default) needs no round: the update commits the result and lists the commit in `resolved` (`commit`, `new_id`,
`conflicts`, `files`); tell the user. When every conflict resolves so, the update applies with exit 0.

Dropped commits: `dropped[]` names each local commit the update dropped, with the baseline commit it landed as
(`upstream`) and its `proof`: `patch_id`, `replayed` or `changed_lines` (the baseline proves it has the change, so
the replay left it out, as `git rebase` does), `empty` (its replay left it without a change), `landed` (a round of
--landed) or `user` (--drop-empty). The first output of a session, its repeat, `bd update --status` and the finish
list them; a round of --continue prints `dropped_count` only. You never drop a commit yourself.

Ask the user only when bd says so, and only once. An output with `ask_user: true` lists its `reasons`: `blocked_paths`
(local files on paths the finish writes, `session.blocked_paths`) and `landed_differs` (a drop as landed loses lines).
An error with `ask_user: true` (`uncommitted_overlap`, `empty_resolution`, `landed_differs`) names the decision in
`message`. Show the user that output and ask about all of it at once; follow everything else as bd says.

A round (`state: conflict`, `session`):
- `session.commit`: the local commit being replayed, its `subject`, `body` and `patch` (the commit as a patch: the
  intent to reapply). `landed_as`: the baseline commit with the identity of this commit (the same author, author time
  and subject; also for a fixup of such a commit), its `local_only` lines (what this commit changes and the baseline
  commit does not: what a drop loses; `+` added, `-` removed) and `upstream_only` lines. Without `local_only`,
  `session.next` drops the commit with --landed: run it, no question. With `local_only`, `ask_user` is true: with the
  user's yes, run `next` with --user; with a no, resolve the files and run `bd update --continue --round <n>`. The
  first output of a session lists every such commit in `session.landed_differs[]` (`commit`, `subject`,
  `landed_as`): ask once about all of them, then use each answer in its round.
- `session.files[]`: `path`, `kind`, and absolute paths: `edit` (the merge result with zdiff3 markers, the file to
  edit), `before` (the commit's parent), `onto` (the new parent: the baseline and the commits replayed so far),
  `commit` (the commit's version), `markers` (blocks left), `upstream_commits` (who changed it upstream), `hint`.
  `session.marker_labels` names the ids after the markers. `session.blocked_paths`: local changes that will block the
  finish (the first 100, all 101 rather than one left out; `blocked_paths_omitted` counts the rest). `session.next`
  is the command that continues. A round prints only its own files; `bd update --status` adds the rounds resolved so
  far. `session.resolved` lists the rounds that resolved themselves by words: they count in `round`, so round 3 can
  be your first.
- Edit only the files at `edit`, never the same path in the working tree. Keep the change of `onto` and reapply the
  intent of the commit; change only the lines between the markers, keep the line ends, and remove every marker. A
  change that the commit needs outside the blocks or in another file goes into a `fixup! <subject>` commit after the
  update, folded with bd fold. Write a copy as UTF-8 without a byte order mark: in Windows PowerShell with
  `[IO.File]::WriteAllText(<edit>, $text)`, because `Set-Content` and `Out-File` add a BOM or write UTF-16.
- `kind` content: edit `edit`. `kind` modify_delete or rename_delete (one side deleted the file; `renamed_from`
  names its path before a rename): `--take onto:<path>`,
  `--take commit:<path>` or `--delete <path>` on the --continue command, or edit `edit`. Every file needs a
  resolution: a missing file is never a delete.

`bd update --continue --round <n>` commits the round's resolution, replays the rest and opens the next round (exit 1)
or applies the update (exit 0). --take and --delete resolve a file without an edit (repeatable). --landed <commit>
takes `onto` for every file and drops the commit as landed; --user adds the user's yes.
`bd update --continue --dry-run` checks the resolution; it moves no ref and writes nothing but git objects. Exit 2:
`unresolved_markers`, `resolution_outside_conflict` (a line outside the blocks changed) or `missing_resolution`, with
`paths` and the copies in the message: fix those copies and run it again. A refusal changes nothing: the round stays
open and the copies keep your edits (`edited` in the round). `round_moved` (exit 3):
another round is open; `bd update` prints it. `empty_resolution` (exit 2, `ask_user`): the resolution leaves the
commit without changes; with `landed_as`, use --landed <commit> instead; else ask the user, then add --allow-empty
(keep an empty commit) or --drop-empty <commit> (drop it). `not_landed_round` (exit 2): the round has no `landed_as`.
`landed_differs` (exit 2, `ask_user`): `local_only` is not empty and --user is missing. `landed_not_empty` (exit 2):
with `onto` the commit still changes `paths`; resolve the files instead.
At exit 0, `resolutions[]` lists per round the `commit`, its `new_id` (null for a dropped commit; `empty: kept`
for an empty commit) and per file `how` (edited, onto, commit, deleted; words for a round that resolved itself,
with `words` on the round) with `dropped_upstream_lines` and `dropped_commit_lines`: the lines of a side's change
that the result lost; a round of --landed has `landed_as` and `local_only` instead. Report each resolution to the
user, these lines first. `verify.paths` names the files whose result no side had as it is (edited, commit, deleted,
words): build, and run the tests of these files; without `verify`, the build is enough. On a failure commit the fix
as `fixup! <subject>` and run `bd fold`, or take the update back with `bd undo <undo_token>`.

`uncommitted_overlap` at the finish (exit 4) keeps the session: once the user moved those files, run
`bd update --continue` again. `finish_pending` (exit 4): the branch moved, but its files did not follow; the message
says what is left.
`session_stale` (exit 4): the branch was rewritten or HEAD left it; `unsupported_conflict` (exit 4): a binary,
file/directory, rename, mode, symlink or submodule conflict, or an invalid path. Both: stop and show the user. `bd update --abort` removes the session
and its copies (nothing else changed); --worktree <id> removes an orphaned session of another worktree, a live one
only with --force.
`bd update --status` prints this worktree's session and every session of the repository (`orphaned` ones too), and
changes nothing; --materialize writes the missing copies, with --force all of them. --dir <path> puts the copies of a
new session into an empty directory outside every worktree, for a sandbox that keeps .git read only.
";

/// `bd help worktrees`: how agent sessions that share a checkout work beside each other, each in a worktree of its own
/// (spec/cli/pick.spec.md); `agent_help_test.rs` checks it with `AGENT_HELP` against the commands and options
pub const WORKTREES_HELP: &str = "\
bd help worktrees: several sessions on one branch. Each session commits in a worktree of its own (a lease: a linked
worktree, often a detached HEAD, sharing the git dir); its work counts once it is on the shared branch of the shared
checkout. One owner rewrites that branch: `bd update`, `bd fold`, `bd group`, `bd rename`, `bd reword`. Every other
session only adds commits to it, with `bd pick`, never with `git cherry-pick`, a rebase or a reset in the shared
checkout; the one rewrite it may run is a `bd reword` of its own landed commit (step 4).

0. Lease your worktree when `bd workspace` says `lease.on` (`branchdeck.agentLease`, on by default; its `next` is the
   command): `bd lease get --holder <your name>` in the shared checkout (or -C <shared checkout>). It
   prints `path`, a worktree of the pool at the HEAD of the shared checkout, detached, `lease_id`, `home` (the
   `checkout` and the `branch` your commits land on) and `next` (the pick of step 2, the return of step 5); work and
   commit only there, never `git worktree add`. A free slot is moved to that HEAD (`reused`: only the diff is written); with
   none free the pool adds one (`created_by`: `clone`, an APFS clone on macOS without what the `delete` clean rules
   delete, or `checkout`), one at a time: a get waits while another adds a slot, says whom it waits for on stderr,
   and takes a slot freed meanwhile (`waited_for_clone_ms`). --branch <name> (-b) creates a branch there. `skipped`
   names free slots with local changes (`dirty`) or processes in them (`busy`); the get stops the Bazel servers of a
   free slot itself (`stopped_servers`). `seeded`: ignored local files that `.worktreeinclude` names, copied from the
   shared checkout.
   `low_disk_space` (exit 4): the volume of the pool has less free space than `branchdeck.leaseMinFree` asks; tell the
   user, never pass --min-free 0 on your own. `idle_leases`: leases whose holder process is gone; the user decides.

1. When `bd workspace` says `subject_branch.on` (`branchdeck.agentSubjectBranch`, off by default), write the prefix
   when you commit, never later; off, write the subject as the repository does, and the owner groups the commits
   (`bd suggest`, issue keys). A subject is `(branch) <one sentence>`. With stacked branches
   (`branchdeck.stacks`) a new stack is declared once, `(parent/leaf) <one sentence>`; once the stack parents know the
   leaf, `(leaf) <one sentence>` is enough, as a plan of `bd suggest` writes it. `(parent/leaf)` on a commit of a
   branch that is a root stacks the whole branch on `parent` (`reparented` in the pick); two parents for one branch
   are conflicting parents, `bd restack <leaf> <parent>` keeps one. The branch is a readable name of a few words, never an issue
   id as the name; an issue id stays in the subject after the prefix, or in the body. A series of commits is a stack
   root with one leaf per logical group; a change on its own gets a root of its own. One deliberate commit per change:
   no `fixup!` subject on the shared branch, `bd fold` would fold it into its target. Rename a branch with
   `bd rename <old> <new>`, which rewrites every prefix, children included. `bd suggest` is for commits that came
   without a prefix.
2. Land each commit that builds and passes its tests at once, from your lease: `bd -C <your lease> lease pick`, the
   `next` of `bd lease get` and `bd workspace`. It is `bd pick --from <your lease> --follow` run in `home.checkout`,
   onto `home.branch`, whatever moved there since: no test again on the new tip, a conflict comes back to you. From a
   worktree that is no lease, with -C <shared checkout>: `bd pick --from <your worktree> --follow`. bd replays the
   commits onto the branch in memory, under the repository lock that every worktree shares, so no fold or update runs
   halfway through it; the files follow, and --follow moves your worktree onto the new tip (its local changes on
   other paths stay). With nothing to land, `bd lease pick` only moves your lease onto the tip (`up_to_date`,
   `followed.state: moved`): the commits of the other sessions.
   - `picked[]`: the copies, each with the `group` it lands in; `grouped_by: none` is a commit without a prefix: fix
     the subject before the owner groups it.
   - `dependencies[]`: a commit whose prefix names a branch that is not built on the branches it needs (`depends_on`:
     its hunks change what their commits wrote), so that branch can't sync. Fix the prefix in your worktree before
     the pick (--dry-run says it too), or after it run `bd group` with its `plan` (the commit moves into the branch
     that holds them) or its `restack`.
   - `already_present[]`: commits whose change the branch has already (`on`: its commit with the same patch; `by`:
     `patch_id`, left out before the replay, or `empty`, the change in another form); never an empty commit.
     `state: up_to_date`: nothing was left to pick.
   - `state: conflict` (exit 1): nothing landed. `bd lease pick` gives the conflict back to your lease: it updates the
     lease onto the tip of `home.branch` (`update`), and the conflict opens a resolve session there (`bd help
     update`); the lease moves only when `bd update --continue` finishes it, `bd update --abort` closes it. Resolve it
     (regenerate a generated file, never keep one side of a code hunk), build and test, then `bd lease pick` again (the
     `next` of the finish). An update that applies at once (the words resolved it) is picked again at once. From a
     worktree that is no lease, do what `conflict.next` says: move the worktree onto the branch, make the commit again
     there, pick again. Never `git cherry-pick` it into the shared checkout: it leaves `CHERRY_PICK_HEAD`, which blocks
     every rewrite.
   - `uncommitted_overlap` (exit 4): local files of the named worktree sit on paths the pick writes; ask the user.
     `range_unknown` or `source_on_newer_baseline` (exit 4): name the commits (`bd pick <first>^..<last>`), or ask the
     owner to update the branch first. In a lease `bd update` onto the baseline is `lease_follows_home` (exit 4): it
     would move the lease past its branch; `bd lease pick` brings it onto the tip, the owner updates the branch.
   - `bd lease pick`: `home_branch_switched` (exit 4): the shared checkout has another branch checked out than at the
     get, nothing landed; ask the user, and pick again once the branch is back (`bd workspace` shows
     `home.switched_to`). `no_home_branch` (exit 4): the lease was cut from a detached HEAD, no branch is named; ask
     the user where the commits go. `no_lease` (exit 2): run it inside your lease. `resolve_in_progress` (exit 4): the
     session of its conflict is still open; finish or close it first. `unchecked_tree` (exit 4): the repository sets
     `pick_requires_check` in `.branchdeck.toml`, and no green build or test recorded the tree of your HEAD (the check
     tool writes `bd-checks.jsonl`; a raw build writes nothing); run the build and the tests on this tree, then pick
     again. `bd lease pick --dry-run` shows `check` (`ok`, `stale`, `missing`). `bd lease pick --unchecked '<reason>'`
     lands without a check: only when the user agreed or the change cannot break a build (docs only), and the reason
     says which.
   - `followed.state` `head_moved` or `blocked`: the branch moved, your worktree did not; the same command again moves
     only the worktree. A build daemon or a file watcher of the worktree may need a restart after it moved.
   - `bd undo <undo_token>` moves the branch and your worktree back.
3. Check that the branch still holds your work after the owner rewrote it: `bd lease pick --dry-run` in your lease
   (elsewhere `bd pick --from <your worktree> --dry-run`)
   (`up_to_date`, every commit `already_present`). Never compare by date (a `git log` limited by time): a rewrite can
   change the dates, and a fold changes the ids; compare subjects without the `(prefix) ` only as a hint, the content
   decides.
4. The owner's rewrites take the same lock and refuse while a cherry-pick, rebase, merge or revert is in progress in
   the shared checkout (`operation_in_progress`). After a rewrite, the owner tells the other sessions its new HEAD
   (`new_head`); they pick again or check with --dry-run. Nobody squashes, amends or redates another session's
   commit. A rewrite gives the commits after it new ids: the next write of a session that read the old HEAD gets
   `head_moved` (exit 3), and `bd group --allow-append` does not help, the old HEAD is gone: read the status again.
   To fix the message of your own commit after it landed, run `bd reword <commit> --expect-head <head> --subject
   '...'` from your worktree with -C <shared checkout>, without --sync (the owner syncs); your worktree needs nothing,
   a later pick or return finds the commit landed by content. A reword of a commit that
   another session made is the user's to approve.
5. Return your worktree once your commits landed: `bd lease return` inside it (or --workspace <dir>; --lease-id <id>
   refuses another lease, and alone returns the slot of that lease from any checkout). It frees the slot, removes the `seeded` files and detaches a branch, which stays. Refusals
   change nothing: `unlanded_commits` (exit 4, `commits`): commits that the branch of the shared checkout lacks by
   content (a dry run of the pick decides, also after the owner rewrote the branch); land them with step 2 first; `resolve_in_progress` (exit 4): the
   resolve session of a pick is open in it, finish it (`bd update --continue`, then `bd lease pick`) or close it (`bd update --abort`); `dirty_worktree` (exit 4, `paths`): commit and land them, or ask the user; `slot_busy` (exit 4,
   `processes`: each with `argv` and `kind`): a process of another session works in it (a shell, a build, a test run);
   read `processes`, never `ps` or `lsof`, and wait until it is done; never kill it. `host_slot` (exit 4, `paths`):
   the worktree is a host slot (holder `bd-host/<n>`) that the host lends to the runs of other machines, never yours;
   `bd lease host …` is the interface of the remote-run agent of a host to those slots, never a command of yours. A repeat after a timed-out call is
   safe: it waits for the running return and answers `joined: true`. Not counted: your own session (bd, its ancestors
   and their process groups: the `jq` of `bd lease return | jq`), git's background maintenance (`maintenance`: a
   repack after a commit, which writes only the shared object store; `maintenance_started`: the return started it
   between leases, `branchdeck.leaseMaintenance`) and `caffeinate`. The Bazel servers of the
   worktree (`--workspace_directory` in it, `kind: bazel_server`) serve only it, with an output base of their own: the
   return stops them with SIGTERM once no other process is there (`stopped_servers`; their output bases stay), so
   never run `bazel shutdown` or kill one first. --confirm-preserved discards the changes and gives up the
   commits, only when the user said so. `bd lease status` lists the slots and their leases from the pool state
   (--processes adds the processes in each; `holder_alive`, `free_bytes`). The return applies the clean rules of
   `.branchdeck.toml` (`cleaned`: stale cargo units, build output, test artifacts; a `delete` moves its paths into the
   trash and deletes them once the slot is free; `clean_error` when the file does not parse, the return still
   succeeds); notes that must outlive the return go outside the paths of the rules:
   `bd lease return --no-clean` applies none, and `bd lease return --keep <n> --age <duration>` changes every cargo
   rule for this call. `bd lease clean` applies the rules now (--free: the free slots; --host: the idle host slots; --dry-run only reports;
   --keep, --age as for the return). The rules are the user's choice: pass `bd lease return --no-clean` only when
   the user asks. `bd lease adopt <dir>...` adds clean linked worktrees of the repository as
   free slots, where they are, or with --move in the pool directory (`moved_from`; the same volume only,
   `other_volume`).
6. The reflog of the branch tells who moved it: `branch-deck: pick`, `undo pick`, `update project`, `undo update`,
   `squash fixups` (`bd fold` or the app's Squash Fixups), `reword <n> commits` (`bd group`, `bd rename`,
   `bd restack`, `bd unstack`, `bd reword`), `undo`,
   `squash <n> commits` (`bd squash` or the app's Squash), `drop <n> commits`, `amend <id>`, `move hunk`; a move whose files could not follow (`pick`,
   `update project`, `unapply`, `reapply` and their undo) also with ` rolled back`. A line without the
   `branch-deck: ` prefix came from outside Branch Deck.
";

/// `bd status --help`: the fields an agent acts on, `landing_needs` first
pub const STATUS_HELP: &str = "\
`bd status` prints the branches and the unassigned commits ahead of the baseline; `bd status --brief` is small
enough to read whole, `bd status --stats` adds the size of each branch. On Space it lists per branch `landing_needs`:
what its Safe Push needs of a commit outside what it lands, the refusals `bd safe-push` would give (a name only
another branch adds, a dangling use), each with its `fix` and `landing_in` (a running Safe Push lands that branch: a
start beside it fails too). Tell the user before a Safe Push and run the fix (`bd restack …`) only after they
approve; `[]` is nothing, absent is unknown. `landed_by`: a merged Safe Push landed the branch, only its `after`
commits are left (`bd safe-push --help`); `parked`: it stays local until the user unparks it; `unapplied`: the
branches `bd unapply` took out; `stacks`: the stack branch of each stack that forks (`bd push --help`);
`prefix_source`: where the branch prefix came from.
";

/// `bd suggest --help`: the fields of the plan
pub const SUGGEST_HELP: &str = "\
`bd suggest` prints a plan, clustered and checked by a trial sync (exit 1: `check.ok` is false, edit the
plan). Per cluster: `change` (kept, renamed, merged, split, new) against its `current_branches`; its commits in
`kept`, `moved` ({commit, leaves}: the branch it leaves, null if unassigned) and `new_path` (the same branch,
another stacked path in the subject): with `moved`, what the plan changes; `why`, one line to quote to the user;
`weak`, review these first; `issue_keys` and `areas`; with stacked branches `parent`, `restacked` (it stacks each
branch on the branches it needs) and `stacked_without_need` (a stack parent the branch needs nothing of: the plan
keeps it, ask the user before an unstack). Whole issue branches that belong together go into `issue_groups`: no
subject changes. `bd suggest --fresh` ignores the current prefixes and issue groups. To look closer:
`bd status --brief`, `bd status --stats` (the size of each branch), `bd commits --brief` (per commit its group,
issue key, fixup target and `areas`; `units`, the commits that must stay together; `dependencies` {from, to, via}:
`from` touches lines `to` added, or with `via` `symbols` uses a name `to` adds, which a stack satisfies too), and
`bd commits <id>... --patch` (the blocks of each file, with their ids) or `bd commits --unassigned` for unclear
commits.
";

/// `bd check --help`: the plan and the result of each branch
pub const CHECK_HELP: &str = "\
`bd check --plan plan.json` checks a plan by a trial sync. A plan is {\"branches\": {\"<path>\": [ids]},
\"unassign\": [ids], \"issue_groups\": {\"<name>\": [keys]}, \"ungroup_issues\": [keys], \"unstack\": [branches],
\"descriptions\": {\"<branch>\": {\"title\", \"body\"}}}; a path is a branch, or `parent/leaf` for a stacked one. Fixups follow their targets; a commit goes with, or stacks
on, the commits it depends on. In Windows PowerShell write the plan to a file and pass the file (a PowerShell 5.1
pipe turns non-ASCII into `?`). After any change of `branches`, `unassign`, `issue_groups` or `ungroup_issues`, run
`bd check --plan plan.json`: exit 1 until the `result` of every branch is `ok`.
- `conflict` {commit, subject, files, missing_commits}: move the missing commits into the branch, or stack it on
  the branch that has them. A conflict of git whose every file the words resolve is no conflict: with `git config
  branchdeck.resolveByWords` on (the default) the sync commits the result as IntelliJ resolves it, all or nothing
  per commit, and the branch is `ok`. `bd sync` lists such commits in `resolved` of the branch, `bd status` and
  `bd commits` in `resolved_by_words` of the commit; nothing to do.
- `dangling_reference` (`dangling_references` {kind, identifier, file, line, removed_by, fixed_by, mutual,
  declared_in}): git merges it, the build fails. The branch removes (`removed_declaration`) or changes
  (`changed_declaration`, e.g. made `suspend`) a declaration (`declared_in` {file, line}) its tree still uses, or
  declares one its tree has in another file (`duplicate_declaration`), and `fixed_by` of another branch changes that
  use or deletes that file. Move `fixed_by` into the branch, or stack it on that branch; `mutual`: that change needs
  the branch too, so `fix` moves it into the branch, as `move_commit` (its `branches` into the plan) or `move_hunk`
  (a ready `command`, once the user approved), each proven by a trial build; no `fix`: none builds. When the user
  says the use means another declaration, `bd check --allow-dangling` keeps the `result` and exits 0.
- `blocked` (`blocked_by`: the parent it can't be built on), `problems`, `error`; `build_failed` (below).
`bd check --fix` searches, by trial builds, the regroup that fixes the conflicts and dangling references: each
`fix.components[].plan` is a whole plan for the next step, and its `steps` say what moves where (show them before
applying one). A branch that conflicts as a whole is restacked on the branch (or a chain of the branches) that
holds the commits it needs, in one trial; dangling references are searched after the conflicts. Each trial is a
line on stderr (`trial 3/12: restack <branch> on <parent> … verified (41 s)`); --max-trials N (12) and
--time-budget SECONDS (180, the whole command) bound it, and `fix.search: budget` is the best plan so far: run it
again, the cache makes it faster. `inverted_stack` {branch, child,
fixes, reason}: the branch needs a commit of a branch stacked on it; each fix (`move_to_child`, `swap`) is
`branches`/`unstack` for the plan, checked by a trial build; `reason` when none builds. `warnings`: a
`possible_dangling_reference` (or `possible_changed_declaration`, `possible_duplicate_declaration`) that no commit
proves: tell the user. `missing_declarations` {branch, identifier, file, used_by, declared_by, declared_in}, the
branch's `result` `missing_declaration` (none when `git config branchdeck.symbols` is `report` or `off`): a use
whose declaration is outside the branch's tree, git merges it and the build fails: stack the branch on
`declared_in`, or move one of the two; when the user says it is a false alarm, `bd check --allow-missing` keeps the
`result` and exits 0. `bd check --build '<command>'`
(--build-paths <path>...) also builds a temporary copy of each tree (`build_failed`): slow on a large repository,
but a build that passed on the same tree is not run again (`cached`; --rebuild runs it after a change outside the
tree).
";

/// `bd group --help`: applying a plan
pub const GROUP_HELP: &str = "\
`bd group` applies a plan. Add `descriptions` (pull request title and body) for the branches that have none (`description` in
`bd status`); replace one only when the user asks. Then `bd group --expect-head <head of bd suggest> --sync --plan
plan.json`: one message-only rewrite, the issue groups and the descriptions under one lock, then a sync. It
prints the new `groups`, `replaced_descriptions` and `sync` (per branch `state`; a `conflict` names the commit, the
files and the commits it needs). Exit 1, a branch not built: `bd check --fix`, or regroup so that a change and what
it needs are in one branch or its parent, then `bd sync`. `head_moved`, `groups_moved`, `stacks_moved` or `locked`
(exit 3): read again and retry; a peer that only added commits on top: `bd group --expect-head <head>
--allow-append` takes them along (`appended`). `bd group --dry-run` prints the changed subjects. `bd rename <old>
<new>`, `bd restack <branch> <parent>` and `bd unstack <branch>` take the same options. `bd describe <branch>
--title ... --body ...` sets the text of one branch. The undo token of a regroup moves HEAD, the issue groups and
the stack parents back (`<head>..<head>`, `groups:<id>..<id>`, `stacks:<id>..<id>`, joined by `+`); the
descriptions stay, and the virtual branches change with the next sync.
";

/// `bd push --help`: what it pushes, its refusals and the stack branches
/// The outputs and refusals of `bd pick` beyond `bd help worktrees` (spec/cli/pick.spec.md, "Remote commits")
pub const PICK_HELP: &str = "\
Outputs and refusals: `bd help worktrees` for the pick of a worktree or of revisions, and for `bd pick --remote <branch>`:
- --remote <branch>: someone else pushed to the remote branch of a virtual branch (`remote.others_behind` of its
  status). bd syncs, fetches that branch and picks the commits a push would drop onto the checked out branch, with
  the `(branch)` prefix on those that group elsewhere: they land in the branch, and its push then drops nothing.
  `picked[]` as for any pick. Show the user what the remote has first.
- --expect-remote <id>: only from the remote head the user saw; another one is `lease_rejected` (exit 4), nothing
  picked: show what the remote has now and ask again. --dry-run fetches the branch and writes nothing else.
- `nothing_to_pick` (exit 4): the remote branch has no commit the branch lacks.
- `state: conflict` (exit 1): nothing changed; resolve it in a lease cut from HEAD (cherry-pick the commits there),
  then `bd lease pick`, or ask the user whether a force push may drop them. `bd undo <undo_token>` takes a pick back.
";

pub const PUSH_HELP: &str = "\
`bd push` pushes virtual branches, each to its own remote branch (the app's Push Branch), never into master: `bd land`
lands a branch there. Show the result to the user; once approved, `bd push <branch>...` (or --all --yes; --with-children adds the
branches stacked on them) syncs, takes the parents along that have commits for the remote (--alone does not: the
branch carries their commits), and pushes parents first in one atomic push with a lease per ref; name every branch
of `branches` to the user (one the remote has as it is is `up_to_date`, nothing of it pushed). Every pull request goes into the baseline: each branch has a ready `gh pr create`; pipe
its `body` into it. An open pull request into a virtual branch (a parent's) is retargeted to the baseline: name
`retargeted` with `approved_by` (GitHub may dismiss those approvals); `retarget_warnings` says what GitHub refused.
--dry-run syncs, then asks the remote. Refusals, nothing pushed:
- `push_refused` (exit 4): a branch didn't sync, or is behind its remote branch (`remote.behind`, only what the
  baseline lacks too). Older versions push without --force: of commits amended or restacked here
  (`remote.amended`, `remote.restacked`), of commits the baseline has (`remote.landed`: after its whole stack
  landed, a member's remote branch has its commits as they were; `replaced_landed` names them, nothing is lost),
  and the remote copies of commits reworded here (`remote.reworded`); a stacked branch whose remote branch lacks
  new commits of the branches it is built on (`remote.ancestors_ahead`) is pushed too: its Safe Push lands them.
- `push_too_large` (exit 4): the commits add more files than `branchdeck.pushSizeLimit` (50 MiB unless set), mostly
  build output committed by mistake; `paths` names the directory and the largest files. Show the user; never raise
  the limit yourself.
- `pushed_over: true` on a pushed branch: its remote had moved without commits it lacks; bd pushed again over it.
- `lease_rejected` (exit 4): the remote changed; bd fetched it. For a branch behind or rejected,
  `error.branches[].remote.dropped_commits` lists what a push drops: show it; once approved, --force
  --expect-remote <remote.head> for that one branch. Commits of someone else (`remote.others_behind`): offer
  `bd pick --remote <branch>` first, which takes them into the branch, then `bd push` again.
- `robot_running` (exit 4): a Safe Push runs on the branch (the push would fail it at the end), or on a branch
  built on it whose commit has commits of it (the message names them; a parent restacked below it after its start
  is pushed). Wait until it merges, or, once approved, --restart-running cancels the branch's own robot, pushes the
  branch alone and restarts it on the pushed commit (`restarted`).
- `plan_changed` (exit 3): another session moved a branch after the sync; nothing was pushed: push again.
- `parked` (exit 4): a named branch is parked, or built on a parked one; with --all or --with-children a parked
  branch is only left out, named in `skipped` with `code: parked`. Tell the user, never unpark it.
- `landed` (exit 4): merged Safe Pushes landed all of every branch asked for (`landed_by` in `bd status`):
  nothing of them is left to push until they are archived. With others to push, `bd push` only leaves a landed
  one out (with `--with-children` its children too), named in `skipped` with who landed it; tell the user. A
  branch it is built on that landed is not pushed along.
`bd sync` prints the push each branch needs as `push`, ready to run once approved. With stacked branches, `stacks`
in `bd sync` and `bd status` is the stack branch of each stack that forks, `{prefix}/stack/<root>`: all its commits
in one branch, for a Safe Push of the whole stack (`state`: `built`, `blocked` by `member`, `conflict`, `stale`);
push none that isn't `built`. `bd push <root> --with-children` pushes it in the same atomic push once it is on the
remote or its pull request is open, when this sync built it (`stack_warnings` names one left out).
";

/// `bd safe-push --help`: the fields of --check, its refusals and the branches it landed
pub const SAFE_PUSH_HELP: &str = "\
`bd safe-push` starts a Patronus Safe Push (the repository on Space, the Safe Push CLI logged in once, else `not_logged_in`). First, always,
`bd safe-push <branch> --check`: it syncs and starts nothing, and prints
- `profile` (default: the one of the last robot of the branch) and the `profiles` to choose from (--profile
  <id|name|none>);
- `review`: `readiness` (`not_required`, `approved`, `missing`, `unknown`), `groups` with their `reviewers` and
  `paths`, `waiting_for` (asked, not approved yet), `not_counted` (a bot's approval counts for nothing),
  `merge_request`; Patronus judges the approvals of the GitHub pull request;
- `push_needed` and the `refusals` a start would meet; `remotes`, what the push replaces and drops of each remote
  branch (a drop refuses with `push_refused`; --force pushes the dropped commits away with the start, only once the
  user approved); a reworded branch needs a push, the robot lands the remote's messages;
- `target_check`: bd fetches the target and replays what lands onto its tip, as Patronus rebases it before its
  checks: `result` `clean`, `conflict` or `not_run`; `fetched: false` means the check used the last fetched tip;
- `lands`, what the one robot lands into the target, root first: a stacked branch lands the branches it is built
  on with it, and the branches built on it stay. `alone` (a stacked branch): `builds` (it could land alone once
  unstacked, unless `uses` names a name of its stack) or `needs` (the commits of its parents it needs, `movable`
  into it). Landing what it is built on is right: name `lands` to the user, and offer the branch alone only as a
  second way.
Once approved, `bd safe-push <branch>` pushes first when `push_needed` (the parents with commits for the remote,
by content), then starts the robot: `robot.url` is its page; a branch with an open pull request into the target
starts from a `/patronus` comment there, where its approvals count (`started_via`); one into a virtual branch is
retargeted first (`retargeted`). --patronus-dry-run starts a Dry Run instead: the checks without the push into the
target and without the review, but it pushes and starts a robot like a Safe Push. The whole stack, its side
branches included, lands in one robot with --whole-stack (from any branch of it; --check first): the robot starts
from the stack branch of its top-level root (`whole_stack.source`; a straight chain starts from its last branch),
`lands` names every branch, and its pull request is the stack's own, where the approvals of the whole stack count.
Refusals, nothing pushed or started:
- `approvals_missing` (exit 4): `error.review` names who has to approve which files: show it. --create-pull-request
  pushes and opens the pull request where they approve, assigned to the user (--no-assign leaves it unassigned;
  --reviewer <login>, repeatable, asks a `github_login` of `review.groups[].reviewers` that the user chose;
  `pull_request.warnings` says whom GitHub refused). Once they approved (`review.approved_on_github`), run it
  again. --without-approvals starts anyway, only when the user says so: Patronus fails it. A merge profile known
  to skip the review starts without them (`review.skipped_by`: `profile`).
- `robot_running` (exit 4): a robot runs on the last commit, lands the branch with a branch of its stack or with the
  whole stack, or lands a branch it is built on; --cancel-running cancels a Safe Push of an older commit. A Dry Run
  replaces no Safe Push: its push into a branch whose Safe Push runs is refused.
- `review_unknown` (exit 4, also in `refusals`): Patronus couldn't answer the code-owner review (its pre-check
  failed): ask the user; --allow-unknown-review starts anyway, and the robot checks the approvals when it starts.
- `dangling_reference` (exit 4, also in `refusals`): what the check of step 2 reports for the branch or a branch it
  lands: fix it there, or, when the user says it is a false alarm, --allow-dangling.
- `missing_declaration` (exit 4, also in `refusals`): a commit of what lands uses a name that a commit of a branch
  outside it adds (`missing_declarations` of `bd check`); the message names the restack that lands that branch too
  (`bd restack <root> <branch>`, after the user approves) or the commit to move. When the user says it is a false
  alarm, `bd safe-push --allow-missing`.
- `push_dismisses_approvals` (exit 4, also in `refusals`): the push of new commits into the approved pull request the
  robot starts from dismisses the approvals Patronus counts (`rules`): push first (`bd push <branch>`), then ask for
  the review again (`bd request-review <branch>`). A push that dismisses approvals Patronus doesn't count starts;
  tell the user `rules.message`.
- `rules_unknown` (exit 4, also in `refusals`): GitHub didn't answer the rules of the target, so the push may dismiss
  the approvals: ask the user; `bd safe-push --allow-unknown-rules` starts anyway, beside --allow-missing.
- `pull_request_other_base` (exit 4, also in `refusals`): the open pull request of the branch goes into another base
  (a release branch), so its approvals count for no Safe Push into the target: ask the user; the retarget `gh pr edit`
  of the message once approved, or --without-approvals (a start through the API) only when the user says so. A pull
  request into a virtual branch is retargeted to the target as a step of the start (`retargeted`).
- `target_conflict` (exit 4, `error.commits`, `error.paths`, also in `refusals`): Patronus can't rebase what lands
  onto the newest target; the message names the commits of the target that change the files. `bd update` resolves
  it locally (after the user approves), then run the Safe Push again.
- `stack_conflict` (exit 4, `error.commits`, `error.paths`): the stack branch conflicts though each branch builds;
  `stack_not_built` (exit 4): a branch of it is blocked or failed, or the stack branch is stale. Land the stack in
  pieces.
- `landed` (exit 4): a merged Safe Push landed all of the branch, or every branch of the whole stack: nothing is
  left to land. `lands` leaves out what merged Safe Pushes landed already.
- `busy` (exit 3, `error.holder`): another process runs what the start needs; retry once it is done. A start whose
  answer was lost (`holder.unknown`: its request went out, no answer came) holds until Patronus lists its robot; see
  `bd robot <branch>`, and only when the user says so `bd operation forget <holder.op>` (Patronus may still run it).
  A start of `bd` whose answer was lost says so in `start_unknown` and is never sent again by itself.
After a merge, until `bd update` archives them, `bd status` names the branches a merged Safe Push landed
(`landed_by`: the `branch` whose Safe Push it was, or the root with `whole_stack`, the `robot`, the `commits` it
landed, and `after`: commits added since, decided by content, the only work left, which a push and a Safe Push
send). bd closes no pull request after the merge: the app closes those of the parents that landed when it sees the
merge; when the app isn't running, tell the user that they stay open.
";

/// `bd land --help`: the routes, the fields of --check and the refusals of a push into the target
pub const LAND_HELP: &str = "\
`bd land <branch>` lands the branch into master by the route of the repository (`branchdeck.landing`, `route`): a
Safe Push of Patronus (`patronus`: what `bd safe-push` does, its output in `safe_push`), or a push of the copy into
the target (`push`: the commit the sync built of the branch on the baseline, `commit`, pushed into `target` as a
fast-forward; it lands without the robot and without the CI of a pull request), or a merge of its pull request on
GitHub (`pull-request`: `pull_request` merges by rebase, else a merge commit, else squash; while its required checks
run, or after the push of a branch its pull request shows an older commit of, `auto_merge: true`: GitHub merges it once
they pass, nothing landed yet). First, always, `bd land <branch> --check`: it
syncs and pushes nothing, and prints `lands` (a stacked branch lands the branches it is built on with it;
--whole-stack the whole stack, from its stack branch once the sync built it), `confirmations` and the `refusals` a
push would meet. A push into master is as final as a merge: show `lands` and every confirmation to the user, and push
only after they approved, as any push. A push that landed prints `landed: true` and `next: bd update`, which brings
the branch onto the target and drops the commits that landed; bd land never updates itself.
Refusals, nothing pushed:
- `checks_not_passed` (exit 4, also in `refusals`): CI did not pass on the commit it lands (no open pull request, one
  that shows another commit, failing or running checks, GitHub listing none), or changes are requested on its pull
  request: ask the user; --without-checks pushes once they agree.
- `no_write_access` (exit 4): the token may not push into the repository (GitHub's `viewerPermission`): someone with
  write access lands it.
- `baseline_requires_pull_request` (exit 4): the rules of the target take a change only through a pull request; a
  maintainer or an admin, whom a ruleset may let bypass it, is asked instead: --bypass-rules once the user agrees.
- `baseline_refused` (exit 4): the target refuses the push: a rule no push of the copy passes (the merge queue,
  restricted updates, signed commits), or the server refused it (a hook, a protected branch; the message says what it
  said). Tell the user.
- `baseline_moved` (exit 3): the target moved since the copy was built on it (no fast-forward); bd fetched it. `bd
  update` brings the branch onto it (after the user approves), then `bd land` again builds the copy anew.
- `push_refused` (exit 4): the baseline is a local branch, the branch is not synced, or the required status checks of
  the target (--bypass-rules once the user agrees); `landed` (exit 4): the baseline has all of it already: `bd update`.
- `dangling_reference`, `missing_declaration`: as for `bd safe-push`; `bd land --allow-dangling` or `bd land
  --allow-missing` only when the user says it is a false alarm. `parked`, `robot_running`, `stack_not_built`: as
  for `bd safe-push`.
- `busy` (exit 3): another process lands into the same target; retry once it is done.
- `no_pull_request` (exit 4): a merge needs an open pull request into the target: `bd safe-push <branch>
  --create-pull-request` opens one. `merge_blocked` (exit 4): approvals, required checks, changes requested or a
  draft hold the merge back: tell the user, it lands once they come. `not_mergeable` (exit 4): a conflict, a head
  behind its base (`bd update`, then `bd push`), or GitHub refused the merge (its words). `landing_route_unsupported`
  (exit 4): the pull requests are off GitHub or on a mirror `origin` doesn't push to.
";

/// `bd request-review --help`: whom it asks and its refusals
pub const REQUEST_REVIEW_HELP: &str = "\
A pull request that asks no code owner of a missing group: `bd request-review <branch> --check` names whom
it would ask (`reviewers`: one suggested code owner of each such group) and what it would push first (`pushed`);
once approved, `bd request-review <branch>` asks them on GitHub (--reviewer <login>, repeatable, asks these
instead): `requested`, `refused` with GitHub's reason, `waiting_for`. `needs_push` (exit 4): the branch has commits
its remote lacks, which the reviewers would not see; once the user approves the push, --push pushes them first
(`pushed`). `no_pull_request`: none is open, or a draft; `no_reviewer`: Patronus names no GitHub login, ask the
user whom; `review_refused`: GitHub asked nobody.
";

/// `bd robot --help`: the robot of a branch and why it failed
pub const ROBOT_HELP: &str = "\
`bd robot <branch>` prints the robot of the branch (`robot.state`; `running`: ask again later; --robot <id>
for another one; a newer robot of the whole stack names `stack_root`), and --whole-stack the robot of the whole
stack. After a failed Safe Push, --full says why (`failure`: `kind`, the failed `checks` with their build ids,
`problems`, the failed `builds` with their failed tests and log tails) and which commits it tested (`commits`, by
the ids of the checked-out branch: `tested`, `rebased`, `changed`, `added`, `removed`, `landed`).
`kind: infrastructure`: nothing to fix, the user restarts it; else fix and check locally, the user pushes and
restarts it. A merged one lists `closes`: the pull requests of the branches it landed, which the app closes; never
close them yourself.
";

/// `bd robot-stats --help`: the statistics
pub const ROBOT_STATS_HELP: &str = "\
`bd robot-stats` prints how the user's Safe Pushes went over the last 30 days (`--period 14d`, `10w`,
`6m`) against everyone else's (`mine`, `everyone`: robots per merge, the time to merge), the time lost (`time`),
why robots of the last 14 days failed and which checks passed only on a retry (`failures`), and the fastest hours
(`start_hours`); --refresh asks Patronus again for the weeks it may still change.
";

/// `bd fold --help`: folding the fixups
pub const FOLD_HELP: &str = "\
`bd fold` (--branch, --expect-head, --dry-run) folds the fixup!, squash! and amend! commits into their
targets and keeps HEAD's tree. A fold whose fixup conflicts with a commit in between or after it (`squash_conflict`,
`commits`, `paths`) or that would leave its target or a later commit empty (`squash_empty`) folds nothing: tell the
user.
";

/// `bd squash --help`: the squash the user asked for
pub const SQUASH_HELP: &str = "\
`bd squash <into> <commit>...` squashes commits into `<into>`, the oldest of them, as the app's Squash does: the
others leave the history, the commits in between stay, HEAD's tree stays, so the files and the index are not touched.
Only for commits the user asked to squash. All of them are ahead of the baseline (`not_ahead_of_baseline`) and in the
branch of `<into>`, else `invalid_arguments` (move them first with `bd group`).
- `bd squash` keeps the message of `<into>`; --subject '<subject>' (the `(prefix)` included) replaces its subject and
  keeps its body; --message '<message>' or --message-file <file> (`-` for stdin) is the whole message. A message that
  moves the commit to another branch is refused (`invalid_arguments`).
- Run `bd squash --dry-run` first: it builds the squash and drops it. Show the user `squashed`, `subject` and
  `rebuilt_branches`, the branches the next sync builds anew (a pushed one needs a push again), and squash once they
  approve, with --expect-head <previous_head>; --sync syncs after it.
- Prints `new_id`, `previous_head`, `new_head` and an `undo_token`: `bd undo <undo_token>` takes it back.
- `squash_conflict` (exit 4): a squashed commit and a commit in between or after it change the same lines (`commits`,
  `paths`); `squash_empty` (exit 4): the squashed commit would have no changes, or a later commit would become empty.
  Both change nothing: tell the user, never squash another way.
";

/// `bd reword --help`: the change of a message
pub const REWORD_HELP: &str = "\
A message to change (a subject too long, a typo): `bd reword <commit> --subject '<subject>'` (the body stays),
--message '<message>' or --message-file <file> (`-` for stdin), or several commits in one rewrite with --plan <file>
(`-` for stdin) of {\"rewrites\": [{\"commit\": \"<id>\", \"subject\": \"...\"}]} (or `message` for the whole
message); --expect-head, --allow-append, --dry-run, --sync, --map as for `bd group`. The subject is the whole line:
keep its `(prefix)`, unless the commit should move to another branch (`changes[].moved` says it, check with
--dry-run first). Every tree, the index and the files stay; the commits after it get new ids. Never an empty
`amend!` commit, an amend with `git commit`, `commit-tree` or `update-ref` for it. `invalid_plan` (exit 2) when a
fixup would fold elsewhere: fold it first, or reword it in the same plan.
";

/// `bd move-hunk --help`: the move of a change and its refusals
pub const MOVE_HUNK_HELP: &str = "\
A change in the wrong commit: `bd move-hunk <from> <to> <path>` (--block <id>, repeatable; --expect-head, --dry-run;
--sync rebuilds the virtual branches at once, else the next sync does) moves the changes of the file `path`, or
only its blocks by the ids that `bd commits <from> --patch` prints, into the commit `to`, older or newer; HEAD's
tree, the index and the working tree stay, and the commits between keep their diff. `hunk_conflict` (exit 4): a
commit between changes the same lines, or lacks the file, also when the words would resolve it: bd can't move it; `block_not_found` (exit 2): the file
changed since the id was read, `error.blocks` are its ids now.
";

/// The skill `bd install` writes and `bd help skill` prints (`SKILL.md`, the Agent Skills format of Claude Code, Codex
/// and others), naming bd by `command`, its full path. It says when to use bd, what `bd workspace` asks, the rules an
/// agent must not miss before it reads any help, and where the workflow is; the rest lives in the binary, so it can't
/// fall out of date: `bd help agent` is the workflow, `bd <command> --help` the outputs and refusals. `allowed-tools` (Claude Code) lets the agent
/// run every bd command without a permission prompt while the skill is active, also the ones that rewrite the branch or
/// push: a push still waits for the user, by the rule of the body
pub fn skill_text(command: &str) -> String {
  let allowed_tools = yaml_quoted(&format!("Bash({command}:*)"));
  format!(
    "\
---
name: branch-deck
description: Use before the first edit or commit in a Git repository: `bd workspace` of Branch Deck says whether to \
work in a leased worktree of your own and how to write the commit subject. Also use to group, split or stack the \
commits of a branch into virtual branches and pull requests, to sync or push them, to fold fixup commits, to change \
the message of a commit or move a hunk into another commit, to update or rebase a branch onto master or main and \
resolve its conflicts, to land the commits of a worktree onto a shared branch, to lease or return a worktree, to start \
a Patronus Safe Push or Dry Run or find out why one failed, to ask code owners to review a pull request, or when the \
user mentions Branch Deck.
allowed-tools: {allowed_tools}
---

# Branch Deck (`bd`)

bd is `{command}`; run it as written, it is quoted for the shell. Below, `bd` stands for it.

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the `(name)` prefix
of their subjects. bd reads those groups and the commits, regroups the commits by rewriting the prefixes, rebuilds the
virtual branches, pushes them and starts their Patronus Safe Push. Only `bd update` (the branch onto its baseline),
`bd pick` (the commits of another worktree onto the branch), `bd unapply` and `bd reapply` (a branch out of it and back) and their undo write the working tree, through the
guarded update.

Before the first edit or commit in a repository, run `bd workspace` and follow it; what the user says in the session
wins over it:
- `lease.on`: work in a worktree of your own. Run the first command of `next` (`bd lease get --holder <your name>`, a
  name of this session) and edit, build and commit only in its `path` (and `subdir` there). In Claude Code, `cd` into
  `path` once: the shell stays there when the pool is in `permissions.additionalDirectories` of the settings, and
  Read, Edit and Write take the files by their paths in `path`. When the shell goes back, `EnterWorktree` with that
  `path` moves the session there; it asks the user. Its `next` (and `bd workspace` inside it) names the rest:
  land each commit that builds and passes its tests at once with `bd -C <path> lease pick`, onto the branch of `home`;
  a conflict comes back to you, `conflict.next` says what to do. Once the task is done and every commit landed,
  `bd lease return --lease-id <lease_id>`. Never `git rebase` or `git merge` into the shared checkout, and never
  `bd update` in a lease. Return only your own lease,
  never one of `active_leases` that another session holds. A lease lacks the `local_changes` of the shared checkout:
  when the task needs them, ask the user.
- `lease.on` false (`reason` says why, when it is set): work where you are; stage only the paths you changed, never
  `git add -A` or `git add .`.
- `subject_branch.on`: start each commit subject with `(branch) `, a readable name of a few words, never an issue id as
  the name; reuse a branch of `bd status --brief` that fits; with `stacks`, a new stack is `(parent/leaf)`. Off: write
  the subject as the repository does, without a prefix.
- Both are the user's settings (Settings → Agents of the app, `branchdeck.agentLease`, `branchdeck.agentSubjectBranch`):
  never change them.
- A lease is the only worktree of your own: never `git worktree add` or a clone for it on your own initiative, also not
  when a bd command fails. Then work where you are when that is safe, or ask the user, and name a lease that survived
  (`path`, `lease_id`, `holder`).
- In a sandbox (Codex) a change of directory does not make `path` writable: before the first edit there, ask for write
  access to exactly `path` (`request_permissions`), never the pool or full access. Refused, return the untouched lease.

To group or push commits or to start a Safe Push, run `bd help agent` first and follow the workflow it prints: it is the
contract of the installed version. To update a branch or resolve its update conflicts, run `bd help update` instead.
When several sessions or worktrees work on one branch, run `bd help worktrees` first. `bd <command> --help` says what a
command prints and what each of its refusals asks.

- Run bd in the repository, or pass `-C <repo>`.
- Land the commits of your lease with `bd lease pick`, of another worktree with `bd pick`, never with
  `git cherry-pick`, which leaves `CHERRY_PICK_HEAD`.
- Every command prints one JSON object on stdout, with the `operation_id` of its run (its log lines, its journal
  entry). Branch on `error.code` and the exit code, never on the message;
  `bd schema <command>` prints the JSON Schema of its output, `bd version` the version of bd and git.
- Never reorder, squash or split commits on your own, except folding fixup!, squash! and amend! commits with `bd fold`;
  squash the commits the user asked to squash with `bd squash`, after showing its --dry-run. Never
  drop a commit yourself: `bd update` drops a commit it proves landed, and `--landed` drops one after the user's yes.
  Move a change between commits only with `bd move-hunk`, change a message only with `bd reword`. Never push, start a Safe
  Push or create pull requests before the user approved them; an `error.next_steps` entry with `needs_approval` waits
  for that approval too.
- Before a Safe Push, run `bd safe-push <branch> --check` first and show the user what it lands: the Safe Push of a
  stacked branch lands the branches it is built on with it, in one robot, and `--whole-stack` lands the whole stack,
  its side branches included, in one robot.
- `bd land` of a repository that lands by a push (`route: push`) pushes into master itself, without a robot and without
  the CI of a pull request: run `bd land <branch> --check`, show the user `lands` and `confirmations`, and run it only
  once they approved, as any push; never pass --without-checks or --bypass-rules on your own. After it, `bd update`.
- A branch `parked` in `bd status` stays local until the user unparks it: `bd push` and `bd safe-push` refuse it
  (`parked`). Never push or unpark it on your own.
- `bd unapply <branch>` takes a branch and its files out of the checked out branch, kept as an unapplied one, and
  `bd reapply <name>` puts it back on top; only when the user asks, and `bd undo <undo_token>` takes either back.
"
  )
}

/// A YAML double-quoted scalar: the shell quoting of a path can hold ` #` or `: `, which end a plain scalar
fn yaml_quoted(value: &str) -> String {
  format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
