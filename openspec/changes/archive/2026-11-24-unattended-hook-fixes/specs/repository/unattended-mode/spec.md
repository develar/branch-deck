## RENAMED Requirements

- FROM: `### Requirement: Install: A file the hook cannot go into stops the install`
- TO: `### Requirement: Install: A file the hook cannot go into stays, the rest is installed`

## ADDED Requirements

### Requirement: Install: The hook runs brd with no shell and never blocks

The hook MUST be a command hook of the exec form, `command` the path of brd as it is and `args` its arguments, which
Claude Code starts with no shell on every platform (a command line would run in PowerShell on Windows without Git
Bash), and its `onFailure` MUST be `continue`, so a hook that fails leaves the request to the prompt of the user.

#### Scenario: No shell, no block
- **WHEN** the entry is written for the app binary under `/Applications/Branch Deck.app` and for `brd.exe` under a Windows home with an apostrophe
- **THEN** `command` is the path as it is, `args` are `cli hook permission-request` and `hook permission-request`, and `onFailure` is `continue`
  [@test] crates/bd-cli/src/claude_hook_test.rs (`the_hook_runs_brd_with_no_shell_and_never_blocks_when_it_fails`)

### Requirement: Install: An edit keeps the writes of Claude Code and the symlink

An edit of the settings file MUST read it again just before its rename and, when it changed since the read, read and
merge again, five times at most, so a write of Claude Code during the edit is kept; a settings file that is a symlink MUST be
written where it points and stay a link.

#### Scenario: Claude Code writes the file during the install
- **WHEN** Claude Code writes the file after the install read it and before its rename
- **THEN** the install reads it again and adds the hook to what Claude Code wrote; a file that keeps changing five times is refused as Claude Code wrote it last
  [@test] crates/bd-cli/src/claude_hook_test.rs (`a_write_of_claude_code_during_the_install_is_kept`)

#### Scenario: A settings file that is a symlink
- **GIVEN** `~/.claude/settings.json` is a relative symlink into a directory of dotfiles
- **WHEN** `brd install` runs, then `brd install --uninstall`
- **THEN** the file of the dotfiles gets the hook, then loses it, and the settings file stays a link
  [@test] crates/bd-cli/src/claude_hook_test.rs (`a_settings_file_that_is_a_symlink_stays_one`)

## MODIFIED Requirements

### Requirement: Setting: The mode is a setting of the app, off by default

Unattended Mode MUST be the setting `appSettings.unattended`, off by default and kept in the file only while on, written
only through the writer of the settings; `brd unattended on` and `brd unattended off`, and the switch of the app, MUST
switch it there and save it before they answer, as the hook reads the file, a save that fails being an error of the
switch; a switch to the state the mode is in MUST change nothing.

#### Scenario: brd turns the mode on and off
- **WHEN** `brd unattended on` runs, then `brd unattended on` again, then `brd unattended off`
- **THEN** the first answers `on: true, changed: true`, the second `changed: false`, the third `on: false, changed: true`
- **AND** the hook reads the mode each one saved
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on`)
  [@test] crates/app-settings/src/unattended_test.rs (`turning_the_mode_on_starts_a_new_list_and_turning_it_off_keeps_it`)

#### Scenario: The switch is saved when it answers
- **GIVEN** a writer of the settings whose next save is an hour away
- **WHEN** the mode is turned on, then off
- **THEN** the settings file holds each state as the switch answers
  [@test] crates/app-settings/src/unattended_test.rs (`turning_the_mode_on_starts_a_new_list_and_turning_it_off_keeps_it`)

#### Scenario: No data directory of the app
- **WHEN** `brd unattended on` runs where brd knows no data directory of the app
- **THEN** it answers `invalid_arguments` and switches nothing
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_without_a_data_directory_is_refused`)

### Requirement: `brd`: brd unattended status lists the mode, its hook and the denials

`brd unattended status` MUST print whether the mode is on, whether the hook of this brd is installed (`installed`,
`stale`, `missing`, or `invalid` for a settings file that cannot hold it), the number of requests denied since the mode
was last turned on and those requests, oldest first, and the journal; while the hook is not installed, `next` MUST name
`brd install`. `on` and `off` MUST print the same.

