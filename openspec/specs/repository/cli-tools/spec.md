---
name: Command-Line Tools
description: Requirements for the teamcity and youtrack CLIs in Settings → Tools - the binary, its version and the servers it is signed in to, the agent skill each bundles (installed with the CLI by default, installed, updated and removed from its line), and the one action Rust picks for each row (install and update through Homebrew, winget or scoop, the sign-in), with the command for a terminal where the app doesn't run it.
style: plain-1
targets:
  - crates/cli-tools/src/environment.rs
  - crates/cli-tools/src/parse.rs
  - crates/cli-tools/src/status.rs
  - crates/cli-tools/src/install.rs
  - crates/cli-tools/src/package_manager.rs
  - crates/cli-tools/src/teamcity_login.rs
  - crates/sync-utils/src/child_process.rs
  - crates/sync-utils/src/known_tools.rs
  - crates/domain-types/src/cli_tools.rs
  - crates/test-utils/src/fake_cli_tools.rs
  - crates/test-server/src/cli_tools_fixtures.rs
  - crates/app-api/src/app/cli_tools.rs
  - crates/test-server/src/host.rs
  - src-tauri/src/services.rs
  - src/stores/cli-tools.ts
  - src/lib/sign-in-run.ts
  - crates/remote-facts/src/sign_in.rs
  - src/lib/cli-tools.ts
  - src/components/settings/ToolsPage.tsx
  - src/components/settings/AgentsPage.tsx
---

# Command-Line Tools

Status: Implemented
Date: 2026-10-23

## Purpose

Two CLIs of the team do what Branch Deck and the agents need beside git: `teamcity` (JetBrains/teamcity-cli) reads the
logs and tests of the builds of a robot, for the prompt of a failed robot ([Safe Push Failure
Prompt](../../branch/safe-push-failure-prompt/spec.md)) and for an agent; `youtrack` (JetBrains/youtrack-cli) reads and
changes the issues of the commits. Settings → Tools shows each: where it is, its version, the servers it is signed in
to, the agent skill it bundles, and the one thing it needs next, which the app does itself where it can (install and
update through Homebrew, winget or scoop, the sign-in of `teamcity`) and otherwise names as a command for a terminal.

## Goals

- Install and update both CLIs without a terminal.
- See at a glance whether a CLI the app and the agents rely on is there, current and signed in.
- One status for every surface: Rust decides the action of a row; the page only names it.

## Non-goals

- Reading the token a CLI keeps: each keeps its own sign-in in its keychain item, which the app never reads.
- Running an install script (`curl | bash`, `irm | iex`) itself: the app runs a package manager or the CLI's own
  `update`, which may run the install script of its release, or shows the command.
- A CLI without a use in Branch Deck: a row is added when the app or its agent skill relies on the tool.

## Requirements

### Requirement: Status: Each tool is looked for as the app looks for its other CLIs

Each tool MUST be looked for as the app looks for its other CLIs (`sync_utils::known_tools`): `PATH`, then Homebrew's
`bin` (Apple silicon, Intel, Linux) and `~/.local/bin` (the install scripts), then the `PATH` of the login shell, as
an app started from the Finder has none of them. It MUST be looked up on every read, so a CLI installed meanwhile is
found.

#### Scenario: A tool before PATH, on PATH and after it
- **WHEN** a read looks for a tool
- **THEN** it looks in `PATH`, then in Homebrew's `bin` and `~/.local/bin`, then in the `PATH` of the login shell
  [@test] crates/sync-utils/src/child_process_test.rs (`finds_a_tool_before_path_then_on_path_then_after_it`)

#### Scenario: A CLI installed meanwhile
- **GIVEN** a CLI installed since the last read
- **WHEN** the status is read
- **THEN** the CLI is found

### Requirement: Status: A read runs the commands of both tools at once

A read MUST run `--version`, `auth status --json` and `skill list`, each in its own thread, both tools at once, 20 s
each, while GitHub is asked for the newest releases.

#### Scenario: Both tools are read
- **WHEN** the status is read
- **THEN** both tools are read at once, each command in its own thread, while the newest releases are asked
  [@test] crates/cli-tools/src/status_test.rs (`reads_every_tool`)
  [@test] crates/app-api/src/app/cli_tools_test.rs (`the_tools_are_found_while_the_releases_are_asked_and_windows_that_ask_meanwhile_share_the_read_after_it`)

### Requirement: Status: The CLIs run as an agent's shell runs them

The CLIs MUST run without colours (`NO_COLOR=1`), prompts or an update check, and without the `TEAMCITY_*` and
`YOUTRACK_*` credential variables of the app's environment, so the status is what an agent's shell gets.

#### Scenario: The credential variables of the app
- **GIVEN** the app's environment has `TEAMCITY_*` or `YOUTRACK_*` credential variables
- **WHEN** a read runs the CLIs
- **THEN** they run without those variables, colours (`NO_COLOR=1`), prompts or an update check

### Requirement: Status: The windows that ask while a read runs share the read after it

