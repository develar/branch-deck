## MODIFIED Requirements

### Requirement: Companion repositories: One writer writes both sides

`brd companion add <path> [--name]`, `brd companion remove <name>`, the section Companions of Settings → Landing (the
tab of the repository) and its Add Companion… (the folder dialog, named by the directory; none where the host has no
folder dialog, the IDE) and Remove MUST write both sides through one writer (`repository_config::write_companion`).

#### Scenario: Add and remove in brd
- **WHEN** `brd companion add <path> [--name]` or `brd companion remove <name>` runs
- **THEN** both sides are written through `repository_config::write_companion`
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

#### Scenario: Add Companion… and Remove in Settings
- **WHEN** the user picks a folder in the folder dialog of Add Companion… in the section Companions of Settings → Landing, the tab of the repository, then removes it
- **THEN** the companion is added under the name of its directory, then removed, both sides written
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it`)
