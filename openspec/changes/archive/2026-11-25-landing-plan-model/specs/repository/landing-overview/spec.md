## MODIFIED Requirements

### Requirement: Units: A unit is a lone branch or a stack, with the node of the whole stack

A unit is what lands in one move: a lone branch, or a stack (its root's row), as Rust decides it
(`RepositoryView.landingUnits`, `domain_policy::landing_plan`). The node and the next step of a stack MUST be those of
the whole stack, as the header of its island shows them (`useStackNextStep`: of its stack branch while the stack
forks, `nextSteps` of the view under `stack:<root>`, else of the last branch of its straight chain).

#### Scenario: A stack is one unit, its root's row with the step of the whole stack
- **GIVEN** a stack of several branches and a lone branch
- **WHEN** the overview lists the user's moves
- **THEN** the stack is one unit, its root's row ("4 branches · 4 commits"), and no row of its other branches
- **AND** a ready stack of 22 branches counts one move
- **AND** the step of its row is the step of the whole stack
  [@test] src/lib/landing-overview.test.ts (`counts the user's moves by unit, a ready stack of 22 branches one move, the most urgent first`)
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)
  [@test] crates/domain-policy/src/landing_plan_test.rs (`the_plan_of_idea_1_lands_two_robots_now_three_branches_next_and_two_units_after_them`)
  [@test] tests/e2e/next-step/land-together.spec.ts (`should land a branch with the branch it needs in one robot from the Safe Push popover, one unit in the Landing overview, and undo it`)

#### Scenario: A stack that forks takes the step of its stack branch
- **GIVEN** a stack that forks
- **WHEN** the view carries the next steps
- **THEN** the unit takes the step of its stack branch, `nextSteps` of the view under `stack:<root>`
  [@test] crates/repository-model/src/derive_test.rs (`the_view_carries_the_next_step_and_the_node_of_each_subject_and_a_patch_only_those_that_changed`)

#### Scenario: A straight chain takes the step of its last branch
- **GIVEN** a stack that is a straight chain
- **WHEN** the overview shows its unit
- **THEN** the node and the step are those of the last branch of the chain