#### Scenario: The morning
- **GIVEN** the hook denied a request during the night
- **WHEN** the user runs `brd unattended off`, then `brd unattended status`
- **THEN** both print `denied: 1` and the request, with `hook` and `next`
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on`)

### Requirement: Install: brd install adds the hook once and keeps the rest of the file

`brd install` MUST add to `hooks.PermissionRequest` of `~/.claude/settings.json` the entry `{"matcher": "*", "hooks":
[{"type": "command", "command": "<brd>", "args": ["hook", "permission-request"], "onFailure": "continue"}]}`, creating
the file, and MUST keep every other key, hook and value of the file as it was written, in its order; an install when
the hook is there MUST write nothing.

#### Scenario: A file of the user
- **GIVEN** a settings file with other keys, a hook of another event and another hook of `PermissionRequest`
- **WHEN** `brd install` runs, then again
- **THEN** the first adds the entry at the end of `PermissionRequest` and keeps the text of the rest; the second writes nothing
  [@test] crates/bd-cli/src/claude_hook_test.rs (`an_install_adds_the_hook_once_and_keeps_every_other_key_and_hook_as_written`)
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)

### Requirement: Install: A moved brd replaces the hook of the old one

A command hook whose `args` end with `hook permission-request`, or of the shell form whose command ends with it, MUST be
read as Branch Deck's: an install MUST replace every such hook of another command or of the shell form by one hook of
this brd, keep the other hooks of its entry, and keep what the user added to the installed hook (a `timeout`, another
`onFailure`); Settings → Tools and `brd unattended` MUST read such a hook as `stale`.

#### Scenario: The app moved
- **GIVEN** the hook of the app binary of another place, with a hook of the user in its entry
- **WHEN** `brd install` runs
- **THEN** the entry keeps the hook of the user, and one entry has the hook of this brd
  [@test] crates/bd-cli/src/claude_hook_test.rs (`an_install_of_a_moved_brd_replaces_the_command_of_the_old_one`)

#### Scenario: A hook of the shell form
- **GIVEN** the hook of this brd as a command line of the shell form, as the first installs wrote it
- **WHEN** Settings → Tools reads it, then `brd install` runs; or `brd install --uninstall` runs
- **THEN** it is `stale`, and the install puts the hook of the exec form in its place; the uninstall removes it
  [@test] crates/bd-cli/src/claude_hook_test.rs (`a_hook_of_the_shell_form_is_replaced_by_the_exec_form`)

### Requirement: Install: A file the hook cannot go into stays, the rest is installed

A settings file that is no JSON object, or whose `hooks` is no object or `hooks.PermissionRequest` no array, MUST stay as
it is; `brd install` MUST install the `brd` command and the skills all the same and say why in `hook_error`, and the
hook MUST read `invalid`. A blank file MUST be read as an empty one, and a file only its owner reads MUST stay so.

#### Scenario: A broken file
- **WHEN** the hook meets an array, a cut file, or `hooks` of the wrong type
- **THEN** it is refused with `install_refused`, which names the file, the file stays as it was, and its hook reads `invalid`
  [@test] crates/bd-cli/src/claude_hook_test.rs (`a_file_that_is_no_object_is_refused_and_stays`)

#### Scenario: The rest is installed
- **GIVEN** a cut settings file
- **WHEN** `brd install` runs, then again after the file is fixed
- **THEN** the first installs the `brd` command and the skills, answers `hook_error`, which names the file, and leaves the file; the toast "Agent CLI installed" of the app is a warning that says why; the second adds the hook
  [@test] crates/bd-cli/src/install_test.rs (`a_settings_file_the_hook_cannot_go_into_stays_and_the_rest_is_installed`)
  [@test] src/hooks/useInstallAgentCli.test.ts (`tells why the hook of Unattended Mode stayed out of a settings file of Claude Code it cannot go into`)

#### Scenario: A private file
- **WHEN** `brd install` adds the hook to a file of mode 0600
- **THEN** the file stays 0600
  [@test] crates/bd-cli/src/claude_hook_test.rs (`a_private_file_stays_private`)

### Requirement: App: A switch on without the hook warns and offers its install

A switch on while the hook is missing, runs another Branch Deck or cannot go into the settings file MUST show a warning
toast "Turned on Unattended Mode" that says the requests still wait for the user, with Install Agent CLI where the host
installs it; a switch on with the hook installed MUST show no toast, the mark says it.

#### Scenario: No hook yet
- **GIVEN** the agent CLI is not installed
- **WHEN** the user turns the mode on
- **THEN** the warning toast offers Install Agent CLI, which installs the agent CLI and its hook
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, offer the install of a missing hook, and turn it off by the mark`)
  [@test] src/presenters/unattended.test.ts (`says nothing of a switch on whose hook is installed, warns of one without it and offers its install where the host installs`)

### Requirement: App: Settings → Tools says when the hook denies nothing

While the agent skill is installed but the hook is missing, runs another Branch Deck or cannot go into the settings file,
the row of `brd` in Settings → Tools MUST say so in the warning tone and offer Install Agent CLI where the host installs
it.

#### Scenario: A hook of another Branch Deck
- **WHEN** Settings → Tools reads a hook of another brd
- **THEN** the row of `brd` says "The hook of Unattended Mode runs another Branch Deck: Install Agent CLI puts this one in its place"
  [@test] src/presenters/unattended.test.ts (`says in Settings → Tools why the hook denies nothing`)
  [@test] tests/e2e/settings/settings.spec.ts (`should show every page of the window in its tallest state`)

#### Scenario: A settings file the hook cannot go into
- **WHEN** Settings → Tools reads a settings file of Claude Code that is no JSON object
- **THEN** the row of `brd` says "The hook of Unattended Mode cannot go into ~/.claude/settings.json, which is no JSON object of Claude Code's settings: fix the file, then Install Agent CLI"
  [@test] src/presenters/unattended.test.ts (`says in Settings → Tools why the hook denies nothing`)
