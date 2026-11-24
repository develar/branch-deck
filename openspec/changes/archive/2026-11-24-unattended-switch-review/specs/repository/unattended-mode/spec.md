## MODIFIED Requirements

### Requirement: Setting: The mode is a setting of the app, off by default

Unattended Mode MUST be the setting `appSettings.unattended`, off by default and kept in the file only while on, written
only through the writer of the settings; `brd unattended on` and `brd unattended off`, and the switch of the app, MUST
switch it there and save it before they answer, as the hook reads the file, a save that fails being an error of the
switch; a switch to the state the mode is in MUST change nothing. A save that fails MUST put the setting back as it was
and take away the hook a switch on added, so the mode, the settings of Claude Code and the windows agree, and the next
switch MUST try again.

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

#### Scenario: A save that fails
- **GIVEN** a settings file of the app that no save can replace
- **WHEN** the mode is turned on twice, the file is fixed and the mode turned on, then the file breaks again and the mode is turned off
- **THEN** each on fails with the mode off and the settings of Claude Code as they were; the on after the fix changes the mode; the off fails with the mode on and its hook in place
  [@test] crates/app-settings/src/unattended_test.rs (`a_switch_whose_save_fails_puts_the_mode_back`)

#### Scenario: No data directory of the app
- **WHEN** `brd unattended on` runs where brd knows no data directory of the app
- **THEN** it answers `invalid_arguments` and switches nothing
  [@test] crates/bd-cli/src/unattended_test.rs (`unattended_without_a_data_directory_is_refused`)

### Requirement: Hook: The hook is in the settings of Claude Code only while the mode is on

Turning the mode on MUST first add the hook to `~/.claude/settings.json`, then switch the setting; a hook that cannot be
written (no home directory or brd, a settings file that cannot take it, a failed write) or a Claude Code that would not
run it (`claude --version` older than 2.1.295, when the host finds `claude`) MUST keep the mode off and answer
`install_refused` with why; a journal of the run before that cannot be removed MUST keep the mode off and take the hook
away again. Turning it off MUST switch the setting first, then remove the hook; a removal that fails MUST be the
`warning` of the answer, the mode off all the same. A crash or a quit while the mode is on MUST leave the hook and the
mode on.

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

#### Scenario: An older Claude Code
- **GIVEN** a `claude` that answers `2.1.294 (Claude Code)` to `--version`
- **WHEN** the mode is turned on
- **THEN** it stays off, the answer says to update Claude Code to 2.1.295 or later, and the settings of Claude Code are not written; with `2.1.295` it turns on
  [@test] crates/app-settings/src/unattended_test.rs (`a_switch_on_with_an_older_claude_code_stays_off`)
  [@test] crates/app-settings/src/claude_hook_test.rs (`the_version_of_claude_code_is_the_first_word_of_its_answer`)

#### Scenario: A journal that cannot be removed
- **GIVEN** a journal of the run before that no removal of a file takes
- **WHEN** the mode is turned on
- **THEN** it stays off, and the settings of Claude Code are as they were
  [@test] crates/app-settings/src/unattended_test.rs (`a_switch_on_whose_journal_cannot_be_removed_takes_the_hook_away`)

#### Scenario: A removal that fails
- **GIVEN** the mode is on and the user broke the settings file of Claude Code
- **WHEN** the mode is turned off
- **THEN** it is off, and the answer warns that the hook stayed
  [@test] crates/app-settings/src/unattended_test.rs (`a_switch_on_whose_hook_cannot_be_written_stays_off`)

### Requirement: Hook file: An edit adds the hook once and keeps the rest of the file

An addition MUST add to `hooks.PermissionRequest` of `~/.claude/settings.json` the entry `{"matcher": "*", "hooks":
[{"type": "command", "command": "<brd>", "args": ["hook", "permission-request"], "onFailure": "continue"}]}`, an entry
of its own, never beside a hook of the user, creating the file, keep every other key, hook and value as written, in its
order, and write nothing when it is there alone in its entry. An edit MUST write the file in its own layout: the indent
of its first key (spaces or a tab), on one line when it was one, with a final line break when it had one; a missing or
blank file as Claude Code writes it, two spaces and a final line break. A file that is no JSON object, or of `hooks` of
another type, MUST be refused and stay; a blank file is an empty one, and a private file stays private.