The windows that ask while a read runs (the Setup dot of each window, Settings → Tools) MUST share the read after it
([below](#sign-in)).

#### Scenario: Two windows ask while a read runs
- **GIVEN** a read of the status runs
- **WHEN** the Setup dot of a window and Settings → Tools ask for the status
- **THEN** they share the read after it
  [@test] crates/app-api/src/app/cli_tools_test.rs (`the_tools_are_found_while_the_releases_are_asked_and_windows_that_ask_meanwhile_share_the_read_after_it`)

### Requirement: Status: The version is the first word with a digit and a dot

The version of a CLI MUST be the first word with a digit and a dot (`teamcity version 1.6.0`, `v0.7.0`).

#### Scenario: The version of each CLI
- **WHEN** `teamcity version 1.6.0` or `v0.7.0` is read
- **THEN** the version is the first word with a digit and a dot
  [@test] crates/cli-tools/src/parse_test.rs (`reads_the_version_of_each_cli`)

### Requirement: Status: Versions order by their numbers

Versions MUST order by their numbers.

#### Scenario: Two versions are compared
- **WHEN** two versions are compared
- **THEN** they order by their numbers
  [@test] crates/cli-tools/src/parse_test.rs (`orders_versions_by_their_numbers`)

### Requirement: Status: The servers of teamcity are read from its auth status

A server of `teamcity auth status --json` MUST be read as signed in (`authenticated`), a guest, or failed with the
CLI's error; the entry of no server is no server; `token_expiry` is its expiry.

#### Scenario: The servers of teamcity with their user and expiry
- **WHEN** `teamcity auth status --json` is read
- **THEN** each server is signed in (`authenticated`), a guest, or failed with the CLI's error, with `token_expiry` as its expiry
- **AND** the entry of no server is no server
  [@test] crates/cli-tools/src/parse_test.rs (`reads_the_servers_of_teamcity_with_their_user_and_expiry`)

### Requirement: Status: The instances of youtrack are one object or an array

`youtrack auth status --json` MUST be read as one object or, with several instances, an array; `not configured` is no
instance.

#### Scenario: One instance, several, none
- **WHEN** `youtrack auth status --json` is one object, an array, or `not configured`
- **THEN** it is one instance, several instances, or no instance
  [@test] crates/cli-tools/src/parse_test.rs (`reads_the_instances_of_youtrack_as_one_object_or_an_array`)

### Requirement: Status: The skill of the agents is the oldest copy against the bundled one

The skill of the agents (`teamcity-cli`, `youtrack-cli`) MUST be the `version:` of the oldest `<skill>/SKILL.md` in
`~/.claude/skills` and `~/.agents/skills`, against the version of `<tool> skill list`.

#### Scenario: Two copies of the skill
- **GIVEN** `~/.claude/skills` and `~/.agents/skills` each hold a `<skill>/SKILL.md`
- **WHEN** the status is read
- **THEN** the skill of the agents is the `version:` of the oldest copy, against the version of `<tool> skill list`
  [@test] crates/cli-tools/src/parse_test.rs (`reads_the_version_of_a_skill_and_of_the_skill_list`)
  [@test] crates/cli-tools/src/status_test.rs (`the_skill_of_each_cli_is_the_oldest_copy_of_the_agents_against_the_bundled_one_beside_the_action`)

### Requirement: Status: Rust says what the line of the skill offers besides Remove

Rust MUST say what the line of the skill offers besides Remove: Install Skill while the agents have none, Update Skill
while theirs is older.

#### Scenario: The agents have no skill, then an older one
- **WHEN** the agents have no copy of the skill
- **THEN** the line offers Install Skill
- **AND** once their copy is older than the bundled one, Update Skill
  [@test] crates/cli-tools/src/status_test.rs (`the_skill_of_each_cli_is_the_oldest_copy_of_the_agents_against_the_bundled_one_beside_the_action`)

### Requirement: The action of a row: Rust picks one action per tool, the most needed first

Rust MUST pick one action per tool, the most needed first: Install (not found), Sign In (signed in to no server),
Update (a newer release than the installed one); none when all is well.

#### Scenario: A tool that is not found
- **WHEN** a tool is not found
- **THEN** its action is Install
  [@test] crates/cli-tools/src/status_test.rs (`a_missing_cli_offers_its_install_through_homebrew_or_a_command`)

#### Scenario: Signed in to no server, with a newer release
- **GIVEN** a newer release than the installed one
- **WHEN** the tool is signed in to no server
- **THEN** its action is Sign In
- **AND** once it is signed in, Update
  [@test] crates/cli-tools/src/status_test.rs (`an_installed_cli_has_its_version_and_servers_and_offers_its_sign_in_first`)

#### Scenario: All is well
- **WHEN** a tool is installed, current and signed in
- **THEN** it has no action

### Requirement: The action of a row: A teamcity before 1.6.2 updates before its sign-in

A `teamcity` before 1.6.2 MUST update before its sign-in ([below](#sign-in)).

#### Scenario: An older teamcity signed in to no server
- **GIVEN** a `teamcity` before 1.6.2, signed in to no server, and a newer release
- **WHEN** Rust picks its action
- **THEN** the action is Update, before the sign-in
  [@test] crates/cli-tools/src/status_test.rs (`a_teamcity_before_its_browser_sign_in_without_a_terminal_updates_first_else_signs_in_in_a_terminal`)

### Requirement: The action of a row: The skill has a line of its own

The skill SHALL have a line of its own, beside the one action of the row.

#### Scenario: A skill to install beside a row without an action
- **GIVEN** a tool that needs nothing, whose skill the agents do not have
- **WHEN** Rust picks its action
- **THEN** the row has no action and the line of the skill offers Install Skill
  [@test] crates/cli-tools/src/status_test.rs (`the_skill_of_each_cli_is_the_oldest_copy_of_the_agents_against_the_bundled_one_beside_the_action`)

### Requirement: The action of a row: The app runs the action where it can, else shows the command

The app MUST run the action where it can, else the row MUST show the command for a terminal with Copy: an install
without Homebrew (the README's command), a sign-in of `youtrack` without the app's sign-in and of a `teamcity` before
1.6.2, an update of a `youtrack` before 0.8.1 that no package manager installed (`youtrack update`, which upgrades only
in a terminal there).

#### Scenario: An install with Homebrew
- **GIVEN** Homebrew is installed
- **WHEN** a tool is not found
- **THEN** the app runs the install and the row has no command to copy
  [@test] crates/cli-tools/src/status_test.rs (`a_missing_cli_offers_its_install_through_homebrew_or_a_command`)

#### Scenario: An install without Homebrew
- **GIVEN** no package manager the app runs
- **WHEN** a tool is not found
- **THEN** the row shows the README's command with Copy, and the app does not run the install
  [@test] crates/cli-tools/src/status_test.rs (`a_missing_cli_offers_its_install_through_homebrew_or_a_command`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should give the command to install a CLI without Homebrew`)
  [@test] crates/cli-tools/src/install_test.rs (`without_a_package_manager_the_install_is_the_users`)

#### Scenario: A sign-in of youtrack without the app's sign-in
- **GIVEN** the app is signed in to no YouTrack
- **WHEN** `youtrack` is signed in to no instance
- **THEN** the row shows the command of its sign-in for a terminal with Copy

#### Scenario: A sign-in of a teamcity before 1.6.2
- **GIVEN** a `teamcity` before 1.6.2 signed in to no server, and no newer release known
- **WHEN** Rust picks its action
- **THEN** the row shows the command of its sign-in for a terminal with Copy
  [@test] crates/cli-tools/src/status_test.rs (`a_teamcity_before_its_browser_sign_in_without_a_terminal_updates_first_else_signs_in_in_a_terminal`)

#### Scenario: An update of a youtrack no package manager installed
- **GIVEN** a `youtrack` that no package manager installed, and a newer release
- **WHEN** it is before 0.8.1
- **THEN** the row shows `youtrack update` for a terminal, which the app does not run
- **AND** from 0.8.1 the app runs the update itself
  [@test] crates/cli-tools/src/status_test.rs (`youtrack_outside_homebrew_updates_itself_from_0_8_1_and_in_a_terminal_before`)
  [@test] crates/cli-tools/src/install_test.rs (`a_youtrack_outside_homebrew_before_0_8_1_updates_only_in_a_terminal`)

### Requirement: Install and update: Homebrew installs and upgrades a tool

Install MUST be `brew install jetbrains/utils/<tool>`, Update of a tool Homebrew installed (its binary links into a
`Cellar`) `brew upgrade`, with `NONINTERACTIVE`, no hints, cleanup or dependents check, its taps updated first (the
tap of a tool lags behind its releases otherwise).

#### Scenario: An install and an update through Homebrew
- **GIVEN** Homebrew is installed
- **WHEN** a missing tool is installed, then updated
- **THEN** the install is `brew install jetbrains/utils/<tool>`, its lines streamed to the row
- **AND** the update is `brew upgrade`, as the binary links into a `Cellar`
  [@test] crates/cli-tools/src/install_test.rs (`installs_and_updates_through_homebrew_streaming_its_lines`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should install a missing CLI through Homebrew with its agent skill, then show its version and its sign-in`)

### Requirement: Install and update: A teamcity no manager installed updates itself

A `teamcity` no manager installed MUST update itself (`teamcity update --yes`).

#### Scenario: A teamcity outside Homebrew
- **GIVEN** a `teamcity` no package manager installed, and a newer release
- **WHEN** the user updates it
- **THEN** the app runs `teamcity update --yes`
  [@test] crates/cli-tools/src/install_test.rs (`a_teamcity_outside_homebrew_updates_itself`)

### Requirement: Install and update: A youtrack 0.8.1 or later no manager installed updates itself

A `youtrack` 0.8.1 or later no manager installed MUST update itself too (`youtrack update --yes --json --quiet`: the
lines of the upgrade on stderr, its status with `upgraded` on stdout, an error as the JSON envelope of the CLI on
stderr).

#### Scenario: A youtrack 0.8.1 outside Homebrew
- **GIVEN** a `youtrack` 0.8.1 no package manager installed, and a newer release
- **WHEN** the user updates it
- **THEN** the app runs `youtrack update --yes --json --quiet` without a terminal, the lines of the upgrade on the row
- **AND** an error of the CLI is its message
  [@test] crates/cli-tools/src/install_test.rs (`a_youtrack_outside_homebrew_updates_itself_without_a_terminal_from_0_8_1`)

#### Scenario: Before 0.8.1
- **GIVEN** a `youtrack` before 0.8.1 no package manager installed
- **WHEN** its update is asked
- **THEN** the app does not run `youtrack update`
  [@test] crates/cli-tools/src/install_test.rs (`a_youtrack_outside_homebrew_before_0_8_1_updates_only_in_a_terminal`)

#### Scenario: The status on stdout
- **WHEN** `youtrack update --yes --json --quiet` writes its status
- **THEN** `upgraded` of it is read
  [@test] crates/cli-tools/src/parse_test.rs (`reads_whether_youtrack_upgraded`)

### Requirement: Install and update: The version read after a self-update decides

After a tool updated itself, the version read after it MUST decide ([below](#agent-skill)).

#### Scenario: A self-update that installed nothing newer
- **WHEN** a self-update ends and the version read after it is not the newer release
- **THEN** the update fails, whatever the CLI answered
  [@test] crates/cli-tools/src/install_test.rs (`a_youtrack_outside_homebrew_updates_itself_without_a_terminal_from_0_8_1`)

### Requirement: Install and update: On Windows winget installs a tool it has a package of

On Windows the manager MUST be winget for a tool it has a package of (`JetBrains.TeamCityCLI`; `youtrack` has none in
winget yet): `winget install --id … --exact --scope user --silent --disable-interactivity --accept-source-agreements --accept-package-agreements`,
`winget upgrade` likewise.

#### Scenario: teamcity through winget
- **GIVEN** winget on Windows
- **WHEN** `teamcity` is installed or updated
- **THEN** the app runs `winget install` or `winget upgrade` of `JetBrains.TeamCityCLI` for the user, without prompts
- **AND** `youtrack` has no package of winget
  [@test] crates/cli-tools/src/package_manager_test.rs (`winget_installs_for_the_user_without_prompts_and_scoop_adds_its_bucket_first`)
  [@test] crates/cli-tools/src/install_test.rs (`the_package_manager_of_the_machine_installs_teamcity`)

### Requirement: Install and update: Else scoop installs a tool, its bucket added first

On Windows, else the manager MUST be scoop, which adds the `jetbrains` bucket (`https://github.com/JetBrains/scoop-utils`,
a failure there counts nothing: it may be added already), then `scoop install jetbrains/<tool>`, `scoop update <tool>`.

#### Scenario: youtrack through scoop
- **GIVEN** scoop on Windows, and no package of winget
- **WHEN** the tool is installed
- **THEN** the app adds the `jetbrains` bucket, whose failure counts nothing, then runs `scoop install jetbrains/<tool>`
- **AND** an update runs `scoop update <tool>`
  [@test] crates/cli-tools/src/package_manager_test.rs (`winget_installs_for_the_user_without_prompts_and_scoop_adds_its_bucket_first`)

### Requirement: Install and update: choco is not run

choco needs an administrator and MUST NOT be run.

#### Scenario: Only choco on Windows
- **GIVEN** choco is the only package manager on Windows
- **WHEN** a tool is installed
- **THEN** the app does not run choco

### Requirement: Install and update: Without winget or scoop the row shows the README's command

On Windows without either manager the row MUST show the README's command (`winget install JetBrains.TeamCityCLI`,
`irm https://jb.gg/youtrack-cli/install.ps1 | iex`).

#### Scenario: Windows without a package manager
- **GIVEN** neither winget nor scoop on Windows
- **WHEN** a tool is not found
- **THEN** the row shows `winget install JetBrains.TeamCityCLI` or `irm https://jb.gg/youtrack-cli/install.ps1 | iex`

### Requirement: Install and update: The path of a binary tells which manager installed it

Which manager installed a CLI MUST be told by its path (`Cellar`, `WinGet`, `scoop`).

#### Scenario: A binary of winget, of scoop, of the install script
- **GIVEN** winget and scoop on the machine
- **WHEN** the binary lies under `WinGet` or `scoop`
- **THEN** that manager installed it
- **AND** a binary of `~/.local/bin` has no manager
  [@test] crates/cli-tools/src/package_manager_test.rs (`the_path_of_a_binary_tells_its_manager_when_the_machine_has_it`)

### Requirement: Install and update: The places of the managers are looked in after PATH

winget's links, scoop's shims and `~/.local/bin` (the install script) MUST be looked in after `PATH`, as the `PATH` of
a running app lacks what was installed after it started.

#### Scenario: A CLI installed after the app started
- **GIVEN** a CLI installed by winget, scoop or the install script after the app started
- **WHEN** the status is read
- **THEN** the CLI is found in winget's links, scoop's shims or `~/.local/bin`, after `PATH`

### Requirement: Install and update: An update also updates an older skill of the agents

An update MUST also update an older skill of the agents ("Updated teamcity to 1.7.0 and its agent skill to 1.5.0").

#### Scenario: An update with an older skill
- **GIVEN** the agents have an older skill than the newer release bundles
- **WHEN** the CLI is updated
- **THEN** the skill is updated too, "Updated teamcity to 1.7.0 and its agent skill to 1.5.0"
  [@test] crates/cli-tools/src/install_test.rs (`an_install_with_the_skill_brings_it_and_one_without_brings_none`)

### Requirement: Install and update: A skill the user removed stays removed

A skill the user removed MUST stay removed on an update.

#### Scenario: An update without a skill of the agents
- **GIVEN** the agents have no skill of the CLI
- **WHEN** the CLI is updated
- **THEN** no skill comes along
  [@test] crates/cli-tools/src/install_test.rs (`installs_and_updates_through_homebrew_streaming_its_lines`)

### Requirement: Agent skill: A missing CLI offers its skill with the install, checked by default

Each CLI bundles a skill that teaches the agents to use it. A missing CLI MUST have "Also install the agent skill
teamcity-cli" under its purpose, checked by default for both (the agents are what the CLIs are here for), kept while
the app runs and never a setting.

#### Scenario: The checkbox of a missing CLI
- **WHEN** a CLI is missing
- **THEN** "Also install the agent skill teamcity-cli" stands under its purpose, checked
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should install a missing CLI through Homebrew with its agent skill, then show its version and its sign-in`)

#### Scenario: The checkbox turned off
- **GIVEN** the user turned the checkbox off
- **WHEN** Install runs
- **THEN** the CLI is installed without its skill
- **AND** the checkbox stays off while the app runs, and is never a setting
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should install a CLI without its skill once its checkbox is off`)

### Requirement: Agent skill: Install runs the install of the skill after the CLI

With the checkbox on, Install MUST run `<tool> skill install <skill>` after the CLI ("Installed teamcity 1.6.0 and its
agent skill").

#### Scenario: An install with the skill
- **GIVEN** the checkbox is on
- **WHEN** Install runs
- **THEN** `<tool> skill install <skill>` runs after the CLI, "Installed teamcity 1.6.0 and its agent skill"
  [@test] crates/cli-tools/src/install_test.rs (`an_install_with_the_skill_brings_it_and_one_without_brings_none`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should install a missing CLI through Homebrew with its agent skill, then show its version and its sign-in`)

### Requirement: Agent skill: A skill that fails is its own error

A skill that fails MUST be its own error beside the toast of the CLI ("Cannot install the teamcity agent skill"); the
line of the skill offers it again.

#### Scenario: The CLI installs, its skill fails
- **WHEN** the CLI installs and its skill fails
- **THEN** "Cannot install the teamcity agent skill" stands beside the toast of the CLI
- **AND** the line of the skill offers it again

### Requirement: Agent skill: An installed CLI has the line of its skill

An installed CLI MUST have the line of its skill: "Agent skill teamcity-cli 1.3.0 · teamcity has 1.4.0" with Update
Skill and Remove Skill, "Agent skill teamcity-cli 1.4.0" with Remove Skill, "Agent skill youtrack-cli · not installed"
with Install Skill; its tooltip names where the skill lives.

#### Scenario: Not installed, current, older
- **WHEN** the agents have no skill of the CLI, then the bundled one, then an older one
- **THEN** the line reads "Agent skill youtrack-cli · not installed" with Install Skill, "Agent skill teamcity-cli 1.4.0" with Remove Skill, "Agent skill teamcity-cli 1.3.0 · teamcity has 1.4.0" with Update Skill and Remove Skill
- **AND** its tooltip names where the skill lives
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should update a CLI to its newest release, and install, update and remove its skill from the line of the skill`)

### Requirement: Agent skill: The CLI finds the agents of the machine itself

The skill actions MUST run through the CLI, which finds the agents of the machine itself (`skill install|update|remove <skill>`).

#### Scenario: Install, update and remove the skill
- **WHEN** the line of the skill installs, updates or removes it
- **THEN** the app runs `<tool> skill install|update|remove <skill>`
  [@test] crates/cli-tools/src/install_test.rs (`installs_updates_and_removes_the_skill_of_each_cli`)

### Requirement: Agent skill: Remove needs no confirmation, its toast has Undo

Remove MUST need no confirmation: its toast has Undo, which installs it again.

#### Scenario: Remove Skill, then Undo
- **WHEN** the user clicks Remove Skill
- **THEN** the skill is removed without a confirmation
- **AND** Undo of its toast installs it again
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should update a CLI to its newest release, and install, update and remove its skill from the line of the skill`)

### Requirement: Agent skill: A skill action shows its progress on its line

A skill action MUST show its progress on the line of the skill.

#### Scenario: A skill action runs
- **WHEN** a skill action runs
- **THEN** its progress shows on the line of the skill

### Requirement: Agent skill: The row shows the running action with Cancel

The row MUST show the last line of the running action and Cancel.

#### Scenario: An action runs
- **WHEN** an action runs
- **THEN** the row shows each line as it comes, the last one standing, and Cancel
  [@test] crates/sync-utils/src/child_process_test.rs (`hands_over_each_line_as_it_comes_and_returns_the_output`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should stop a running install on Cancel, without an error`)

### Requirement: Agent skill: Cancel ends the whole process group and toasts nothing

Cancel MUST end the action's whole process group (SIGTERM, SIGKILL after 3 s), so the Ruby of `brew` holds no lock, and
toast nothing.

#### Scenario: Cancel of a running install
- **WHEN** the user clicks Cancel of a running `brew` install
- **THEN** its whole process group ends (SIGTERM, SIGKILL after 3 s)
- **AND** no toast shows
  [@test] crates/sync-utils/src/child_process_test.rs (`a_cancel_ends_the_whole_process_group`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should stop a running install on Cancel, without an error`)

### Requirement: Agent skill: One action per tool, at most 10 minutes

An action MUST run at most 10 minutes; one action per tool at a time; the other buttons of the row wait for it.

#### Scenario: An action runs
- **WHEN** an action of a tool runs
- **THEN** no other action of the tool starts and the other buttons of the row wait for it
- **AND** it ends after 10 minutes at most

### Requirement: Agent skill: The pipes of an exited child are read half a second more

Once the child exited its pipes MUST be read half a second more, not to their end: a child another thread spawned
meanwhile may have inherited them (a process group makes std fork on macOS) and would hold them open as long as it
runs.

#### Scenario: A neighbour holds the pipes
- **GIVEN** a child another thread spawned holds the pipes of the action's child
- **WHEN** the action's child exits
- **THEN** its pipes are read half a second more, and the action does not wait for the neighbour
  [@test] crates/sync-utils/src/child_process_test.rs (`a_child_that_exited_does_not_wait_for_a_neighbour_that_holds_its_pipes`)

### Requirement: Agent skill: The version is read again after an action

The version MUST be read again after an action: an update that installed nothing newer fails with "Homebrew has no
teamcity 1.7.0 yet: teamcity stays at 1.6.0".

#### Scenario: The tap has no newer release yet
- **WHEN** an update installed nothing newer
- **THEN** it fails with "Homebrew has no teamcity 1.7.0 yet: teamcity stays at 1.6.0"
  [@test] crates/cli-tools/src/install_test.rs (`an_update_that_installs_nothing_new_says_the_tap_has_none_yet`)

### Requirement: Agent skill: A failed brew names its reason

A failed `brew` MUST name what the user can act on (another brew holds its lock, the prefix is not writable), else its
`Error:` line.

#### Scenario: Another brew holds the lock
- **WHEN** `brew` fails as another brew holds its lock, or the prefix is not writable
- **THEN** the error names that
- **AND** another failure names its `Error:` line
  [@test] crates/cli-tools/src/install_test.rs (`a_failed_brew_names_its_reason`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should name why brew failed`)

### Requirement: Agent skill: What an action did is a toast of the Settings window

What an action did MUST be a toast of the Settings window: "Installed teamcity 1.6.0", "Updated youtrack to 0.7.0",
"Installed the teamcity agent skill 1.4.0", "Updated the teamcity agent skill to 1.5.0", "Removed the teamcity agent
skill"; a failure "Cannot install youtrack", "Cannot remove the teamcity agent skill" with the reason.

#### Scenario: An action succeeds
- **WHEN** an action succeeds
- **THEN** the Settings window toasts what it did: "Installed teamcity 1.6.0", "Updated youtrack to 0.7.0", "Installed the teamcity agent skill 1.4.0", "Updated the teamcity agent skill to 1.5.0", "Removed the teamcity agent skill"
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should update a CLI to its newest release, and install, update and remove its skill from the line of the skill`)

#### Scenario: An action fails
- **WHEN** an action fails
- **THEN** the toast says "Cannot install youtrack" or "Cannot remove the teamcity agent skill" with the reason
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should name why brew failed`)

### Requirement: Agent skill: After an action of teamcity the robot's prompt looks for the CLI again

After an action of `teamcity` the prompt of a failed robot MUST look for the CLI again.

#### Scenario: teamcity installed while the prompt shows
- **WHEN** an action of `teamcity` ends
- **THEN** the prompt of a failed robot looks for the CLI again

### Requirement: Sign-in: Sign In of teamcity starts the browser sign-in of the CLI

Sign In of `teamcity` MUST start the CLI's own browser sign-in on every OS, `teamcity auth login --server <url> --web
--json --no-input`: the CLI opens the browser for its PKCE flow and keeps its token in its own keychain item; the app
never sees the token and passes none. The CLI needs no terminal, and its stdin stays empty.

#### Scenario: Sign In of teamcity
- **WHEN** the user clicks Sign In of `teamcity` with a server
- **THEN** the app runs `teamcity auth login --server <url> --web --json --no-input` without a terminal, its stdin empty
- **AND** the app passes no token and sees none
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`signs_in_through_the_browser_login_of_the_cli_without_a_terminal_and_names_the_user`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign teamcity in through its own browser sign-in, to the server an open names`)

### Requirement: Sign-in: The stdout of the sign-in is one JSON line per event

The stdout of the sign-in MUST be read as one JSON line per event: `authorize` with the page it opened, then `login`
with the user.

#### Scenario: The events of the browser sign-in
- **WHEN** the CLI writes `authorize`, then `login`
- **THEN** the page it opened and the user are read from them
  [@test] crates/cli-tools/src/parse_test.rs (`reads_the_events_of_the_browser_sign_in_of_teamcity`)

### Requirement: Sign-in: The row shows the sign-in with Open in Browser and Cancel

The row MUST show the last line that is not JSON and "Open in Browser" with the authorize page, for a browser that
didn't come up; it waits 6 minutes, longer than the 5 of the CLI, and Cancel ends it.

#### Scenario: The browser did not come up
- **GIVEN** a sign-in of `teamcity` runs
- **WHEN** the CLI wrote its `authorize` event
- **THEN** the row shows the last line that is not JSON and "Open in Browser" with the authorize page
- **AND** it waits 6 minutes, and Cancel ends it

### Requirement: Sign-in: An exit without the login event signs nobody in

An exit without the `login` event MUST sign nobody in.

#### Scenario: The CLI ends without the login event
- **WHEN** the CLI exits without the `login` event
- **THEN** nobody is signed in
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`an_end_without_the_login_event_signed_nobody_in`)

### Requirement: Sign-in: The toast names the user of the login event

The toast MUST name the user of the `login` event, "Signed in to ci.example as jane", and the status is read again.

#### Scenario: A sign-in succeeds
- **WHEN** the CLI wrote the `login` event and exited
- **THEN** the toast says "Signed in to ci.example as jane"
- **AND** the status is read again, the server line naming the user
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`signs_in_through_the_browser_login_of_the_cli_without_a_terminal_and_names_the_user`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign teamcity in through its own browser sign-in, to the server an open names`)

### Requirement: Sign-in: A failure is the message of the JSON envelope of the CLI

A failure MUST be the `message` of the JSON envelope the CLI writes to stderr (`{"error":{"code","message","suggestion"}}`).

#### Scenario: A denied sign-in
- **WHEN** the CLI writes lines, then its JSON error to stderr
- **THEN** the failure is the `message` of the error, the lines before it without the JSON
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`a_denied_sign_in_says_the_message_of_the_error_of_the_cli`)
  [@test] crates/cli-tools/src/parse_test.rs (`reads_the_message_of_the_json_error_of_a_cli_after_the_lines_before_it`)

### Requirement: Sign-in: A TeamCity before 2026.1 fails at once

A TeamCity before 2026.1 has no browser sign-in and MUST fail at once: "Cannot sign teamcity in: browser sign-in is
not available on https://old.example".

#### Scenario: A server without the browser sign-in
- **WHEN** Sign In runs on a TeamCity before 2026.1
- **THEN** it fails at once: "Cannot sign teamcity in: browser sign-in is not available on https://old.example"
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`a_server_without_the_browser_sign_in_fails_with_the_words_of_the_cli`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line`)

### Requirement: Sign-in: A teamcity before 1.6.2 updates first, else signs in in a terminal

`--web` came with `teamcity` 1.6.2. An older one signed in to no server MUST offer Update while a newer release is
known, else `teamcity auth login` to run in a terminal.

#### Scenario: An older teamcity, a newer release known, then none
- **GIVEN** a `teamcity` before 1.6.2 signed in to no server
- **WHEN** a newer release is known
- **THEN** it offers Update
- **AND** without a newer release, `teamcity auth login` to run in a terminal
  [@test] crates/cli-tools/src/status_test.rs (`a_teamcity_before_its_browser_sign_in_without_a_terminal_updates_first_else_signs_in_in_a_terminal`)
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should offer an older teamcity its update before the sign-in, else the sign-in of a terminal`)

### Requirement: Sign-in: Sign In Again of an older teamcity fails before the CLI runs

Sign In Again of a `teamcity` before 1.6.2 MUST fail before the CLI runs: "teamcity 1.6.1 signs in only in a
terminal: update it to 1.6.2 or later, or run teamcity auth login".

#### Scenario: Sign In Again of teamcity 1.6.1
- **WHEN** the user clicks Sign In Again of a `teamcity` 1.6.1
- **THEN** it fails before the CLI runs: "teamcity 1.6.1 signs in only in a terminal: update it to 1.6.2 or later, or run teamcity auth login"
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`a_teamcity_before_its_browser_sign_in_without_a_terminal_is_not_run`)

### Requirement: Sign-in: The server is the one an open of Settings names

The server MUST be the one an open of Settings names (`SettingsTarget.server`: the server of a robot's build, from the
toast of a copied prompt), else the first one the CLI knows, else typed.

#### Scenario: An open of Settings names the server
- **WHEN** Settings opens with `SettingsTarget.server`, from the toast of a copied prompt
- **THEN** the sign-in is to that server
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign teamcity in through its own browser sign-in, to the server an open names`)

