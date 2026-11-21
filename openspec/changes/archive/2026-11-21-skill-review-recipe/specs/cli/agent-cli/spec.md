## ADDED Requirements

### Requirement: `brd install`: The skill says how to review a change by `brd review-plan`

The skill MUST say how an agent reviews a commit, a branch or a range: `brd review-plan`, its `reading` first, with
nothing to read said so; its `units` in the order printed, each `mechanical` kind the output prints passed by name,
every other unit read; a move read where it stands now; no removal skipped; a report of the changes of behaviour
alone, with their file and lines, or that there is none. Every name in backticks of the recipe MUST be a field or a
value of that output.

#### Scenario: The review recipe
- **WHEN** an agent asked to review a commit, a branch or a range reads the skill
- **THEN** it runs `brd review-plan` and reads its `reading` first, then its `units` in their order
- **AND** it passes only the `mechanical` units, each of their kinds named as the output spells it, reads every
  other kind of unit, a move where it stands now and every removal
- **AND** it reports only the changes of behaviour, each with its file, its lines and what observably changes, or
  says there is none
- **AND** a new kind of unit or of mechanical change, or a name in backticks the output does not print, fails the test
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_reviews_by_the_reading_and_the_units_of_the_plan_and_passes_only_the_mechanical_ones`)

## MODIFIED Requirements

### Requirement: `brd install`: The description of the skill says when to use brd

The description of the skill MUST say when to use brd: first of all before the first edit or commit in a Git
repository, for `brd workspace` ([brd workspace](../workspace/spec.md)); then grouping into branches and pull requests,
stacked when they build on each other, and updating onto the baseline with its conflicts, landing a worktree's commits,
fixups, a change in the wrong commit, the review of a commit, a branch or a range, a review request, why a robot failed.

#### Scenario: The triggers of the skill
- **WHEN** an agent reads the description of the skill
- **THEN** it names `brd workspace` before the first edit or commit, then the tasks of brd
- **AND** the landing of a worktree's commits onto a shared branch is among its triggers
- **AND** so is the review of a commit, a branch or a range
  [@test] crates/bd-cli/src/agent_help_test.rs (`the_skill_reviews_by_the_reading_and_the_units_of_the_plan_and_passes_only_the_mechanical_ones`)

### Requirement: `brd install`: The details of an output belong in the help of the binary

The details of an output MUST belong in the binary's help, not in the skill: the skill names only the fields its rules
and its recipe of a review act on.

#### Scenario: The details of an output
- **WHEN** an agent needs the fields of an output
- **THEN** it finds them in the help of the binary, not in the skill
