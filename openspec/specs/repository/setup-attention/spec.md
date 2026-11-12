---
name: Setup Dot
description: Requirements for the dot on the Settings button of the header - what of the setup of the machine waits for the user (a downloaded update of the app, a failed app update check, a newer CLI or skill, a CLI sign-in that expires or expired, the app's YouTrack sign-in that stopped), derived in Rust, each with its action and its skip, instead of a toast nobody asked for.
style: plain-1
targets:
  - crates/cli-tools/src/setup.rs
  - crates/cli-tools/src/kept_statuses.rs
  - crates/domain-types/src/setup.rs
  - crates/remote-facts/src/cli_tool_releases.rs
  - crates/app-api/src/app/cli_tools.rs
  - crates/app-api/src/transport.rs
  - src-tauri/src/auto_update.rs
  - crates/test-server/src/updater.rs
  - tests/e2e/helpers/test-repository.ts
  - crates/test-server/src/cli_tools_fixtures.rs
  - src/stores/setup.ts
  - src/lib/setup.ts
  - src/lib/setup-actions.ts
  - src/components/repository/SettingsButton.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/pages/SettingsPage.tsx
  - src/pages/IndexPage.tsx
  - src/hooks/useAutoUpdate.ts
  - src/stores/settings/appSettings.ts
---

# Setup Dot

Status: Implemented
Date: 2026-10-23

## Purpose

A toast says what just happened; a newer release nobody asked for is no event, and a sticky toast closed once is gone.
Activity is the history of what the app did. What is missing between them is the state of the setup that waits for
the user: an update of the app ready to install, a failed app update check, a newer `teamcity` or `youtrack`, a CLI sign-in that expires. That is
a dot on the Settings button of the header, as IntelliJ badges its gear for the updates of the IDE and its plugins; a
click lists each item with its action and its skip, then Settings.

## Goals

- See what of the setup needs the user, in every state of the window, and act on it in one click.
- Quiet when all is well: an item only for what the app or the agents rely on and the user can fix now.

## Non-goals

- A bell of notifications: Activity is the history, the toasts are the events, the Dock badge the work that waits.
- A tool that is not installed: the user may not want it; Settings → Tools offers it.
- Another native menu item: Help → Check for Updates… stays; with an update downloaded it toasts Install and Restart.

## Requirements

### Requirement: Items: Rust derives the items from explicit facts

Rust MUST derive the items (`get_setup_items`, `cli_tools::setup`) from explicit facts: the downloaded update or the
last failed check of the app, the last statuses of the CLIs with their newest releases, the app's YouTrack sign-in.

#### Scenario: The items of the facts
- **GIVEN** a downloaded update of the app, a CLI with a newer release and sign-ins, and the app's YouTrack sign-in that stopped working
- **WHEN** Rust derives the items
- **THEN** each of those facts is an item
  [@test] crates/cli-tools/src/setup_test.rs (`the_app_update_then_each_tool_then_the_youtrack_sign_in`)

### Requirement: Items: The app, then each tool, then YouTrack

The items MUST come in this order: the app, each tool, YouTrack.

#### Scenario: The order of the items
- **WHEN** the app, a tool and YouTrack each have an item
- **THEN** the item of the app comes first, then those of each tool, then YouTrack's
  [@test] crates/cli-tools/src/setup_test.rs (`the_app_update_then_each_tool_then_the_youtrack_sign_in`)

### Requirement: Items: A downloaded update takes the app's slot instead of a failed check

A downloaded update MUST take the app's slot instead of a failed check.

#### Scenario: A downloaded update and a failed check
- **GIVEN** a downloaded update of the app and a failed check
- **WHEN** the items are derived
- **THEN** the app's item is the downloaded update
  [@test] crates/cli-tools/src/setup_test.rs (`a_failed_app_update_check_is_the_app_item_unless_an_update_waits`)

### Requirement: Items: A failed app update check is the app's item

When the last check for app updates failed and no downloaded update waits, the app's item MUST say "Cannot check for
updates", show the error, and offer Check Again and Remind Tomorrow.

#### Scenario: A failed check without a downloaded update
- **GIVEN** the last check for app updates failed and no downloaded update waits
- **WHEN** the items are derived
- **THEN** the app's item says "Cannot check for updates" and shows the error
- **AND** it offers Check Again and Remind Tomorrow
  [@test] crates/cli-tools/src/setup_test.rs (`a_failed_app_update_check_is_the_app_item_unless_an_update_waits`)
  [@test] src/lib/setup.test.ts (`names each item, its action and its skip`)
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should show a failed update check and clear it when Check Again succeeds`)

#### Scenario: Remind Tomorrow of a failed check
- **WHEN** the user chooses Remind Tomorrow on the item of a failed check
- **THEN** it is hidden for a day
  [@test] crates/cli-tools/src/setup_test.rs (`a_failed_app_update_check_can_be_reminded_tomorrow`)
  [@test] src/lib/setup.test.ts (`skips an update for good and a reminder for a day, and counts the items in the name of Settings`)

### Requirement: Items: A successful check removes the failed check

A successful check for app updates MUST remove the item of the failed check.

#### Scenario: Check Again succeeds
- **GIVEN** the item of a failed check
- **WHEN** a check succeeds
- **THEN** the item is gone
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should show a failed update check and clear it when Check Again succeeds`)
  [@test] src-tauri/src/auto_update.rs (`a_successful_update_check_clears_the_last_failure`)

