---
name: Unattended Mode
description: Requirements for Unattended Mode, which the user turns on while they leave coding agents alone - the PermissionRequest hook of Claude Code that brd install adds denies every permission request with a way on and records it, brd unattended switches the mode and lists what was denied.
style: plain-1
targets:
  - crates/domain-types/src/unattended.rs
  - crates/domain-types/src/settings.rs
  - crates/app-settings/src/unattended.rs
  - crates/app-settings/src/service.rs
  - crates/bd-cli/src/unattended.rs
  - crates/bd-cli/src/claude_hook.rs
  - crates/bd-cli/src/install.rs
  - crates/bd-cli/src/lib.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/bd-core/src/context.rs
  - crates/app-api/src/app/unattended.rs
  - crates/app-api/src/app/agent_cli.rs
  - crates/test-server/src/host.rs
  - src/lib/unattended.ts
  - src/presenters/unattended.ts
  - src/stores/unattended.ts
  - src/stores/settings/appSettings.ts
  - src/components/repository/UnattendedModeButton.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/unattended/DeniedRequestsDialog.tsx
  - src/lib/command-palette.ts
  - src/components/settings/ToolsPage.tsx
  - src/components/settings/GeneralPage.tsx
---

# Unattended Mode

Status: Implemented
Date: 2026-11-24

## Purpose

An agent team left running overnight stops at the first permission prompt of its lead or of any of its background
subagents: the prompt waits for a human until morning, and the whole team with it. While the user has turned
Unattended Mode on, the `PermissionRequest` hook of Claude Code that `brd install` adds answers every such prompt with
a denial whose message tells the agent to go on without the access, and records what was denied; the user reads that
list in the morning and decides what to allow. While the mode is off, the prompts are as they were. The user switches it
in the command palette, in Settings → General or with `brd unattended`; the title bar marks it while it is on and its
mark turns it off, and the switch off says how many requests the night denied and lists them.

## Goals

- No prompt waits for the user while the mode is on, of any session or subagent.
- The mode is the user's: an explicit setting, off by default, never turned on by what Branch Deck detects.
- The morning tells what was denied: the tool and what it asked for, never the content of a file.

## Non-goals

- Allowing anything: the hook only denies or stays silent, so the mode never widens what an agent may do.
- Agents other than Claude Code: their prompts have no such hook.
- A history of every night: a run of the mode keeps its own list, the next run starts a new one.

## Concepts

The hook is a command of Claude Code's settings, run before every permission prompt with the hook's JSON on stdin; a
JSON answer with `"behavior": "deny"` denies the request and its message reaches the model, no answer leaves the
prompt to the user. Claude Code runs it for the main session and for foreground and background subagents alike.

## Requirements

### Requirement: Setting: The mode is a setting of the app, off by default

Unattended Mode MUST be the setting `appSettings.unattended`, off by default and kept in the file only while on, written
only through the writer of the settings; `brd unattended on` and `brd unattended off` MUST switch it there and save it
before they answer, and a switch to the state the mode is in MUST change nothing.

#### Scenario: brd turns the mode on and off
- **WHEN** `brd unattended on` runs, then `brd unattended on` again, then `brd unattended off`
- **THEN** the first answers `on: true, changed: true`, the second `changed: false`, the third `on: false, changed: true`
- **AND** the hook reads the mode each one saved
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on`)
  [@test] crates/app-settings/src/unattended_test.rs (`turning_the_mode_on_starts_a_new_list_and_turning_it_off_keeps_it`)

#### Scenario: No data directory of the app
- **WHEN** `brd unattended on` runs where brd knows no data directory of the app
- **THEN** it answers `invalid_arguments` and switches nothing
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_without_a_data_directory_is_refused`)

### Requirement: Hook: In the mode every permission request is denied with a way on

While the mode is on, `brd hook permission-request` MUST answer every permission request of Claude Code, of the main
session and of every subagent, with the denial `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":
{"behavior":"deny","message":…}}}` on one line, whose message says that nobody can approve the request and tells the
agent to do without new access or to skip the step, name it in its report and go on.

#### Scenario: A subagent asks to run a command
- **GIVEN** the mode is on
- **WHEN** Claude Code runs the hook for a `Bash` command of a subagent
- **THEN** the hook prints the denial with its message on one line
  [@test] crates/bd-cli/src/unattended_test.rs (`the_hook_denies_a_request_in_unattended_mode_and_records_it`, `the_hook_answers_nothing_to_an_input_that_is_no_permission_request`)