#### Scenario: A file of the user
- **GIVEN** a settings file with other keys, a hook of another event and another hook of `PermissionRequest`
- **WHEN** the hook is added, then again
- **THEN** the first adds the entry at the end of `PermissionRequest` and keeps the text of the rest; the second writes nothing
  [@test] crates/app-settings/src/claude_hook_test.rs (`an_add_writes_the_hook_once_and_keeps_every_other_key_and_hook_as_written`)

#### Scenario: The layout of the file
- **GIVEN** a settings file of four spaces, one of tabs without a final line break, one on one line
- **WHEN** the hook is added and removed in each, and added to a missing file
- **THEN** each added keeps its indent, its line or its final line break, each removal gives the file back byte for byte, and the missing file is written with two spaces and a final line break
  [@test] crates/app-settings/src/claude_hook_test.rs (`an_edit_keeps_the_layout_of_the_file`)

#### Scenario: A hook of the user in the entry of Branch Deck
- **GIVEN** the hook of this brd and a hook of the user in one entry
- **WHEN** the hook is added, then removed
- **THEN** the add writes the hook of Branch Deck into an entry of its own, and the removal takes that entry alone, the entry of the user as the add wrote it
  [@test] crates/app-settings/src/claude_hook_test.rs (`the_hook_of_branch_deck_never_shares_an_entry_with_a_hook_of_the_user`)

#### Scenario: A broken file
- **WHEN** an edit meets an array, a cut file, or `hooks` of the wrong type
- **THEN** it is refused with why, naming the file, and the file stays as it was
  [@test] crates/app-settings/src/claude_hook_test.rs (`a_file_that_is_no_object_is_refused_and_stays`)

#### Scenario: A private file
- **WHEN** the hook is added to a file of mode 0600
- **THEN** the file stays 0600
  [@test] crates/app-settings/src/claude_hook_test.rs (`a_private_file_stays_private`)

### Requirement: Install: brd install keeps the hook as the mode has it

`brd install` MUST write no hook of its own: while the mode is off it MUST remove a hook of Branch Deck an earlier
install wrote, while it is on it MUST write the hook anew for this brd, deciding by the mode in the settings file of
the app under the lock of a switch, and name the file in `hook_file` when it changed it; a settings file it cannot
change MUST stay as it is, the `brd` command and the skills installed all the same and `hook_error` saying why.
`brd install --uninstall` MUST turn the mode off through the switch of `brd unattended off`, which removes the hook,
and answer `unattended_off` when the mode was on, so no mode stays on without its hook.

#### Scenario: The hook of an earlier install
- **GIVEN** the hook of an earlier install, of the shell form, beside a value of the user
- **WHEN** `brd install` runs with the mode off, then again; then with the mode on; then `brd install --uninstall` with the mode on, then again
- **THEN** the first removes the hook and keeps the value, the second changes nothing, the install while on writes the hook of this brd, the uninstall turns the mode off with `unattended_off: true` and removes the hook, and the second uninstall changes nothing
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
hooks of Branch Deck, an entry they leave empty, a `PermissionRequest` and a `hooks` they leave empty but those the file
had empty before the first add of the run (kept in `unattended-hook.json` of the app data directory until the removal),
and keep everything else as written; a file without such a hook MUST stay unwritten. A file the user did not change
while the mode was on MUST so come back byte for byte, in its layout, unless the user's file had a hook of theirs in the
entry of Branch Deck or its text was in no layout of one indent (`{ "model": "opus" }`, line breaks of `\r\n`), whose removal gives back the
same JSON.

#### Scenario: Uninstall
- **WHEN** the hook is removed after it was added to a file of the user
- **THEN** the file is what it was before, byte for byte
  [@test] crates/app-settings/src/claude_hook_test.rs (`a_removal_takes_only_the_hook_of_branch_deck_and_gives_the_file_back`)
  [@test] crates/bd-cli/src/install_test.rs (`an_install_keeps_the_hook_of_unattended_mode_as_the_mode_has_it`)

