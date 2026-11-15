## ADDED Requirements

### Requirement: Creating a pull request: The backend refuses a second pull request of a branch

The plan and the run of a creation MUST refuse a second pull request of a branch that has an open one, or one merged
into the target, before any push (`pullRequestOpen`, `pullRequestMerged`), as the menus offer Open Pull Request in its
place, for whoever asks; one merged into a parent's branch, and a closed one, leave the way to a new one.

#### Scenario: A creation asked anyway
- **GIVEN** JIRA-456 has the open pull request #760
- **WHEN** a window of a state before or another client asks for its creation
- **THEN** its plan has the blocker and its run refuses it with `plan_changed`: "Nothing was pushed or created: JIRA-456 has the open pull request #760: Open Pull Request shows it"; nothing is pushed, GitHub gets no creation
  [@test] crates/operations/src/pull_request_test.rs (`a_creation_pushes_first_what_the_remote_lacks_then_opens_the_pull_request`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should refuse in the backend a second pull request of a branch that has an open one, before any push`)
