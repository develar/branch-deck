## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: The popover shows the reason of the gate with its fix

The popover of the start (S, a click, Dry Run) MUST then open first (a confirm reason, `landing`) and show the reason
before the start, short ([Landing Plan](../landing-plan/spec.md#popover)), with the fix as its button, which restacks and leaves the popover open to
read the start again.

#### Scenario: The fix in the popover
- **WHEN** the start of a branch the gate refuses is asked
- **THEN** the popover shows the reason and the fix "Stack JIRA-456 on ABC-123"
- **AND** the fix restacks and leaves the popover open
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say in the Safe Push popover why the build fails before the start, and stack the branch from it`)
  [@test] src/presenters/land.test.ts (`opens the popover when the build of what it lands needs a branch outside it: the reason, the fix and Start Anyway before the start`)
