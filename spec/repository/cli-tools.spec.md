---
name: Command-Line Tools
description: Requirements for the teamcity and youtrack CLIs in Settings → Tools - the binary, its version and the servers it is signed in to, the agent skill each bundles (installed with the CLI by default, installed, updated and removed from its line), and the one action Rust picks for each row (install and update through Homebrew, winget or scoop, the sign-in), with the command for a terminal where the app doesn't run it.
style: plain-1
targets:
  - ../../crates/cli-tools/src/environment.rs
  - ../../crates/cli-tools/src/parse.rs
  - ../../crates/cli-tools/src/status.rs
  - ../../crates/cli-tools/src/install.rs
  - ../../crates/cli-tools/src/package_manager.rs
  - ../../crates/cli-tools/src/terminal_login.rs
  - ../../crates/sync-utils/src/child_process.rs
  - ../../crates/sync-utils/src/known_tools.rs
  - ../../crates/domain-types/src/cli_tools.rs
  - ../../crates/test-utils/src/fake_cli_tools.rs
  - ../../crates/test-server/src/cli_tools_fixtures.rs
  - ../../crates/app-api/src/app/cli_tools.rs
  - ../../crates/test-server/src/host.rs
  - ../../src-tauri/src/services.rs
  - ../../src/stores/cli-tools.ts
  - ../../src/lib/sign-in-run.ts
  - ../../crates/remote-facts/src/sign_in.rs
  - ../../src/lib/cli-tools.ts
  - ../../src/components/settings/ToolsPage.tsx
  - ../../src/components/settings/AgentsPage.tsx
---

# Command-Line Tools

Status: Implemented
Date: 2026-10-23

## Summary

Two CLIs of the team do what Branch Deck and the agents need beside git: `teamcity` (JetBrains/teamcity-cli) reads the
logs and tests of the builds of a robot, for the prompt of a failed robot ([Safe Push Failure
Prompt](../branch/safe-push-failure-prompt.spec.md)) and for an agent; `youtrack` (JetBrains/youtrack-cli) reads and
changes the issues of the commits. Settings → Tools shows each: where it is, its version, the servers it is signed in
to, the agent skill it bundles, and the one thing it needs next, which the app does itself where it can (install and
update through Homebrew, winget or scoop, the sign-in of `teamcity`) and otherwise names as a command for a terminal.

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
- A read runs `--version`, `auth status --json` and `skill list`, each in its own thread, both tools
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
- The skill of the agents (`teamcity-cli`, `youtrack-cli`) is the `version:` of the oldest `<skill>/SKILL.md` in
  `~/.claude/skills` and `~/.agents/skills`, against the version of `<tool> skill list`; Rust says what its line offers
  besides Remove: Install Skill while the agents have none, Update Skill while theirs is older.
  [@test] ../../crates/cli-tools/src/parse_test.rs (`reads_the_version_of_a_skill_and_of_the_skill_list`)
  [@test] ../../crates/cli-tools/src/status_test.rs (`the_skill_of_each_cli_is_the_oldest_copy_of_the_agents_against_the_bundled_one_beside_the_action`)

### The action of a row

- Rust picks one action per tool, the most needed first: Install (not found), Sign In (signed in to no server),
  Update (a newer release than the installed one); none when all is well. The skill has a line of its own.
  [@test] ../../crates/cli-tools/src/status_test.rs (`an_installed_cli_has_its_version_and_servers_and_offers_its_sign_in_first`)
