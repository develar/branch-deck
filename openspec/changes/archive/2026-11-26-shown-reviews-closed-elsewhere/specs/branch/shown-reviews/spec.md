## ADDED Requirements

### Requirement: Close: A review closed elsewhere loses its branches

The listing MUST ask the user's shown reviews closed in the last 7 days, at the open of the session and then once an
hour, with the tips each ended at. An hour after its close, while no open
shown review of the branch is listed, the branches the remote still has at those tips MUST be deleted in one atomic
push leased on them (`Intent::DeleteClosedShownReview`); a branch gone or moved since stays. An open of a review whose
base the remote lacks MUST push its branches again.

#### Scenario: Closed on GitHub, a newer review of the branch open
- **WHEN** the review #12 of auth is closed on GitHub while #13 of auth is open on the same branches, then #13 is closed
  too
- **THEN** the branches stay while #13 is open, and once both are closed, an hour after the search of the closed ones
  deletes them, writing nothing to the pull requests
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should delete the branches of a review after landing closed on GitHub an hour after, and keep them while a newer review of the branch is open`)
  [@test] crates/domain-policy/src/shown_test.rs (`a_review_closed_elsewhere_an_hour_ago_loses_the_branches_still_at_its_tips_unless_the_branch_has_an_open_one`)
  [@test] crates/repository-model/src/facts_test.rs (`a_shown_review_closed_elsewhere_is_told_an_hour_after_its_close_once_while_no_open_review_of_its_branch_is_listed`)
  [@test] src/lib/shown-news.test.ts (`deletes the branches of a review after landing closed on GitHub and toasts a failure of it`)

#### Scenario: The search of the closed ones
- **WHEN** the listing asks the closed shown reviews, at the open of the session and an hour after
- **THEN** a head of exactly `{prefix}/review/<branch>/head` of the repository is one, with the tips it ended at and the
  tips of its branches the remote has; a list within the hour asks none
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_closed_shown_reviews_are_a_search_of_their_own_with_what_the_remote_has_of_their_branches`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_closed_shown_reviews_are_asked_by_the_first_list_then_once_an_hour`)
  [@test] crates/domain-policy/src/refresh_test.rs (`the_closed_shown_reviews_are_asked_by_the_first_list_of_the_session_then_once_an_hour`)

#### Scenario: Only the branches at its tips
- **WHEN** a newer review of auth took the head since #12 closed, and later both branches are at the tips of #12
- **THEN** the first delete takes only the base, the second both in one push, a repeat nothing; a review open again
  without its base pushes its branches first
  [@test] crates/sync-core/src/review_head_test.rs (`a_delete_of_a_review_closed_elsewhere_takes_only_the_branches_still_at_its_tips`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_delete_of_a_shown_review_closed_elsewhere`)
  [@test] crates/operations/src/shown_test.rs (`a_review_closed_elsewhere_deletes_the_branches_still_at_its_tips_and_a_reopened_one_without_its_base_pushes_it_again`, `the_branches_of_a_review_closed_on_github_are_deleted_where_they_are_at_its_tips`)
