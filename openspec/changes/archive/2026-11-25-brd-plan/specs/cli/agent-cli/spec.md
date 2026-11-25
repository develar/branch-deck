## ADDED Requirements

### Requirement: `brd plan`: It prints the landing plan, read only

`brd plan` MUST print the landing plan of the repository (`units`, `edges`, `waves`, `parked`, `proposals`, `stale`,
`checks_unknown`) and write nothing; `brd schema plan` MUST describe every field it prints; the agent help MUST name it,
and the `command` of a proposal MUST wait for the user's approval like a `fix` of `brd status`.

#### Scenario: Read only, every field in the schema
- **WHEN** the agent runs `brd plan`
- **THEN** every field it prints is in `brd schema plan` and HEAD does not move
  [@test] crates/bd-remote/src/plan_test.rs (`plan_prints_the_units_the_edge_the_waves_and_land_together_with_its_price_and_command`)
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)
