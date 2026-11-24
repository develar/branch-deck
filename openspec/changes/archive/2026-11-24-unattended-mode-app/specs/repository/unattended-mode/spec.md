## ADDED Requirements

### Requirement: App: The title bar marks the mode while it is on, and the mark turns it off

While the mode is on, the title bar of the main window MUST show its mark before Settings, in every state of the window:
the moon in the warning tone, with "Unattended" where the header has room; a click on it MUST turn the mode off through
`set_unattended_mode`, the core of `brd unattended`. While the mode is off, the title bar MUST show nothing of it.

#### Scenario: The mark comes and goes
- **WHEN** the user turns the mode on from the palette, then clicks the mark
- **THEN** the mark reads "Unattended", then the title bar shows nothing of the mode
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on from the palette, show its mark, offer the install of a missing hook, and turn it off by the mark`)

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
