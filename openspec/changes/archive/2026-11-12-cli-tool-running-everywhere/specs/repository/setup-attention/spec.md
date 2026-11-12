## ADDED Requirements

### Requirement: The dot: An item of a CLI whose action runs shows it

An item of a CLI whose action runs MUST show its last line instead of its buttons, wherever the action started (the
item, Settings → Tools), as Rust keeps the actions that run ([Command-Line Tools](../cli-tools/spec.md)).

#### Scenario: The dot opened again while an update runs
- **GIVEN** the user chose Update on "teamcity 1.7.0 is out"
- **WHEN** the dot opens again while the update runs
- **THEN** the item shows its last line, without Update or Skip This Version
  [@test] tests/e2e/repository/setup-dot.spec.ts (`should show the update an item started as running, in the dot and in Settings opened meanwhile, which offers no second one`)
