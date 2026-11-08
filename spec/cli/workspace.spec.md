---
name: brd workspace
description: Requirements for brd workspace, which a coding agent runs before its first edit, and the three rules it reads, branchdeck.agentLease (work in a lease of the pool and land each commit with brd lease pick, on by default), branchdeck.agentSubjectBranch (start each commit subject with its branch, on by default) and branchdeck.agentPullRequest (push the branch after the pick and open its pull request without asking, off by default), which the user changes in Settings → Agents.
style: plain-1
targets:
  - ../../crates/bd-lease/src/workspace_command.rs
  - ../../crates/lease-pool/src/agent_workspace.rs
  - ../../crates/sync-core/src/repository_config.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/bd-cli/src/install.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/bd-lease/src/lease_command.rs
  - ../../crates/git-ops/src/lease_slot.rs
  - ../../crates/domain-types/src/landing_route.rs
---

# brd workspace

Status: Implemented
Date: 2026-10-21

## Summary

Where the skill of Branch Deck is installed ([Agent CLI](agent-cli.spec.md), `brd install`), a coding agent runs
`brd workspace` before its first edit or commit in a repository, and follows it. Three rules of the git config decide it:
`branchdeck.agentLease`, on by default, makes the agent lease a worktree of the pool ([brd lease](lease.spec.md)),
commit there and land each commit at once with `brd pick` ([brd pick](pick.spec.md)), then return the lease;
`branchdeck.agentSubjectBranch`, on by default, makes it start each commit subject with its `(branch)`. The lease is
on everywhere, since isolation harms no repository; the subject branch is on everywhere too, since the prefix is what
groups an agent's commit into its branch and the copies of the virtual branches drop it. A `(branch)` prefix reaches
the remote as it is only where the commits land themselves, not their virtual branches: such a repository turns the
rule off, or names its own form of a subject in its agent instructions, which win. With the rule on or off, the subject
after the prefix has the form of the repository (its agent instructions, else its `git log`), which `[commit] check`
can enforce at the pick ([brd pick](pick.spec.md#the-commit-rules)). `branchdeck.agentPullRequest`, off by default,
decides how far the landing of an agent's commit goes: off, it stops at the pick on the local branch and the push
waits for the user; on, the agent goes on to push the branch and open its pull request, without asking. It is off
everywhere by default, since a push publishes; a user who lets agents open their pull requests turns it on, for a
repository or for all, instead of writing it into the agent instructions of a repository, where it would contradict
the skill. Nothing that lands into master (a Safe Push, `brd land`) follows from it. The global config gives every repository its value, the repository's own `.git/config` wins,
and the user changes both in Settings → Agents, the first on its tab "All repositories", the second on the tab of the
repository ([Settings](../repository/settings.spec.md#agents)). What the user says
in the session wins over both.

## Goals

- One read-only command answers where to work, how to write a subject and how far a landing goes, with the next
  command to run.
- No surprise: a rule changes only by an explicit value; nothing is detected.
- An agent that runs where a lease makes no sense (a worktree of its own, no working tree, a sandbox that can't write
  the pool) is told so, with the reason, instead of failing at `brd lease get`.

## Non-goals

- A hook of Claude Code or Codex: the skill's description makes the agent load the skill, which is no guarantee.
- A rule per agent or per session: the user says it in the session.

## Requirements

- `brd workspace` (`-C <dir>`, else the current directory) must print `{"schema":2,"kind","lease","subject_branch",
  "top_level","subdir","shared_checkout","home","lease_id","holder","local_changes","active_leases","next"}`; each rule
  is `{on, source, invalid, reason}`, `source` the config scope that sets it (`local`, `worktree`, `command`, `global`,
  `system`) or `default`. By default `lease` and `subject_branch` are on, `pull_request` off.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`nothing_set_is_the_defaults`)
- With `pull_request.on` the `next` of a lease, in `brd workspace` and of `brd lease get` alike, must hold
  `brd -C <shared checkout> push <branch>` between the pick and the return; `brd lease get` reads the key in its one
  read of the pool config.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`the_pull_request_rule_pushes_the_branch_after_the_pick`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_agent_keys_are_written_globally_and_a_repository_keeps_its_own`)
- The pull request must follow the route into master ([Landing Routes](../branch/landing-routes.spec.md#setting)): under
  the Push route of the repository's own config (`.git/config`, the worktree config, `git -c`) the work lands without a
  pull request, so `pull_request` set on must answer `on: false` with the reason "The route into master is Push
  (branchdeck.landing): the work lands without a pull request", and neither `next` (of `brd workspace` and of
  `brd lease get`) holds the push. A `push` of the global or the system config is unset and changes nothing: both still
  hold it. One function of `domain-types` decides it (`LandingSetting::agent_pull_request_off`); `brd lease get` reads
  the route with its scope in its one read of the pool config, and Settings → Agents reads the same reason
  (`RepositoryConfig.agentPullRequestOff`).
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`the_push_route_turns_the_pull_request_rule_off_and_a_global_push_route_does_not`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_push_route_of_the_repository_turns_the_agent_pull_request_off`)
  [@test] ../../crates/domain-types/src/landing_route_test.rs (`only_the_push_route_turns_the_agent_pull_request_off`)
- In the main worktree (`kind: main`) with the lease on, `next` must be `brd lease get --holder <your name> -C
  <top level>`; `subdir` names where brd runs below the top level, and `local_changes` counts the local changes of the
  checkout, which a lease does not have. Inside a leased slot (`kind: lease`) `shared_checkout` must be the source the
  slot was leased from, in the form of `top_level` (backslashes on Windows), `home` the same as `brd lease get` prints
  it ([brd lease](lease.spec.md)), `lease_id` and `holder` its lease, and `next` the same as the get's: `brd -C <top
  level> lease pick` and the return of this lease. When the shared checkout has another branch checked out now than
  at the get, `home.switched_to` names it (`null`: a detached HEAD), so the agent asks the user before it picks. `active_leases` must list every
  ready lease of the pool, so a session that lost its path finds its own by `holder`, with `holder_alive` (omitted
  without a scan of the processes, on Windows, where the get records no holder process) and, for one whose holder is
  gone, `idle` as `brd lease status` shows it (`idle_with_work` with its work: tell the user). In a slot
  whose lease a get or `brd lease reclaim` took over, `brd workspace` of another session than the one that holds it now
  is `lease_reclaimed` (exit 4), so a resumed session of the old holder learns it before its first edit.
  [@test] ../../crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`a_command_of_another_session_in_a_taken_over_slot_is_refused_as_lease_reclaimed`)
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
- The output must match `brd schema` (`workspace`).
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- The skill's description must start with the first edit ("Use before the first edit or commit in a Git repository:
  `brd workspace` …") and stay within the 1024 characters of the Agent Skills format; its body must say to run
  `brd workspace` first and follow it, the user's words winning: with the lease, work only in its `path`, land each
  commit that builds and passes its tests at once with `brd lease pick`, never by a `git rebase` or `git merge` into
  the shared checkout and never with `brd update` in the lease, return only its own lease and ask the user when the
  task needs the `local_changes`; its own files ignored by git and kept by its harness in the shared checkout (a plan,
  notes) are not `local_changes` and may be updated there at their original paths, never copied into the lease or
  committed; without it, stage only its own paths; with the
  subject branch, `(branch) ` first, a branch of `brd status --brief` reused where it fits, a new branch `(name)`,
  `(parent/leaf)` only on an existing virtual branch, never the baseline; with `pull_request.on`, the push of `next`
  and the `gh pr create` it prints without asking, off ask first, off with a `reason` (the Push route) neither a push
  nor a pull request, a Safe Push, `brd land`, a push with --force and a
  parked branch waiting for the user either way; never change any of the three rules.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_description_starts_with_the_first_edit_and_fits_the_limit`, `the_skill_says_that_the_repository_form_of_a_subject_follows_the_prefix`, `the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)
- The skill must carry the isolation rules a repository would otherwise repeat in its own agent instructions: a lease is
  the agent's only worktree, never a `git worktree add` or a clone on its own initiative, also not when a brd command
  fails (then it works where it is when that is safe, or asks, and names a lease that survived). The rule of the lease
  must name no agent: it is the same for every agent, and a note for each kind of agent follows it. In Claude Code
  the agent does a `cd` into the lease's `path` first, which stays where the pool is in
  `permissions.additionalDirectories`, else `EnterWorktree` with that path, which asks the user. In a sandbox (Codex)
  it asks for write access to exactly the lease's `path` (`request_permissions`) before the first edit there, and
  returns the untouched lease when that is refused. When its shell does not keep the current directory (Junie, for
  example), it gives each file tool an absolute path in `path` and starts each shell command with `cd <path> && `; the
  Junie note says its plan is `.junie/plans/<name>.md` of the shared checkout and that it updates statuses there, by
  that absolute path, not in `path`.
  The skill carries the rules of the pool and of a pushed branch too: never `git stash` in a lease (every worktree of
  the pool shares one stash); a change to a pushed branch (an open pull request) is a `(branch) fixup! <subject>`
  commit, folded only when the user asks, and a promise of the description it breaks is fixed with `gh pr edit`.
  A repository's instructions then say only that it relies on the skill.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agents_plan_in_the_shared_checkout`)
  [@test] ../../crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)
- `brd help worktrees` must lease the worktree only when `brd workspace` says `lease.on`, and write the prefix only when it
  says `subject_branch.on`.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`the_worktrees_help_writes_the_prefix_first_and_checks_by_content`)

## Testing / Local Run

- `cargo nextest run -p bd-lease workspace_test` runs the command end to end on `TestRepo` fixtures, the pool in a
  temporary home directory and the global git config of brd a file of the fixture (`Environment::git_config_global`,
  the `GIT_CONFIG_GLOBAL` of every git brd starts). Every other brd test reads an empty global config
  (`EMPTY_GLOBAL_CONFIG`), as CI has: the developer's `branchdeck.*` never reaches one.

## Open Questions / Risks

- The skill is loaded by its description, which an agent may skip for a small edit: then it works as it would without
  Branch Deck. A SessionStart hook would make it certain, but writes the user's agent settings.
- `local_changes` costs one `status` of the shared checkout, a few seconds in a monorepo without fsmonitor.

## References

- [Agent CLI guide](../../docs/contributing/agent-cli.md): `brd workspace`.
- [Git Operations](../../docs/design/git-operations.md): "Repository settings", "Lease".
