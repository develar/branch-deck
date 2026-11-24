## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: A refusal of the gate offers the fix and Start Anyway

A refusal of the gate MUST start nothing and push nothing (`missing_declaration`, `dangling_reference`, with every
refusal and its fix, `ErrorDetails::Landing`): its error toast MUST offer the fix as its action, a restack of the root
of what lands, so the next start lands that branch too, and "Start Anyway" below it, the same start with the override
of what refused it (a name the user knows is a false alarm), planned again.

#### Scenario: The toast of a refused whole stack
- **WHEN** the gate refuses the Safe Push of a whole stack
- **THEN** the error toast "Cannot start the Safe Push of the whole lane-run stack" says "“Show each scenario” of
  lane-run uses Stills (lane/src/BuildLog.kt:3), which “Name lossy stills” of stills-codec adds: git merges it, but the
  build of the whole lane-run stack fails. Stack lane-run on stills-codec to land it too." (one more refusal: "One more
  name breaks it too.")
- **AND** its actions are "Stack lane-run on stills-codec" and "Start Anyway" below it
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway`)
  [@test] src/presenters/outcome.test.ts (`offers the fix and Start Anyway of the landing gate, Update Project of a conflict with the target, the start again of a changed plan`)
  [@test] src/presenters/landing.test.ts (`refuses the robot of 2026-10-21 with the name, both commits and the restack that lands the branch that adds it`)

### Requirement: Starting a Safe Push or a Dry Run: An unassigned commit or a cycle offers only Start Anyway

For a commit unassigned the text MUST name its group into the branch that needs it, for a restack that would close a
cycle the start that lands both, never a move; "Start Anyway" MUST be the only action of the toast.

#### Scenario: The fix is the move of a commit
- **WHEN** the fix would be a commit unassigned or a restack that would close a cycle
- **THEN** the text names "Group ecebf000 into lane-run", or "The Safe Push of acp lands both.", and "Start Anyway" is
  the only action
  [@test] src/presenters/landing.test.ts (`counts the other refusals, names an unassigned commit by its hash, and offers no action for a move`)
  [@test] src/presenters/reasons.test.ts (`words every reason of the catalog Rust writes as the page says it`)
