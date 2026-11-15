## ADDED Requirements

### Requirement: Inside a lease the build config that its home branch changed is named in home

In a lease with a home branch, `home.build_config_changed` MUST list the build config files that the branch changed
since the fork point of the lease (the merge base of its HEAD and the branch) and that the lease has in another form,
in path order, and MUST be omitted when there is none.

#### Scenario: The home branch changed the cargo config and the toolchain
- **GIVEN** a lease, and a commit on its home branch that changes `.cargo/config.toml`, `tools/x/.cargo/config.toml`,
  `rust-toolchain.toml` and `src/a.rs`
- **WHEN** the agent runs `brd workspace` in the lease
- **THEN** `home.build_config_changed` is `[".cargo/config.toml", "rust-toolchain.toml", "tools/x/.cargo/config.toml"]`
- **AND** the first command of `next` is `brd -C <top level> lease pick`, which moves the lease onto the tip
  [@test] crates/bd-lease/src/workspace_test.rs (`a_lease_names_the_build_config_that_its_home_branch_changed_until_it_follows`)

#### Scenario: The lease follows
- **GIVEN** `home.build_config_changed` names files
- **WHEN** `brd lease pick` lands the commits of the lease, or only follows
- **THEN** `brd workspace` in the lease omits `home.build_config_changed`
  [@test] crates/bd-lease/src/workspace_test.rs (`a_lease_names_the_build_config_that_its_home_branch_changed_until_it_follows`)

### Requirement: The build config is what the builds of a lease read from its tree

The build config MUST be every `.cargo/config.toml`, `.cargo/config`, `rust-toolchain.toml` and `rust-toolchain`, a
`Cargo.toml` whose top-level `profile` table the home branch changed, and the top-level `host` table of
`.branchdeck.toml` at the root, since the return and the pick read the other tables from the home checkout.

#### Scenario: A dependency, a commit rule and the holder's own profile are no build config
- **GIVEN** a lease that committed a change of the `profile.dev` table of `Cargo.toml`
- **AND** a home commit that changes only the dependencies of `Cargo.toml` and `crates/a/Cargo.toml`
- **WHEN** the agent runs `brd workspace` in the lease
- **THEN** `home.build_config_changed` is omitted
- **AND** after a home commit that changes the `profile.dev` table of `Cargo.toml` and the `[commit]` table of
  `.branchdeck.toml`, it is `["Cargo.toml"]`
- **AND** after a home commit that changes the `[host]` table, it is `[".branchdeck.toml", "Cargo.toml"]`
  [@test] crates/bd-lease/src/workspace_test.rs (`only_the_profile_of_a_manifest_and_the_host_table_of_branchdeck_toml_count`)

### Requirement: A build config file the lease has in the form of its home branch is not named

A build config file whose blob, or for a table entry whose table, in the lease HEAD is that of the home tip MUST NOT
be named, so a change that the holder committed in the lease and a copy of a commit of the lease on a rewritten home
branch are not named.

#### Scenario: The holder's own change and a copy on a rewritten home branch
- **GIVEN** a lease that committed a change of `.cargo/config.toml` and picked it, and a home branch whose tip was then
  rewritten, so that the fork point of the lease is no longer on the branch
- **AND** a commit on the home branch that changes `rust-toolchain.toml`, and a commit in the lease that changes
  `tools/x/.cargo/config.toml`
- **WHEN** the agent runs `brd workspace` in the lease
- **THEN** `home.build_config_changed` is `["rust-toolchain.toml"]`
  [@test] crates/bd-lease/src/workspace_test.rs (`a_config_change_of_the_lease_or_its_copy_on_a_rewritten_home_is_not_named`)

### Requirement: A read of the build config that fails does not fail brd workspace

When the build config cannot be read (no merge base, a home branch that no longer exists), `brd workspace` MUST omit
`home.build_config_changed` and answer as it does without it.

#### Scenario: Two commits without a merge base
- **GIVEN** a lease whose HEAD shares no commit with its home branch, and a home branch with another `.cargo/config.toml`
- **WHEN** the agent runs `brd workspace` in the lease
- **THEN** it exits 0 and `home.build_config_changed` is omitted
  [@test] crates/bd-lease/src/workspace_test.rs (`a_failed_read_of_the_build_config_omits_the_field`)
  [@test] crates/git-ops/src/paths_test.rs (`the_paths_changed_since_the_fork_name_both_blobs_and_no_fork_is_an_error`)

#### Scenario: The home branch no longer exists
- **GIVEN** a lease whose home branch was renamed in the shared checkout
- **WHEN** the agent runs `brd workspace` in the lease
- **THEN** it exits 0, `home.switched_to` names the new name, and `home.build_config_changed` is omitted
  [@test] crates/bd-lease/src/workspace_test.rs (`a_failed_read_of_the_build_config_omits_the_field`)

## MODIFIED Requirements

### Requirement: `brd workspace` runs one git process per read

`brd workspace` MUST run one `rev-parse`, one `git config` of the `branchdeck.*` keys, in the main worktree with the
lease on one more of the pool config, in the main worktree or a lease one `status` of the shared checkout (`GIT_OPTIONAL_LOCKS=0`), and in a
lease one more `rev-parse` of the branch the shared checkout has checked out now; in a lease with a home branch one
`diff-tree` from the fork point, and one `cat-file` only when the diff names a build config file.

#### Scenario: The git processes of `brd workspace`
- **WHEN** an agent runs `brd workspace`
- **THEN** it runs one `rev-parse` and one `git config` of the `branchdeck.*` keys
- **AND** in the main worktree with the lease on one more `git config` of the pool config
- **AND** in the main worktree or a lease one `status` of the shared checkout (`GIT_OPTIONAL_LOCKS=0`)
- **AND** in a lease one more `rev-parse` of the branch the shared checkout has checked out now
- **AND** in a lease with a home branch one `diff-tree` from the fork point, and one `cat-file` only when the diff names
  a build config file
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_workspace`)

### Requirement: With the lease the skill keeps the agent in its path and lands at once

With the lease, the skill MUST say to work only in its `path`, land each commit that builds and passes its tests at once
with `brd lease pick`, never by a `git rebase` or `git merge` into the shared checkout and never with `brd update` in
the lease, return only its own lease and ask the user when the task needs the `local_changes`; and to run
`brd workspace` in the lease when the session resumes and before a full build or test run.

#### Scenario: The rule of the lease
- **GIVEN** the lease on
- **WHEN** the agent reads the skill
- **THEN** it works only in its `path` and lands each commit that builds and passes its tests at once with
  `brd lease pick`
- **AND** never by a `git rebase` or `git merge` into the shared checkout, never with `brd update` in the lease
- **AND** it returns only its own lease and asks the user when the task needs the `local_changes`
- **AND** it runs `brd workspace` in the lease when the session resumes and before a full build or test run, and when
  `home.build_config_changed` names files, it runs `brd -C <path> lease pick` before the build, once its commits pass
  their tests, or at once when nothing is unlanded
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)