#### Scenario: No server named
- **WHEN** no open names a server
- **THEN** the server is the first one the CLI knows

#### Scenario: A typed server
- **WHEN** neither names a server
- **THEN** the user types it
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line`)

### Requirement: Sign-in: The server is an http or https URL

The server of a sign-in MUST be an `http://` or `https://` URL, trimmed and without its trailing `/`: Sign In stays
disabled until the field holds one, and a sign-in to another server fails before the CLI runs: "the URL of the
TeamCity server starts with https://".

#### Scenario: A server without its scheme
- **WHEN** a sign-in of `teamcity` to `ci.example` runs
- **THEN** it fails before the CLI runs: "the URL of the TeamCity server starts with https://"
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`a_sign_in_needs_the_url_of_the_server`)

#### Scenario: A server with a trailing slash
- **WHEN** a sign-in of `teamcity` to `https://ci.example/` runs
- **THEN** the CLI runs with `--server https://ci.example`
  [@test] crates/cli-tools/src/teamcity_login_test.rs (`signs_in_through_the_browser_login_of_the_cli_without_a_terminal_and_names_the_user`)

#### Scenario: An empty field
- **GIVEN** no open names a server and the CLI knows none
- **WHEN** the field of the server is empty
- **THEN** Sign In is disabled
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line`)

### Requirement: Sign-in: Each server line has Sign In Again

Each server line MUST have Sign In Again (a token that expires or was refused).

#### Scenario: Sign In Again from a server's line
- **GIVEN** a server line of `teamcity`
- **WHEN** the user clicks its Sign In Again
- **THEN** the browser sign-in of the CLI runs on that server
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line`)

