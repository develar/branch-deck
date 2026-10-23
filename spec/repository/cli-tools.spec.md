---
name: Command-Line Tools
description: Requirements for the teamcity and youtrack CLIs in Settings → Tools - the binary, its version and the servers it is signed in to, the TeamCity skill of the agents, and the one action Rust picks for each row (install and update through Homebrew, the skill), with the command for a terminal where the app doesn't run it.
style: plain-1
targets:
  - ../../crates/cli-tools/src/environment.rs
  - ../../crates/cli-tools/src/parse.rs
  - ../../crates/cli-tools/src/status.rs
  - ../../crates/cli-tools/src/install.rs
  - ../../crates/sync-utils/src/child_process.rs
  - ../../crates/sync-utils/src/known_tools.rs
  - ../../crates/domain-types/src/cli_tools.rs
  - ../../crates/test-utils/src/fake_cli_tools.rs
  - ../../crates/test-server/src/cli_tools_fixtures.rs
  - ../../src-tauri/src/commands/cli_tools.rs
  - ../../src-tauri/src/commands/patronus.rs
  - ../../src/stores/cli-tools.ts
  - ../../src/lib/cli-tools.ts
  - ../../src/components/settings/ToolsPage.tsx
  - ../../src/components/settings/AgentsPage.tsx
---

# Command-Line Tools

Status: In progress
Date: 2026-10-23

## Summary

Two CLIs of the team do what Branch Deck and the agents need beside git: `teamcity` (JetBrains/teamcity-cli) reads the
logs and tests of the builds of a robot, for the prompt of a failed robot ([Safe Push Failure
Prompt](../branch/safe-push-failure-prompt.spec.md)) and for an agent; `youtrack` (JetBrains/youtrack-cli) reads and
changes the issues of the commits. Settings → Tools shows each: where it is, its version, the servers it is signed in
to, the skill of the agents (`teamcity`), and the one thing it needs next, which the app does itself where it can
(install and update through Homebrew, the skill) and otherwise names as a command for a terminal.

## Goals

- Install and update both CLIs without a terminal.
- See at a glance whether a CLI the app and the agents rely on is there, current and signed in.
- One status for every surface: Rust decides the action of a row; the page only names it.

## Non-goals

- Reading the token a CLI keeps: each keeps its own sign-in in its keychain item, which the app never reads.
- Running an install script (`curl | bash`, `irm | iex`): the app runs a package manager or shows the command.
- A CLI without a use in Branch Deck: a row is added when the app or its agent skill relies on the tool.

## Requirements

### Status

- Each tool is looked for as the app looks for its other CLIs (`sync_utils::known_tools`): `PATH`, then Homebrew's
  `bin` (Apple silicon, Intel, Linux) and `~/.local/bin` (the install scripts), then the `PATH` of the login shell, as
  an app started from the Finder has none of them; looked up on every read, so a CLI installed meanwhile is found.
  [@test] ../../crates/sync-utils/src/child_process_test.rs (`finds_a_tool_before_path_then_on_path_then_after_it`)
- A read runs `--version`, `auth status --json` and, for `teamcity`, `skill list`, each in its own thread, both tools
  at once, 20 s each; the CLIs run without colours, prompts or an update check, and without the `TEAMCITY_*` and
  `YOUTRACK_*` credential variables of the app's environment, so the status is what an agent's shell gets.
  [@test] ../../crates/cli-tools/src/status_test.rs (`reads_every_tool`)
- The version is the first word with a digit and a dot (`teamcity version 1.6.0`, `v0.7.0`); versions order by their
  numbers.
  [@test] ../../crates/cli-tools/src/parse_test.rs (`reads_the_version_of_each_cli`, `orders_versions_by_their_numbers`)
- A server of `teamcity auth status --json` is signed in (`authenticated`), a guest, or failed with the CLI's error;
  the entry of no server is no server; `token_expiry` is its expiry. `youtrack auth status --json` is one object or,
  with several instances, an array; `not configured` is no instance.
  [@test] ../../crates/cli-tools/src/parse_test.rs (`reads_the_servers_of_teamcity_with_their_user_and_expiry`, `reads_the_instances_of_youtrack_as_one_object_or_an_array`)
