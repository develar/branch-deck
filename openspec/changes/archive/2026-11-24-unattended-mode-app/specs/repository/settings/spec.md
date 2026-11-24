## RENAMED Requirements

- FROM: `### Requirement: Pages: General has Auto-sync on Focus and Suggest branch names`
- TO: `### Requirement: Pages: General has Auto-sync on Focus, Suggest branch names and Unattended Mode`

## MODIFIED Requirements

### Requirement: Pages: General has Auto-sync on Focus, Suggest branch names and Unattended Mode

General SHALL have "Sync", Auto-sync on Focus, the setting of the item of the File menu, whose check the main window
keeps; "AI", Suggest branch names ([Branch Name Suggestions](../../branch/branch-name-suggestions/spec.md)); and
"Coding agents", Unattended Mode ([Unattended Mode](../unattended-mode/spec.md)).

#### Scenario: Auto-sync on Focus
- **WHEN** a window changes Auto-sync on Focus
- **THEN** the main window checks the item of the File menu as the setting is
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)

#### Scenario: Suggest branch names
- **WHEN** the user turns Suggest branch names on or off under "AI"
- **THEN** the AI suggestions of branch names follow it
  [@test] tests/e2e/inline-branch-creator-ai/inline-branch-creator-ai.spec.ts (`should turn AI on and off in Settings`)

#### Scenario: Unattended Mode
- **WHEN** the user turns Unattended Mode on or off under "Coding agents"
- **THEN** the mode follows it, the mark of the main window with it
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should turn the mode on and off from Settings → General, the mark of the main window following it`)
