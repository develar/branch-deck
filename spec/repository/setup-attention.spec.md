---
name: Setup Dot
description: Requirements for the dot on the Settings button of the header - what of the setup of the machine waits for the user (a downloaded update of the app, a newer CLI or skill, a CLI sign-in that expires or expired, the app's YouTrack sign-in that stopped), derived in Rust, each with its action and its skip, instead of a toast nobody asked for.
style: plain-1
targets:
  - ../../crates/cli-tools/src/setup.rs
  - ../../crates/domain-types/src/setup.rs
  - ../../crates/remote-facts/src/cli_tool_releases.rs
  - ../../src-tauri/src/commands/cli_tools.rs
  - ../../crates/test-server/src/cli_tools_fixtures.rs
  - ../../src/stores/setup.ts
  - ../../src/lib/setup.ts
  - ../../src/lib/setup-actions.ts
  - ../../src/components/repository/SettingsButton.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/pages/SettingsPage.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useAutoUpdate.ts
  - ../../src/stores/settings/appSettings.ts
---

# Setup Dot

Status: Implemented
Date: 2026-10-23

## Summary

A toast says what just happened; a newer release nobody asked for is no event, and a sticky toast closed once is gone.
Activity is the history of what the app did. What is missing between them is the state of the setup that waits for
the user: an update of the app ready to install, a newer `teamcity` or `youtrack`, a CLI sign-in that expires. That is
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

### Items

- Rust derives the items (`get_setup_items`, `cli_tools::setup`) from explicit facts: the downloaded update of the app,
  the last statuses of the CLIs with their newest releases, the app's YouTrack sign-in; in this order: the app, each
  tool, YouTrack.
  [@test] ../../crates/cli-tools/src/setup_test.rs (`the_app_update_then_each_tool_then_the_youtrack_sign_in`)
- An installed CLI is an item when its newest release is newer than its version, else when the agents have an older
  skill than it bundles (a skill never installed is the user's choice); a sign-in of a CLI is one when its token expires
  within 7 days or expired. A CLI that is not installed is none.
  [@test] ../../crates/cli-tools/src/setup_test.rs (`a_tool_not_installed_and_a_skill_never_installed_are_no_items`)
- The newest release is GitHub's (`releases/latest` of JetBrains/teamcity-cli and JetBrains/youtrack-cli), asked with
  the user's token, conditional on the last answer, at most once a day and kept in `cli-tool-releases.json` of the app
  data; a failed answer keeps the last release and asks again next time.
  [@test] ../../crates/remote-facts/src/cli_tool_releases_test.rs (`asks_github_once_a_day_and_keeps_the_answer_in_its_file`, `a_failed_answer_keeps_the_last_release_and_asks_again_next_time`)
- The first read of the app runs the CLIs; later reads take their last statuses. The main window reads the CLIs again
  every 6 hours, Settings → Tools on each open and focus; each read tells every window (`setup-changed`), as does a
  YouTrack sign-in.

### The dot

- While there are items, the Settings button of the header has a dot and the name "Settings · 2 to do"; a click opens
  the list: each item's words, its action, its skip, then Settings…. Without items the button opens Settings at once.
  ⌘, (Ctrl+,) always opens Settings.
  [@test] ../../tests/e2e/repository/setup-dot.spec.ts (`should open Settings at once while nothing waits`)
- The update of the app that the silent check after startup downloaded is an item, not a toast: "Branch Deck 1.2.0 is
  ready to install", Install and Restart.
  [@test] ../../tests/e2e/repository/setup-dot.spec.ts (`should keep a downloaded update of the app in the dot, not in a toast, and install it from there`)
- "teamcity 1.7.0 is out" updates the CLI as Settings → Tools does (its toast in the main window), an older skill of the
  agents with it; "The teamcity agent skill 1.4.0 is ready to install" updates the skill.
  [@test] ../../tests/e2e/repository/setup-dot.spec.ts (`should update a CLI from the dot, and skip a version until a newer one`)
- "The teamcity sign-in to ci.example expires Jan 5" and "… expired" open Settings → Tools at the tool to sign in; "The
  YouTrack sign-in stopped working" opens Settings → YouTrack.
  [@test] ../../tests/e2e/repository/setup-dot.spec.ts (`should remind of an expiring sign-in tomorrow, and open Tools to sign in`)
- Settings → Tools has the dot in the sidebar while a CLI is an item.
  [@test] ../../tests/e2e/settings/tools-settings.spec.ts (`should mark Tools in the sidebar while a CLI waits for you, until it is done`)

### Skips

- Skip This Version hides an update until another version (another item); Remind Tomorrow hides an expiring sign-in for
  a day. An expired sign-in and the YouTrack sign-in can't be skipped: they go once fixed. The skips are a setting of
  every window (`appSettings.setupSkips`), which Rust applies.
  [@test] ../../crates/cli-tools/src/setup_test.rs (`a_skip_hides_an_update_until_another_version_and_a_reminder_until_its_time`)
  [@test] ../../tests/e2e/repository/setup-dot.spec.ts (`should keep an expired sign-in without a skip`)
  [@test] ../../src/lib/setup.test.ts (`names each item, its action and its skip`, `skips an update for good and a reminder for a day, and counts the items in the name of Settings`)

## User Experience

- The dot is `size-1.5` `bg-primary` at the top end of the icon, hidden from the accessible name, which counts the
  items. The list is a `Popover` aligned to the end, `w-96`: an item is its words and a row of an `xs` outline action
  and a ghost skip.

## Testing / Local Run

- `cargo nextest run -p cli-tools -E 'test(/setup/)'`, `cargo nextest run -p remote-facts -E 'test(/cli_tool_releases/)'`.
- `pnpm e2e tests/e2e/repository/setup-dot.spec.ts`: the test server derives the items from its fixtures (the update,
  the fakes of the CLIs, the fake YouTrack), its now the start of the page's clock.