- The app runs the action where it can, else the row shows the command for a terminal with Copy: an install without
  Homebrew (the README's command), a sign-in, an update of a `youtrack` outside Homebrew (`youtrack update`, which
  upgrades only in a terminal).
  [@test] ../../crates/cli-tools/src/status_test.rs (`a_missing_cli_offers_its_install_through_homebrew_or_a_command`, `youtrack_outside_homebrew_updates_in_a_terminal`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should give the command to install a CLI without Homebrew`)

### Install and update

- Install is `brew install jetbrains/utils/<tool>`, Update of a tool Homebrew installed (its binary links into a
  `Cellar`) `brew upgrade`, with `NONINTERACTIVE`, no hints, cleanup or dependents check, its taps updated first (the
  tap of a tool lags behind its releases otherwise). A `teamcity` no manager installed updates itself
  (`teamcity update --yes`).
- On Windows the manager is winget for a tool it has a package of (`JetBrains.TeamCityCLI`; `youtrack` has none in
  winget yet): `winget install --id … --exact --scope user --silent --disable-interactivity
  --accept-source-agreements --accept-package-agreements`, `winget upgrade` likewise; else scoop, which adds the
  `jetbrains` bucket (`https://github.com/JetBrains/scoop-utils`, a failure there counts nothing: it may be added
  already), then `scoop install jetbrains/<tool>`, `scoop update <tool>`. choco needs an administrator and is not run.
  Without either the row shows the README's command (`winget install JetBrains.TeamCityCLI`, `irm
  https://jb.gg/youtrack-cli/install.ps1 | iex`). Which manager installed a CLI is told by its path (`Cellar`,
  `WinGet`, `scoop`); winget's links, scoop's shims and `~/.local/bin` (the install script) are looked in after `PATH`,
  as the `PATH` of a running app lacks what was installed after it started.
  [@test] ../../crates/cli-tools/src/package_manager_test.rs (`winget_installs_for_the_user_without_prompts_and_scoop_adds_its_bucket_first`, `the_path_of_a_binary_tells_its_manager_when_the_machine_has_it`)
  [@test] ../../crates/cli-tools/src/install_test.rs (`the_package_manager_of_the_machine_installs_teamcity`)
  [@test] ../../crates/cli-tools/src/install_test.rs (`installs_and_updates_through_homebrew_streaming_its_lines`, `a_teamcity_outside_homebrew_updates_itself`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should install a missing CLI through Homebrew with its agent skill, then show its version and its sign-in`)
- An update also updates an older skill of the agents ("Updated teamcity to 1.7.0 and its agent skill to 1.5.0"); a
  skill the user removed stays removed.
  [@test] ../../crates/cli-tools/src/install_test.rs (`an_install_with_the_skill_brings_it_and_one_without_brings_none`)

### Agent skill

- Each CLI bundles a skill that teaches the agents to use it. A missing CLI has "Also install the agent skill
  teamcity-cli" under its purpose, checked by default for both (the agents are what the CLIs are here for), kept while
  the app runs and never a setting: Install then runs `<tool> skill install <skill>` after the CLI ("Installed teamcity
  1.6.0 and its agent skill"). A skill that fails is its own error beside the toast of the CLI ("Cannot install the
  teamcity agent skill"); the line of the skill offers it again.
  [@test] ../../crates/cli-tools/src/install_test.rs (`an_install_with_the_skill_brings_it_and_one_without_brings_none`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should install a missing CLI through Homebrew with its agent skill, then show its version and its sign-in`, `should install a CLI without its skill once its checkbox is off`)
- An installed CLI has the line of its skill: "Agent skill teamcity-cli 1.3.0 · teamcity has 1.4.0" with Update Skill
  and Remove Skill, "Agent skill teamcity-cli 1.4.0" with Remove Skill, "Agent skill youtrack-cli · not installed"
  with Install Skill; its tooltip names where the skill lives. The CLI finds the agents of the machine itself
  (`skill install|update|remove <skill>`). Remove needs no confirmation: its toast has Undo, which installs it again.
  A skill action shows its progress on that line.
  [@test] ../../crates/cli-tools/src/install_test.rs (`installs_updates_and_removes_the_skill_of_each_cli`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should update a CLI to its newest release, and install, update and remove its skill from the line of the skill`)
- The row shows the last line of the running action and Cancel; Cancel ends the action's whole process group (SIGTERM,
  SIGKILL after 3 s), so the Ruby of `brew` holds no lock, and toasts nothing. An action runs at most 10 minutes; one
  action per tool at a time; the other buttons of the row wait for it. Once the child exited its pipes are read half a
  second more, not to their end: a child another thread spawned meanwhile may have inherited them (a process group
  makes std fork on macOS) and would hold them open as long as it runs.
  [@test] ../../crates/sync-utils/src/child_process_test.rs (`hands_over_each_line_as_it_comes_and_returns_the_output`, `a_cancel_ends_the_whole_process_group`, `a_child_that_exited_does_not_wait_for_a_neighbour_that_holds_its_pipes`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should stop a running install on Cancel, without an error`)
