---
name: bd workspace
description: Requirements for bd workspace, which a coding agent runs before its first edit, and the two rules it reads, branchdeck.agentLease (work in a lease of the pool and land each commit with bd lease pick, on by default) and branchdeck.agentSubjectBranch (start each commit subject with its branch, off by default), which the user changes in Settings → Agents.
style: plain-1
targets:
  - ../../crates/bd-lease/src/workspace_command.rs
  - ../../crates/lease-pool/src/agent_workspace.rs
  - ../../crates/sync-core/src/repository_config.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/bd-cli/src/install.rs
  - ../../crates/bd-cli/src/schema.rs
---

# bd workspace

Status: Implemented
Date: 2026-10-21

## Summary

Where the skill of Branch Deck is installed ([Agent CLI](agent-cli.spec.md), `bd install`), a coding agent runs
`bd workspace` before its first edit or commit in a repository, and follows it. Two rules of the git config decide it:
`branchdeck.agentLease`, on by default, makes the agent lease a worktree of the pool ([bd lease](lease.spec.md)),
commit there and land each commit at once with `bd pick` ([bd pick](pick.spec.md)), then return the lease;
`branchdeck.agentSubjectBranch`, off by default, makes it start each commit subject with its `(branch)`. The lease is
on everywhere, since isolation harms no repository; the subject branch is off until the user turns it on, because a
`(branch)` prefix reaches the remote as it is wherever Branch Deck does not build the branches (only the virtual
branch copies drop it). The global config gives every repository its value, the repository's own `.git/config` wins,
and the user changes both in Settings → Agents ([Settings](../repository/settings.spec.md#agents)). What the user says
in the session wins over both.

## Goals

- One read-only command answers where to work and how to write a subject, with the next command to run.
- No surprise: a rule changes only by an explicit value; nothing is detected.
- An agent that runs where a lease makes no sense (a worktree of its own, no working tree, a sandbox that can't write
  the pool) is told so, with the reason, instead of failing at `bd lease get`.

## Non-goals

- A hook of Claude Code or Codex: the skill's description makes the agent load the skill, which is no guarantee.
- A rule per agent or per session: the user says it in the session.

## Requirements

- `bd workspace` (`-C <dir>`, else the current directory) must print `{"schema":2,"kind","lease","subject_branch",
  "stacks","top_level","subdir","shared_checkout","home","lease_id","holder","local_changes","active_leases","next"}`; each
  rule is `{on, source, invalid, reason}`, `source` the config scope that sets it (`local`, `worktree`, `command`,
  `global`, `system`) or `default`. By default the lease is on and the subject branch off.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`nothing_set_is_the_defaults`)
- In the main worktree (`kind: main`) with the lease on, `next` must be `bd lease get --holder <your name> -C
  <top level>`; `subdir` names where bd runs below the top level, and `local_changes` counts the local changes of the
  checkout, which a lease does not have. Inside a leased slot (`kind: lease`) `shared_checkout` must be the source the
  slot was leased from, in the form of `top_level` (backslashes on Windows), `home` the same as `bd lease get` prints
  it ([bd lease](lease.spec.md)), `lease_id` and `holder` its lease, and `next` the same as the get's: `bd -C <top
  level> lease pick` and the return of this lease. When the shared checkout has another branch checked out now than
  at the get, `home.switched_to` names it (`null`: a detached HEAD), so the agent asks the user before it picks. `active_leases` must list every
  ready lease of the pool, so a session that lost its path finds its own by `holder`.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_home_that_switched_its_branch`)
- A value of the global config must set a rule for every repository, a value of the repository's own config must win
  over it, and a value git does not read as a boolean must leave the default with `invalid`.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`the_global_config_turns_a_rule_off_and_a_repository_keeps_its_own`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_agent_keys_are_written_globally_and_a_repository_keeps_its_own`)
- A linked worktree outside the pool (`kind: linked`) must answer the lease off with the reason "A worktree of its own
  already", and `next` empty; outside a working tree (`kind: none`: a bare repository, a `.git` directory, no
  repository) the answer is no error, the lease off and `next` empty.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`a_worktree_of_its_own_needs_no_lease_and_no_working_tree_is_an_answer`)
- With the lease on in the main worktree, a pool directory (or its first existing parent) that can't be written, as in
  a sandbox, must turn the lease off with the reason; the probe file is removed at once.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`a_pool_that_cant_be_written_turns_the_lease_off`)
  [@test] ../../crates/lease-pool/src/agent_workspace_test.rs (`the_probe_of_the_pool_writes_its_first_existing_parent_and_removes_its_file`, `the_subdir_is_the_path_below_the_top_level`)
- It must write no ref, config, index or working tree, and run one `rev-parse`, one `git config` of the `branchdeck.*`
  keys, in the main worktree one more of the pool config, in the main worktree or a lease one `status` of the
  shared checkout (`GIT_OPTIONAL_LOCKS=0`), and in a lease one more `rev-parse` of the branch the shared checkout has
  checked out now.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_workspace`)
- The output must match `bd schema` (`workspace`).
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- The skill's description must start with the first edit ("Use before the first edit or commit in a Git repository:
  `bd workspace` …") and stay within the 1024 characters of the Agent Skills format; its body must say to run
  `bd workspace` first and follow it, the user's words winning: with the lease, work only in its `path` (in Claude Code
  `EnterWorktree` with that path), land each commit that builds and passes its tests at once with `bd lease pick`,
  never by a `git rebase` or `git merge` into the shared checkout and never with `bd update` in the lease, return only its own
  lease and ask the user when the task needs the `local_changes`; without it, stage only its own paths; with the
  subject branch, `(branch) ` first, a branch of `bd status --brief` reused where it fits, `(parent/leaf)` for a new
  stack; never change either rule.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_description_starts_with_the_first_edit_and_fits_the_limit`)
- The skill must carry the isolation rules a repository would otherwise repeat in its own agent instructions: a lease is
  the agent's only worktree, never a `git worktree add` or a clone on its own initiative, also not when a bd command
  fails (then it works where it is when that is safe, or asks, and names a lease that survived); in a sandbox (Codex) it
  asks for write access to exactly the lease's `path` (`request_permissions`) before the first edit there, and returns
  the untouched lease when that is refused. A repository's instructions then say only that it relies on the skill.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_bd_fails_and_in_a_sandbox`)
  [@test] ../../crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)
- `bd help worktrees` must lease the worktree only when `bd workspace` says `lease.on`, and write the prefix only when it
  says `subject_branch.on`.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_worktrees_help_writes_the_prefix_first_and_checks_by_content`)

## Testing / Local Run

- `cargo nextest run -p bd-lease workspace_test` runs the command end to end on `TestRepo` fixtures, the pool in a
  temporary home directory and the global git config of bd a file of the fixture (`Environment::git_config_global`,
  the `GIT_CONFIG_GLOBAL` of every git bd starts). Every other bd test reads an empty global config
  (`EMPTY_GLOBAL_CONFIG`), as CI has: the developer's `branchdeck.*` never reaches one.

## Open Questions / Risks

- The skill is loaded by its description, which an agent may skip for a small edit: then it works as it would without
  Branch Deck. A SessionStart hook would make it certain, but writes the user's agent settings.
- `local_changes` costs one `status` of the shared checkout, a few seconds in a monorepo without fsmonitor.

## References

- [Agent CLI guide](../../docs/guidelines/agent-cli.md): `bd workspace`.
- [Git Operations](../../docs/design/git-operations.md): "Repository settings", "Lease".
