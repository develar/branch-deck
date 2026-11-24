## MODIFIED Requirements

### Requirement: Main menu: The items are those of the native menu

The items MUST be those of the native menu (`src/lib/main-menu.ts`), in File, View and Help. There MUST be no Edit
menu: the webview's Ctrl+C/V/X/A stay.

#### Scenario: File
- **WHEN** the user opens File
- **THEN** it holds Sync Branches (Ctrl+R, disabled with the reason of the Sync button), Update Project (Ctrl+T),
  Auto-sync on Focus (checked), Settings… (Ctrl+,), Unattended Mode (checked while the mode is on,
  [Unattended Mode](../unattended-mode/spec.md))

#### Scenario: View
- **WHEN** the user opens View
- **THEN** it holds Command Palette… (Ctrl+K, [Command Palette](../command-palette/spec.md)), Branches (Ctrl+1),
  Reviews (Ctrl+2) and Landing (Ctrl+3) ([Main Views](../main-views/spec.md)), Actual Size (Ctrl+0), Zoom In
  (Ctrl+=), Zoom Out (Ctrl+-), Activity ([Activity](../activity/spec.md))

#### Scenario: Help
- **WHEN** the user opens Help
- **THEN** it holds Getting Started, Install Agent CLI…, Check for Updates… (a build with the `auto-update` feature,
  as the native menu: `checkForUpdates` of the title bar), GitHub Repository

#### Scenario: No Edit menu
- **WHEN** the user presses Ctrl+C/V/X/A in the webview
- **THEN** they do what the webview does, as no Edit menu takes them

### Requirement: Main menu: The native menu sends its commands to the main window

The native menu MUST send a command to the main window as an event named as it (`sync_branches`, `update_project`,
`check_for_updates`, `install_agent_cli`, `zoom` with its step, `open_settings`, `unattended_mode`): the main window
opens Settings ([Settings](../settings/spec.md)), as it has the current settings to start it from, and switches
Unattended Mode with its toasts.

#### Scenario: Settings… of the native menu
- **WHEN** the user chooses Settings… in the native menu
- **THEN** the main window gets the event `open_settings` and opens Settings from its current settings
