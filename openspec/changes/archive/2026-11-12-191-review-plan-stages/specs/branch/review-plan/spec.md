## RENAMED Requirements

- FROM: `### Requirement: In the App: The plan comes first without its usages, then with them`
- TO: `### Requirement: In the App: A fact of the plan shows when its query answered`

## MODIFIED Requirements

### Requirement: In the App: A fact of the plan shows when its query answered

`get_review_plan` MUST send the plan on its channel as it stands, each with what it still waits for (`waits`): the
plan of git alone before the index is asked (`index`), when there is a query to ask; the plan with the usages, the
registrations and the entry points once they answered, while the tests of the steps are still asked (`tests`); then
the plan whole. The window replaces what it shows with each.

#### Scenario: A plan with a query to ask
- **WHEN** `get_review_plan` reads a plan with a query of the index to ask
- **THEN** it first sends the plan of git alone (`waits` is `index`): no usages of its API, its `verdict` `pending`, no file counting `breaks`, no registration, no entry point of a new module, no test of a step
- **AND** then the plan whole (`waits` null), and the window replaces what it shows with each
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
  [@test] crates/review-plan/src/registrations_test.rs (`the_plan_sent_while_the_index_is_asked_has_no_registration`)

#### Scenario: The tests of the steps are still asked
- **GIVEN** a plan whose steps have code and whose range has tests: the tests of its steps are a query of the index
- **WHEN** the usages, the registrations and the entry points answered while that query still runs
- **THEN** it sends the plan with them (`waits` is `tests`), each step whose code is asked `awaited` and none `answered`
- **AND** then the plan whole, with the tests that reach the code of its steps, no step `awaited`
  [@test] crates/review-plan/src/stages_test.rs (`the_plan_shows_the_usages_while_the_tests_of_its_steps_are_asked`)
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the uses left outside while the index is still asked the tests of the steps, each step saying its tests are being found, then the tests that reach it`)

#### Scenario: The tests of the steps are no query of the index
- **WHEN** the range has no test, so the index is asked nothing of the tests of its steps
- **THEN** no plan waits for the tests alone and no step is `awaited`: the plan of git alone, then the plan whole
  [@test] crates/review-plan/src/stages_test.rs (`a_range_without_tests_has_no_stage_of_its_tests`)

#### Scenario: The words while the usages are pending
- **WHEN** the window shows a plan that waits for `index`
- **THEN** it words an API without usages "Finding the uses…"
  [@test] src/presenters/review-plan.test.ts (`says the uses are being found while Rust asks the index`)

#### Scenario: The words once only the tests are asked
- **WHEN** the window shows a plan that waits for `tests`
- **THEN** an API without usages is "The uses are unknown", never "Finding the uses…": its usages answered
  [@test] src/presenters/review-story.test.ts (`says what a plan that waits for the index still finds: the uses only while nothing answered, the tests of each step Rust asks them of`)

#### Scenario: The stages in the window
- **WHEN** the events of a read come: the plan that waits for `index`, the one that waits for `tests`, the plan whole
- **THEN** the window shows each as it comes, and the read ends with the plan whole
  [@test] src/lib/review-plan-answer.test.ts (`shows each stage of a read as it comes: the plan that waits for the index, the one that waits for its tests, the plan whole`)

### Requirement: In the App: A plan whose every query answered is kept in memory

A plan whose every query answered (`index_note` null) and none cut short (`ReviewPlan::is_final`) MUST be kept in the
memory of the app by what it is of (the repository, the prefix, the branch, `base`, `head`, the reviewed pair and
`sinceReviewed`), at most 64 MiB of its JSON and 10 minutes unread. A plan sent while a query still runs (`waits`)
MUST never be kept: only the plan a reading ends with is.

#### Scenario: The same plan asked again
- **GIVEN** a kept plan, read less than 10 minutes ago
- **WHEN** it is asked again
- **THEN** it comes whole in one event, with no git past the resolve of its subject and no query of the index
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

#### Scenario: A reading that sent the plan in stages
- **GIVEN** a reading that sent the plan that waits for `index`, the one that waits for `tests`, then the plan whole
- **WHEN** a window asks the plan again
- **THEN** it gets the plan whole in one event, never a plan that waits
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_window_that_joins_a_reading_gets_each_stage_it_published_then_the_plan`)

