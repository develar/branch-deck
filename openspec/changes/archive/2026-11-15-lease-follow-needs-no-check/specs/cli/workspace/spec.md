## MODIFIED Requirements

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
- **AND** a lease with nothing to land follows also where `pick_requires_check` is set, without a green check
  [@test] crates/bd-lease/src/workspace_test.rs (`a_lease_names_the_build_config_that_its_home_branch_changed_until_it_follows`)
  [@test] crates/bd-lease/src/workspace_test.rs (`a_lease_with_nothing_to_land_follows_a_build_config_change_without_a_check`)

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
  their tests, or at once when nothing is unlanded, which needs no green check
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_keeps_the_agent_to_its_lease_also_when_brd_fails_and_in_a_sandbox`)
