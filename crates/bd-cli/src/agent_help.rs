/// What the reading commands write, for `bd --help` and `bd help agent`; a macro, so that `concat!` takes it
macro_rules! read_only {
  () => {
    "Read only: status, commits, suggest, check, schema, version, the dry runs of update and fold, and the session status
of update write no ref, config, index or working tree; at most git objects (the trial sync of check and suggest, the
replay of update) and the object cache (none with --no-cache)."
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
   it stacks each branch on the branches it needs. Whole issue branches that belong together go into `issue_groups`
   of the plan: no subject changes. `bd suggest --fresh` ignores the current prefixes and issue groups. To look closer: `bd status --brief`, and `bd commits --brief` (per commit its group, issue key, fixup
   target and `areas`; `units`, the commits that must stay together; `dependencies` {from, to}: `from` touches lines
   `to` added). `bd commits <id>... --patch` or `bd commits --unassigned` for unclear commits, `bd status --stats`
   for the size of each branch.
2. The plan (in Windows PowerShell write it to a file and pass the file: a pipe of PowerShell 5.1 turns non-ASCII
   into `?`): {\"branches\": {\"<path>\": [ids]}, \"unassign\": [ids], \"issue_groups\": {\"<name>\": [keys]},
   \"ungroup_issues\": [keys], \"descriptions\": {\"<branch>\": {\"title\", \"body\"}}}. A path is a branch name, or
   `parent/leaf` for a stacked branch (`git config branchdeck.stacks true`). Fixups follow their targets; a commit
   goes with, or stacks on, the commits it depends on. After any change of `branches`, `unassign`, `issue_groups` or
   `ungroup_issues`: `bd check --plan plan.json`. Per branch `ok`, `conflict` {commit, subject, files,
   missing_commits}, `blocked_by` or `problems`. Exit 1 until every branch is ok: move the missing commits into the
   branch, or stack it on the branch that has them.
3. Add `descriptions` (pull request title and body) for the branches that have none (`description` in
   `bd status`); replace one only when the user asks. `<branch>` is the leaf for a stacked branch. Then
   `bd group --expect-head <head of suggest> --sync --plan plan.json`: one message-only rewrite, the issue groups,
   the descriptions and a sync, under one repository lock. It prints the new `groups`, an `undo_token`,
   `replaced_descriptions` and `sync` (per branch `status`; a `conflict` names the commit, the files and the commits
   it needs). Exit 1 on a conflict: regroup so that a change and what it needs are in one branch or its parent, and
   `bd sync` again. Another exit with `error` and an `undo_token`: the rewrite stands (a failed description write
   skips the sync); `bd sync` again, or undo. `head_moved`, `groups_moved` or `locked` (exit 3): read again and
   retry. `bd group --dry-run` prints the changed subjects and writes nothing. Also `bd rename <old> <new>`, `bd restack <branch> <parent>` and `bd unstack <branch>`, with the
   same options; `bd describe <branch> --title ... --body ...` for one branch. `bd undo <undo_token>` moves HEAD and
   the issue groups back (pass the token as printed, with full ids: `<head>..<head>`, `groups:<id>..<id>`, or both
   joined by `+`); the descriptions stay, and the virtual branches change only with the next `bd sync`.
4. Show the result to the user. Only after the user approves: `bd push <branch>...` (or --all --yes; add
   --with-children for the branches stacked on them): it syncs, takes the branches a stacked branch is built on along
   when they have commits for the remote (--alone does not), pushes parents first with a lease per ref, and prints a
   ready `gh pr create` per branch (pipe its `body` into it); name every branch of `branches` to the user.
   `push_refused` (exit 4): a branch didn't sync, is behind its remote branch, or, with --alone, its parent isn't on
   the remote. `lease_rejected` (exit 3): the remote changed,
   nothing was pushed, and bd fetched it. For a branch behind or rejected, `error.branches[].remote.dropped_commits`
   lists what a push drops: show it, and only after the user approves,
   `bd push <branch> --force --expect-remote <remote.head>`.

`bd update` (--no-fetch, --dry-run) brings the checked out branch onto its baseline as the app's Update Project;
`uncommitted_overlap` (exit 4) lists the `paths` that block it (never stash them: ask the user). A conflict starts a
resolve session (exit 1): `bd help update` explains it. `bd undo update:<id>..<id>` takes an update back
(`undo_stale`, exit 4, once the branch moved on). `bd fold` (--branch, --expect-head, --dry-run) folds fixup! commits.

Rules: never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`; only
`bd update` and its undo write the working tree, through the guarded update; a plan only moves commits between
branches. Exit 2 (invalid_plan, not_ahead_of_baseline): change the plan.

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
  finish; ask the user about them first. `session.next` is the command that continues.
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
or reorganize commits into branches or pull requests, to stack branches, to sync or push virtual branches, to update \
a branch onto its baseline or resolve its update conflicts, or mentions Branch Deck.
---

# Branch Deck (`bd`)

bd is `{command}`; run it as written, it is quoted for the shell. Below, `bd` stands for it.

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the `(name)` prefix
of their subjects. bd reads those groups and the commits, regroups the commits by rewriting the prefixes, rebuilds the
virtual branches and pushes them. Only `bd update` (the branch onto its baseline) and its undo write the working tree,
through the guarded update.

Before anything else, run `bd help agent` and follow the workflow it prints: it is the contract of the installed
version. To update a branch or resolve its update conflicts, run `bd help update` instead.

- Run bd in the repository, or pass `-C <repo>`.
- Every command prints one JSON object on stdout. Branch on `error.code` and the exit code, never on the message;
  `bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.
- Never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`, and never
  push or create pull requests before the user approved them.
"
  )
}