### Requirement: Hook: Otherwise the hook answers nothing and Claude Code asks

With the mode off, a settings file the hook cannot read, no data directory, or an input that is no permission request
(not JSON, no `tool_name`, another `hook_event_name`), the hook MUST print nothing and record nothing; it MUST never
allow a request, MUST exit 0, MUST write nothing to stderr, and MUST start no git and no network.

#### Scenario: The mode is off or unreadable
- **WHEN** the hook runs with the mode off, without a settings file, or with a settings file that does not parse
- **THEN** it prints nothing, records nothing and exits 0
  [@test] crates/bd-cli/src/unattended_test.rs (`the_hook_answers_nothing_while_the_mode_is_off_or_its_setting_cannot_be_read`)

#### Scenario: An input that is no permission request
- **GIVEN** the mode is on
- **WHEN** the hook gets an empty input, text, an array, an input without `tool_name` or of another event
- **THEN** it prints nothing, records nothing and exits 0
  [@test] crates/bd-cli/src/unattended_test.rs (`the_hook_answers_nothing_to_an_input_that_is_no_permission_request`)

### Requirement: Journal: Each denial is one line of what was asked, never of a file

Each denial MUST append one JSON line to `unattended-denials.jsonl` beside the settings of the app: the time, the
session, the subagent and its kind when a subagent asked, the working directory, the tool and a summary of what it
asked for, at most 200 characters, never the content of a file or an edit. A line MUST be written whole by one write,
and a reader MUST skip a line that does not parse.

#### Scenario: A command and a write of a file
- **WHEN** the hook denies a `Bash` command of a subagent and a `Write` of the main session
- **THEN** the journal has two lines: the command, the subagent and its kind; the path of the file, without its content
  [@test] crates/bd-cli/src/unattended_test.rs (`the_hook_denies_a_request_in_unattended_mode_and_records_it`)

#### Scenario: The summary of each tool
- **WHEN** a request of `Bash`, `Edit`, `WebFetch`, `Grep` or an MCP tool is denied
- **THEN** its summary is the command, the path, the URL, the pattern, or the names of the arguments, cut to 200 characters with `…`
  [@test] crates/bd-cli/src/unattended_test.rs (`a_summary_names_what_the_tool_was_asked_for_and_is_cut`)

#### Scenario: Hooks that deny at once, a cut line
- **WHEN** many hooks deny at the same moment, or a line was cut
- **THEN** every whole line reads back, and the cut one is skipped
  [@test] crates/app-settings/src/unattended_test.rs (`hooks_that_deny_at_once_append_whole_lines`, `the_journal_keeps_the_lines_that_parse`)

### Requirement: Journal: Turning the mode on starts a new list, off keeps it

Turning the mode on from off MUST remove the journal of the run before, so its list starts empty; turning it off MUST
keep the journal, so the list of the night stays readable until the mode is turned on again.

#### Scenario: A night and the next
- **GIVEN** a journal of the night before
- **WHEN** the mode is turned on, a request is denied, the mode is turned on again, then off, then on
- **THEN** the first on empties the list, the second keeps the denial, off keeps it, the last on empties the list
  [@test] crates/app-settings/src/unattended_test.rs (`turning_the_mode_on_starts_a_new_list_and_turning_it_off_keeps_it`)
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on`)

### Requirement: `brd`: brd unattended status lists the mode, its hook and the denials

`brd unattended status` MUST print whether the mode is on, whether the hook of this brd is installed (`installed`,
`stale`, `missing`), the number of requests denied since the mode was last turned on and those requests, oldest first,
and the journal; while the hook is not installed, `next` MUST name `brd install`. `on` and `off` MUST print the same.

#### Scenario: The morning
- **GIVEN** the hook denied a request during the night
- **WHEN** the user runs `brd unattended off`, then `brd unattended status`
- **THEN** both print `denied: 1` and the request, with `hook` and `next`
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on`)

### Requirement: Install: brd install adds the hook once and keeps the rest of the file

`brd install` MUST add to `hooks.PermissionRequest` of `~/.claude/settings.json` the entry `{"matcher": "*", "hooks":
[{"type": "command", "command": "<brd> hook permission-request"}]}`, creating the file, and MUST keep every other key,
hook and value of the file as it was written, in its order; an install when the hook is there MUST write nothing.

#### Scenario: A file of the user
- **GIVEN** a settings file with other keys, a hook of another event and another hook of `PermissionRequest`
- **WHEN** `brd install` runs, then again
- **THEN** the first adds the entry at the end of `PermissionRequest` and keeps the text of the rest; the second writes nothing
  [@test] crates/bd-cli/src/claude_hook_test.rs (`an_install_adds_the_hook_once_and_keeps_every_other_key_and_hook_as_written`)
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)

