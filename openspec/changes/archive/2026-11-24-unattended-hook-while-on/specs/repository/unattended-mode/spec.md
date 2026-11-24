## ADDED Requirements

### Requirement: Hook: The hook is in the settings of Claude Code only while the mode is on

Turning the mode on MUST first add the hook to `~/.claude/settings.json`, then switch the setting; a hook that cannot be
written (no home directory or brd, a settings file that cannot take it, a failed write) MUST keep the mode off and
answer `install_refused` with why. Turning it off MUST switch the setting first, then remove the hook; a removal that
fails MUST be the `warning` of the answer, the mode off all the same. A crash or a quit while the mode is on MUST leave
the hook and the mode on.

#### Scenario: On adds, off gives the file back
- **GIVEN** settings of Claude Code with a hook of the user
- **WHEN** the mode is turned off, on, then off
- **THEN** the first off writes nothing, on adds the hook beside the user's, and the last off gives the file back byte for byte
  [@test] crates/app-settings/src/unattended_test.rs (`the_hook_is_in_the_settings_of_claude_code_only_while_the_mode_is_on`)
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on`)
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, add the hook to the settings of Claude Code, and turn it off by the mark, which gives the file back`)

#### Scenario: A settings file that cannot take the hook
- **GIVEN** a settings file of Claude Code that is no JSON object
- **WHEN** the mode is turned on
- **THEN** it stays off, the answer is `install_refused` naming the file, and the file stays as it was
  [@test] crates/app-settings/src/unattended_test.rs (`a_switch_on_whose_hook_cannot_be_written_stays_off`)
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_on_whose_hook_cannot_be_written_stays_off`)
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should stay off and say why when the settings of Claude Code cannot take the hook`)

#### Scenario: A removal that fails
- **GIVEN** the mode is on and the user broke the settings file of Claude Code
- **WHEN** the mode is turned off
- **THEN** it is off, and the answer warns that the hook stayed
  [@test] crates/app-settings/src/unattended_test.rs (`a_switch_on_whose_hook_cannot_be_written_stays_off`)

## MODIFIED Requirements

### Requirement: Hook: Otherwise the hook answers nothing and Claude Code asks

With the mode off, a settings file the hook cannot read, no data directory, or an input that is no permission request
(not JSON, no `tool_name`, another `hook_event_name`), the hook MUST print nothing and record nothing, its entry in the
settings of Claude Code or not; it MUST never allow a request, MUST exit 0, MUST write nothing to stderr, and MUST start
no git and no network.

#### Scenario: The mode is off or unreadable
- **WHEN** the hook runs with the mode off, without a settings file, or with a settings file that does not parse
- **THEN** it prints nothing, records nothing and exits 0
  [@test] crates/bd-cli/src/unattended_test.rs (`the_hook_answers_nothing_while_the_mode_is_off_or_its_setting_cannot_be_read`)

#### Scenario: The entry stayed, the mode is off
- **GIVEN** the hook is in the settings of Claude Code
- **WHEN** the mode goes off without its removal and the hook runs
- **THEN** it prints nothing and records nothing
  [@test] crates/bd-cli/src/unattended_test.rs (`the_hook_answers_nothing_while_the_mode_is_off_with_its_entry_in_place`)

#### Scenario: An input that is no permission request
- **GIVEN** the mode is on
- **WHEN** the hook gets an empty input, text, an array, an input without `tool_name` or of another event
- **THEN** it prints nothing, records nothing and exits 0
  [@test] crates/bd-cli/src/unattended_test.rs (`the_hook_answers_nothing_to_an_input_that_is_no_permission_request`)

### Requirement: `brd`: brd unattended status lists the mode, its hook and the denials

`brd unattended status` MUST print whether the mode is on, whether the settings of Claude Code hold the hook (`present`,
`absent`), the number of requests denied since the mode was last turned on and those requests, oldest first, and the
journal. `on` and `off` MUST print the same, `off` with the `warning` of a removal that failed.