- The version is read again after an action: an update that installed nothing newer fails with "Homebrew has no
  teamcity 1.7.0 yet: teamcity stays at 1.6.0". A failed `brew` names what the user can act on (another brew holds
  its lock, the prefix is not writable), else its `Error:` line.
  [@test] ../../crates/cli-tools/src/install_test.rs (`an_update_that_installs_nothing_new_says_the_tap_has_none_yet`, `a_failed_brew_names_its_reason`, `without_a_package_manager_the_install_is_the_users`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should name why brew failed`)
- What an action did is a toast of the Settings window: "Installed teamcity 1.6.0", "Updated youtrack to 0.7.0",
  "Installed the teamcity agent skill 1.4.0", "Updated the teamcity agent skill to 1.5.0", "Removed the teamcity agent
  skill"; a failure "Cannot install youtrack", "Cannot remove the teamcity agent skill" with the reason. After an
  action of `teamcity` the prompt of a failed robot looks for the CLI again.

### Sign-in

- Sign In of `teamcity` (macOS, Linux) starts the CLI's own browser sign-in, `teamcity auth login --server <url>`: the
  CLI opens the browser for its PKCE flow and keeps its token in its own keychain item; the app never sees the token
  and passes none. The CLI signs in through the browser only with a terminal on stdin, so `script` runs it in a
  pseudo-terminal (BSD `script -q /dev/null …`, util-linux `script -q -e -c …`), 4096 columns wide, without escape
  sequences; its stdin stays empty. The row shows the last line and "Open in Browser" with the authorize page the CLI
  printed, for a browser that didn't come up; it waits 5 minutes, Cancel ends it. Then the status is read again and
  the toast says "Signed in to ci.example as jane".
  [@test] ../../crates/cli-tools/src/terminal_login_test.rs (`signs_in_through_the_browser_login_of_the_cli_in_a_terminal_and_names_the_user`, `a_denied_sign_in_says_the_last_words_of_the_cli`, `a_terminal_line_loses_its_escapes_and_the_echo_of_the_end_of_input`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should sign teamcity in through its own browser sign-in, to the server an open names`)
- Only TeamCity 2026.1 and later, which sign in through the browser: a CLI that asks to paste a token is ended at once,
  "Cannot sign teamcity in: https://old.example has no browser sign-in (TeamCity 2026.1+)".
  [@test] ../../crates/cli-tools/src/terminal_login_test.rs (`a_server_without_the_browser_sign_in_fails_at_once_instead_of_waiting_for_a_token`, `a_sign_in_needs_the_url_of_the_server`)
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line`)
- The server is the one an open of Settings names (`SettingsTarget.server`: the server of a robot's build, from the toast
  of a copied prompt), else the first one the CLI knows, else typed; each server line has Sign In Again (a token that
  expires or was refused). Windows, without `script`, shows `teamcity auth login` to run in a terminal.
- `youtrack` signed in to no instance, with the app signed in to a YouTrack, offers Use Branch Deck Sign-In
  (`signInWith`): one browser sign-in, after which the CLI has a permanent token of that YouTrack
  ([YouTrack](youtrack.spec.md#the-youtrack-cli)). It is the sign-in of the app's commands, `sign_in({ provider:
  "youTrackCli", url }, channel)` (`signInYoutrackWithApp` of `src/stores/cli-tools.ts`): the row waits with
  "Waiting for the browser…" and Open in Browser once the channel tells the Hub page (`browser`), which opens in the
  browser, then "Signing youtrack in…" while the token is made and handed over; Cancel is `cancel_sign_in`. A failure is
  "Cannot sign youtrack in" with the reason, a cancel says nothing. Without the app's sign-in it shows `youtrack auth
  login` to run in a terminal.
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should sign youtrack in with a permanent token of the app's YouTrack sign-in, through one browser sign-in`)
  [@test] ../../crates/remote-facts/src/sign_in_test.rs (`a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first`)
- A read of the status asked while one runs runs once more after it: the one running may have started before the
  change.

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

- The newest release comes from GitHub once a day ([Setup Dot](setup-attention.spec.md#items)); a tap that lags
  behind it makes an update say so instead of looping.
- Windows: no sign-in of `teamcity` starts (no `script`), the row shows its command; `youtrack` installs through scoop
  only, until winget has it. `the_package_manager_of_the_machine_installs_teamcity` is ignored: it installs for real, in
  the check on Windows ([Platform Checks](../../docs/guidelines/platform-checks.md)).
