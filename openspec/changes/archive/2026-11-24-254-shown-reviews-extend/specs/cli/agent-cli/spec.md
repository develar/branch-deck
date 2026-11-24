## ADDED Requirements

### Requirement: `brd request-review`: A branch with a shown review opens it or takes the fixes that landed

`brd request-review <branch>` of a branch whose review is a shown one MUST open it or, once a fix of the branch landed by
its own Safe Push, add the fix to it as the app does (`shown.extended`: `fixes`, `head`, `not_applied`), with or without
`--after-landing`, also of a branch that left the table; a repeat adds nothing. `brd robot <branch>` MUST name the
shown review a running or merged Safe Push of a fix extends (`extends`).

#### Scenario: A fix that landed
- **WHEN** `brd robot auth` runs while a fix runs, then `brd request-review auth` after it merged, then again
- **THEN** the first names the review in `extends`, the second adds the fix and asks alice again, the third adds nothing
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_fix_that_landed_goes_onto_the_shown_review_and_its_reviewers_are_asked_again`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_after_landing`)
