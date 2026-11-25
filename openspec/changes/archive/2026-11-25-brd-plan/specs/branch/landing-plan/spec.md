## ADDED Requirements

### Requirement: Units: `brd plan` prints the plan the app derives

`brd plan` MUST print, read only, the plan of the session over what `brd status` reads: the units, the edges with their
kind, state, evidence and actions (each with its command), the waves, the parked branches, and the proposals of Land
Together with their price and their `command` (`brd restack <from> <onto>`), which waits for the user's approval.

#### Scenario: The plan of flow and util
- **WHEN** on Space `flow` uses a name `util` adds and `ui` is built on `flow`, and the agent runs `brd plan`
- **THEN** it prints the units `flow` (with `ui`) and `util`, the open edge with Land Together and Land util First, the
  waves `next: util`, `after util: flow`, and the proposal with its price and command; HEAD stays
  [@test] crates/bd-remote/src/plan_test.rs (`plan_prints_the_units_the_edge_the_waves_and_land_together_with_its_price_and_command`)
