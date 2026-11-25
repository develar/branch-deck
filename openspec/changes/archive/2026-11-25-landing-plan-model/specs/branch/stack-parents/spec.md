## ADDED Requirements

### Requirement: Restack, unstack, Stack Together and rename: Land Together is a restack along the edge

Land Together of the landing plan MUST be a restack of the root of the unit that waits onto the branch it needs
(`RewriteIntent::Restack` naming the refusal it fixes), directional by the edge, one undo; never Stack Together, which
chains by the age of the commits.

#### Scenario: Land Together of lane-run
- **WHEN** the user presses Land Together of `lane-run`, which needs `stills-codec`
- **THEN** one restack stacks `lane-run` on `stills-codec`, and one undo puts it back
  [@test] crates/domain-policy/src/landing_plan_test.rs (`land_together_restacks_onto_the_branch_a_need_of_the_build_nothing_lands_with_its_price`)
  [@test] tests/e2e/next-step/land-together.spec.ts (`should land a branch with the branch it needs in one robot from the Safe Push popover, one unit in the Landing overview, and undo it`)
