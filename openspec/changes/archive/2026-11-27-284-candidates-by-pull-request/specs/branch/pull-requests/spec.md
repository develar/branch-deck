## ADDED Requirements

### Requirement: Matching pull requests to branches: A merged pull request keeps its merge commit beside the one of its branch

The listing MUST keep every merged pull request of the owner's repository that GitHub names a merge commit of
(`mergedAt`, `mergeCommit` with its author email), of any head, beside the one pull request of each branch: the session
keeps them out of the view and of the frame, for the update check of its syncs and Update Project. A pull request
Patronus closed has none.

#### Scenario: A merged pull request beside the open one of its branch
- **WHEN** a branch has an open pull request and a merged one, and another head a merged one
- **THEN** the branch shows the open one, and both merged ones are kept with their merge commits; a closed one of
  Patronus and a fork's are not
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_listing_keeps_every_merged_pull_request_with_its_merge_commit_beside_the_open_one_of_its_branch`)

#### Scenario: Out of the view and the frame
- **WHEN** a listing differs from the last only in its merged pull requests
- **THEN** no part of the view changes, a failure GitHub recovers from keeps them, and a sign-out drops them
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`the_merged_pull_requests_of_a_listing_stay_out_of_the_view_and_the_frame`)
