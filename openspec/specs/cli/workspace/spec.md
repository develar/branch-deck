---
name: brd workspace
description: Requirements for brd workspace, which a coding agent runs before its first edit, and the three rules it reads, branchdeck.agentLease (work in a lease of the pool and land each commit with brd lease pick, on by default), branchdeck.agentSubjectBranch (start each commit subject with its branch, on by default) and branchdeck.agentPullRequest (push the branch after the pick and open its pull request without asking, off by default), which the user changes in Settings → Agents.
style: plain-1
targets:
  - crates/bd-lease/src/workspace_command.rs
  - crates/lease-pool/src/agent_workspace.rs
  - crates/sync-core/src/repository_config.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/bd-cli/src/install.rs
  - crates/bd-cli/src/schema.rs
  - crates/bd-lease/src/lease_command.rs
  - crates/git-ops/src/lease_slot.rs
  - crates/domain-types/src/landing_route.rs
---

# brd workspace

Status: Implemented
Date: 2026-10-21

## Purpose

Where the skill of Branch Deck is installed ([Agent CLI](../agent-cli/spec.md), `brd install`), a coding agent runs
`brd workspace` before its first edit or commit in a repository, and follows it. Three rules of the git config decide it:
`branchdeck.agentLease`, on by default, makes the agent lease a worktree of the pool ([brd lease](../lease/spec.md)),
commit there and land each commit at once with `brd pick` ([brd pick](../pick/spec.md)), then return the lease;
`branchdeck.agentSubjectBranch`, on by default, makes it start each commit subject with its `(branch)`. The lease is
on everywhere, since isolation harms no repository; the subject branch is on everywhere too, since the prefix is what
groups an agent's commit into its branch and the copies of the virtual branches drop it. A `(branch)` prefix reaches
the remote as it is only where the commits land themselves, not their virtual branches: such a repository turns the
rule off, or names its own form of a subject in its agent instructions, which win. With the rule on or off, the subject
after the prefix has the form of the repository (its agent instructions, else its `git log`), which `[commit] check`
can enforce at the pick ([brd pick](../pick/spec.md#the-commit-rules)). `branchdeck.agentPullRequest`, off by default,
decides how far the landing of an agent's commit goes: off, it stops at the pick on the local branch and the push
waits for the user; on, the agent goes on to push the branch and open its pull request, without asking. It is off
everywhere by default, since a push publishes; a user who lets agents open their pull requests turns it on, for a
repository or for all, instead of writing it into the agent instructions of a repository, where it would contradict
the skill. Nothing that lands into master (a Safe Push, `brd land`) follows from it. The global config gives every repository its value, the repository's own `.git/config` wins,
and the user changes both in Settings → Agents, the first on its tab "All repositories", the second on the tab of the
repository ([Settings](../../repository/settings/spec.md#agents)). What the user says
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

### Requirement: `brd workspace` prints the rules and where to work

`brd workspace` (`-C <dir>`, else the current directory) MUST print `{"schema":2,"kind","lease","subject_branch",
"top_level","subdir","shared_checkout","home","lease_id","holder","local_changes","active_leases","next"}`; each rule
MUST be `{on, source, invalid, reason}`, `source` the config scope that sets it (`local`, `worktree`, `command`,
`global`, `system`) or `default`.

#### Scenario: The answer names the place, the rules and the next command
- **WHEN** an agent runs `brd workspace` in a directory of the main worktree, or with `-C <dir>`
- **THEN** it prints `kind`, `lease`, `subject_branch`, `top_level`, `subdir`, `shared_checkout`, `home`, `lease_id`,
  `holder`, `local_changes`, `active_leases` and `next`
- **AND** each rule is `{on, source, invalid, reason}`, its `source` the config scope that sets it (`local`,
  `worktree`, `command`, `global`, `system`) or `default`
  [@test] crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)

### Requirement: By default the lease and the subject branch are on, the pull request off

By default `lease` and `subject_branch` MUST be on, `pull_request` off.

#### Scenario: Nothing is set
- **GIVEN** no config sets a rule
- **WHEN** an agent runs `brd workspace`
- **THEN** `lease` and `subject_branch` are on with the source `default`
- **AND** `pull_request` is off with the source `default`
  [@test] crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)
  [@test] crates/sync-core/src/repository_config_test.rs (`nothing_set_is_the_defaults`)

### Requirement: With the pull request rule the next of a lease pushes after the pick

With `pull_request.on` the `next` of a lease, in `brd workspace` and of `brd lease get` alike, MUST hold
`brd -C <shared checkout> push <branch>` between the pick and the return; `brd lease get` reads the key in its one
read of the pool config.

#### Scenario: The get and `brd workspace` in the lease push the branch between the pick and the return
- **GIVEN** `pull_request.on`
- **WHEN** an agent leases a worktree, then runs `brd workspace` in it
- **THEN** the `next` of `brd lease get` and of `brd workspace` alike holds `brd -C <shared checkout> push <branch>`
  between the pick and the return
  [@test] crates/bd-lease/src/workspace_test.rs (`the_pull_request_rule_pushes_the_branch_after_the_pick`)

#### Scenario: The key of the repository turns the rule on
- **WHEN** the repository's own config sets `branchdeck.agentPullRequest` on, the default being off
- **THEN** the rule is on with the source `local`
- **AND** `brd lease get` reads the key in its one read of the pool config
  [@test] crates/sync-core/src/repository_config_test.rs (`the_agent_keys_are_written_globally_and_a_repository_keeps_its_own`)

### Requirement: The Push route of the repository turns the pull request rule off

The pull request MUST follow the route into master ([Landing Routes](../../branch/landing-routes/spec.md#setting)):
under the Push route of the repository's own config (`.git/config`, the worktree config, `git -c`) the work lands
without a pull request, so `pull_request` set on MUST answer `on: false` with the reason, and neither `next` (of
`brd workspace` and of `brd lease get`) holds the push.

#### Scenario: The repository's own config names the Push route
- **GIVEN** `pull_request` set on
- **WHEN** the Push route is set in `.git/config`, the worktree config or by `git -c`
- **THEN** `pull_request` answers `on: false` with the reason "The route into master is Push (branchdeck.landing): the
  work lands without a pull request"
- **AND** neither `next` (of `brd workspace` and of `brd lease get`) holds the push
  [@test] crates/bd-lease/src/workspace_test.rs (`the_push_route_turns_the_pull_request_rule_off_and_a_global_push_route_does_not`)

### Requirement: A Push route of the global or the system config changes nothing

A `push` of the global or the system config MUST be unset and change nothing: both `next` still hold the push.

#### Scenario: The global config names the Push route
- **GIVEN** `pull_request` set on
- **WHEN** the global or the system config sets the route `push`
- **THEN** the route is unset and the rule stays on
- **AND** the `next` of `brd workspace` and of `brd lease get` both still hold the push
  [@test] crates/bd-lease/src/workspace_test.rs (`the_push_route_turns_the_pull_request_rule_off_and_a_global_push_route_does_not`)
  [@test] crates/sync-core/src/repository_config_test.rs (`the_push_route_of_the_repository_turns_the_agent_pull_request_off`)

### Requirement: One function of domain-types decides the Push route of the rule

One function of `domain-types` MUST decide it (`LandingSetting::agent_pull_request_off`); `brd lease get` reads the
route with its scope in its one read of the pool config, and Settings → Agents reads the same reason
(`RepositoryConfig.agentPullRequestOff`).

#### Scenario: Only the Push route turns the rule off
- **WHEN** the route into master is Push
- **THEN** `LandingSetting::agent_pull_request_off` gives the reason
- **AND** another route gives none
  [@test] crates/domain-types/src/landing_route_test.rs (`only_the_push_route_turns_the_agent_pull_request_off`)

#### Scenario: Settings → Agents reads the same reason
- **WHEN** the repository's own config names the Push route
- **THEN** `RepositoryConfig.agentPullRequestOff` holds the same reason, which Settings → Agents reads
- **AND** `brd lease get` reads the route with its scope in its one read of the pool config
  [@test] crates/sync-core/src/repository_config_test.rs (`the_push_route_of_the_repository_turns_the_agent_pull_request_off`)

### Requirement: In the main worktree the next command leases a worktree

In the main worktree (`kind: main`) with the lease on, `next` MUST be `brd lease get --holder <your name> -C <top level>`;
`subdir` names where brd runs below the top level, and `local_changes` counts the local changes of the checkout, which
a lease does not have.

#### Scenario: The main worktree with the lease on
- **GIVEN** the lease on
- **WHEN** an agent runs `brd workspace` in the main worktree
- **THEN** `kind` is `main` and `next` is `brd lease get --holder <your name> -C <top level>`
- **AND** `local_changes` counts the local changes of the checkout, which a lease does not have
  [@test] crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)

#### Scenario: brd runs below the top level
- **WHEN** an agent runs `brd workspace` in a directory below the top level
- **THEN** `subdir` names that directory, the path below the top level
- **AND** at the top level, or outside it, `subdir` is empty
  [@test] crates/lease-pool/src/agent_workspace_test.rs (`the_subdir_is_the_path_below_the_top_level`)

### Requirement: Inside a leased slot the answer names its lease and its landing

Inside a leased slot (`kind: lease`) `shared_checkout` MUST be the source the slot was leased from, in the form of
`top_level` (backslashes on Windows), `home` the same as `brd lease get` prints it ([brd lease](../lease/spec.md)),
`lease_id` and `holder` its lease, and `next` the same as the get's: `brd -C <top level> lease pick` and the return of
this lease.

#### Scenario: `brd workspace` in a lease
- **WHEN** an agent runs `brd workspace` in its leased slot
- **THEN** `kind` is `lease`, `shared_checkout` the source the slot was leased from, in the form of `top_level`
- **AND** `home` is the same as `brd lease get` prints it, `lease_id` and `holder` its lease
- **AND** `next` is the same as the get's: `brd -C <top level> lease pick` and the return of this lease
  [@test] crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)

### Requirement: A shared checkout that switched its branch is named in home

When the shared checkout has another branch checked out now than at the get, `home.switched_to` MUST name it (`null`:
a detached HEAD), so the agent asks the user before it picks.

#### Scenario: The shared checkout switched to another branch
- **WHEN** the shared checkout has another branch checked out now than at the get
- **THEN** `home.switched_to` names it, so the agent asks the user before it picks
- **AND** once the branch of the get is checked out again, `home.switched_to` is gone
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_home_that_switched_its_branch`)

#### Scenario: The shared checkout has a detached HEAD
- **WHEN** the shared checkout has a detached HEAD now
- **THEN** `home.switched_to` is `null`

### Requirement: Every ready lease of the pool is listed with its holder

`active_leases` MUST list every ready lease of the pool, so a session that lost its path finds its own by `holder`,
with `holder_alive` (omitted without a scan of the processes, on Windows, where the get records no holder process)
and, for one whose holder is gone, `idle` as `brd lease status` shows it (`idle_with_work` with its work: tell the
user).

#### Scenario: A session that lost its path finds its lease by holder
- **WHEN** an agent runs `brd workspace` in the shared checkout or in a lease
- **THEN** `active_leases` lists every ready lease of the pool with its `holder`
- **AND** with `holder_alive`, omitted without a scan of the processes, on Windows, where the get records no holder
  process
  [@test] crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)

#### Scenario: The holder of a lease is gone
- **WHEN** the holder of a ready lease is gone
- **THEN** its entry has `idle` as `brd lease status` shows it
- **AND** `idle_with_work` with its work: tell the user

### Requirement: A lease taken over refuses `brd workspace` of another session

In a slot whose lease a get or `brd lease reclaim` took over, `brd workspace` of another session than the one that
holds it now MUST be `lease_reclaimed` (exit 4), so a resumed session of the old holder learns it before its first
edit.

#### Scenario: A resumed session of the old holder
- **GIVEN** a get or `brd lease reclaim` took over the lease of a slot
- **WHEN** another session than the one that holds it now runs `brd workspace` in the slot
- **THEN** it is `lease_reclaimed` (exit 4), before its first edit
  [@test] crates/bd-lease/src/takeover_test.rs (`a_command_of_another_session_in_a_taken_over_slot_is_refused_as_lease_reclaimed`)

### Requirement: The global config sets a rule and the repository's own wins

A value of the global config MUST set a rule for every repository, and a value of the repository's own config MUST
win over it.

#### Scenario: The global config turns a rule off, a repository keeps its own
- **WHEN** the global config sets a rule off
- **THEN** the rule is off in every repository, with the source `global`
- **AND** a value of the repository's own config wins over it
  [@test] crates/bd-lease/src/workspace_test.rs (`the_global_config_turns_a_rule_off_and_a_repository_keeps_its_own`)
  [@test] crates/sync-core/src/repository_config_test.rs (`the_agent_keys_are_written_globally_and_a_repository_keeps_its_own`)

### Requirement: A value that is no boolean leaves the default with invalid

A value git does not read as a boolean MUST leave the default with `invalid`.

#### Scenario: A rule set to maybe
- **WHEN** a config sets a rule to a value git does not read as a boolean
- **THEN** the rule keeps the default, with `invalid` naming the value
  [@test] crates/bd-lease/src/workspace_test.rs (`the_global_config_turns_a_rule_off_and_a_repository_keeps_its_own`)

### Requirement: A linked worktree outside the pool needs no lease

A linked worktree outside the pool (`kind: linked`) MUST answer the lease off with the reason "A worktree of its own
already", and `next` empty.

#### Scenario: `brd workspace` in a linked worktree
- **WHEN** an agent runs `brd workspace` in a linked worktree outside the pool
- **THEN** `kind` is `linked`, the lease off with the reason "A worktree of its own already"
- **AND** `next` is empty
  [@test] crates/bd-lease/src/workspace_test.rs (`a_worktree_of_its_own_needs_no_lease_and_no_working_tree_is_an_answer`)

### Requirement: Outside a working tree the answer is no error

Outside a working tree (`kind: none`: a bare repository, a `.git` directory, no repository) the answer MUST be no
error, the lease off and `next` empty.

#### Scenario: No working tree
- **WHEN** an agent runs `brd workspace` in a bare repository, a `.git` directory or no repository
- **THEN** the answer is no error: `kind` is `none`, the lease off and `next` empty
  [@test] crates/bd-lease/src/workspace_test.rs (`a_worktree_of_its_own_needs_no_lease_and_no_working_tree_is_an_answer`)

### Requirement: A pool that can't be written turns the lease off

With the lease on in the main worktree, a pool directory (or its first existing parent) that can't be written, as in a
sandbox, MUST turn the lease off with the reason; the probe file MUST be removed at once.

#### Scenario: A sandbox can't write the pool
- **GIVEN** the lease on in the main worktree
- **WHEN** the pool directory can't be written, as in a sandbox
- **THEN** the lease is off with the reason
  [@test] crates/bd-lease/src/workspace_test.rs (`a_pool_that_cant_be_written_turns_the_lease_off`)

#### Scenario: A pool that does not exist yet
- **WHEN** the pool directory does not exist yet
- **THEN** the probe writes its first existing parent
- **AND** the probe file is removed at once, nothing is left behind
  [@test] crates/lease-pool/src/agent_workspace_test.rs (`the_probe_of_the_pool_writes_its_first_existing_parent_and_removes_its_file`)

### Requirement: `brd workspace` writes no ref, config, index or working tree

`brd workspace` MUST write no ref, config, index or working tree.

#### Scenario: A read-only command
- **WHEN** an agent runs `brd workspace`
- **THEN** no ref, config, index or working tree changes
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

### Requirement: `brd workspace` runs one git process per read

`brd workspace` MUST run one `rev-parse`, one `git config` of the `branchdeck.*` keys, in the main worktree one more of
the pool config, in the main worktree or a lease one `status` of the shared checkout (`GIT_OPTIONAL_LOCKS=0`), and in a
lease one more `rev-parse` of the branch the shared checkout has checked out now.

#### Scenario: The git processes of `brd workspace`
- **WHEN** an agent runs `brd workspace`
- **THEN** it runs one `rev-parse` and one `git config` of the `branchdeck.*` keys
- **AND** in the main worktree one more `git config` of the pool config
- **AND** in the main worktree or a lease one `status` of the shared checkout (`GIT_OPTIONAL_LOCKS=0`)
- **AND** in a lease one more `rev-parse` of the branch the shared checkout has checked out now
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_workspace`)

### Requirement: The output of `brd workspace` matches `brd schema`

The output MUST match `brd schema` (`workspace`).

#### Scenario: The schema of the output
- **WHEN** `brd schema` lists its outputs
- **THEN** it lists `workspace`, and the output of `brd workspace` matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

### Requirement: The skill's description starts with the first edit and fits the limit

The skill's description MUST start with the first edit ("Use before the first edit or commit in a Git repository: `brd workspace` …")
and stay within the 1024 characters of the Agent Skills format.

#### Scenario: The description of the skill
- **WHEN** the skill is written
- **THEN** its description starts with "Use before the first edit or commit in a Git repository: `brd workspace` …"
- **AND** it has 1024 characters at most
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_description_starts_with_the_first_edit_and_fits_the_limit`)

### Requirement: The skill says to run `brd workspace` first and follow it

The skill's body MUST say to run `brd workspace` first and follow it, the user's words winning.

#### Scenario: The body of the installed skill
- **WHEN** `brd install` writes the skill
- **THEN** its body says to run `brd workspace` before the first edit or commit and follow it
- **AND** the user's words win over it
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)

### Requirement: With the lease the skill keeps the agent in its path and lands at once

With the lease, the skill MUST say to work only in its `path`, land each commit that builds and passes its tests at once
with `brd lease pick`, never by a `git rebase` or `git merge` into the shared checkout and never with `brd update` in
the lease, return only its own lease and ask the user when the task needs the `local_changes`.

#### Scenario: The rule of the lease
- **GIVEN** the lease on
- **WHEN** the agent reads the skill
- **THEN** it works only in its `path` and lands each commit that builds and passes its tests at once with
  `brd lease pick`
- **AND** never by a `git rebase` or `git merge` into the shared checkout, never with `brd update` in the lease
- **AND** it returns only its own lease and asks the user when the task needs the `local_changes`
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)

### Requirement: The agent's own ignored files stay in the shared checkout

The skill MUST say that its own files ignored by git and kept by its harness in the shared checkout (a plan, notes) are
not `local_changes` and SHALL allow them to be updated there at their original paths, never copied into the lease or
committed.

#### Scenario: The agent's plan in the shared checkout
- **GIVEN** the lease on and a plan or notes the harness keeps in the shared checkout, ignored by git
- **WHEN** the agent updates them
- **THEN** they are not `local_changes`, and it updates them there at their original paths
- **AND** never copies them into the lease or commits them
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agents_plan_in_the_shared_checkout`)

