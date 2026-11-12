## MODIFIED Requirements

### Requirement: Tests That Reach It: The code of the steps is asked of accio in one query

Beside the other queries of the plan, on its own for the whole reading, every declaration of the code of the steps
MUST be asked in one `accio tests --at <path>:<line> --ref <head> --tests <file>… --resolved`, the test files of the
tests of the change given: the given tests that reach it through its callers at any depth, only by calls a type or an
import ties. Its answer MUST hold no other fact of the plan.

#### Scenario: The declarations of the steps asked once
- **WHEN** the plan has its steps
- **THEN** every declaration of the code of the steps is asked in one query, the test files of the tests of the
  change given
- **AND** a fixture of a test is no code a step asks the tests of
  [@test] crates/review-plan/src/steps_test.rs (`the_tests_that_reach_the_code_of_a_step_and_the_code_none_reaches`, `a_fixture_of_a_test_is_no_code_a_step_asks_the_tests_of`)

#### Scenario: The cost of the query
- **WHEN** a plan of a branch of a large monorepo asks its tests, cold after each new tree of the branch ([#191](https://github.com/develar/branch-deck/issues/191))
- **THEN** the usages of its API, its registrations and its entry points show once they answered, while that query still runs
- **AND** the tests of the steps follow when it answers
  [@test] crates/review-plan/src/stages_test.rs (`the_plan_shows_the_usages_while_the_tests_of_its_steps_are_asked`)

## ADDED Requirements

### Requirement: Tests That Reach It: A step says its tests are being found while the index is asked

While the index is asked the tests of the steps, each step with code it is asked of (`StepTests::awaited`, and each
group of a large step that holds a file of that code) MUST say "Finding its tests…" in the place of its tests: in the
contents, in its opening and in the opening of its group. A step without code MUST say nothing of it, and neither
does any step once the index answered or when it does not answer.

#### Scenario: A plan that waits for the tests of its steps
- **WHEN** the window shows a plan the index is still asked the tests of
- **THEN** a step with code says "Finding its tests…" in the contents and in its opening, a step without code nothing of its tests
- **AND** once the index answered, the step shows the tests that reach it ("Tested by ManagerTest.kt") and its code none reaches
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the uses left outside while the index is still asked the tests of the steps, each step saying its tests are being found, then the tests that reach it`)

#### Scenario: The words of a step, of its group and of a step Rust does not mark
- **WHEN** Rust marks the tests of a step, or of a group of a large step, `awaited`
- **THEN** its facts say "Finding its tests…", no test and no warning; the opening of a step whose groups open leaves it to them
- **AND** a step Rust does not mark says nothing of it, whatever the plan waits for
  [@test] src/presenters/review-story.test.ts (`says what a plan that waits for the index still finds: the uses only while nothing answered, the tests of each step Rust asks them of`)

#### Scenario: The steps the index is asked of
- **WHEN** the tests of the steps are a query of the index
- **THEN** each step with code is `awaited` in the plans sent while it runs, and none in the plan the reading answers
- **AND** of a large step, each group that holds a file of that code is `awaited` with it, the contract and the mechanical group not
  [@test] crates/review-plan/src/stages_test.rs (`the_plan_shows_the_usages_while_the_tests_of_its_steps_are_asked`, `a_range_without_tests_has_no_stage_of_its_tests`)
  [@test] crates/review-plan/src/groups_test.rs (`the_groups_that_hold_the_code_asked_wait_for_their_tests`)