### Requirement: Install: A moved brd replaces the hook of the old one

A command hook whose command ends with `hook permission-request` MUST be read as Branch Deck's: an install MUST replace
every such hook of another command by one hook of this brd, keep the other hooks of its entry, and keep what the user
added to the installed hook (a `timeout`); Settings → Tools and `brd unattended` MUST read such a hook as `stale`.

#### Scenario: The app moved
- **GIVEN** the hook of the app binary of another place, with a hook of the user in its entry
- **WHEN** `brd install` runs
- **THEN** the entry keeps the hook of the user, and one entry has the hook of this brd
  [@test] crates/bd-cli/src/claude_hook_test.rs (`an_install_of_a_moved_brd_replaces_the_command_of_the_old_one`)

### Requirement: Install: A file the hook cannot go into stops the install

A settings file that is no JSON object, or whose `hooks` is no object or `hooks.PermissionRequest` no array, MUST make
`brd install` answer `install_refused` before it writes anything, and stay as it is; a blank file MUST be read as an
empty one, and a file only its owner reads MUST stay so.

#### Scenario: A broken file
- **WHEN** `brd install` meets an array, a cut file, or `hooks` of the wrong type
- **THEN** it answers `install_refused`, which names the file, and the file stays as it was
  [@test] crates/bd-cli/src/claude_hook_test.rs (`a_file_that_is_no_object_is_refused_and_stays`)

#### Scenario: A private file
- **WHEN** `brd install` adds the hook to a file of mode 0600
- **THEN** the file stays 0600
  [@test] crates/bd-cli/src/claude_hook_test.rs (`a_private_file_stays_private`)

### Requirement: Install: brd install --uninstall removes only the hook of Branch Deck

`brd install --uninstall` MUST remove the hooks of Branch Deck, an entry they leave empty, a `PermissionRequest` and a
`hooks` they leave empty, keep everything else, and name the file in `hook_file`; a file without such a hook MUST stay
unwritten.

#### Scenario: Uninstall
- **WHEN** `brd install --uninstall` runs after `brd install`
- **THEN** the file is what it was before the install
  [@test] crates/bd-cli/src/claude_hook_test.rs (`an_uninstall_removes_only_the_hook_of_branch_deck`)
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)

### Requirement: App: The title bar marks the mode while it is on, and the mark turns it off

While the mode is on, the title bar of the main window MUST show its mark before Settings, in every state of the window:
the moon in the warning tone, with "Unattended" where the header has room; a click on it MUST turn the mode off through
`set_unattended_mode`, the core of `brd unattended`. While the mode is off, the title bar MUST show nothing of it.

#### Scenario: The mark comes and goes
- **WHEN** the user turns the mode on from the palette, then clicks the mark
- **THEN** the mark reads "Unattended", then the title bar shows nothing of the mode
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, offer the install of a missing hook, and turn it off by the mark`)

#### Scenario: The row keeps one line with the mode on
- **GIVEN** the mode is on, between ☰ and the window buttons of a custom title bar
- **WHEN** the window narrows from 1440 px to the minimum
- **THEN** every control stays on the one line, apart; "Unattended" shows from 1088 px of the header, the tinted moon alone below
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the toolbar between the main menu and the window buttons on one line at every width with Unattended Mode on, its word where the header has room`)

### Requirement: App: The command palette and Settings → General switch the mode by one item

Commands of the palette MUST hold Turn On Unattended Mode while the mode is off and Turn Off Unattended Mode while it is
on, and Settings → General MUST hold the switch Unattended Mode under "Coding agents": one item for them and the mark
(`unattendedModeItem`); every window MUST hear the setting as the writer saves it.

#### Scenario: On and off by the palette
- **WHEN** the user runs Turn On Unattended Mode, then Turn Off Unattended Mode, from the palette
- **THEN** the mark of the title bar shows, then goes
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, offer the install of a missing hook, and turn it off by the mark`)
  [@test] src/presenters/unattended.test.ts (`names the switch by what a choice of it does`)

#### Scenario: On and off in Settings
- **WHEN** the user turns the switch of Settings → General on, then off
- **THEN** the mark of the main window shows, then goes; the toast of the switch off in Settings tells the count, without the list
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on and off from Settings → General, the mark of the main window following it`)
  [@test] src/presenters/unattended.test.ts (`tells how many requests a switch off found and offers their list only when there are some, in the main window`)

