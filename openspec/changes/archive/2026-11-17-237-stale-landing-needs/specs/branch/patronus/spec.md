## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: The gate of a branch and of a whole stack

The gate of a branch MUST be its fresh landing needs, which the session computes after its sync (below): the engine
waits for those of tips a sync moved and decides the start on them; the gate of a whole stack the engine MUST check
before the effects of its start (`operations::land_session`, through the object cache of the repository).

#### Scenario: The gate of a whole stack
- **WHEN** the Safe Push of a whole stack starts
- **THEN** the engine checks its gate before the effects
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway`)

#### Scenario: The gate of a branch while the needs of new tips are computed
- **WHEN** a start of a branch runs while the session computes the needs of tips a sync moved
- **THEN** the engine waits for them and decides the start on them, never on the needs before
  [@test] crates/operations/src/land_test.rs (`a_start_planned_while_the_needs_of_new_tips_are_computed_waits_for_them_and_is_decided_on_them`)

### Requirement: Starting a Safe Push or a Dry Run: Start Anyway starts with the overrides of what refused it

The start of that popover MUST be "Start Anyway": the start with the overrides of what refused it (`Land.allow`),
planned again on fresh needs, decided as a click is, offered only there and in the refusal's toast. It MUST lift
only the refusals it showed and those it lifted before (`anyway`) and accept only the confirmations it showed: a newer
refusal refuses the start, whose toast asks again, a newer confirmation opens the popover; the flags of
`brd safe-push` lift every refusal of their kind.

#### Scenario: Start Anyway from the popover
- **WHEN** the user presses "Start Anyway" in the popover
- **THEN** the start is planned again with the override of what refused it, and runs
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should start anyway from the Safe Push popover with the override of what refuses it`)
  [@test] crates/domain-policy/src/land_test.rs (`the_landing_gate_refuses_unless_allowed_then_a_conflict_with_the_target`)
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`)

#### Scenario: The needs changed after the popover showed them
- **WHEN** the user presses "Start Anyway" and the fresh needs refuse the start with a refusal the popover didn't show
- **THEN** that refusal refuses the start, and its toast offers its fix and Start Anyway, which lifts it with those lifted before
  [@test] crates/domain-policy/src/land_test.rs (`start_anyway_lifts_only_the_refusals_it_showed_and_newer_needs_ask_again`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should ask again when newer needs refuse a Start Anyway, and start anyway from its toast past the refusals of both`)
  [@test] src/presenters/landing.test.ts (`refuses the robot of 2026-10-21 with the name, both commits and the restack that lands the branch that adds it`)
  [@test] src/presenters/outcome.test.ts (`offers the fix and Start Anyway of the landing gate, Update Project of a conflict with the target, the start again of a changed plan`)

#### Scenario: A confirmation the popover didn't show
- **WHEN** the user presses "Start Anyway" and the start planned again asks a confirmation its popover or toast didn't show (the cancel of a robot that started meanwhile)
- **THEN** nothing starts, and the popover opens with it; the start accepts only the confirmations it showed (`accepted` of `execute`)
  [@test] src/presenters/land.test.ts (`lets Start Anyway accept only the confirmations its popover or toast showed: one the plan asks now asks again`)
