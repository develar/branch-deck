## MODIFIED Requirements

### Requirement: Files section: Inline is the default, a stored choice wins

Inline MUST be the default, the first of the toggle (Inline · Unified · Split, from the most compact one), where
IntelliJ's diff opens side by side: one column reads at any width of its pane. A stored choice keeps winning over it;
a choice of the default is not stored.

#### Scenario: Nothing stored
- **WHEN** a diff view opens and the user never chose a view
- **THEN** it shows Inline, the first of Inline · Unified · Split, pressed
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)
  [@test] tests/e2e/settings/settings.spec.ts (`should set every diff from Diffs, the Whitespace menu naming its choice`)

#### Scenario: A stored choice
- **GIVEN** the user chose another view before
- **WHEN** a diff view opens
- **THEN** it shows the stored choice, not the Inline default
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should keep a stored diff view mode over the Inline default`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Split shows an added file as its one side, with its line numbers, as IntelliJ's one-sided viewer`)

#### Scenario: Inline chosen again
- **GIVEN** a stored choice of Unified
- **WHEN** the user chooses Inline
- **THEN** nothing of the layout is stored, as only what differs from the default is
  [@test] tests/e2e/settings/settings.spec.ts (`should set every diff from Diffs, the Whitespace menu naming its choice`)

## RENAMED Requirements

- FROM: `### Requirement: Files section: Split is the default, a stored choice wins`
- TO: `### Requirement: Files section: Inline is the default, a stored choice wins`