#### Scenario: The morning
- **GIVEN** the hook denied a request during the night
- **WHEN** the user runs `brd unattended off`, then `brd unattended status`
- **THEN** both print `denied: 1` and the request, with `hook: absent`
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on`)

### Requirement: App: A switch off tells how many requests were denied and opens them

A switch off MUST show the toast "Turned off Unattended Mode" with the number of requests denied while the mode was on,
and, when there are some, the action Show Denied Requests… that opens their list; when the hook stayed in the settings
of Claude Code the toast MUST be a warning that says why. A switch on MUST show no toast: the mark says it.

#### Scenario: A night with denials
- **GIVEN** the hook denied two requests while the mode was on
- **WHEN** the user turns the mode off
- **THEN** the toast says "2 permission requests were denied while it was on" and its action opens Denied Requests
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should tell on a switch off how many requests the night denied and list them`)
  [@test] src/presenters/unattended.test.ts (`tells how many requests a switch off found and offers their list only when there are some, in the main window`)

#### Scenario: A quiet night
- **WHEN** the user turns the mode off and nothing was denied
- **THEN** the toast says "No permission request was denied while it was on", without an action
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, add the hook to the settings of Claude Code, and turn it off by the mark, which gives the file back`)

#### Scenario: The hook stayed
- **WHEN** the switch off answers the warning of a removal that failed
- **THEN** the toast is a warning that adds the warning and that the hook denies nothing while the mode is off
  [@test] src/presenters/unattended.test.ts (`warns on a switch off whose hook stayed in the settings of Claude Code`)

#### Scenario: A switch on
- **WHEN** the user turns the mode on, or switches it to the state it is in
- **THEN** no toast shows
  [@test] src/presenters/unattended.test.ts (`says nothing of a switch on, nor of a switch that changed nothing: the mark of the title bar says it`)

### Requirement: Install: brd install keeps the hook as the mode has it

`brd install` MUST write no hook of its own: while the mode is off it MUST remove a hook of Branch Deck an earlier
install wrote, while it is on it MUST write the hook anew for this brd, and name the file in `hook_file` when it changed
it; a settings file it cannot change MUST stay as it is, the `brd` command and the skills installed all the same and
`hook_error` saying why.

#### Scenario: The hook of an earlier install
- **GIVEN** the hook of an earlier install, of the shell form, beside a value of the user
- **WHEN** `brd install` runs with the mode off, then again; then with the mode on; then `brd install --uninstall`
- **THEN** the first removes the hook and keeps the value, the second changes nothing, the install while on writes the hook of this brd, and the uninstall removes it
  [@test] crates/bd-cli/src/install_test.rs (`an_install_keeps_the_hook_of_unattended_mode_as_the_mode_has_it`)

#### Scenario: A user who never turns the mode on
- **WHEN** `brd install` runs, then `brd install --uninstall`, without settings of Claude Code
- **THEN** neither writes them, and `hook_file` is `null`
  [@test] crates/bd-cli/src/install_test.rs (`installs_the_command_and_the_skills_and_uninstalls_them`)

#### Scenario: A file it cannot change
- **GIVEN** a cut settings file of Claude Code
- **WHEN** `brd install` runs
- **THEN** it installs the `brd` command and the skills, answers `hook_error`, which names the file, and leaves the file
  [@test] crates/bd-cli/src/install_test.rs (`an_install_keeps_the_hook_of_unattended_mode_as_the_mode_has_it`)

### Requirement: Install: brd install --uninstall removes only the hook of Branch Deck

A removal of the hook (a switch off, `brd install` while the mode is off, `brd install --uninstall`) MUST remove the
hooks of Branch Deck, an entry they leave empty, a `PermissionRequest` and a `hooks` they leave empty, and keep
everything else as written; a file without such a hook MUST stay unwritten.

#### Scenario: Uninstall
- **WHEN** the hook is removed after it was added to a file of the user
- **THEN** the file is what it was before, byte for byte
  [@test] crates/app-settings/src/claude_hook_test.rs (`a_removal_takes_only_the_hook_of_branch_deck_and_gives_the_file_back`)
  [@test] crates/bd-cli/src/install_test.rs (`an_install_keeps_the_hook_of_unattended_mode_as_the_mode_has_it`)

## REMOVED Requirements

### Requirement: App: A switch on without the hook warns and offers its install

**Reason**: the switch on adds the hook itself, or stays off and says why.
**Migration**: none.

### Requirement: App: Settings → Tools says when the hook denies nothing

**Reason**: the hook comes and goes with the mode; Settings → Tools has no hook to tell.
**Migration**: none.

## RENAMED Requirements

- FROM: `### Requirement: Install: A file the hook cannot go into stays, the rest is installed`
- TO: `### Requirement: Install: brd install keeps the hook as the mode has it`
