/// What the reading commands write, for `bd --help` and `bd help agent`; a macro, so that `concat!` takes it
macro_rules! read_only {
  () => {
    "Read only: status, commits, suggest, check, schema and version write no ref, config, index or working tree; at
most git objects (the trial sync of check and suggest) and the object cache (none with --no-cache)."
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
Errors: {\"schema\":1,\"error\":{\"code\",\"message\",\"commits\",\"retryable\"}}. Exit codes: 0 ok, 1 conflict,
2 invalid input, 3 stale (read again and retry), 4 refused, 5 git or network error.
",
  read_only!(),
  "
`bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.

Workflow:
1. `bd suggest`: a plan, clustered and checked by a trial sync. Per cluster `change` (kept, renamed, merged, split,
   new) against the branches in `from`, `why` (one line to quote to the user), `weak` (review these first),
   `issue_keys` and `areas`. Whole issue branches that belong together go into `issue_groups` of the plan: no subject
   changes. --fresh ignores the current prefixes and issue groups. To look closer: `bd status --brief`, and
   `bd commits --brief` (per commit its group, issue key, fixup target and `areas`; `units`, the commits that must
   stay together; `dependencies` {from, to}: `from` touches lines `to` added). `bd commits <id>... --patch` or
   `bd commits --unassigned` for unclear commits, `bd status --stats` for the size of each branch.
2. The plan: {\"branches\": {\"<path>\": [ids]}, \"unassign\": [ids], \"issue_groups\": {\"<name>\": [keys]},
   \"ungroup_issues\": [keys], \"descriptions\": {\"<branch>\": {\"title\", \"body\"}}}. A path is a branch name, or
   `parent/leaf` for a stacked branch (`git config branchdeck.stacks true`). Fixups follow their targets; a commit
   goes with, or stacks on, the commits it depends on. Only after you move commits: `bd check --plan plan.json`.
   Per branch `ok`, `conflict` {commit, subject, files, missing_commits}, `blocked_by` or `problems`. Exit 1 until
   every branch is ok: move the missing commits into the branch, or stack it on the branch that has them. Merges are
   cached: variants are fast.
3. Add the pull request title and body of each branch to `descriptions`, then
   `bd group --expect-head <head of suggest> --sync --plan plan.json`: one message-only rewrite, the issue groups,
   the descriptions and a sync, under one repository lock. It prints the new `groups`, an `undo_token` and `sync`:
   per branch created, updated, unchanged, blocked, error or merge_conflict (the commit, the files and the commits
   it likely needs). Exit 1 on a conflict: regroup so that a change and what it needs are in one branch or its
   parent, and `bd sync` again. `head_moved`, `groups_moved` or `locked` (exit 3): read again and retry. --dry-run
   prints the changed subjects and writes nothing. Also `bd rename <old> <new>`, `bd restack <branch> <parent>` and
   `bd unstack <branch>`, with the same options; `bd describe <branch> --title ... --body ...` for one branch.
   `bd undo <undo_token>` moves HEAD and the issue groups back; pass the token as printed, with full ids
   (`<head>..<head>`, `groups:<id>..<id>`, or both joined by `+`).
4. Show the result to the user. Only after the user approves: `bd push <branch>...` (or --all --yes): it syncs,
   pushes parents first with a lease per ref, and prints a ready `gh pr create` per branch (pipe its `body` into
   it). `push_refused` (exit 4): a branch didn't sync, is behind its remote branch, or its parent isn't on the
   remote. `lease_rejected` (exit 3): the remote changed, nothing was pushed, and bd fetched it. For a branch behind
   or rejected, `error.branches[].remote.dropped_commits` lists what a push drops: show it, and only after the user
   approves, `bd push <branch> --force --expect-remote <remote.head>`.

Rules: never reorder, squash or drop commits; never touch the working tree; a plan only moves commits between
branches. Exit 2 (invalid_plan, not_ahead_of_baseline): change the plan.

`bd install` puts `bd` into ~/.local/bin (macOS, Linux) and a skill for coding agents into ~/.claude/skills and
~/.agents/skills; `bd help skill` prints that skill.
"
);

/// The skill `bd install` writes and `bd help skill` prints (`SKILL.md`, the Agent Skills format of Claude Code, Codex
/// and others), naming bd by `command`, its full path. It only says when to use bd and where the workflow is, so it
/// can't fall out of date with the binary: `bd help agent` is the workflow
pub fn skill_text(command: &str) -> String {
  format!(
    "\
---
name: branch-deck
description: Group the commits of a Git branch into virtual branches and stacked pull requests with the `bd` command \
line of Branch Deck. Use when the user asks to split, group, cluster or reorganize commits into branches or pull \
requests, to stack branches, to sync or push virtual branches, or mentions Branch Deck.
---

# Branch Deck (`bd`)

bd is `{command}` (quote the path in a shell); below, `bd` stands for it.

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the `(name)` prefix
of their subjects. bd reads those groups and the commits, regroups the commits by rewriting the prefixes, rebuilds the
virtual branches and pushes them. It never touches the working tree.

Before anything else, run `bd help agent` and follow the workflow it prints: it is the contract of the installed
version.

- Run bd in the repository, or pass `-C <repo>`.
- Every command prints one JSON object on stdout. Branch on `error.code` and the exit code, never on the message;
  `bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.
- Never reorder, squash or drop commits, and never push or create pull requests before the user approved them.
"
  )
}
