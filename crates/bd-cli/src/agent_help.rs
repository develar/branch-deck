/// What the reading commands write, for `bd --help` and `bd help agent`; a macro, so that `concat!` takes it
macro_rules! read_only {
  () => {
    "Read only: status, commits, suggest, check, robot, schema, version, the dry runs of update, fold and move-hunk,
and the session status of update write no ref, config, index or working tree; at most git objects (the trial sync of
check and suggest, the replay of update, the trees of move-hunk) and the object cache (none with --no-cache);
`bd check --build` runs its command in a temporary copy of each tree."
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

Global options: -C <repo>, --prefix <p> (else the app setting, else `git config branchdeck.branchPrefix`),
--full-ids (ids are abbreviated by default), --no-cache, -v.
Errors: {\"schema\":1,\"error\":{\"code\",\"message\",\"commits\",\"paths\",\"retryable\"}}. Exit codes: 0 ok, 1 conflict,
2 invalid input, 3 stale (read again and retry), 4 refused, 5 git or network error.
",
  read_only!(),
  "
`bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.

Workflow:
1. `bd suggest`: a plan, clustered and checked by a trial sync (exit 1: `check.ok` is false, edit the plan). Per
   cluster `change` (kept, renamed, merged, split, new) against the branches in `from`, `why` (one line to quote to
   the user), `weak` (review these first), `issue_keys` and `areas`; with stacked branches `parent` and `restacked`:
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
   missing_commits}, `dangling_reference` (git merges it, the build fails: `dangling_references` {identifier, file,
   line, removed_by, fixed_by, mutual}: the branch removes a declaration its tree still uses, and `fixed_by` of another
   branch changes that use), `blocked_by` or `problems`. Exit 1 until every branch is ok: move the missing commits (or
   `fixed_by`) into the branch, or stack it on the branch that has them; `mutual`: the change in `fixed_by` needs the
   branch too, so `fix` moves it into the branch: `move_commit` (its `branches` into the plan) or `move_hunk` (a ready
   `bd move-hunk` `command`, after the user approves), each proven by a trial build; no `fix`: none builds, ask
   `bd check --fix`. `inverted_stack` {branch, child, fixes, reason}: the branch needs a commit of a branch stacked on it; each
   fix (`move_to_child`, `swap`) is `branches`/`unstack` to add to the plan, checked by a trial build; `reason` when
   none builds. `bd check --fix` also finds the regroup that fixes them, by trial builds: each
   `fix.components[].plan` is a whole plan for `bd group`, and its `steps` say what moves where; show them to the user
   before applying one. `warnings` lists a `possible_dangling_reference` that no commit
   proves: tell the user. With `git config branchdeck.symbols on`, `missing_declarations` {branch, identifier, file,
   used_by, declared_by, declared_in} names a use whose declaration is outside the branch's tree: stack the branch on
   `declared_in`, or move one of the two. `bd check --build '<command>'` (--build-paths <pathspec>...) also runs a build in a
   temporary copy of each tree (`build_failed`); slow on a large repository.
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
   when they have commits for the remote (--alone does not), pushes parents first with a lease per ref, and prints a
   ready `gh pr create` per branch (pipe its `body` into it); name every branch of `branches` to the user.
   `push_refused` (exit 4): a branch didn't sync, is behind its remote branch (`remote.behind`; older versions of
   commits amended or restacked here, `remote.amended` and `remote.restacked`, push without --force), or, with
   --alone, its parent isn't on the remote. `robot_running` (exit 4): a Safe Push runs on the branch, or on a branch
   built on it that lands it: a push won't land, and it fails the Safe Push at the end; a branch taken along that a
   Safe Push lands stays as the robot took it. Wait until it merges, or, only after the user approves,
   `bd push <branch> --restart-running` cancels it, pushes and restarts it on the pushed commit (`restarted`).
   `lease_rejected` (exit 3): the remote changed,
   nothing was pushed, and bd fetched it. For a branch behind or rejected, `error.branches[].remote.dropped_commits`
   lists what a push drops: show it, and only after the user approves,
   `bd push <branch> --force --expect-remote <remote.head>`. `bd sync` prints the push each branch needs as `push`,
   ready to run as written once the user approved it.
5. A Patronus Safe Push (the repository on Space, the Safe Push CLI logged in once, else `not_logged_in`): first
   `bd safe-push <branch> --check`. It prints the merge `profile` (default: the one of the last robot of the
   branch), the `profiles` to choose from, the code-owner `review` (`readiness`: `not_required`, `approved`,
   `missing` or `unknown`; `groups` with their `reviewers` and `paths`, `waiting_for`, `not_counted`, `merge_request`),
   `push_needed` and the `refusals` of a start. Patronus judges the approvals of the pull request: a bot's approval
   counts for nothing (`not_counted`). Show them to the user; only after the
   user approves, `bd safe-push <branch>` (--profile <id|name|none>; --dry-run runs the checks without the push into
   the target and checks no review). It pushes the branch first when `push_needed` (by content: a branch only rebuilt
   on a newer baseline is not pushed), as `bd push` does, then starts the robot:
   `robot.url` is its page; a branch with an open pull request into the target starts from a `/patronus` comment
   there, where its approvals count (`started_via`). `approvals_missing` (exit 4): nothing was pushed or started,
   `error.review` names who has to approve which files: show it. The approvals count in the GitHub pull request of
   the branch: `bd safe-push <branch> --create-pull-request` pushes the branch and opens it, assigned to the user
   (--no-assign leaves it unassigned); `--reviewer <login>` (repeatable) asks a reviewer: the `github_login` of the
   suggested `review.groups[].reviewers`, once the user chose them. `pull_request.warnings` says whom GitHub refused.
   `review.waiting_for` are the code owners asked who haven't approved yet. Once they approved it there
   (`review.approved_on_github` names the approvals that count), run it again.
   --without-approvals starts anyway, only when the user says so: Patronus fails it without them. `robot_running`:
   a robot runs on the last commit, or lands the branch with a branch of its stack; a Safe Push of an older commit
   is canceled with --cancel-running, after the user approves. `stack_not_landed`: Safe Push the root first, or
   --land-stack lands the parents with the branch. Its message ends with a trial build of the branch alone: it builds
   alone (`bd unstack <branch>`, after the user approves), or it needs commits of its parents (move them first).
   `dangling_reference`: the branch or a parent removes a declaration that a commit of another branch still changes
   the use of, as check reports it: fix it, or `bd safe-push <branch> --allow-dangling` when the user says it is a
   false alarm.
   A pull request that exists but asks no code owner of a missing group: `bd request-review <branch> --check` names
   whom it would ask (`reviewers`: one suggested code owner of each such group); only after the user approves,
   `bd request-review <branch>` asks them on GitHub (--reviewer <login>, repeatable, asks these instead and doesn't
   ask Patronus): `requested`, `refused` with GitHub's reason, `waiting_for`. `no_pull_request` (exit 4): none is
   open, or a draft; `no_reviewer`: Patronus names no GitHub login, ask the user whom; `review_refused`: GitHub
   asked nobody.
   `bd robot <branch>` prints the robot of the branch (`robot.status`, `running`: run it again later; --robot <id> for
   another one). After a failed Safe Push, `bd robot <branch> --full` says why (`failure`: `kind`, the failed
   `checks` with their build ids, `problems`, and the failed `builds` with their problems, failed tests and log
   tails) and which commits it tested (`commits`, by the ids of the checked-out branch: `tested`, `rebased`,
   `changed`, `added`, `removed`, `landed`). `kind: infrastructure`: nothing to fix, the user restarts it. Fix and
   check locally; the user pushes and restarts the Safe Push.

`bd update` (--no-fetch, --dry-run) brings the checked out branch onto its baseline as the app's Update Project;
`uncommitted_overlap` (exit 4) lists the `paths` that block it (never stash them: ask the user). A conflict starts a
resolve session (exit 1): `bd help update` explains it. `bd undo update:<id>..<id>` takes an update back
(`undo_stale`, exit 4, once the branch moved on). `bd fold` (--branch, --expect-head, --dry-run) folds fixup! commits.
A change that landed in the wrong commit: `bd move-hunk <from> <to> <path>` (--hunk <n>, repeatable; --expect-head,
--dry-run, --sync) moves the changes of the file `path`, or only its hunks numbered as in the patch of `from`, into the
commit `to`, older or newer; HEAD's tree, the index and the working tree stay, the commits between keep their diff.
`bd commits <from> --patch` shows the hunks. `hunk_conflict` (exit 1): a commit between changes the same lines, or
lacks the file; `hunk_not_found` (exit 2). It prints an `undo_token`; the virtual branches change with the next sync.

Rules: never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`; move a
change between commits only with `bd move-hunk`, never with plumbing; only `bd update` and its undo write the working
tree, through the guarded update; a plan only moves commits between branches. Exit 2 (invalid_plan,
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
At exit 0, `resolutions[]` lists per round the `commit`, the `resolved` commit (`empty`: kept or dropped) and per file
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

/// The skill `bd install` writes and `bd help skill` prints (`SKILL.md`, the Agent Skills format of Claude Code, Codex
/// and others), naming bd by `command`, its full path. It only says when to use bd and where the workflow is, so it
/// can't fall out of date with the binary: `bd help agent` is the workflow
pub fn skill_text(command: &str) -> String {
  format!(
    "\
---
name: branch-deck
description: Group the commits of a Git branch into virtual branches and stacked pull requests with the `bd` command \
line of Branch Deck, update onto the baseline and resolve conflicts. Use when the user asks to split, group, cluster \
or reorganize commits into branches or pull requests, to stack branches, to sync or push virtual branches, to start a \
Patronus Safe Push or Dry Run of a branch or to fix one that failed, to update a branch onto its baseline or resolve its update conflicts, or \
mentions Branch Deck.
---

# Branch Deck (`bd`)

bd is `{command}`; run it as written, it is quoted for the shell. Below, `bd` stands for it.

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the `(name)` prefix
of their subjects. bd reads those groups and the commits, regroups the commits by rewriting the prefixes, rebuilds the
virtual branches, pushes them and starts their Patronus Safe Push. Only `bd update` (the branch onto its baseline)
and its undo write the working tree, through the guarded update.

Before anything else, run `bd help agent` and follow the workflow it prints: it is the contract of the installed
version. To update a branch or resolve its update conflicts, run `bd help update` instead.

- Run bd in the repository, or pass `-C <repo>`.
- Every command prints one JSON object on stdout. Branch on `error.code` and the exit code, never on the message;
  `bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.
- Never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`; move a
  change between commits only with `bd move-hunk`; never push, start a Safe Push or create pull requests before the
  user approved them.
"
  )
}