### Requirement: Without the lease the agent stages only its own paths

Without the lease, the skill MUST say to stage only its own paths.

#### Scenario: The lease off
- **GIVEN** the lease off
- **WHEN** the agent commits
- **THEN** it stages only its own paths

### Requirement: With the subject branch the subject starts with its branch

With the subject branch, the skill MUST say to write `(branch) ` first, a branch of `brd status --brief` reused where it
fits, a new branch `(name)`, `(parent/leaf)` only on an existing virtual branch, never the baseline.

#### Scenario: The prefix of a subject
- **GIVEN** the subject branch on
- **WHEN** the agent writes a commit subject
- **THEN** it writes `(branch) ` first, a branch of `brd status --brief` reused where it fits
- **AND** a new branch is `(name)`, `(parent/leaf)` only on an existing virtual branch, never the baseline
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_says_that_the_repository_form_of_a_subject_follows_the_prefix`)

### Requirement: The pull request rule decides whether the agent pushes and opens it

With `pull_request.on`, the skill MUST say to run the push of `next` and the `gh pr create` it prints without asking;
off, to ask first; off with a `reason` (the Push route), neither a push nor a pull request.

#### Scenario: The rule on
- **GIVEN** `pull_request.on`
- **WHEN** the commit is picked
- **THEN** the agent runs the push of `next` and the `gh pr create` it prints without asking
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)

#### Scenario: The rule off
- **GIVEN** the rule off without a reason
- **WHEN** the commit is picked
- **THEN** the agent asks first
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)

#### Scenario: The rule off with a reason
- **GIVEN** the rule off with a `reason` (the Push route)
- **WHEN** the commit is picked
- **THEN** neither a push nor a pull request
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)

### Requirement: A Safe Push, a land, a force push and a parked branch wait for the user

The skill MUST say that a Safe Push, `brd land`, a push with --force and a parked branch wait for the user either way.

#### Scenario: Either way
- **WHEN** the rule is on or off
- **THEN** a Safe Push, `brd land`, a push with --force and a parked branch wait for the user
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)

### Requirement: The agent never changes any of the three rules

The skill MUST say never to change any of the three rules.

#### Scenario: The three rules stay the user's
- **WHEN** the agent works by the skill
- **THEN** it never changes any of the three rules
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)

### Requirement: A lease is the agent's only worktree, also when brd fails

The skill MUST carry the isolation rules a repository would otherwise repeat in its own agent instructions: a lease is
the agent's only worktree, never a `git worktree add` or a clone on its own initiative, also not when a brd command
fails (then it works where it is when that is safe, or asks, and names a lease that survived).

#### Scenario: A brd command fails
- **WHEN** a brd command fails
- **THEN** the agent makes no `git worktree add` or clone on its own initiative
- **AND** it works where it is when that is safe, or asks, and names a lease that survived
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)

### Requirement: The rule of the lease names no agent

The rule of the lease MUST name no agent: it is the same for every agent, and a note for each kind of agent follows it.

#### Scenario: The notes of the agents follow the rule
- **WHEN** the skill states the rule of the lease
- **THEN** the rule names no agent
- **AND** a note for each kind of agent follows it
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)

### Requirement: In Claude Code the agent moves into the lease with a cd first

In Claude Code the agent MUST do a `cd` into the lease's `path` first, which stays where the pool is in
`permissions.additionalDirectories`, else `EnterWorktree` with that path, which asks the user.

#### Scenario: Claude Code takes the lease
- **WHEN** an agent of Claude Code takes the lease
- **THEN** it does a `cd` into the lease's `path` first, which stays where the pool is in
  `permissions.additionalDirectories`
- **AND** else `EnterWorktree` with that path, which asks the user
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)

### Requirement: In a sandbox the agent asks for write access to the lease alone

In a sandbox (Codex) the agent MUST ask for write access to exactly the lease's `path` (`request_permissions`) before the
first edit there, and return the untouched lease when that is refused.

#### Scenario: Codex takes the lease
- **WHEN** an agent in a sandbox (Codex) takes the lease
- **THEN** it asks for write access to exactly the lease's `path` (`request_permissions`) before the first edit there
- **AND** it returns the untouched lease when that is refused
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)

### Requirement: A shell that does not keep the directory gets absolute paths

When its shell does not keep the current directory (Junie, for example), the agent MUST give each file tool an absolute
path in `path` and start each shell command with `cd <path> && `.

#### Scenario: Junie takes the lease
- **WHEN** an agent whose shell does not keep the current directory (Junie, for example) works in the lease
- **THEN** it gives each file tool an absolute path in `path`
- **AND** it starts each shell command with `cd <path> && `
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)

### Requirement: The Junie note keeps its plan in the shared checkout

The Junie note MUST say that its plan is `.junie/plans/<name>.md` of the shared checkout and that it updates statuses
there, by that absolute path, not in `path`.

#### Scenario: Junie updates its plan
- **WHEN** Junie updates the statuses of its plan
- **THEN** it updates `.junie/plans/<name>.md` of the shared checkout, by that absolute path, not in `path`
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agents_plan_in_the_shared_checkout`)

