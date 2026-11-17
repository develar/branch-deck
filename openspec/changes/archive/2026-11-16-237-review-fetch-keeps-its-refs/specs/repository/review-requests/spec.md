## ADDED Requirements

### Requirement: Fetch and refs: A fetch keeps its refs before the demand of its window

A window asks for its fetch apart from its demand, which can reach the session after the fetch: the refs of a fetch
MUST stay from its start, before it writes (`Sessions::review_fetching`), until its window closes, the layers of a
stack with its top, so a listing that withdraws the pull request in between prunes nothing of them.

#### Scenario: A fetch before the demand of its window
- **GIVEN** a review window fetched its pull request before its demand reached the session
- **WHEN** a listing no longer names the pull request, then the demand of the window comes
- **THEN** neither prunes its refs, which go once the window shows another pull request
  [@test] crates/repository-model/src/session_test.rs (`a_fetch_before_the_demand_of_its_window_keeps_its_refs_at_a_listing_that_withdraws_it`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_fetches_of_a_window_keep_their_numbers_until_it_closes_with_the_layers_of_its_stack`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the refs of a review whose window tells the session of it after the listing that withdrew it`)

## MODIFIED Requirements

### Requirement: Read ahead: The review refs of a waiting request stay

The session MUST prune the review refs of every pull request but those the review windows show or fetch and those that
wait, each time that set changes: a change of the windows and a listing alike.

#### Scenario: A listing and a window
- **WHEN** a listing names #7 and #8, then a window shows #9
- **THEN** the first prune keeps #7 and #8, the next #7, #8 and #9
  [@test] crates/repository-model/src/session_test.rs (`a_listing_reads_ahead_its_waiting_requests_and_the_prunes_keep_their_refs`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)