### Requirement: App: A switch on without the hook warns and offers its install

A switch on while the hook is missing or runs another Branch Deck MUST show a warning toast "Turned on Unattended Mode"
that says the requests still wait for the user, with Install Agent CLI where the host installs it; a switch on with the
hook installed MUST show no toast, the mark says it.

#### Scenario: No hook yet
- **GIVEN** the agent CLI is not installed
- **WHEN** the user turns the mode on
- **THEN** the warning toast offers Install Agent CLI, which installs the agent CLI and its hook
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, offer the install of a missing hook, and turn it off by the mark`)
  [@test] src/presenters/unattended.test.ts (`says nothing of a switch on whose hook is installed, warns of one without it and offers its install where the host installs`)

### Requirement: App: A switch off tells how many requests were denied and opens them

A switch off MUST show the toast "Turned off Unattended Mode" with the number of requests denied while the mode was on,
and, when there are some, the action Show Denied Requests… that opens their list.

#### Scenario: A night with denials
- **GIVEN** the hook denied two requests while the mode was on
- **WHEN** the user turns the mode off
- **THEN** the toast says "2 permission requests were denied while it was on" and its action opens Denied Requests
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should tell on a switch off how many requests the night denied and list them`)
  [@test] src/presenters/unattended.test.ts (`tells how many requests a switch off found and offers their list only when there are some, in the main window`)

#### Scenario: A quiet night
- **WHEN** the user turns the mode off and nothing was denied
- **THEN** the toast says "No permission request was denied while it was on", without an action
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, offer the install of a missing hook, and turn it off by the mark`)

### Requirement: App: Denied Requests lists each request, oldest first

Denied Requests MUST be a dialog of the main window that lists the denied requests oldest first, each with the tool,
what it asked for, the time, who asked (the main session, or the kind and id of a subagent) and the last part of the
working directory; Escape MUST close it.

#### Scenario: The list of the night
- **WHEN** the dialog opens from the toast of a switch off
- **THEN** it lists `Bash cat /etc/hosts` of `general-purpose (a7)` in `app`, then `WebFetch` of the main session
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should tell on a switch off how many requests the night denied and list them`)
  [@test] src/presenters/unattended.test.ts (`words who asked and where`)

### Requirement: App: Settings → Tools says when the hook denies nothing

While the agent skill is installed but the hook is missing or runs another Branch Deck, the row of `brd` in Settings →
Tools MUST say so in the warning tone and offer Install Agent CLI where the host installs it.

#### Scenario: A hook of another Branch Deck
- **WHEN** Settings → Tools reads a hook of another brd
- **THEN** the row of `brd` says "The hook of Unattended Mode runs another Branch Deck: Install Agent CLI puts this one in its place"
  [@test] src/presenters/unattended.test.ts (`says in Settings → Tools why the hook denies nothing`)
  [@test] tests/e2e/settings/settings.spec.ts (`should show every page of the window in its tallest state`)

## User Experience

- The message of a denial: "Branch Deck is in Unattended Mode: nobody can approve this. Do it without new access (a path
  inside the working directories, the pool path of a lease's notes), or skip this step, name it in your report and go
  on."
- `brd unattended on`, `brd unattended off`, `brd unattended status`; `brd hook permission-request` is hidden, Claude
  Code runs it.
- The title bar while on: the mark Unattended Mode, `MoonStarIcon` and "Unattended" in the warning tone; its tooltip is
  "Turn Off Unattended Mode" with "While it is on, coding agents' permission requests are
  denied with a way on instead of waiting for you".
- The palette: "Turn On Unattended Mode" (`MoonIcon`), "Turn Off Unattended Mode"; Settings → General: "Coding agents",
  "While you leave them alone", the switch "Unattended Mode".
- The toasts: "Turned on Unattended Mode" (warning) with "Claude Code has no hook of Unattended Mode, so its permission
  requests still wait for you: Install Agent CLI adds it" and Install Agent CLI; "Turned off Unattended Mode" with
  "2 permission requests were denied while it was on" and Show Denied Requests….
- The dialog: "Denied Requests"; each request its tool, its summary, "Today 2:32 PM · general-purpose (a7) · app".

## References

- [Agent CLI](../../../../docs/contributing/agent-cli.md) ("`brd unattended on|off|status`, `brd hook permission-request`",
  "`brd install [--uninstall]`, `brd help skill`")
- [Hooks of Claude Code](https://code.claude.com/docs/en/hooks) (`PermissionRequest`)
