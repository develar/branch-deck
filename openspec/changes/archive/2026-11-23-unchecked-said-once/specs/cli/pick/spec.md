## MODIFIED Requirements

### Requirement: The check of the tree: --allow-unchecked lands without a check and says why

`--allow-unchecked <reason>` MUST land without a check, of a worktree or of revisions: the reason is required (a blank
one is `invalid_arguments`). A pick that landed commits MUST repeat it in the output (`unchecked`) and name it once on
stderr; a dry run, a conflict and a pick that landed nothing MUST do neither.

#### Scenario: A pick with a reason
- **WHEN** a pick of a worktree or of revisions runs with `--allow-unchecked <reason>`
- **THEN** it lands without a check, the output repeats the reason (`unchecked`) and stderr names it
  [@test] crates/bd-update/src/pick_test.rs (`allow_unchecked_lands_a_pick_without_a_check_and_says_why`)

#### Scenario: A blank reason
- **WHEN** the reason of `--allow-unchecked` is blank
- **THEN** the pick is `invalid_arguments`
  [@test] crates/bd-update/src/pick_test.rs (`allow_unchecked_lands_a_pick_without_a_check_and_says_why`)