### Requirement: Items: An installed CLI is an item for a newer release or an older skill

An installed CLI MUST be an item when its newest release is newer than its version, else when the agents have an older
skill than it bundles (a skill never installed is the user's choice).

#### Scenario: A newer release
- **GIVEN** an installed CLI whose newest release is newer than its version
- **WHEN** the items are derived
- **THEN** it is an item
  [@test] crates/cli-tools/src/setup_test.rs (`the_app_update_then_each_tool_then_the_youtrack_sign_in`)

#### Scenario: An older skill of the agents
- **GIVEN** an installed CLI of the newest release whose skill the agents have older than it bundles
- **WHEN** the items are derived
- **THEN** it is an item
  [@test] crates/cli-tools/src/setup_test.rs (`a_tool_not_installed_and_a_skill_never_installed_are_no_items`)

#### Scenario: A skill never installed
- **GIVEN** an installed CLI whose skill was never installed
- **WHEN** the items are derived
- **THEN** the skill is no item: it is the user's choice
  [@test] crates/cli-tools/src/setup_test.rs (`a_tool_not_installed_and_a_skill_never_installed_are_no_items`)

### Requirement: Items: A sign-in of a CLI is an item when it expires within 7 days or expired

A sign-in of a CLI MUST be an item when its token expires within 7 days or expired.

#### Scenario: Sign-ins that expire soon, later, or expired
- **GIVEN** sign-ins of a CLI whose tokens expire within 7 days, later, or expired
- **WHEN** the items are derived
- **THEN** the one that expires within 7 days and the expired one are items, the other is none
  [@test] crates/cli-tools/src/setup_test.rs (`the_app_update_then_each_tool_then_the_youtrack_sign_in`)

### Requirement: Items: A CLI that is not installed is no item

A CLI that is not installed MUST be no item.

#### Scenario: A CLI not installed
- **GIVEN** a CLI that is not installed, with a newest release
- **WHEN** the items are derived
- **THEN** it is no item
  [@test] crates/cli-tools/src/setup_test.rs (`a_tool_not_installed_and_a_skill_never_installed_are_no_items`)

### Requirement: Items: The newest release is GitHub's `releases/latest`

The newest release MUST be GitHub's (`releases/latest` of JetBrains/teamcity-cli and JetBrains/youtrack-cli), asked
with the user's token, conditional on the last answer.

#### Scenario: The newest release of each tool
- **WHEN** the newest releases are asked
- **THEN** each is the `releases/latest` of JetBrains/teamcity-cli and JetBrains/youtrack-cli on GitHub, asked with the user's token, conditional on the last answer
  [@test] crates/remote-facts/src/cli_tool_releases_test.rs (`asks_github_once_a_day_and_keeps_the_answer_in_its_file`)

### Requirement: Items: The releases are asked at most once a day and kept in their file

The newest releases MUST be asked at most once a day, and kept in `cli-tool-releases.json` of the app data.

#### Scenario: Asked once a day
- **WHEN** the releases are read again within the day, then a day later
- **THEN** within the day they come from `cli-tool-releases.json` with no request, and a day later GitHub is asked again
  [@test] crates/remote-facts/src/cli_tool_releases_test.rs (`asks_github_once_a_day_and_keeps_the_answer_in_its_file`)

### Requirement: Items: Both tools are asked at once

The newest releases of both tools MUST be asked at once.

#### Scenario: Both requests at once
- **WHEN** the releases are asked
- **THEN** the requests of both tools reach GitHub before either is answered
  [@test] crates/remote-facts/src/cli_tool_releases_test.rs (`asks_for_every_tool_at_once`)

### Requirement: Items: A failed answer keeps the last release

A failed answer MUST keep the last release and ask again next time.

#### Scenario: GitHub fails
- **GIVEN** a kept release
- **WHEN** GitHub answers with a failure
- **THEN** the last release is kept, and the next read asks again
  [@test] crates/remote-facts/src/cli_tool_releases_test.rs (`a_failed_answer_keeps_the_last_release_and_asks_again_next_time`)

### Requirement: Items: The first read runs the CLIs, later reads take their last statuses

The first read of the app MUST run the CLIs; later reads MUST take their last statuses.

#### Scenario: The first read and the later ones
- **WHEN** the app reads the items first, and again later
- **THEN** the first read runs the CLIs, and the later ones take their last statuses

### Requirement: Items: The statuses are those of the read that started last

The last statuses MUST be those of the read that started last: reads run at once and end in any order, and one that
started before an update and ends after the read that followed it brings back nothing the update changed.

#### Scenario: A read that started before an update ends last
- **GIVEN** a read that started before an update, and one that started after it
- **WHEN** the later read ends first
- **THEN** the statuses are those of the later read, and the earlier one brings back nothing the update changed
  [@test] crates/cli-tools/src/kept_statuses_test.rs (`a_read_that_started_before_the_last_one_kept_keeps_nothing`)

#### Scenario: The CLIs change under a running read
- **GIVEN** a read of the CLIs that runs
- **WHEN** the CLIs change under it and the last statuses are dropped
- **THEN** that read brings back nothing, and the read that starts after it is kept
  [@test] crates/cli-tools/src/kept_statuses_test.rs (`forget_drops_the_statuses_and_the_reads_running`)

### Requirement: Items: The main window reads the CLIs every 6 hours, Settings → Tools on each open

The main window MUST read the CLIs again every 6 hours, Settings → Tools on each open and focus.

#### Scenario: Six hours in the main window
- **WHEN** 6 hours pass in the main window
- **THEN** it reads the CLIs again

#### Scenario: Settings → Tools opens or takes the focus
- **WHEN** Settings → Tools opens or takes the focus
- **THEN** it reads the CLIs again

### Requirement: Items: Each read tells every window

Each read of the CLIs MUST tell every window (`setup-changed`), and so MUST a YouTrack sign-in.

#### Scenario: A read after an update of a CLI
- **WHEN** a read of the CLIs ends after an update
- **THEN** every window hears `setup-changed` and shows the items as they are now
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should mark Tools in the sidebar while a CLI waits for you, until it is done`)

#### Scenario: The YouTrack sign-in
- **WHEN** the YouTrack sign-in of the app changes
- **THEN** every window hears it, and shows its item
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should mark Accounts in the sidebar while the YouTrack sign-in of the app stopped working`)