### Requirement: In the App: A plan is read once whoever asks it

A plan MUST be read once, whoever asks it: a window that asks the plan another window reads now MUST wait for that
reading and get what it sent, the latest plan with what it waits for at once and each later one as it comes, then the
plan whole; when that reading ends without a plan, the window MUST read the plan itself and still get it.

#### Scenario: A window asks the plan being read ahead
- **GIVEN** a window whose reading sent the plan that waits for `index` and asks the index
- **WHEN** another window asks the same plan
- **THEN** it gets that plan at once, the plan that waits for `tests` once the reading sends it, then the plan whole
- **AND** the index is asked once
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_window_that_joins_a_reading_gets_each_stage_it_published_then_the_plan`)

#### Scenario: The reading it waited for ends without a plan
- **WHEN** that reading ends without a plan: its window asked another plan
- **THEN** the window reads the plan itself and still gets it
- **AND** the request the other window went on from gets no plan after the one it had
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_window_that_waited_for_a_superseded_reading_reads_the_plan_itself`)

### Requirement: In the Review Window: The plan of the same changes stays whole until the next one

The plan of the same changes, read again after Mark Reviewed, MUST stay whole until the next one comes whole, neither
the plan that waits for the index nor the one that waits for its tests replacing it: the steps of the tree, its commit
rows and the story of the diffs stay as they were; a plan of another head or scope MUST give the tree and the diffs
none of its facts.

#### Scenario: Mark Reviewed reads the plan again
- **WHEN** Mark Reviewed reads the plan of the same changes again
- **THEN** the steps of the tree, its commit rows and the story of the diffs stay as they were until the next plan comes whole
  [@test] src/lib/review-plan-answer.test.ts (`keeps the whole plan of the same changes while Mark Reviewed reads it again, until the next one is whole`)

#### Scenario: The plan of another head or scope
- **WHEN** the plan of another head or scope is read
- **THEN** the tree and the diffs keep none of the facts of the plan before while the next one is read, but the plan of the branch for its card
  [@test] src/lib/review-plan-answer.test.ts (`keeps no facts of a plan of another head or scope while the next one is read, but the plan of the branch for its card`)

### Requirement: In the Review Window: The overview keeps its place and says its state

The overview MUST keep a block above the headers and the messages while the diffs are read, and say what is awaited:
"Reading the review plan…" while the plan is read, "Finding the uses left outside…" while nothing of the index
answered, "Finding the tests of the change…" once only the tests of the steps are asked, and "Cannot read the review
plan: …" when the read failed, in the review of a pull request too.

#### Scenario: The overview while the plan is read
- **WHEN** the plan is read, Rust asks the index, or the read failed
- **THEN** the overview keeps its block above the headers and the messages and says "Reading the review plan…", "Finding the uses left outside…" or "Cannot read the review plan: …"
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)

#### Scenario: The overview once only the tests of the steps are asked
- **WHEN** the window shows the plan that waits for `tests`
- **THEN** the overview says "Finding the tests of the change…" above the uses left outside it already tells, and nothing of it once the plan is whole
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the uses left outside while the index is still asked the tests of the steps, each step saying its tests are being found, then the tests that reach it`)
  [@test] src/presenters/review-story.test.ts (`says what a plan that waits for the index still finds: the uses only while nothing answered, the tests of each step Rust asks them of`)

#### Scenario: The review of a pull request
- **WHEN** the review of a pull request shows the plan once the repository turns it on
- **THEN** its overview stands above its changes in the same way, its commits in the tree
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`)

## ADDED Requirements

### Requirement: In the App: The read ahead yields to a window that asks its plan

A window that asks the plan being read ahead MUST NOT wait for a reading led below the app: that reading MUST end
without a plan, its queries of accio killed, and the window MUST read the plan itself, its queries at the priority of
the app. The read ahead goes on with its next branch.

#### Scenario: A window asks the plan being read ahead
- **GIVEN** a read ahead that sent no plan to anyone and asks the index below the app
- **WHEN** a window asks the same plan
- **THEN** it gets the plan as the read ahead had it, the query of the read ahead is killed and its reading ends without a plan
- **AND** the window reads the plan itself at the priority of the app, and the memory keeps it
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_read_ahead_yields_to_the_window_that_asks_its_plan`)