- The skill of the agents is the `version:` of the oldest `teamcity-cli/SKILL.md` in `~/.claude/skills` and
  `~/.agents/skills`, against the version of `teamcity skill list`.
  [@test] ../../crates/cli-tools/src/parse_test.rs (`reads_the_version_of_a_skill_and_of_the_skill_list`)
  [@test] ../../crates/cli-tools/src/status_test.rs (`the_teamcity_skill_is_behind_until_the_agents_have_the_bundled_version`)

### The action of a row

- Rust picks one action per tool, the most needed first: Install (not found), Sign In (signed in to no server),
  Update (a newer release than the installed one), Update Skill (`teamcity`: the agents have none, or an older one);
  none when all is well.
  [@test] ../../crates/cli-tools/src/status_test.rs (`an_installed_cli_has_its_version_and_servers_and_offers_its_sign_in_first`)
- The app runs the action where it can, else the row shows the command for a terminal with Copy: an install without
  Homebrew (the README's command), a sign-in, an update of a `youtrack` outside Homebrew (`youtrack update`, which
  upgrades only in a terminal).
  [@test] ../../crates/cli-tools/src/status_test.rs (`a_missing_cli_offers_its_install_through_homebrew_or_a_command`, `youtrack_outside_homebrew_updates_in_a_terminal`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should give the command to install a CLI without Homebrew`)

### Install and update

- Install is `brew install jetbrains/utils/<tool>`, Update of a tool Homebrew installed (its binary links into a
  `Cellar`) `brew upgrade`, with `NONINTERACTIVE`, no hints, cleanup or dependents check, its taps updated first (the
  tap of a tool lags behind its releases otherwise). A `teamcity` outside Homebrew updates itself
  (`teamcity update --yes`).
  [@test] ../../crates/cli-tools/src/install_test.rs (`installs_and_updates_through_homebrew_streaming_its_lines`, `a_teamcity_outside_homebrew_updates_itself`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should install a missing CLI through Homebrew, then show its version and how to sign it in`, `should update a CLI to its newest release, then the skill of the agents`)
- Update Skill is `teamcity skill install teamcity-cli` without a skill, `skill update` with one; the CLI finds the
  agents of the machine itself.
  [@test] ../../crates/cli-tools/src/install_test.rs (`installs_the_teamcity_skill_then_updates_it`)
- The row shows the last line of the running action and Cancel; Cancel ends the action's whole process group (SIGTERM,
  SIGKILL after 3 s), so the Ruby of `brew` holds no lock, and toasts nothing. An action runs at most 10 minutes; one
  action per tool at a time.
  [@test] ../../crates/sync-utils/src/child_process_test.rs (`hands_over_each_line_as_it_comes_and_returns_the_output`, `a_cancel_ends_the_whole_process_group`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should stop a running install on Cancel, without an error`)
- The version is read again after an action: an update that installed nothing newer fails with "Homebrew has no
  teamcity 1.7.0 yet: teamcity stays at 1.6.0". A failed `brew` names what the user can act on (another brew holds
  its lock, the prefix is not writable), else its `Error:` line.
  [@test] ../../crates/cli-tools/src/install_test.rs (`an_update_that_installs_nothing_new_says_the_tap_has_none_yet`, `a_failed_brew_names_its_reason`, `without_homebrew_the_install_is_the_users`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should name why brew failed`)
- What an action did is a toast of the Settings window: "Installed teamcity 1.6.0", "Updated youtrack to 0.7.0",
  "Updated the TeamCity skill of the agents to 1.4.0"; a failure "Cannot install youtrack" with the reason. After an
  action of `teamcity` the prompt of a failed robot looks for the CLI again.

### Where it is

- Settings → Tools, after Agents; Agents says "Agents also run teamcity and youtrack" with Open Tools.
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should open from the line of Settings → Agents`)
- The page reads the status on each open and focus and shows the last answer meanwhile; it fits 720 × 540 in its
  tallest state (a CLI on two servers, the command of the other).
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should fit every page at 720 × 540 without scrolling`)

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

- The newest release is known only to the fixture of the test server for now; the app learns it from the releases on
  GitHub in the Setup indicator's change.
- Windows: no package manager runs yet; the row shows the command.
