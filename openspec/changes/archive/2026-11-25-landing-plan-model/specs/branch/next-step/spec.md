## MODIFIED Requirements

### Requirement: What the Safe Push needs: Row 1′ names the branch the build needs and runs the fix

Row 1′ MUST map the edges of the needs to the step, no rule of its own (`domain_policy::next_step`): "Needs
stills-codec", its hover the first edge in one sentence, a click the primary action of the edge
(`domain_policy::actions::edge_actions`): Land Together, the restack of the root of what lands onto that branch;
"⋯" MUST offer the actions of the edge first. A need no action runs MUST open the Safe Push popover.

#### Scenario: The step, its hover and its fix
- **WHEN** `lane-run` needs `stills-codec`
- **THEN** the step is "Needs stills-codec", its hover the edge in one sentence ("Uses Stills of stills-codec in BuildLog.kt. Stack lane-run on stills-codec to land it too."), and a click Land Together: the restack of the root of what lands onto that branch (`EdgeAction`, `formatEdgeAction`); "⋯" offers the actions of the edge first ("Land Together", "Land stills-codec First")
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_needs_a_branch_outside_what_the_safe_push_lands_and_keeps_the_start`)
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the build needs and runs the fix, or opens the Safe Push`)
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say on the row which branch its Safe Push needs before any press, and stack it on that branch from the slot`, `should land the branch with the branch its Safe Push needs from the menu of a branch built on it`)

#### Scenario: A fix no action runs
- **WHEN** the need is a declared order of the branch itself, or a lost binding
- **THEN** a click opens the Safe Push popover
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the user declared lands first, opens the Safe Push, and unstacks a branch that inherits the need`)
