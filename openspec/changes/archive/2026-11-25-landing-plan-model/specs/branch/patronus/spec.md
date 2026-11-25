## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: The popover shows the reason of the gate with its fix

The popover of the start (S, a click, Dry Run) MUST then open first (a confirm reason, `landing`) and show the reason
before the start, a short detail of the gate's (the whole sentence stays the toast's and `brd`'s), with the actions of
its edge as buttons, Land Together first, which restacks and leaves the popover open to read the start again.

#### Scenario: The fix in the popover
- **WHEN** the start of a branch the gate refuses is asked
- **THEN** the popover shows the reason and the actions "Land Together" and "Land ABC-123 First"
- **AND** Land Together restacks and leaves the popover open
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say in the Safe Push popover why the build fails before the start, and land the branch together with it`)
  [@test] src/presenters/land.test.ts (`opens the popover when the build of what it lands needs a branch outside it: the reason, the fix and Start Anyway before the start`)
