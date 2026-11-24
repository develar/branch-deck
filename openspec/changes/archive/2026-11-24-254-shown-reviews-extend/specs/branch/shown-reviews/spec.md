## MODIFIED Requirements

### Requirement: Reviewers: One available member of each owner group, never the author

Without `--reviewer`, a review after landing MUST ask one reviewer of each owner group of the paths the branch changes
(the `OWNERSHIP` files of the target, the members from the CodeOwners service): from the reviewers of the group when
it names any, else from its members, available, not passed by the rotation, with a GitHub login, never the author; one
chosen for a group covers every other group that has them. A lookup that names no reviewers, only members whose
`isReviewer` is null, MUST read, and a member whose availability is not `AVAILABLE` (a vacation) MUST NOT be asked.

#### Scenario: The reviewers of a group before its members
- **WHEN** a group names reviewers and members, the author among them
- **THEN** the first available reviewer but the author is asked, and a group of which nobody can be asked is named
  [@test] crates/domain-policy/src/shown_test.rs (`one_available_reviewer_of_each_owner_group_never_the_author`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_refuses_a_branch_with_a_pull_request`)
  [@test] crates/codeowners/src/client_test.rs (`a_lookup_names_the_reviewers_and_the_members_with_their_logins_and_availability`, `a_group_the_service_lacks_has_no_members_and_a_rejected_token_is_an_error`)

#### Scenario: The lookup as the real service answers it
- **WHEN** the service answers members only, `isReviewer` null, one of them away on vacation, with fields Branch Deck
  doesn't read
- **THEN** the lookup reads, and the member away is not asked
  [@test] crates/codeowners/src/client_test.rs (`a_lookup_of_the_real_shape_has_members_without_a_reviewers_list_and_tells_who_is_away`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_member_away_is_not_asked_of_a_group_as_the_real_service_answers_it`)

### Requirement: Open: Once the robot merged, the review opens and asks its reviewers

Once a merged Safe Push landed the branch, its draft shown review MUST open: its branches go to the commit the robot
landed and its fork point, it is marked ready for review, and the reviewers its body records are asked; a review ready
already MUST only ask again those it asked who neither approved nor are asked now; before such a merge it MUST be
refused (`no_shown_review`). A draft whose branch the fetched baseline landed whole by content while no robot of the
branch is listed (its robot is older than the listing) MUST open on the head it has. A reviewer the user adds
(`--reviewer`) MUST be asked and recorded in its body, the plan built with them.

#### Scenario: The open after the merge
- **WHEN** the robot of the branch merged and the review opens
- **THEN** its head is the commit the robot took, it is ready for review, and its reviewers are asked
  [@test] crates/operations/src/shown_test.rs (`a_draft_opens_on_what_the_robot_landed_and_asks_the_reviewers_its_body_records`, `the_open_and_the_close_of_a_shown_review_against_origin_and_github`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)

#### Scenario: Landed with no robot listed, a reviewer added
- **WHEN** the baseline has the branch whole and no robot of it is listed, or the user adds a reviewer to a review open
  already
- **THEN** the draft opens on its head without a push, and the added reviewer is asked and recorded
  [@test] crates/operations/src/shown_test.rs (`a_draft_the_baseline_landed_opens_on_its_head_and_a_reviewer_the_user_adds_is_recorded`)
  [@test] crates/domain-policy/src/shown_test.rs (`a_draft_whose_branch_the_baseline_landed_opens_while_no_robot_of_it_is_listed`)
  [@test] crates/repository-model/src/facts_test.rs (`a_draft_whose_branch_landed_by_content_opens_once_the_robots_are_known_and_none_of_it_is_listed`)

### Requirement: News: The app opens and closes them by the news of the session

The session of a repository MUST tell once a draft whose branch landed (`ShownReviewLanded`), once each fix that landed
after a review opened (`ShownReviewFixLanded`, by its robot) and once a review every reviewer approved
(`ShownReviewApproved`); the page MUST ask the open, the extend and the close of each, and toast what failed. A run of
one that failed MUST be told again at the next answer of the listing or the robots, never at once, three times per
session at most.

#### Scenario: The news of the listing and the robots
- **WHEN** a merged Safe Push of the branch of a draft and an approved review are known
- **THEN** the first is told to open, the second to close, and the page asks each
  [@test] crates/domain-policy/src/shown_test.rs (`a_draft_opens_once_a_safe_push_of_its_branch_merged_and_an_approved_one_closes`)
  [@test] src/lib/shown-news.test.ts (`opens the review after landing whose Safe Push merged, closes the approved one, and toasts what failed`)

#### Scenario: A fix, and a run that failed
- **WHEN** the Safe Push of a fix of an open review merged, and the run of its extend failed
- **THEN** the extend is told once, the page asks it and toasts the failure, and it is told again at the next answer
  [@test] crates/repository-model/src/facts_test.rs (`a_fix_of_a_shown_review_is_waited_for_and_its_merge_is_told_once_and_again_after_a_failed_run`)
  [@test] src/lib/shown-news.test.ts (`adds the fix that landed to its review after landing and toasts a failure of it`)

## ADDED Requirements

### Requirement: Extend: A fix that landed goes onto the same review

Once the Safe Push of a fix of the branch merged, the open shown review MUST take it: the commits the robot took past
the target replayed onto the head the remote has (signed per `commit.gpgsign`) and pushed leased on that head, its
reviewers asked again once GitHub shows it, then its body records the fix. A fix that doesn't apply MUST leave the head and be named in a
comment once. A repeat after a stop between the steps MUST add no commit and no comment.

#### Scenario: The fix on the head, its reviewers asked again
- **WHEN** a fix of auth merged after alice requested changes
- **THEN** the head of the review has the fix on its commits, alice is asked again, and its body records the fix
  [@test] crates/operations/src/shown_test.rs (`an_extend_adds_the_fixes_asks_the_reviewers_again_and_records_them_last`, `the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_landed_goes_onto_the_head_and_a_repeat_adds_nothing`, `a_fix_on_the_head_is_signed_as_commit_gpgsign_says`, `a_head_the_repository_lacks_is_fetched_first`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_extend_of_a_shown_review_by_a_fix_that_landed`)

#### Scenario: A fix that doesn't apply
- **WHEN** the target changed the lines of the review after it landed and the fix is of that change
- **THEN** the head keeps what applied, and a comment names the fix once ("Landed a follow-up as <sha>; it does not
  apply on this review's commits: <url>"), a repeat comments nothing again
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_does_not_apply_leaves_the_head_with_what_applied_before_it`)
  [@test] crates/operations/src/shown_test.rs (`the_extend_of_a_shown_review_against_origin_and_github`)

### Requirement: State: A fix that lands is waited for

While a Safe Push of the branch of an open shown review runs, and once it merged until the review took it, the review
MUST wait for the fix (`extending`): it is not the author's move, and it MUST NOT close, whatever its reviews say.

#### Scenario: A fix runs, then merged
- **WHEN** alice requested changes and a fix of the branch runs, then merged
- **THEN** the review is `extending`, no "Returned to you", no close, until its record names the fix
  [@test] crates/domain-policy/src/shown_test.rs (`a_fix_that_merged_after_the_review_opened_waits_until_the_review_took_it`)
  [@test] src/lib/outgoing-attention.test.ts (`names after the rows the reviews after landing a reviewer handed back, also of a branch that left the table`)