### Requirement: The skill forbids `git stash` in a lease

The skill MUST carry the rules of the pool too: never `git stash` in a lease (every worktree of the pool shares one
stash).

#### Scenario: Work in progress in a lease
- **WHEN** the agent would keep work in progress in a lease
- **THEN** it never runs `git stash`, since every worktree of the pool shares one stash
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)

### Requirement: A change to a pushed branch is a fixup commit

The skill MUST carry the rules of a pushed branch: a change to a pushed branch (an open pull request) is a
`(branch) fixup! <subject>` commit, folded only when the user asks, and a promise of the description it breaks is fixed
with `gh pr edit`.

#### Scenario: A change to an open pull request
- **WHEN** the agent changes a pushed branch (an open pull request)
- **THEN** the change is a `(branch) fixup! <subject>` commit, folded only when the user asks
- **AND** a promise of the description it breaks is fixed with `gh pr edit`
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_opens_the_pull_request_only_where_the_rule_says_and_keeps_pushed_history`)

### Requirement: A repository's instructions only rely on the skill

With these rules in the skill, a repository's instructions SHALL say only that it relies on the skill.

#### Scenario: The agent instructions of a repository
- **WHEN** a repository writes its agent instructions
- **THEN** they say only that it relies on the skill

### Requirement: `brd help worktrees` leases only when `brd workspace` says so

`brd help worktrees` MUST lease the worktree only when `brd workspace` says `lease.on`.

#### Scenario: The lease of the worktrees help
- **WHEN** an agent follows `brd help worktrees`
- **THEN** it leases the worktree only when `brd workspace` says `lease.on`
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_worktrees_help_writes_the_prefix_first_and_checks_by_content`)

### Requirement: `brd help worktrees` writes the prefix only with the subject branch

`brd help worktrees` MUST write the prefix only when `brd workspace` says `subject_branch.on`.

#### Scenario: The prefix of the worktrees help
- **WHEN** an agent follows `brd help worktrees`
- **THEN** it writes the prefix only when `brd workspace` says `subject_branch.on`
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_worktrees_help_writes_the_prefix_first_and_checks_by_content`)

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

- [Agent CLI guide](../../../../docs/contributing/agent-cli.md): `brd workspace`.
- [Git Operations](../../../../docs/design/git-operations.md): "Repository settings", "Lease".