### Requirement: The dot: The Settings button has a dot while there are items

While there are items, the Settings button of the header MUST have a dot and the name "Settings · 2 to do".

#### Scenario: Items wait
- **GIVEN** items
- **WHEN** the header shows
- **THEN** the Settings button has a dot, and its name counts the items: "Settings · 2 to do"
  [@test] src/lib/setup.test.ts (`skips an update for good and a reminder for a day, and counts the items in the name of Settings`)
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should keep a downloaded update of the app in the dot, not in a toast, and install it from there`)

### Requirement: The dot: A click on the dot opens the list of the items

While there are items, a click on the Settings button MUST open the list: each item's words, its action, its skip,
then Settings….

#### Scenario: A click with items
- **GIVEN** items
- **WHEN** the user clicks the Settings button
- **THEN** the list opens: each item's words, its action, its skip, then Settings…
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should keep a downloaded update of the app in the dot, not in a toast, and install it from there`)

### Requirement: The dot: Without items the button opens Settings at once

Without items the Settings button MUST open Settings at once.

#### Scenario: Nothing waits
- **GIVEN** no items
- **WHEN** the user clicks the Settings button
- **THEN** it has no dot, and Settings opens at once
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should open Settings at once while nothing waits`)

### Requirement: The dot: ⌘, always opens Settings

⌘, (Ctrl+,) MUST always open Settings.

#### Scenario: The key with items or without
- **WHEN** the user presses ⌘, (Ctrl+,), with items or without
- **THEN** Settings opens

### Requirement: The dot: A downloaded update of the app is an item, not a toast

The update of the app that the silent check after startup and every 6 hours downloaded MUST be an item, not a toast:
"Branch Deck 1.2.0 is ready to install", Install and Restart.

#### Scenario: The silent check after startup
- **WHEN** the silent check after startup downloads an update
- **THEN** no toast shows, and the item says "Branch Deck 1.2.0 is ready to install" with Install and Restart
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should keep a downloaded update of the app in the dot, not in a toast, and install it from there`)

