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
---

# Unattended Mode

Status: Implemented
Date: 2026-11-24

## Purpose

An agent team left running overnight stops at the first permission prompt of its lead or of any of its background
subagents: the prompt waits for a human until morning, and the whole team with it. While the user has turned
Unattended Mode on, the `PermissionRequest` hook of Claude Code that `brd install` adds answers every such prompt with
a denial whose message tells the agent to go on without the access, and records what was denied; the user reads that
list in the morning and decides what to allow. While the mode is off, the prompts are as they were.

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

## User Experience

- The message of a denial: "Branch Deck is in Unattended Mode: nobody can approve this. Do it without new access (a path
  inside the working directories, the pool path of a lease's notes), or skip this step, name it in your report and go
  on."
- `brd unattended on`, `brd unattended off`, `brd unattended status`; `brd hook permission-request` is hidden, Claude
  Code runs it.

## References

- [Agent CLI](../../../../docs/contributing/agent-cli.md) ("`brd unattended on|off|status`, `brd hook permission-request`",
  "`brd install [--uninstall]`, `brd help skill`")
- [Hooks of Claude Code](https://code.claude.com/docs/en/hooks) (`PermissionRequest`)
