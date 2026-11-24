## MODIFIED Requirements

### Requirement: Sources: The groups stand in their order

The groups MUST stand in this order (`paletteCommands`): Views, Activity, Commands (Add Pull Request by Link…, Show Disk
Usage…, Open Repository…, the items of the app's menu, then the switch of Unattended Mode), the actions of the branch
at hand (below) and Branches.

#### Scenario: The groups of the palette
- **WHEN** the palette opens with a branch at hand
- **THEN** its groups are Views, Activity, Commands, the actions of the branch at hand and Branches, in this order
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)

#### Scenario: The order of Commands
- **GIVEN** a repository selected
- **WHEN** the palette opens
- **THEN** Commands lists Add Pull Request by Link…, Show Disk Usage…, Open Repository…, the items of the app's menu, then Turn On Unattended Mode
  [@test] src/lib/command-palette.test.ts (`offers Show Disk Usage… with a repository, before Open Repository…`, `offers the switch of Unattended Mode after the items of the app's menu, and runs the item the title bar runs`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should show the views, Activity and every command of the app without a scroll in the window of the start`)