#### Scenario: Containers the file had empty
- **GIVEN** settings of Claude Code with `"hooks": {}`, or `"PermissionRequest": []` alone in `hooks` or beside another event
- **WHEN** the mode is turned on, on again by a brd that moved, then off
- **THEN** the file comes back byte for byte with its empty containers, and the memo of the run is gone; a removal that knows nothing of them takes an emptied container, as an add made it
  [@test] crates/app-settings/src/claude_hook_test.rs (`the_containers_the_file_had_empty_stay_after_the_removal`)
  [@test] crates/app-settings/src/unattended_test.rs (`the_containers_the_settings_had_empty_stay_after_the_mode`)

### Requirement: App: The app menu holds Unattended Mode, checked while it is on

The menu bar MUST hold the check item "Unattended Mode", after Settings… (the app menu of macOS, File on Windows and
Linux, File of ☰ under a custom title bar), checked while the mode is on. Choosing it MUST switch the mode through the
item of the palette, with its toasts and errors, and with no main window (closed while an IDE keeps the app) through
the core of the switch itself, to the other state of the mode; the item MUST follow every change of the setting the app
sends its windows, from any surface or `brd unattended`, from Rust with no window too, and the main window MUST check it
as well; a switch on that fails MUST leave it unchecked.

#### Scenario: The item of the native menu
- **GIVEN** the native menu, built with the item checked as the stored mode
- **WHEN** the user chooses Unattended Mode, then again
- **THEN** the item checks itself back, the main window gets `unattended_mode` and turns the mode on, then off with the toast of a switch off, and checks the item each time
  [@test] src-tauri/src/menu_test.rs (`unattended_mode_is_checked_as_the_stored_mode`, `a_click_of_unattended_mode_shows_the_mode_again_and_sends_the_switch_to_the_main_window`)
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should switch the mode from Unattended Mode of the app menu, with the toasts of the mode, and check the item as the mode is, whichever surface switched it`)

#### Scenario: No main window
- **GIVEN** the app with no main window, an IDE keeping it
- **WHEN** the user chooses Unattended Mode, or `brd unattended` switches the mode
- **THEN** the click switches the mode to its other state through the core, and the item follows each change of the setting
  [@test] src-tauri/src/menu_test.rs (`a_click_of_unattended_mode_with_no_main_window_switches_the_mode_itself`, `unattended_mode_follows_each_change_of_the_setting_the_windows_get`)

#### Scenario: Another surface, a failed switch
- **WHEN** the palette turns the mode on, Settings → General turns it off, then a switch on from the menu fails
- **THEN** the item is checked, then unchecked, and stays unchecked after the error toast
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should switch the mode from Unattended Mode of the app menu, with the toasts of the mode, and check the item as the mode is, whichever surface switched it`)

#### Scenario: The main menu of a custom title bar
- **WHEN** the user chooses Unattended Mode in File of ☰, then again
- **THEN** the mode turns on, the item reads checked, and the mode turns off
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should switch the mode from Unattended Mode of the main menu of a custom title bar, checked as the mode is`)
  [@test] src/lib/main-menu.test.ts (`disables Sync with the reason of the Sync button, checks Auto-sync and Unattended Mode, offers updates in a release build`)

## ADDED Requirements

### Requirement: Setting: One switch at a time across processes

A switch of the app, of `brd unattended` and the hook refresh of `brd install` MUST run one at a time across processes,
under the lock `unattended.lock` of the app data directory, held by the read of the mode, the decision and the writes of
the hook and the setting, never across the network; a switch MUST decide by the mode as the settings file has it, what
another process saved a moment ago included, not as the copy of its writer had it. So the mode in the file MUST always
have its hook, whichever processes switch it at once.

#### Scenario: Two processes switch at once
- **GIVEN** two writers of the settings of one app data directory, the second loaded before the first turns the mode on
- **WHEN** the second turns it off, then both switch it ten times at once, then `brd install` refreshes the hook
- **THEN** the off finds the mode on and removes the hook; after the switches the hook is in the settings of Claude Code exactly while the mode is on in the file, and the refresh keeps it so
  [@test] crates/app-settings/src/unattended_test.rs (`switches_of_two_processes_take_turns_and_decide_by_the_file`)