### Requirement: Sign-in: youtrack offers Use Branch Deck Sign-In

`youtrack` signed in to no instance, with the app signed in to a YouTrack, MUST offer Use Branch Deck Sign-In
(`signInWith`): one browser sign-in, after which the CLI has a permanent token of that YouTrack
([YouTrack](../youtrack/spec.md#the-youtrack-cli)).

#### Scenario: Use Branch Deck Sign-In
- **GIVEN** the app is signed in to a YouTrack and `youtrack` to no instance
- **WHEN** the user clicks Use Branch Deck Sign-In
- **THEN** after one browser sign-in the CLI has a permanent token of that YouTrack
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign youtrack in with a permanent token of the app's YouTrack sign-in, through one browser sign-in`)

### Requirement: Sign-in: Use Branch Deck Sign-In is the sign-in of the app's commands

It MUST be the sign-in of the app's commands, `sign_in({ provider: "youTrackCli", url }, channel)`
(`signInYoutrackWithApp` of `src/stores/cli-tools.ts`): the row waits with "Waiting for the browser…" and Open in
Browser once the channel tells the Hub page (`browser`), which opens in the browser, then "Signing youtrack in…" while
the token is made and handed over; Cancel is `cancel_sign_in`.

#### Scenario: The row while the sign-in runs
- **WHEN** Use Branch Deck Sign-In runs
- **THEN** the row waits with "Waiting for the browser…" and Open in Browser once the channel tells the Hub page (`browser`)
- **AND** then "Signing youtrack in…" while the token is made and handed over
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should sign youtrack in with a permanent token of the app's YouTrack sign-in, through one browser sign-in`)
  [@test] crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)

#### Scenario: Cancel
- **WHEN** the user clicks Cancel
- **THEN** the app calls `cancel_sign_in`

### Requirement: Sign-in: A failed youtrack sign-in names its reason, a cancel nothing

A failure MUST be "Cannot sign youtrack in" with the reason; a cancel says nothing.

#### Scenario: The sign-in fails or is cancelled
- **WHEN** the sign-in of `youtrack` fails
- **THEN** the toast says "Cannot sign youtrack in" with the reason
- **AND** a cancel says nothing

### Requirement: Sign-in: Without the app's sign-in youtrack shows its terminal sign-in

Without the app's sign-in it MUST show `youtrack auth login` to run in a terminal.

#### Scenario: The app is signed in to no YouTrack
- **GIVEN** the app is signed in to no YouTrack
- **WHEN** `youtrack` is signed in to no instance
- **THEN** the row shows `youtrack auth login` to run in a terminal

### Requirement: Sign-in: A read asked while one runs runs once more after it

A read of the status asked while one runs MUST run once more after it: the one running may have started before the
change. Rust decides it for every window (`sync_utils::single_flight::NextFlight`): the windows that ask while a read
runs share one read that starts once it ended, also a window that comes to it only after it ended.

#### Scenario: Windows ask while a read runs
- **GIVEN** a read of the status runs
- **WHEN** windows ask for the status
- **THEN** they share one read that starts once it ended
  [@test] crates/app-api/src/app/cli_tools_test.rs (`the_tools_are_found_while_the_releases_are_asked_and_windows_that_ask_meanwhile_share_the_read_after_it`)
  [@test] crates/sync-utils/src/single_flight_test.rs (`callers_that_ask_while_a_read_runs_share_the_read_after_it`, `a_follow_up_whose_leader_went_away_is_read_by_a_caller_that_waited`)

#### Scenario: A window comes after the follow-up ended
- **WHEN** a window comes to the follow-up only after it ended
- **THEN** it takes the answer of that read
  [@test] crates/sync-utils/src/single_flight_test.rs (`a_caller_that_comes_to_the_follow_up_after_it_ended_takes_its_answer`)

### Requirement: Where it is: Settings → Tools, its first row brd, then teamcity and youtrack

The tools MUST stand in Settings → Tools, its first row `brd`, whose skill coding agents read (installed, with the
path of brd, else Install Agent CLI), then teamcity and youtrack.

#### Scenario: The rows of Tools
- **WHEN** the user opens Settings → Tools
- **THEN** the first row is `brd`, installed with the path of brd, else Install Agent CLI
- **AND** teamcity and youtrack follow it
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should install the agent skill from the page, which Tools then names`)

### Requirement: Where it is: The page reads the status on each open and focus

The page MUST read the status on each open and focus and show the last answer meanwhile.

#### Scenario: The page opens or takes the focus
- **WHEN** the page opens or takes the focus
- **THEN** it reads the status, showing the last answer meanwhile

### Requirement: Where it is: The page fits 880 × 640 in its tallest state

The page MUST fit 880 × 640 in its tallest state (a CLI on two servers, the command of the other).

#### Scenario: A CLI on two servers, the command of the other
- **GIVEN** a CLI on two servers and the command of the other
- **WHEN** the page shows
- **THEN** it fits 880 × 640 without scrolling
  [@test] tests/e2e/settings/settings.spec.ts (`should fit every page at 880 × 640 without scrolling`)

## User Experience

- A row: the command name in mono, its version and path (`~` for the home), the button of its action at the end; under
  it what the tool is for, a line per server ("buildserver.labs.intellij.net · jane.doe · expires Nov 21"), the
  running line or the command with Copy, and a problem of the read in `text-warning`.

## Testing / Local Run

- `cargo nextest run -p cli-tools -p sync-utils`: the fakes of `test_utils::fake_cli_tools` (`teamcity`, `youtrack`,
  `brew` in a directory of their own, their argv in a log).
- `pnpm e2e tests/e2e/settings/tools-settings.spec.ts`: the test server runs the real cores on the fakes of the test
  repository (`POST /repositories/{id}/cli-tools`, its log by `GET`); no test runs the tools of the machine.

## Open Questions / Risks

- The newest release comes from GitHub once a day ([Setup Dot](../setup-attention/spec.md#items)); a tap that lags
  behind it makes an update say so instead of looping.
- Windows: `youtrack` installs through scoop only, until winget has it. The sign-in of `teamcity` runs the same command
  as on macOS; only the fakes check it there. `the_package_manager_of_the_machine_installs_teamcity` is ignored: it installs for real, in
  the check on Windows ([Platform Checks](../../../../docs/contributing/platform-checks.md)).