#### Scenario: The check every 6 hours
- **WHEN** the app stays open for 6 hours and the check then downloads an update
- **THEN** the item says "Branch Deck 1.2.0 is ready to install"
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should find an update after the app has stayed open for six hours`)

### Requirement: The dot: A failed check says "Cannot check for updates"

A failed check MUST be named "Cannot check for updates", its message MUST be the detail, and its action Check Again.

#### Scenario: The item of a failed check
- **WHEN** the check for app updates failed
- **THEN** its item is named "Cannot check for updates", its message is the detail, and its action is Check Again
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should show a failed update check and clear it when Check Again succeeds`)

### Requirement: The dot: A check reuses the release already downloaded

A check that finds the release already downloaded MUST reuse it; a newer release MUST replace it.

#### Scenario: The manifest names the downloaded release, or a newer one
- **GIVEN** a downloaded release
- **WHEN** a check finds the same release, or a newer one
- **THEN** the same one is reused, and a newer one replaces it
  [@test] src-tauri/src/auto_update.rs (`reuses_the_downloaded_release_when_the_manifest_still_names_it`)

### Requirement: The dot: Concurrent checks share one answer

Concurrent checks MUST share one answer, so they never download the same release twice.

#### Scenario: Checks at once
- **WHEN** checks run at once
- **THEN** they share one answer, and the same release is never downloaded twice
  [@test] crates/sync-utils/src/single_flight_test.rs (`callers_that_ask_while_a_read_runs_get_its_answer`)

### Requirement: The dot: A newer CLI updates as Settings → Tools does

"teamcity 1.7.0 is out" MUST update the CLI as Settings → Tools does (its toast in the main window), an older skill of
the agents with it.

#### Scenario: Update from the dot
- **WHEN** the user chooses Update on "teamcity 1.7.0 is out"
- **THEN** the CLI updates as Settings → Tools does, its toast in the main window, and an older skill of the agents with it
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should update a CLI from the dot, and skip a version until a newer one`)

### Requirement: The dot: An older skill of the agents updates the skill

"The teamcity agent skill 1.4.0 is ready to install" MUST update the skill.

#### Scenario: The item of the skill
- **WHEN** the user acts on "The teamcity agent skill 1.4.0 is ready to install"
- **THEN** the skill updates

### Requirement: The dot: An item of a CLI whose action runs shows it

An item of a CLI whose action runs MUST show its last line instead of its buttons, wherever the action started (the
item, Settings → Tools), as Rust keeps the actions that run ([Command-Line Tools](../cli-tools/spec.md)).

#### Scenario: The dot opened again while an update runs
- **GIVEN** the user chose Update on "teamcity 1.7.0 is out"
- **WHEN** the dot opens again while the update runs
- **THEN** the item shows its last line, without Update or Skip This Version
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should show the update an item started as running, in the dot and in Settings opened meanwhile, which offers no second one`)

### Requirement: The dot: A sign-in item opens where to sign in

"The teamcity sign-in to ci.example expires Jan 5" and "… expired" MUST open Settings → Tools at the tool to sign in;
"The YouTrack sign-in stopped working" MUST open Settings → Accounts.

