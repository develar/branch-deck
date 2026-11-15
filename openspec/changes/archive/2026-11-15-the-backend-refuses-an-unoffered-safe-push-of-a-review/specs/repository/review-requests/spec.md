## ADDED Requirements

### Requirement: Safe Push of a reviewed pull request: The backend holds to the decision of the menus

The backend MUST hold to the decision of the menus for whoever asks (a window of a state before, another client): the
plan and the run of Approve and Safe Push MUST refuse a Safe Push the session offers not, before the approval, with
the code of its blocker.

#### Scenario: A Safe Push the session offers not, asked anyway
- **WHEN** a window of a state before or another client asks for Approve and Safe Push the session offers not: while Patronus has not answered, into another base, where Patronus merges nothing
- **THEN** the backend refuses its plan and its run before the approval, with the code of its blocker (`plan_changed` "Waits for the answer of Patronus", `pull_request_other_base`, `not_on_space`) and "nothing was approved or started"; GitHub gets no review and no comment
  [@test] crates/operations/src/review_test.rs (`a_safe_push_of_a_review_runs_only_where_the_session_offers_it_and_each_blocker_refuses_it_by_its_code`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should refuse in the backend an Approve and Safe Push the session offers not, before the approval: while Patronus has not answered, and into another base`)
