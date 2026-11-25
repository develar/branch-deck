## ADDED Requirements

### Requirement: The whole stack: A branch of it says a need of its unit as a state

A branch of a stack whose root's row hosts its whole stack, every need of whose start is an edge of that unit, MUST
say it as a state (row 1′, `StepKind::WithStack`, its node a wait): "Needs stills-codec with lane-run", quiet, no step
in its slot, its press the row of the root, which shows the need once and fixes it. A need the unit lacks stays its
own row 1′, one that is landing "Lands after stills-codec"; its own start still reads its needs.

#### Scenario: One need of the unit, once
- **WHEN** lane-run forks and its branches inherit its need of stills-codec
- **THEN** exactly one slot of the stack says "Needs stills-codec", the root's; lane-docs and ui-perf-nightly say "Needs stills-codec with lane-run", whose press shows the row of lane-run
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_of_a_branch_of_a_hosted_whole_stack_is_a_state_while_the_unit_carries_its_need`)
  [@test] src/presenters/next-step.test.ts (`row 1′: a branch of a stack whose root's row hosts its whole stack says a need of the unit as a state that shows the root's row`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say once, on the root's row, what the whole stack needs outside it, the branches built on it a state that shows that row, and start it anyway from its popover past the refusal`)

#### Scenario: A need of its own commit
- **WHEN** a commit of login, a branch of the forking auth stack, uses a name only codec adds
- **THEN** the need is one edge of the unit: auth's row shows it, login says "Needs codec with auth", and its own start is refused without Start Anyway, in the app and in brd
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
