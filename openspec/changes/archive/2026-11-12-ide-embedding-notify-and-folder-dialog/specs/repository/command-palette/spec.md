## MODIFIED Requirements

### Requirement: Sources: Open Repository is the folder button of the selector

Commands MUST hold Open Repository…, the folder button of the repository selector, disabled with "Wait for the sync to
finish" while a sync runs, as the selector is; where the host has no folder dialog (the IDE,
[IDE Embedding](../ide-embedding/spec.md)), neither has it.

#### Scenario: Open Repository during a sync
- **WHEN** a sync runs
- **THEN** Open Repository… is disabled with "Wait for the sync to finish", as the selector is
  [@test] src/lib/command-palette.test.ts (`hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)

#### Scenario: A host without a folder dialog
- **WHEN** the page runs where the host has no folder dialog
- **THEN** Commands holds no Open Repository…
  [@test] src/lib/command-palette.test.ts (`offers no Open Repository… where the host has no folder dialog`)
