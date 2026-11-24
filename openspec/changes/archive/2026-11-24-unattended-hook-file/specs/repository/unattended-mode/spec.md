## RENAMED Requirements

- FROM: `### Requirement: Install: brd install adds the hook once and keeps the rest of the file`
- TO: `### Requirement: Hook file: An edit adds the hook once and keeps the rest of the file`
- FROM: `### Requirement: Install: The hook runs brd with no shell and never blocks`
- TO: `### Requirement: Hook file: The hook runs brd with no shell and never blocks`
- FROM: `### Requirement: Install: An edit keeps the writes of Claude Code and the symlink`
- TO: `### Requirement: Hook file: An edit keeps the writes of Claude Code and the symlink`
- FROM: `### Requirement: Install: A moved brd replaces the hook of the old one`
- TO: `### Requirement: Hook file: A hook of another brd or of the shell form is replaced`

## MODIFIED Requirements

### Requirement: Hook file: An edit adds the hook once and keeps the rest of the file

An addition of the hook MUST add to `hooks.PermissionRequest` of `~/.claude/settings.json` the entry `{"matcher": "*",
"hooks": [{"type": "command", "command": "<brd>", "args": ["hook", "permission-request"], "onFailure": "continue"}]}`,
creating the file, and MUST keep every other key, hook and value of the file as it was written, in its order; an
addition when the hook is there MUST write nothing. A file that is no JSON object, or whose `hooks` is no object or
`hooks.PermissionRequest` no array, MUST be refused and stay as it is; a blank file MUST be read as an empty one, and a
file only its owner reads MUST stay so.

#### Scenario: A file of the user
- **GIVEN** a settings file with other keys, a hook of another event and another hook of `PermissionRequest`
- **WHEN** the hook is added, then again
- **THEN** the first adds the entry at the end of `PermissionRequest` and keeps the text of the rest; the second writes nothing
  [@test] crates/app-settings/src/claude_hook_test.rs (`an_add_writes_the_hook_once_and_keeps_every_other_key_and_hook_as_written`)

#### Scenario: A broken file
- **WHEN** an edit meets an array, a cut file, or `hooks` of the wrong type
- **THEN** it is refused with why, naming the file, and the file stays as it was
  [@test] crates/app-settings/src/claude_hook_test.rs (`a_file_that_is_no_object_is_refused_and_stays`)

#### Scenario: A private file
- **WHEN** the hook is added to a file of mode 0600
- **THEN** the file stays 0600
  [@test] crates/app-settings/src/claude_hook_test.rs (`a_private_file_stays_private`)

### Requirement: Hook file: A hook of another brd or of the shell form is replaced

A command hook whose `args` end with `hook permission-request`, or of the shell form whose command ends with it, MUST be
read as Branch Deck's: an addition MUST replace every such hook of another command or of the shell form by one hook of
this brd, keep the other hooks of its entry, and keep what the user added to the hook of this brd (a `timeout`, another
`onFailure`); a removal MUST remove each of them.

#### Scenario: The app moved
- **GIVEN** the hook of the app binary of another place, with a hook of the user in its entry
- **WHEN** the hook is added
- **THEN** the entry keeps the hook of the user, and one entry has the hook of this brd
  [@test] crates/app-settings/src/claude_hook_test.rs (`an_add_of_a_moved_brd_replaces_the_command_of_the_old_one`)

#### Scenario: A hook of the shell form
- **GIVEN** the hook of this brd as a command line of the shell form, as the first installs wrote it
- **WHEN** the hook is added; or removed
- **THEN** the hook of the exec form takes its place; the removal removes it
  [@test] crates/app-settings/src/claude_hook_test.rs (`a_hook_of_the_shell_form_is_replaced_by_the_exec_form`)
