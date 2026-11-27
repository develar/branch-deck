## MODIFIED Requirements

### Requirement: `brd request-review`: It prints whom it asked and whom GitHub refused

It MUST print `reviewers` (whom it asks), `requested`, `refused` with GitHub's reason, `waiting_for` (the reviewers the
pull request asks after the request), the `readiness` before it, each person with the full name Patronus knows, and
`pushed`, the branches it pushed first. `--dry-run` MUST print the same object and push and ask nothing. Of a draft
that asks nobody yet, `reviewers` MUST name whom its body records.

#### Scenario: The output of a request
- **WHEN** a request ran
- **THEN** it prints `reviewers`, `requested`, `refused` with GitHub's reason, `waiting_for`, the `readiness` before it
  and `pushed`, each person with the full name Patronus knows
  [@test] crates/bd-remote/src/request_review_test.rs (`asks_the_logins_it_is_given_and_names_whom_github_refuses`)

#### Scenario: A dry run
- **WHEN** `brd request-review <branch> --dry-run` runs
- **THEN** it prints the same object and pushes and asks nothing

#### Scenario: A reviewer recorded in a draft
- **WHEN** `brd request-review auth --reviewer bob` runs for the marked draft of auth, recording alice, before auth landed
- **THEN** it records bob beside alice in its body, asks nobody, and prints alice and bob in `reviewers`, `requested`
  empty
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_reviewer_added_to_a_draft_before_the_landing_is_recorded_and_named`)

### Requirement: `brd close-review`: It closes the shown review of a branch once it is approved

`brd close-review <branch>` MUST close the shown review of a branch and delete its branches once every reviewer it
asked approved, printing its number, state, who approved it, the comment and the branches it deleted (`deleted`); it
MUST refuse one not approved (`review_not_approved`) unless `--allow-unapproved`, never a next step, and one the branch
lacks (`no_shown_review`); `--dry-run` closes nothing. Of a review closed on GitHub it MUST delete the branches left at
its tips.

#### Scenario: A close before and after the approvals
- **WHEN** `brd close-review auth` runs while alice waits, then after she approved with `--dry-run`, then without it
- **THEN** the first is `review_not_approved` with no next step, the second names both branches in `deleted` and
  closes nothing, the third closes it, deletes both branches and names them in `deleted`
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)

#### Scenario: A review closed on GitHub
- **WHEN** the review of auth was closed on GitHub and `brd close-review auth --dry-run` runs, then without it, then again
- **THEN** the first names both branches in `deleted` and deletes nothing, the second deletes them, the third is
  `no_shown_review`
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_close_of_a_review_closed_on_github_deletes_its_branches_still_at_its_tips`)
