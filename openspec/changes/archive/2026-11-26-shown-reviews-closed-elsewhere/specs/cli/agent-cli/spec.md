## MODIFIED Requirements

### Requirement: `brd close-review`: It closes the shown review of a branch once it is approved

`brd close-review <branch>` MUST close the shown review of a branch and delete its branches once every reviewer it
asked approved, printing its number, state, who approved it and the comment; it MUST refuse one not approved
(`review_not_approved`) unless `--allow-unapproved`, never a next step, and one the branch lacks (`no_shown_review`);
`--dry-run` closes nothing. Of a review closed on GitHub it MUST delete the branches left at its tips (`deleted`).

#### Scenario: A close before and after the approvals
- **WHEN** `brd close-review auth` runs while alice waits, then after she approved
- **THEN** the first is `review_not_approved` with no next step, the second closes it and deletes both branches
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)

#### Scenario: A review closed on GitHub
- **WHEN** the review of auth was closed on GitHub and `brd close-review auth --dry-run` runs, then without it, then again
- **THEN** the first names both branches in `deleted` and deletes nothing, the second deletes them, the third is
  `no_shown_review`
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_close_of_a_review_closed_on_github_deletes_its_branches_still_at_its_tips`)
