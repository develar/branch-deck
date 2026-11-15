## MODIFIED Requirements

### Requirement: Pull requests: A comment can't choose a merge profile

A comment can't choose a merge profile: the popover MUST offer none, and "Safe Push With Profile…" MUST be disabled.
The plan of a start MUST refuse a profile the user chose for it (`profileOnComment`), never drop it and start the
default checks in its place; the profile the last robot ran with is no choice.

#### Scenario: Safe Push With Profile is disabled
- **GIVEN** a branch starts by a comment on #7
- **WHEN** the user looks at "Safe Push With Profile…"
- **THEN** it is disabled ("A comment on #7 runs the default checks: it can't choose a profile"), and the popover offers no profile
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)

#### Scenario: A branch whose own profile is another one
- **GIVEN** the own profile of the branch is Large Scale Change
- **WHEN** the popover of its start by a comment opens
- **THEN** it says "A comment runs the default checks: it can't choose Large Scale Change."

#### Scenario: A profile chosen anyway
- **GIVEN** a branch starts by a comment on #7
- **WHEN** a start with a profile the user chose is asked anyway (a window of a state before, another client)
- **THEN** its plan has the blocker "A comment on #7 runs the default checks: it can't choose a profile", and its run refuses it (`patronus_refused`) before any push or comment
- **AND** with the default checks the comment starts it
  [@test] crates/domain-policy/src/land_test.rs (`the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should refuse in the backend a Safe Push with a profile the user chose that the pull request starts by its comment, never drop the profile`)
