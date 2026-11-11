## MODIFIED Requirements

### Requirement: Pages: Diffs sets every diff view

Diffs SHALL have one setting of every diff view in each row, as IntelliJ's diff settings: Layout (Inline · Unified ·
Split, the default first) and Whitespace (the menu above the diffs, which says the choice here), the controls above every diff too; then
"Soft wraps" and "Ligatures", each with what it does ([Commit Files](../../commit/commit-files/spec.md#soft-wraps)).

#### Scenario: Layout and Whitespace
- **WHEN** the user chooses a Layout (Inline · Unified · Split) or a Whitespace in Diffs
- **THEN** every diff view follows it
- **AND** the Whitespace menu above the diffs says the choice
  [@test] tests/e2e/settings/settings.spec.ts (`should set every diff from Diffs, the Whitespace menu naming its choice`)

#### Scenario: Soft wraps
- **WHEN** the user turns "Soft wraps" on or off in Diffs
- **THEN** every diff view follows it
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should break a long line with Soft Wraps on in Settings, and scroll it with them off`)

#### Scenario: Ligatures
- **WHEN** the user opens Diffs
- **THEN** "Ligatures" is a row with what it does