#### Scenario: A sign-in of a CLI that expires
- **WHEN** the user chooses Sign In on "The teamcity sign-in to ci.example expires Jan 5"
- **THEN** Settings → Tools opens at the tool to sign in
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should remind of an expiring sign-in tomorrow, and open Tools to sign in`)

#### Scenario: A sign-in of a CLI that expired
- **WHEN** the user acts on "… expired"
- **THEN** Settings → Tools opens at the tool to sign in

#### Scenario: The YouTrack sign-in of the app
- **WHEN** the user acts on "The YouTrack sign-in stopped working"
- **THEN** Settings → Accounts opens

### Requirement: The dot: The sidebar of Settings marks Tools and Accounts

Settings → Tools MUST have the dot in the sidebar while a CLI is an item, Settings → Accounts while the app's YouTrack
sign-in stopped working.

#### Scenario: A CLI is an item
- **GIVEN** a CLI that is an item
- **WHEN** Settings opens
- **THEN** Tools has the dot in the sidebar until the CLI is done
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should mark Tools in the sidebar while a CLI waits for you, until it is done`)

#### Scenario: The YouTrack sign-in stopped working
- **GIVEN** the app's YouTrack sign-in stopped working
- **WHEN** Settings opens
- **THEN** Accounts has the dot in the sidebar, and Tools none
  [@test] tests/e2e/settings/tools-settings.spec.ts (`should mark Accounts in the sidebar while the YouTrack sign-in of the app stopped working`)

### Requirement: Skips: Skip This Version hides an update until another version

Skip This Version MUST hide an update until another version (another item).

#### Scenario: A skipped version, then a newer one
- **GIVEN** an update the user skipped with Skip This Version
- **WHEN** the items are derived again, then a newer version is out
- **THEN** the skipped version stays hidden, and the newer one is another item
  [@test] crates/cli-tools/src/setup_test.rs (`a_skip_hides_an_update_until_another_version_and_a_reminder_until_its_time`)
  [@test] src/lib/setup.test.ts (`skips an update for good and a reminder for a day, and counts the items in the name of Settings`)

### Requirement: Skips: Remind Tomorrow hides an item for a day

Remind Tomorrow MUST hide an expiring sign-in or failed app update check for a day.

#### Scenario: A reminder of an expiring sign-in or a failed check
- **WHEN** the user chooses Remind Tomorrow on an expiring sign-in or a failed app update check
- **THEN** it is hidden for a day, and is an item again once its time is over
  [@test] crates/cli-tools/src/setup_test.rs (`a_skip_hides_an_update_until_another_version_and_a_reminder_until_its_time`, `a_failed_app_update_check_can_be_reminded_tomorrow`)
  [@test] src/lib/setup.test.ts (`skips an update for good and a reminder for a day, and counts the items in the name of Settings`)

### Requirement: Skips: An expired sign-in and the YouTrack sign-in can't be skipped

An expired sign-in and the YouTrack sign-in MUST NOT be skippable: they go once fixed.

#### Scenario: An expired sign-in
- **GIVEN** an expired sign-in of a CLI
- **WHEN** its item shows
- **THEN** it has no skip, and a skip of it is not applied
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should keep an expired sign-in without a skip`)
  [@test] crates/cli-tools/src/setup_test.rs (`a_skip_hides_an_update_until_another_version_and_a_reminder_until_its_time`)

#### Scenario: The YouTrack sign-in
- **WHEN** the item of the YouTrack sign-in shows
- **THEN** it has no skip
  [@test] src/lib/setup.test.ts (`names each item, its action and its skip`)

### Requirement: Skips: The skips are a setting of every window, which Rust applies

The skips MUST be a setting of every window (`appSettings.setupSkips`), which Rust applies.

#### Scenario: A skip in one window
- **WHEN** the user skips an item
- **THEN** the skip is kept in `appSettings.setupSkips`, and Rust leaves the item out of the items of every window
  [@test] crates/cli-tools/src/setup_test.rs (`a_skip_hides_an_update_until_another_version_and_a_reminder_until_its_time`)

## User Experience

- The dot is `size-1.5` `bg-primary` at the top end of the icon, hidden from the accessible name, which counts the
  items. The list is a `Popover` aligned to the end, `w-96`: an item is its words and a row of an `xs` outline action
  and a ghost skip, or the running line of its CLI (`CliToolRunningLine`: a spinner and the last line in mono).

## Testing / Local Run

- `cargo nextest run -p cli-tools -E 'test(/setup/)'`, `cargo nextest run -p remote-facts -E 'test(/cli_tool_releases/)'`.
- `pnpm e2e tests/e2e/repository/setup-dot.spec.ts`: the test server derives the items from its fixtures (the update,
  the fakes of the CLIs, the fake YouTrack), its now the start of the page's clock.
