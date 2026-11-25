## ADDED Requirements

### Requirement: What the Safe Push needs: The whole stack takes the needs of its branches outside it

The whole stack MUST take the landing needs of its branches outside it, those of its unit in the landing plan
(`landing_edges::whole_stack_refusals`, its edges by `landing_edges`), a need of a branch of it met by the whole stack:
its step takes rows 0 and 1′ as a branch's, so its root's row, the overview, the Plan and `brd status` say one thing,
"Safe Push · N" ready only without an open need. Its start reads the fresh needs, else its gate checks it.

#### Scenario: A branch of the stack needs a branch outside it
- **WHEN** lane-run, the root of a stack that forks, uses Stills, which stills-codec adds
- **THEN** the step of the whole stack is row 1′, "Needs stills-codec", on the row of lane-run as in the landing overview, its node a step of the user
- **AND** its popover says why before the start, its start Start Anyway
  [@test] crates/repository-model/src/derive_test.rs (`the_whole_stack_takes_the_needs_of_its_branches_outside_it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say on the root's row and in the popover what the whole stack needs outside it before the start, and start it anyway past the refusal`)

#### Scenario: What it needs is landing
- **WHEN** a running Safe Push lands the branch the whole stack needs
- **THEN** its step is "Lands after stills-codec", a wait: no start in the slot of the row of its root
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say on the root's row that the whole stack lands after the running Safe Push of what it needs, with no start in its slot`)

#### Scenario: A need of a branch of the stack
- **WHEN** a branch of the stack needs another branch of it
- **THEN** the whole stack, which lands both, has no need of it, nor of a declared need one branch of it inherits from another
  [@test] crates/domain-policy/src/landing_edges_test.rs (`the_whole_stack_is_refused_by_what_its_branches_need_outside_it_each_need_once`)
  [@test] crates/repository-model/src/derive_test.rs (`the_whole_stack_takes_the_needs_of_its_branches_outside_it`)

#### Scenario: Needs of new tips are computed
- **WHEN** the needs of tips that moved are computed
- **THEN** the step keeps the need before, and the gate of the start checks the whole stack
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack in the gate of its start while the needs of new tips are computed, with the restack and Start Anyway`)

## REMOVED Requirements

### Requirement: What the Safe Push needs: The whole stack has no landing needs of its own

**Reason**: the whole stack takes the needs of its branches outside it, as its unit in the plan.
**Migration**: "What the Safe Push needs: The whole stack takes the needs of its branches outside it".
