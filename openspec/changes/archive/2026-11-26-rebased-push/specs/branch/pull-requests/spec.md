## MODIFIED Requirements

### Requirement: Creating a pull request: Create is one intent of the engine that pushes first

Opening the dialog MUST push nothing; Create (and "Continue on GitHub") MUST be one intent of the engine
(`CreatePullRequest`, `operations::pull_request`; "Continue on GitHub" with `onGitHub`), whose plan pushes first what the
push button would (`derive::action_push_of`), and a subject only rebuilt on a newer baseline, its progress in the
button ("Pushing 45%"); Escape and a click outside MUST NOT close the dialog meanwhile.

#### Scenario: Create pushes first, Cancel pushes nothing
- **WHEN** the user opens the dialog of a branch not on the remote and cancels it, or creates the pull request
- **THEN** Cancel pushes nothing
- **AND** Create pushes the branch, its progress in the button ("Pushing 45%"), then creates the pull request
  [@test] tests/e2e/next-step/next-step.spec.ts (`should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`)

#### Scenario: What the plan pushes
- **WHEN** the plan of `CreatePullRequest` is made
- **THEN** it pushes first what the remote lacks: the branch and the ancestors with work, root first; the pull requests
  of what it pushes into a virtual branch are retargeted into the target before, D3a; the stack branch of a whole stack
- **AND** no push toast shows; Activity records it
  [@test] crates/operations/src/pull_request_test.rs (`a_creation_pushes_first_what_the_remote_lacks_then_opens_the_pull_request`)

#### Scenario: A subject only rebuilt on a newer baseline
- **GIVEN** the remote has the commits of the subject on an older baseline
- **WHEN** the plan of `CreatePullRequest` is made
- **THEN** it pushes the subject first, with "Continue on GitHub" the wait for the mirror after it, so the builds of the new pull request test the current baseline
- **AND** the plan of a review request of it pushes nothing
  [@test] crates/operations/src/pull_request_test.rs (`a_creation_pushes_a_subject_only_rebuilt_on_a_newer_baseline_first_and_a_review_request_does_not`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_sends_the_rebased_push_of_a_branch_only_rebuilt_on_a_newer_baseline`)
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_pull_request_of_a_branch_only_rebuilt_on_a_newer_baseline_is_opened_after_its_push`)
