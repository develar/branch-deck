## ADDED Requirements

### Requirement: Fetch and refs: A fetch goes with its window

The refs of a fetch MUST go when the window that showed its pull request closes or moves on, also while the fetch runs:
the prunes keep nothing of it from that change, and what it wrote after that prune goes with the next one. The head it
brought MUST stay its window's, the one the readiness of the pull request is asked at, whatever change of the windows
came meanwhile. The layers of a stack go with its top, but those another stack that is kept has.

#### Scenario: A fetch whose window moved on while it ran
- **GIVEN** a review window shows #7 while its fetch runs
- **WHEN** the window shows another pull request, or closes, before the fetch ends
- **THEN** the prune of that change keeps no #7, the end of the fetch prunes nothing, and no later prune keeps #7; so
  for a stack and its top
  [@test] crates/repository-model/src/session_test.rs (`a_fetch_that_ends_after_its_window_moved_on_keeps_none_of_its_refs`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_fetch_that_ends_after_its_window_moved_on_keeps_nothing`)

#### Scenario: A head whose pull request another window dropped
- **GIVEN** a window fetches #7, its demand still to come
- **WHEN** a stack window that showed #7 closes before the fetch ends
- **THEN** once the demand of the window of #7 came, the readiness of #7 is asked at the head the fetch brought
  [@test] crates/repository-model/src/session_test.rs (`a_fetch_keeps_its_head_when_another_window_drops_its_pull_request_while_it_runs`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_fetch_keeps_its_head_for_its_window_when_another_window_dropped_its_pull_request_while_it_ran`)

#### Scenario: Two stacks share a layer
- **WHEN** the window of one of two stacks that share #8 closes
- **THEN** #8 stays kept with the other stack
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_layers_of_a_closed_stack_stay_while_another_stack_has_them`)

#### Scenario: A stack window opened again
- **WHEN** a stack window closes and opens again
- **THEN** it decides Approve N Pull Requests on no layer until its own fetch ended, then on the heads that fetch
  brought
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_stack_window_opened_again_shows_no_stack_until_its_own_fetch_ended`)

### Requirement: Fetch and refs: A fetch that fails or is stopped keeps nothing

A fetch that fails or is stopped, also in its wait for the prunes, MUST keep nothing once it ended while no window shows
its pull request and no other fetch of it runs, what it wrote going with the next prune; a window that shows the pull
request keeps its refs, and a stack that is kept keeps it as its layer, every layer with its head.

#### Scenario: A fetch that fails or is stopped
- **WHEN** a review fetch fails or is stopped, or its operation goes
- **THEN** it keeps nothing when no window shows its pull request, its refs going with the next prune; a window that
  shows it keeps them
  [@test] crates/repository-model/src/session_test.rs (`a_fetch_that_ends_without_a_head_keeps_its_refs_only_while_its_window_shows_them`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_fetch_that_ends_without_a_head_goes_unless_a_window_shows_it`)
  [@test] crates/operations/src/review_fetch_test.rs (`a_review_fetch_that_fails_keeps_nothing_for_the_prunes`)
  [@test] crates/operations/src/review_fetch_test.rs (`a_stop_or_a_drop_of_a_review_fetch_that_waits_for_a_prune_ends_it`)

#### Scenario: A failed fetch of a layer
- **WHEN** the fetch of #8 fails while a stack that is kept has #8 as a layer
- **THEN** #8 stays kept, and the stack shows every layer with its head
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_failed_fetch_of_a_layer_leaves_the_stack_that_has_it_whole`)

### Requirement: Fetch and refs: The prunes run in order, before a fetch writes

The prunes of a session MUST run one after another in the order the session decided them, on the blocking pool and
never waited for by the session; one that has not started gives its place to a newer one, so a burst of changes runs at
most two. A review fetch MUST write only once every prune decided before it started has ended
(`Sessions::review_fetching`); a stop of the fetch ends that wait, canceled, as does a wait of 120 s.

#### Scenario: Two prunes
- **WHEN** the session decides a prune while the one before still runs
- **THEN** it runs once that one ended
  [@test] crates/repository-model/src/session_test.rs (`the_prunes_of_a_session_run_in_the_order_it_decided_them`)

