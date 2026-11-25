## MODIFIED Requirements

### Requirement: `brd install`: The skill says how to review a change by `brd review-plan`

The skill MUST say how an agent reviews a commit, a branch or a range: `brd review-plan`, its `reading` first,
nothing to read said so; its `units` in their order, each `mechanical` kind the output prints passed by name,
every other unit read; a move read where it stands now, between files (`moved_to`, `moved_from`) once; no removal
skipped; a report of the changes of behaviour alone, with file and lines, or none. Every name in backticks of the
recipe MUST be a field or value of that output.

#### Scenario: The review recipe
- **WHEN** an agent asked to review a commit, a branch or a range reads the skill
- **THEN** it runs `brd review-plan` and reads its `reading` first, then its `units` in their order
- **AND** it passes only the `mechanical` units, each of their kinds named as the output spells it, reads every
  other kind of unit, a move where it stands now and every removal
- **AND** it reads why a verdict breaks by `verdict_by` and the `outside_sites` of those entries, and passes the
  generated files a list counts past its first 20 by their kind
- **AND** it reports only the changes of behaviour, each with its file, its lines and what observably changes, or
  says there is none
- **AND** a new kind of unit or of mechanical change, or a name in backticks the output does not print, fails the test
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_reviews_by_the_reading_and_the_units_of_the_plan_and_passes_only_the_mechanical_ones`)

#### Scenario: A move between files
- **GIVEN** a plan whose `removed` unit has `moved_to` and whose `added` unit has `moved_from`, the two places of the
  same code ([Review Units](../../branch/review-units/spec.md#requirement-units-a-declaration-moved-whole-to-another-file-says-where))
- **WHEN** an agent reviews it by the skill
- **THEN** it reads the move once, at its new place, and does not report the removal as a loss
- **AND** the recipe names `moved_to` and `moved_from` as the output spells them, which the test checks against its
  schema
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_reviews_by_the_reading_and_the_units_of_the_plan_and_passes_only_the_mechanical_ones`)
