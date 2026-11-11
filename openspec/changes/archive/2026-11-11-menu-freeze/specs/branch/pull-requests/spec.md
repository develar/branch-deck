## MODIFIED Requirements

### Requirement: Request Review: One builder makes the item of every surface

`getRequestReviewItem` (`src/presenters/review.ts`) MUST build the item, id `request-review`, "Request Review…", of the
open pull request and the actions of the subject its caller selected, for every surface that offers it (the row's next
step, the branch menu, `getRequestReviewItemNow`). Where git names the GitHub repository it stands in its place from
the first view: without an open pull request disabled with "Waits for GitHub to list the pull requests" before the
listing, then with what comes first, "JIRA-456 has no open pull request: Create Pull Request… asks for its review"
(`noOpenPullRequest`), or why GitHub can't list them.

#### Scenario: A branch with an open pull request
- **WHEN** the branch has an open pull request and Rust offers the action
- **THEN** the item "Request Review…" asks the chosen code owners on it, its press opening the popover
  [@test] src/presenters/review.test.ts (`asks the chosen code owners on the open pull request, its press opens the popover`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)

#### Scenario: A branch without an open pull request
- **WHEN** the branch has no open pull request on GitHub
- **THEN** the item is disabled with what creates one, or why GitHub can't list them; of a whole stack that forks the same, with "Create Pull Request of the Stack…"
- **AND** off GitHub, or while Rust doesn't offer the action, it is not there
  [@test] src/presenters/review.test.ts (`is in its place without an open pull request, disabled with what creates one, and not there while Rust doesn't offer it`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`, `the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)
