/// `bd help agent`: the workflow of an agent that groups commits into virtual branches (docs/guidelines/agent-cli.md)
pub const AGENT_HELP: &str = "\
bd: Branch Deck for agents. JSON on stdout (every object has \"schema\": 1), logs on stderr (-v for debug).

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the
`(name)` prefix of their subjects, or by an issue key (`ABC-123 Fix ...`). A sync rebuilds each group as
`{prefix}/virtual/{name}` on the baseline; the working tree is never touched.

Global options: -C <repo>, --prefix <p> (else the app setting, else `git config branchdeck.branchPrefix`),
--full-ids (ids are abbreviated by default), -v.

Errors: {\"schema\":1,\"error\":{\"code\",\"message\",\"commits\",\"retryable\"}}.
Exit codes: 0 ok, 1 conflict, 2 invalid input, 3 stale (read again and retry), 4 refused, 5 git or network error.
`bd schema` prints the JSON Schema of every output.

Workflow:
1. `bd status` for the branches and the unassigned commits (with stacked branches on, also each
   branch's `parent`, `parent_state`, `problems` and the `forest` order), then `bd commits --files`, and
   `bd commits --patch --branch <b>` for the clusters that are unclear.
2. Cluster the commits by intent. `dependencies` ({from, to, files}: `from` touches lines `to` added) and
   `cluster_overlap` (files two branches both change) show which commits can't be separated: a commit
   goes with, or stacks on, the commits it depends on.
3. Write the plan: {\"branches\": {\"<path>\": [ids]}, \"unassign\": [ids]}. A path is a branch name, or
   `parent/leaf` for a stacked branch (`git config branchdeck.stacks true`). Fixups follow their targets.
   `bd group --dry-run < plan.json` prints the resulting subjects and the stack forest and writes nothing;
   fix the plan until it is accepted. Then `bd check < plan.json`: a trial sync that writes no refs. Per
   branch `ok`, `conflict` {commit, subject, files, missing_commits}, `blocked_by` or `problems`, and the
   branch it is `built_on`. Exit 1 until every branch is ok: move the missing commits into the branch, or
   stack it on the branch that has them, and check again. Variants are fast: merges are cached.
4. `bd group --expect-head <head from bd status> < plan.json`. It rewrites commit messages only, in one
   rewrite, and prints the new groups and an `undo_token`. `head_moved` or `locked` (exit 3): read again
   and retry. Also: `bd rename <old> <new>`, `bd restack <branch> <parent>`, `bd unstack <branch>`, all
   with --dry-run and --expect-head; `bd undo <undo_token>` restores the previous HEAD.
   Then `bd sync`: it rebuilds the virtual branches and prints per branch created/updated/unchanged/
   error/merge_conflict (the commit, the files and the commits it likely needs). Exit 1 on a conflict:
   regroup so that the commits a change depends on are in the same branch or its parent, and sync again.
5. `bd describe <branch> --title ... --body ...` (or --body-file) for each branch: the pull request text,
   kept in `branch.<name>.description`. `bd describe <branch>` prints it; --clear removes it.
6. Show the result to the user. `bd push` only after the user approves. (not yet available)

Rules: never reorder, squash or drop commits; never touch the working tree; a plan only moves commits
between branches. Errors with exit 2 (invalid_plan, not_ahead_of_baseline) mean the plan must change.
";
