## MODIFIED Requirements

### Requirement: `brd request-review`: A branch with a shown review opens it or takes the fixes that landed

`brd request-review <branch>` of a branch whose review is a shown one MUST open it or, once a fix of the branch landed by
its own Safe Push, add the fix to it as the app does (`shown.extended`), with or without `--after-landing`, also of a
branch that left the table; a repeat adds nothing. `brd robot <branch>` MUST name the review a fix `extends`. With
`--after-landing`, a branch without one MUST be marked ahead of the landing (`shown.marked`), one that landed get it now.

#### Scenario: A fix that landed
- **WHEN** `brd robot auth` runs while a fix runs, then `brd request-review auth` after it merged, then again
- **THEN** the first names the review in `extends`, the second adds the fix and asks alice again, the third adds nothing
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_fix_that_landed_goes_onto_the_shown_review_and_its_reviewers_are_asked_again`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_after_landing`)

#### Scenario: A mark before the landing
- **WHEN** `brd request-review auth --after-landing` runs before auth landed, after a robot of an earlier auth merged,
  then again, then after the plain Safe Push of auth merged
- **THEN** the first opens the draft at the tip of auth (`shown.marked`), the second is `no_shown_review`, the third
  opens it and asks alice
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_mark_before_landing_opens_by_what_lands_the_branch_and_never_by_an_older_robot_of_the_name`)
