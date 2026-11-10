## ADDED Requirements

### Requirement: Branch menu: Two menus of one branch at once each follow the stores

Every surface of the menu of a branch (the right-click, its "⋯", the "⋯" of the preview, the command palette) MUST read
it on its own: two of them on one branch at once, a menu that still fades out beside the palette opened on its row,
each show the menu as the stores are now, and the window keeps its rows. The palette MUST read the menu of the branch at
hand only while it shows.

#### Scenario: The palette opens on a row whose menu still fades out
- **GIVEN** the menu of a row still fades out after its Unstack was pressed, the first sync of a warm start held
- **WHEN** the palette opens on the row, and the sync of the unstack comes
- **THEN** the window keeps its rows, the branch at the root
- **AND** the palette lists the menu of the branch as it is now, without the Unstack
  [@test] tests/e2e/startup/warm-start.spec.ts (`keeps the window when the palette opens on a row whose menu still fades out and the sync of the unstack pressed in it comes`)

#### Scenario: The menu of the row the palette listed last, during a sync
- **GIVEN** the palette listed the actions of a row and closed
- **WHEN** the menu of that row is open while a sync runs
- **THEN** it keeps its items, and the window its rows
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should keep the window when the row the palette listed last opens its menu and a sync runs`)
