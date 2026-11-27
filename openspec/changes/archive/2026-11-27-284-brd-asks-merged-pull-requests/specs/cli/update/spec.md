## ADDED Requirements

### Requirement: Update: brd update asks GitHub for the merged pull requests once, only after a conflict

`brd update` and `--dry-run` MUST ask GitHub for the merged pull requests only when the check conflicts with local
commits nothing else paired, once, before the repository lock: one search of the user's pull requests merged since a day
before those commits. A clean update MUST make no request; signed out or a failure pairs as before. The app's listing
(the 50 most recently updated) and this search may find other pull requests.

#### Scenario: A squash under a noreply address
- **WHEN** `brd update` conflicts on the commits of a pull request GitHub squashed under the noreply address of the user
- **THEN** it asks GitHub once, and the update applies with both landed as the squash
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_conflicting_brd_update_of_a_pull_request_squashed_under_a_noreply_address_asks_github_once`)
  [@test] crates/remote-facts/src/merged_pull_requests_test.rs (`the_merged_pull_requests_since_the_commits_are_one_search`)

#### Scenario: A squash with an edit of the review
- **WHEN** the squash edited the squashed line in the review
- **THEN** the round of each commit has `landed_as` with `by: pull_request`, the edit in `upstream_only` and `ask_user`, and
  `--landed` drops it only with `--user`
  [@test] crates/bd-update/src/resolve_test.rs (`the_round_of_a_pull_request_squashed_with_a_review_edit_shows_the_edit_and_asks_the_user`)

#### Scenario: Without a sign-in
- **WHEN** no GitHub token is found
- **THEN** `brd update` stops on the round of the commit without `landed_as`, as before
  [@test] crates/bd-update/src/update_test.rs (`without_sign_in_brd_update_stops_on_its_round_as_before`)

#### Scenario: Inside a runtime
- **WHEN** the merged pull requests are asked from inside a runtime
- **THEN** they answer none and send nothing
  [@test] crates/bd-core/src/merged_pull_requests_test.rs (`the_merged_pull_requests_are_not_asked_inside_a_runtime`)
