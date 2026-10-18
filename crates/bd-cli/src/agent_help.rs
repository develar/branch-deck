/// What the reading commands write, for `bd --help` and `bd help agent`; a macro, so that `concat!` takes it
macro_rules! read_only {
  () => {
    "Read only: `bd status`, `bd commits`, `bd suggest`, `bd check`, `bd robot`, `bd schema`, `bd version`,
`bd describe <branch>` without a setter, `bd request-review --check`, `bd update --status` without --materialize, and
`bd group --dry-run`, `bd rename --dry-run`, `bd restack --dry-run`, `bd unstack --dry-run`, `bd update --dry-run`
(also with --continue), `bd pick --dry-run`, `bd fold --dry-run` and `bd move-hunk --dry-run` write no ref, config,
index or working tree: at most git objects (a trial sync, a replay, the trees of a move) and the object cache (none
with --no-cache); `bd check --build` runs its command in a temporary copy of each tree. `bd push --dry-run` and
`bd safe-push --check` sync first."
  };
}

/// The read-only line of `bd --help`
pub const READ_ONLY: &str = read_only!();

/// `bd help agent`: the workflow of an agent that groups commits into virtual branches (docs/guidelines/agent-cli.md);
/// the first line names the version (`version::VERSION`). `agent_help_test.rs` checks it against the commands and
/// options
pub const AGENT_HELP: &str = concat!(
  "bd ",
  env!("BD_VERSION"),
  ": Branch Deck for agents. JSON on stdout (every object has \"schema\": 1), logs on stderr (-v for debug).

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches: by the `(name)`
prefix of a subject, else by the issue group of its issue key, else by the issue key (`ABC-123 Fix ...`). A sync
rebuilds each group as `{prefix}/virtual/{name}` on the baseline; the working tree is never touched.

Global options: -C <repo>, --prefix <p> (else `branchdeck.branchPrefix` of the repository's own git config, else the
app setting, else the global `branchdeck.branchPrefix`), --full-ids (ids are abbreviated by default), --no-cache, -v.
Errors: {\"schema\":1,\"error\":{\"code\",\"message\",\"commits\",\"paths\",\"retryable\"}}. Exit codes: 0 ok, 1 conflict,
2 invalid input, 3 stale (read again and retry), 4 refused, 5 git or network error.
",
  read_only!(),
  "
`bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.

Workflow:
1. `bd suggest`: a plan, clustered and checked by a trial sync (exit 1: `check.ok` is false, edit the plan). Per
   cluster `change` (kept, renamed, merged, split, new) against the branches in `from`, its commits in `kept`,
   `moved` ({commit, leaves}: the branch it leaves, null if unassigned) and `new_path` (same branch, another stacked path
   in the subject: with `moved`, what the plan changes; refuse a plan that changes another session's commit), `why` (one line to quote to the user), `weak`
   (review these first), `issue_keys` and `areas`; with stacked branches `parent` and `restacked`:
   it stacks each branch on the branches it needs. `stacked_without_need`: the stack parents stack the branch on a
   parent it needs no commit of; the plan keeps it, ask the user before `bd unstack`. Whole issue branches that belong together go into `issue_groups`
   of the plan: no subject changes. `bd suggest --fresh` ignores the current prefixes and issue groups. To look closer: `bd status --brief`, and `bd commits --brief` (per commit its group, issue key, fixup
   target and `areas`; `units`, the commits that must stay together; `dependencies` {from, to, via}: `from` touches
   lines `to` added, or with `via` `symbols` uses a name `to` adds, which a stack satisfies too). `bd commits <id>... --patch` or `bd commits --unassigned` for unclear commits, `bd status --stats`
   for the size of each branch.
2. The plan (in Windows PowerShell write it to a file and pass the file: a pipe of PowerShell 5.1 turns non-ASCII
   into `?`): {\"branches\": {\"<path>\": [ids]}, \"unassign\": [ids], \"issue_groups\": {\"<name>\": [keys]},
   \"ungroup_issues\": [keys], \"unstack\": [branches], \"descriptions\": {\"<branch>\": {\"title\", \"body\"}}}. A path
   is a branch name, or `parent/leaf` for a stacked branch (`git config branchdeck.stacks true`): the stack parents
   (`refs/branch-deck/stack-parents`) stack `leaf` on `parent`, so an issue branch or issue group stacks without a
   rewrite and a commit that needs a prefix gets the bare `(leaf)`; `unstack` makes branches roots. Fixups follow their targets; a commit
   goes with, or stacks on, the commits it depends on. After any change of `branches`, `unassign`, `issue_groups` or
   `ungroup_issues`: `bd check --plan plan.json`. Per branch `ok`, `conflict` {commit, subject, files,
   missing_commits}, `dangling_reference` (git merges it, the build fails: `dangling_references` {kind, identifier, file,
   line, removed_by, fixed_by, mutual}: the branch removes (`removed_declaration`) or changes (`changed_declaration`,
   e.g. made `suspend`) a declaration its tree still uses, or declares one its tree has in another file
   (`duplicate_declaration`), and `fixed_by` of another branch changes that use or deletes that file), `blocked_by` or
   `problems`. Exit 1 until every branch is ok: move the missing commits (or
   `fixed_by`) into the branch, or stack it on the branch that has them; `mutual`: the change in `fixed_by` needs the
   branch too, so `fix` moves it into the branch: `move_commit` (its `branches` into the plan) or `move_hunk` (a ready
   `bd move-hunk` `command`, after the user approves), each proven by a trial build; no `fix`: none builds, ask
   `bd check --fix`. `inverted_stack` {branch, child, fixes, reason}: the branch needs a commit of a branch stacked on it; each
   fix (`move_to_child`, `swap`) is `branches`/`unstack` to add to the plan, checked by a trial build; `reason` when
   none builds. `bd check --fix` also finds the regroup that fixes them, by trial builds: each
   `fix.components[].plan` is a whole plan for `bd group`, and its `steps` say what moves where; show them to the user
   before applying one. `warnings` lists a `possible_dangling_reference` (or
   `possible_changed_declaration`, `possible_duplicate_declaration`) that no commit proves: tell the user. `missing_declarations` {branch, identifier, file, used_by, declared_by, declared_in}
   (unless `git config branchdeck.symbols` is `report` or `off`) names a use whose declaration is outside the
   branch's tree: stack the branch on `declared_in`, or move one of the two. `bd check --build '<command>'` (--build-paths <pathspec>...) also runs a build in a
   temporary copy of each tree (`build_failed`); slow on a large repository, but a build that passed on the same
   tree is not run again (`cached`; `--rebuild` runs it after a change outside the tree).
3. Add `descriptions` (pull request title and body) for the branches that have none (`description` in
   `bd status`); replace one only when the user asks. `<branch>` is the leaf for a stacked branch. Then
   `bd group --expect-head <head of suggest> --sync --plan plan.json`: one message-only rewrite, the issue groups,
   the descriptions and a sync, under one repository lock. It prints the new `groups`, an `undo_token`,
   `replaced_descriptions` and `sync` (per branch `status`; a `conflict` names the commit, the files and the commits
   it needs). Exit 1 on a conflict: `bd check --fix` finds a regroup that fixes it; else regroup so that a change and
   what it needs are in one branch or its parent, and `bd sync` again. Another exit with `error` and an `undo_token`: the rewrite stands (a failed description write
   skips the sync); `bd sync` again, or undo. `head_moved`, `groups_moved`, `stacks_moved` or `locked` (exit 3):
   read again and retry. A peer that only added commits on top: `bd group --expect-head <head> --allow-append` takes
   them along and lists them in `appended`. `bd group --dry-run` prints the changed subjects and writes nothing. Also `bd rename <old> <new>`, `bd restack <branch> <parent>` and `bd unstack <branch>`, with the
   same options; `bd describe <branch> --title ... --body ...` for one branch. `bd undo <undo_token>` moves HEAD,
   the issue groups and the stack parents back (pass the token as printed, with full ids: `<head>..<head>`,
   `groups:<id>..<id>`, `stacks:<id>..<id>`, joined by `+`); the descriptions stay, and the virtual branches change only with the next `bd sync`.
4. Show the result to the user. Only after the user approves: `bd push <branch>...` (or --all --yes; add
   --with-children for the branches stacked on them): it syncs, takes the branches a stacked branch is built on along
   when they have commits for the remote (--alone does not: a branch carries their commits), pushes parents first with
   a lease per ref, and prints a ready `gh pr create` per branch, into the baseline as every pull request (pipe its
   `body` into it); name every branch of `branches` to the user. An open pull request of a pushed branch into a
   virtual branch (a parent's) is retargeted to the baseline: name `retargeted` to the user, with `approved_by` (GitHub
   may dismiss those approvals); `retarget_warnings` says what GitHub refused.
   `push_refused` (exit 4): a branch didn't sync, or is behind its remote branch (`remote.behind`; older versions of
   commits amended or restacked here, `remote.amended` and `remote.restacked`, push without --force). `robot_running` (exit 4): a Safe Push runs on the branch, or on a branch
   built on it that lands it: a push won't land, and it fails the Safe Push at the end; a branch taken along that a
   Safe Push lands stays as the robot took it. Wait until it merges, or, only after the user approves,
   `bd push <branch> --restart-running` cancels it, pushes and restarts it on the pushed commit (`restarted`).
   `lease_rejected` (exit 4): the remote changed,
   nothing was pushed, and bd fetched it. For a branch behind or rejected, `error.branches[].remote.dropped_commits`
   lists what a push drops: show it, and only after the user approves,
   `bd push <branch> --force --expect-remote <remote.head>`. `bd sync` prints the push each branch needs as `push`,
   ready to run as written once the user approved it. With stacked branches, `stacks` in `bd sync` and `bd status`
   is the stack branch of each stack that forks, `{prefix}/stack/<root>`: all its commits in one branch, for a Safe
   Push of the whole stack (`state`: `built`, `blocked` by `member`, `conflict`, `stale`); push none that isn't
   `built`. `bd push <root> --with-children` pushes it with the branches, in the same atomic push, when this sync
   built it (`stacks` of the output; `stack_warnings` names one left out).
5. A Patronus Safe Push (the repository on Space, the Safe Push CLI logged in once, else `not_logged_in`): first
   `bd safe-push <branch> --check`, always. It prints `lands`, the merge `profile` (default: the one of the last robot
   of the branch), the `profiles` to choose from, the code-owner `review` (`readiness`: `not_required`, `approved`,
   `missing` or `unknown`; `groups` with their `reviewers` and `paths`, `waiting_for`, `not_counted`, `merge_request`),
   `push_needed` and the `refusals` of a start. `lands` is what the one robot lands into the target, root first: a
   stacked branch carries the commits of the branches it is built on, so its Safe Push lands them with it, and the
   branches built on it stay. For a stacked branch `alone` is a trial build of the branch without them: `builds` (it
   could land alone after `bd unstack <branch>`, unless `uses` names a name of its stack) or `needs` (the commits of
   its parents it needs, `movable` into it). Landing what it is built on is always right: name `lands` to the user,
   offer the branch alone only as a second way, and unstack or move only when the user chooses it. The whole stack,
   its side branches included, lands in one robot with `bd safe-push <branch> --whole-stack` (any branch of the
   stack; `--check` first, as always): the robot starts from the stack branch of its top-level root
   (`whole_stack.source`; a straight chain has none and starts from its last branch), only that branch is pushed,
   `lands` names every branch of the stack, and its pull request is the stack's own
   (`bd safe-push <branch> --whole-stack --create-pull-request`), where the approvals of the whole stack count.
   `conflict` (exit 1): the stack branch conflicts though each branch builds (`error.commits`, `error.paths`);
   `stack_not_built` (exit 1): a branch of it is blocked or failed, or the stack branch is stale: tell the user, and
   land it in pieces. Patronus judges
   the approvals of the pull request: a bot's approval counts for nothing (`not_counted`). Show them to the user; only
   after the user approves, `bd safe-push <branch>` (--profile <id|name|none>; --dry-run runs the checks without the
   push into the target and checks no review). It pushes the branch first when `push_needed`, with the parents that
   have commits for the remote (by content: a branch only rebuilt on a newer baseline is not pushed), as `bd push`
   does, then starts the robot:
   `robot.url` is its page; a branch with an open pull request into the target starts from a `/patronus` comment
   there, where its approvals count (`started_via`); one into a virtual branch is retargeted to the target first
   (`retargeted`, also those of the parents it pushes). `approvals_missing` (exit 4): nothing was pushed or started,
   `error.review` names who has to approve which files: show it. The approvals count in the GitHub pull request of
   the branch: `bd safe-push <branch> --create-pull-request` pushes the branch and opens it, assigned to the user
   (--no-assign leaves it unassigned); `--reviewer <login>` (repeatable) asks a reviewer: the `github_login` of the
   suggested `review.groups[].reviewers`, once the user chose them. `pull_request.warnings` says whom GitHub refused.
   `review.waiting_for` are the code owners asked who haven't approved yet. Once they approved it there
   (`review.approved_on_github` names the approvals that count), run it again.
   --without-approvals starts anyway, only when the user says so: Patronus fails it without them. `robot_running`:
   a robot runs on the last commit, lands the branch with a branch of its stack or with the whole stack, or lands a
   branch it is built on (for the whole stack: a robot of a branch of it); a Safe Push of an older commit is canceled
   with --cancel-running, after the user approves.
   `dangling_reference`: the branch or a parent removes a declaration that a commit of another branch still changes
   the use of, as check reports it: fix it, or `bd safe-push <branch> --allow-dangling` when the user says it is a
   false alarm. bd closes no pull request after the merge: the app closes those of the parents that landed with the
   branch when it sees the robot merge; if the app isn't running then, tell the user that they stay open.
   A pull request that exists but asks no code owner of a missing group: `bd request-review <branch> --check` names
   whom it would ask (`reviewers`: one suggested code owner of each such group); only after the user approves,
   `bd request-review <branch>` asks them on GitHub (--reviewer <login>, repeatable, asks these instead and doesn't
   ask Patronus): `requested`, `refused` with GitHub's reason, `waiting_for`. `no_pull_request` (exit 4): none is
   open, or a draft; `no_reviewer`: Patronus names no GitHub login, ask the user whom; `review_refused`: GitHub
   asked nobody.
   `bd robot <branch>` prints the robot of the branch (`robot.status`, `running`: run it again later; --robot <id> for
   another one; a newer robot of the whole stack names `stack_root`), and `bd robot <branch> --whole-stack` the robot
   of the whole stack. After a failed Safe Push, `bd robot <branch> --full` says why (`failure`: `kind`, the failed
   `checks` with their build ids, `problems`, and the failed `builds` with their problems, failed tests and log
   tails) and which commits it tested (`commits`, by the ids of the checked-out branch: `tested`, `rebased`,
   `changed`, `added`, `removed`, `landed`). `kind: infrastructure`: nothing to fix, the user restarts it. Fix and
   check locally; the user pushes and restarts the Safe Push.

`bd update` (--no-fetch, --dry-run) brings the checked out branch onto its baseline as the app's Update Project;
`uncommitted_overlap` (exit 4) lists the `paths` that block it (never stash them: ask the user). A conflict starts a
resolve session (exit 1): `bd help update` explains it. `bd undo update:<id>..<id>` takes an update back
(`undo_stale`, exit 4, once the branch moved on). `bd pick --from <worktree> --follow` lands the commits of another
worktree on the checked out branch without a cherry-pick (or `bd pick <rev>`, `<a>..<b>`; --expect-head, --dry-run):
replayed in memory under the repository lock, which every worktree shares, the files following; --follow moves that
worktree onto the new tip. `already_present` lists the commits the branch has already (never an empty commit),
`picked[].grouped_by` `none` a commit without a prefix; `state: conflict` (exit 1) changes nothing and
`conflict.next` says what to do; `bd undo pick:…` moves both back. `bd help worktrees` says how sessions in worktrees of their
own share one branch. `bd fold` (--branch, --expect-head, --dry-run) folds fixup! commits.
A change that landed in the wrong commit: `bd move-hunk <from> <to> <path>` (--hunk <n>, repeatable; --expect-head,
--dry-run, --sync) moves the changes of the file `path`, or only its hunks numbered as in the patch of `from`, into the
commit `to`, older or newer; HEAD's tree, the index and the working tree stay, the commits between keep their diff.
`bd commits <from> --patch` shows the hunks. `hunk_conflict` (exit 1): a commit between changes the same lines, or
lacks the file; `hunk_not_found` (exit 2). It prints an `undo_token`; the virtual branches change with the next sync.

Rules: never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`; move a
change between commits only with `bd move-hunk`, never with plumbing; only `bd update`, `bd pick` and their undo write
the working tree, through the guarded update; a plan only moves commits between branches. Exit 2 (invalid_plan,
not_ahead_of_baseline): change the plan.

`bd install` puts `bd` into ~/.local/bin and a skill into ~/.claude/skills and ~/.agents/skills; `bd help skill`
prints the skill.
"
);

/// `bd help update`: the update and its resolve session (spec/cli/update.spec.md); `agent_help_test.rs` checks it with
/// `AGENT_HELP` against the commands and options
pub const UPDATE_HELP: &str = "\
bd update: bring the checked out branch onto its baseline (e.g. origin/master), as the app's Update Project, and
resolve its conflicts outside the working tree. Other sessions may work in the same tree: bd never stashes, resets or
checks out, and only the finish writes the working tree, through the guarded update (only the files the update
changes; local changes on them refuse it, `uncommitted_overlap` with `paths`).

`bd update` fetches the baseline (--no-fetch: the last fetched state; --dry-run: the check only, no fetch, nothing
written but git objects). Exit 0: `state` is up_to_date, fast_forward or rebase, with an `undo_token`. Exit 1: a
commit conflicts, and a resolve session opens, one round per conflicting commit. The session belongs to this worktree
and branch, and keeps the baseline it started with; peers' commits on top of HEAD join it as more rounds. Running
`bd update` again prints the open round (and writes its missing copies); `state: pending` means every commit is
resolved and `session.next` finishes. Local commits whose patch the baseline has already (`git cherry`) are left out
as `git rebase` does: `landed_dropped`.

A round (`state: conflict`, `session`):
- `session.commit`: the local commit being replayed, its `subject`, `body` and `patch` (the commit as a patch: the
  intent to reapply). `hint` \"likely landed\": the baseline has it already, take `onto` for each file.
- `session.files[]`: `path`, `kind`, and absolute paths: `edit` (the merge result with zdiff3 markers, the file to
  edit), `before` (the commit's parent), `onto` (the new parent: the baseline and the commits replayed so far),
  `commit` (the commit's version), `markers` (blocks left), `upstream_commits` (who changed it upstream), `hint`.
  `session.marker_labels` names the ids after the markers. `session.blocked_paths`: local changes that will block the
  finish (the first 100, `blocked_paths_omitted` counts the rest); ask the user about them first. `session.next` is
  the command that continues. A round prints only its own files; `bd update --status` adds the rounds resolved so far.
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
or applies the update (exit 0). --take and --delete resolve a file without an edit (repeatable).
`bd update --continue --dry-run` checks the resolution; it moves no ref and writes nothing but git objects. Exit 2:
`unresolved_markers`, `resolution_outside_conflict` (a line outside the blocks changed) or `missing_resolution`, with
`paths` and the copies in the message: fix those copies and run it again. A refusal changes nothing: the round stays
open and the copies keep your edits (`edited` in the round). `round_moved` (exit 2):
another round is open; `bd update` prints it. `empty_resolution` (exit 2): the resolution leaves the commit without
changes; ask the user, then add --allow-empty (keep an empty commit) or --drop-empty <commit> (drop it).
At exit 0, `resolutions[]` lists per round the `commit`, its `new_id` (`empty`: kept or dropped) and per file
`how` (edited, onto, commit, deleted) with `dropped_upstream_lines` and `dropped_commit_lines`: the lines of a side's
change that the result lost. Report each resolution to the user, these lines first. Then build and test: on a failure
commit the fix as `fixup! <subject>` and run `bd fold`, or take the update back with `bd undo <undo_token>`.

`uncommitted_overlap` at the finish (exit 4) keeps the session: once the user moved those files, run
`bd update --continue` again. `finish_pending` (exit 4): the branch moved, but its files did not follow; the message
says what is left.
`session_stale` (exit 4): the branch was rewritten or HEAD left it; `unsupported_conflict` (exit 4): a binary,
file/directory, rename, mode or symlink conflict. Both: stop and show the user. `bd update --abort` removes the session
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
checkout. One owner rewrites that branch: `bd update`, `bd fold`, `bd group`, `bd rename`. Every other session only
adds commits to it, with `bd pick`, never with `git cherry-pick`, a rebase or a reset in the shared checkout.

1. Write the prefix when you commit, never later. A subject is `(branch) <one sentence>`, with stacked branches
   (`branchdeck.stacks`) `(parent/leaf) <one sentence>`. The branch is a readable name of a few words, never an issue
   id as the name; an issue id stays in the subject after the prefix, or in the body. A series of commits is a stack
   root with one leaf per logical group; a change on its own gets a root of its own. One deliberate commit per change:
   no `fixup!` subject on the shared branch, `bd fold` would fold it into its target. Rename a branch with
   `bd rename <old> <new>`, which rewrites every prefix, children included. `bd suggest` is for commits that came
   without a prefix.
2. Land your commits, with -C <shared checkout>: `bd pick --from <your worktree> --follow`. bd replays them onto the
   branch in memory, under the repository lock that every worktree shares, so no fold or update runs halfway through
   it; the files follow, and --follow moves your worktree onto the new tip (its local changes on other paths stay).
   - `picked[]`: the copies, each with the `group` it lands in; `grouped_by: none` is a commit without a prefix: fix
     the subject before the owner groups it.
   - `already_present[]`: commits whose change the branch has already (`on`: its commit with the same patch); never an
     empty commit. `state: up_to_date`: nothing was left to pick.
   - `state: conflict` (exit 1): nothing changed. Do what `conflict.next` says: move your worktree onto the branch,
     make the commit again there, regenerate a generated file, never keep one side of a code hunk; then pick again.
   - `uncommitted_overlap` (exit 4): local files of the named worktree sit on paths the pick writes; ask the user.
     `range_unknown` or `source_on_newer_baseline` (exit 4): name the commits (`bd pick <first>^..<last>`), or let the
     owner update the branch first.
   - `followed.state` `head_moved` or `blocked`: the branch moved, your worktree did not; the same command again moves
     only the worktree. A build daemon or a file watcher of the worktree may need a restart after it moved.
   - `bd undo <undo_token>` moves the branch and your worktree back.
3. Check that the branch still holds your work after the owner rewrote it: `bd pick --from <your worktree> --dry-run`
   (`up_to_date`, every commit `already_present`). Never compare by date (a `git log` limited by time): a rewrite can
   change the dates, and a fold changes the ids; compare subjects without the `(prefix) ` only as a hint, the content
   decides.
4. The owner's rewrites take the same lock and refuse while a cherry-pick, rebase, merge or revert is in progress in
   the shared checkout (`operation_in_progress`). After one, the owner tells the other sessions its new HEAD
   (`new_head`); they pick again or check with --dry-run. Nobody squashes, amends or redates
   another session's commit.
5. The reflog of the branch tells who moved it: `branch-deck: pick`, `undo pick`, `update project`, `undo update`,
   `squash fixups` (`bd fold` or the app's Squash Fixups), `reword <n> commits` (`bd group`, `bd rename`), `undo`,
   `squash <n> commits`, `drop <n> commits`, `amend <id>`, `move hunk`. Any other line came from outside Branch Deck.
";

/// The skill `bd install` writes and `bd help skill` prints (`SKILL.md`, the Agent Skills format of Claude Code, Codex
/// and others), naming bd by `command`, its full path. It only says when to use bd and where the workflow is, so it
/// can't fall out of date with the binary: `bd help agent` is the workflow. `allowed-tools` (Claude Code) lets the agent
/// run every bd command without a permission prompt while the skill is active, also the ones that rewrite the branch or
/// push: a push still waits for the user, by the rule of the body
pub fn skill_text(command: &str) -> String {
  let allowed_tools = yaml_quoted(&format!("Bash({command}:*)"));
  format!(
    "\
---
name: branch-deck
description: Group the commits of a Git branch into virtual branches and pull requests, stacked when they build on \
each other, with the `bd` command line of Branch Deck, update onto the baseline and resolve conflicts, and land commits \
from one worktree onto the branch of another. Use when the user asks to split, group, cluster \
or reorganize commits into branches or pull requests, to stack branches, to sync or push virtual branches, to start a \
Patronus Safe Push or Dry Run of a branch or to fix one that failed, to update a branch onto its baseline or resolve its update conflicts, \
to move, pick or land commits of a worktree or lease onto a shared branch, or mentions Branch Deck.
allowed-tools: {allowed_tools}
---

# Branch Deck (`bd`)

bd is `{command}`; run it as written, it is quoted for the shell. Below, `bd` stands for it.

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the `(name)` prefix
of their subjects. bd reads those groups and the commits, regroups the commits by rewriting the prefixes, rebuilds the
virtual branches, pushes them and starts their Patronus Safe Push. Only `bd update` (the branch onto its baseline),
`bd pick` (the commits of another worktree onto the branch) and their undo write the working tree, through the
guarded update.

Before anything else, run `bd help agent` and follow the workflow it prints: it is the contract of the installed
version. To update a branch or resolve its update conflicts, run `bd help update` instead. When several sessions or
worktrees work on one branch, run `bd help worktrees` first.

- Run bd in the repository, or pass `-C <repo>`.
- Write the `(branch)` prefix of a commit's subject when you commit, a readable name, never an issue id as the name.
- Every command prints one JSON object on stdout. Branch on `error.code` and the exit code, never on the message;
  `bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.
- Never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`; move a
  change between commits only with `bd move-hunk`; never push, start a Safe Push or create pull requests before the
  user approved them.
- Before a Safe Push, run `bd safe-push <branch> --check` first and show the user what it lands: the Safe Push of a
  stacked branch lands the branches it is built on with it, in one robot, and `--whole-stack` lands the whole stack,
  its side branches included, in one robot.
"
  )
}

/// A YAML double-quoted scalar: the shell quoting of a path can hold ` #` or `: `, which end a plain scalar
fn yaml_quoted(value: &str) -> String {
  format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
