## MODIFIED Requirements

### Requirement: `brd`: brd review-plan prints the reading of the pair

`brd review-plan` MUST print the `reading` of its pair, the same `RangeReading` the window reads, with `"schema": 38`.

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** the output of `brd review-plan` with its `reading` matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