#### Scenario: A burst of changes
- **WHEN** the windows change four times while a prune runs
- **THEN** the prune that runs and the newest run, and a fetch that started meanwhile waits for both
  [@test] crates/repository-model/src/session_test.rs (`the_prunes_decided_while_one_runs_run_as_the_newest`)

#### Scenario: A fetch while a prune runs
- **WHEN** a review fetch starts while a prune the session decided before runs
- **THEN** the session answers its start once that prune ended, and at once when none runs
  [@test] crates/repository-model/src/session_test.rs (`a_review_fetch_writes_once_the_prunes_decided_before_it_ended`)

#### Scenario: A stop while a fetch waits
- **WHEN** a fetch that waits for a prune is stopped, or its operation goes
- **THEN** it ends canceled with nothing fetched, and keeps nothing
  [@test] crates/operations/src/review_fetch_test.rs (`a_stop_or_a_drop_of_a_review_fetch_that_waits_for_a_prune_ends_it`)

#### Scenario: A prune that hangs
- **WHEN** a fetch waits for a prune as long as a fetch may stay silent, 120 s
- **THEN** it fails with why, nothing fetched, and keeps nothing
  [@test] crates/operations/src/review_fetch_test.rs (`a_review_fetch_that_waits_for_a_prune_past_its_limit_ends`)

## MODIFIED Requirements

### Requirement: Fetch and refs: The session prunes the review refs of the others

The review refs MUST be those of the pull requests the review windows show, those their fetches keep and those that
wait: the session deletes the others each time that set changes, at a change of the review windows (a window shows one,
another one, none, or goes) or at a listing (`prune_review_refs`, one CAS transaction, on the blocking pool), an earlier
start's with its first prune, even an empty one; the same windows, and a poll, MUST prune nothing.

#### Scenario: A change of the review windows
- **WHEN** a window shows another pull request, none, or goes
- **THEN** the session deletes the review refs of the pull requests no review window shows
  [@test] crates/repository-model/src/session_test.rs (`a_change_of_the_review_windows_prunes_the_review_refs_of_the_others`)
  [@test] crates/sync-core/src/review_fetch_test.rs (`prune_deletes_the_review_refs_not_kept`)

#### Scenario: The same windows, a poll
- **WHEN** a demand names the same windows, or a poll comes, while a request shown was withdrawn
- **THEN** nothing is pruned, and the review keeps its refs
  [@test] crates/repository-model/src/session_test.rs (`a_change_of_the_review_windows_prunes_the_review_refs_of_the_others`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)

#### Scenario: A start with nothing to keep
- **WHEN** a session starts, its demand names no review window, and its first listing has no waiting request
- **THEN** that listing prunes once, every review ref going, and the same listing again prunes nothing
  [@test] crates/repository-model/src/session_test.rs (`the_first_prune_of_a_session_that_keeps_nothing_prunes_once`)

### Requirement: Fetch and refs: A fetch keeps its refs before the demand of its window

A window asks for its fetch apart from its demand, which can reach the session after the fetch: the refs of a fetch
MUST stay from its start, before it writes (`Sessions::review_fetching`), so a listing that withdraws the pull request
in between prunes nothing of them. They stay until a window shows the pull request and stops showing it, or until the
session closes: a fetch whose window moved on before its demand came keeps them, and the next start deletes them.

#### Scenario: A fetch before the demand of its window
- **GIVEN** a review window fetched its pull request before its demand reached the session
- **WHEN** a listing no longer names the pull request, then the demand of the window comes
- **THEN** neither prunes its refs, which go once the window shows another pull request
  [@test] crates/repository-model/src/session_test.rs (`a_fetch_before_the_demand_of_its_window_keeps_its_refs_at_a_listing_that_withdraws_it`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_fetches_of_a_window_keep_their_numbers_until_it_closes_with_the_layers_of_its_stack`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the refs of a review whose window tells the session of it after the listing that withdrew it`)

#### Scenario: A fetch whose head came before any demand named it
- **GIVEN** a fetch brought the head of #7 before any demand named #7
- **WHEN** another window changes, then a window shows #7 and moves on
- **THEN** the prunes keep #7 until that move, and the layer of a stack whose own fetch runs stays with it
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_fetch_before_the_demand_of_its_window_keeps_its_head_until_the_window_closes`)
