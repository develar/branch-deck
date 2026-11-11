## MODIFIED Requirements

### Requirement: Facts without an answer: Create Pull Request… waits for the listing

"Create Pull Request…" of "⋯" MUST be there from the first view wherever git names the GitHub repository of the
branches (`LandingRemote::github`), disabled with "Waits for GitHub to list the pull requests" while the branch may
have one; once GitHub lists a live one, "Open Pull Request #7" takes its place. Off GitHub it is never shown. Create
Stack Pull Request… of a whole stack waits for the listing the same way.

#### Scenario: Before GitHub lists the pull requests
- **WHEN** GitHub hasn't listed the pull requests yet
- **THEN** "Create Pull Request…" of "⋯" is disabled with "Waits for GitHub to list the pull requests"
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should never offer Create Pull Request… before GitHub lists the pull requests, say a failed listing, and keep the last listing through a failed refresh`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

#### Scenario: GitHub lists an open pull request of the branch
- **WHEN** "⋯" shows "Create Pull Request…" waiting, and GitHub lists the open #7 of the branch
- **THEN** "Open Pull Request #7" is in the place of "Create Pull Request…", and the items above it stay in their places
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge, Create Pull Request… and Request Review… in their places while GitHub lists, the open pull request in the place of Create @webkit`)

### Requirement: Facts without an answer: The places of the menu stand from the first view

Whether the menu of a branch shows its starts and its pull request MUST be decided on what git tells at the first
view: whether Patronus knows the repository (`LandingRemote::patronus`) and the GitHub repository of the branches
(`LandingRemote::github`). An answer of Patronus or GitHub MUST only enable such an item or say why it is disabled: a
shown item never leaves, so the item under a resting pointer stays.

#### Scenario: A repository on Space
- **WHEN** `origin` is on Space and Patronus hasn't answered
- **THEN** "⋯" shows Safe Push and "Patronus ›", Safe Push disabled with "Waits for the answer of Patronus"
- **AND** after the answers the same items are in the same places, Safe Push enabled, and the item under the pointer is the one it rested on
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Safe Push in its place while Patronus answers, disabled until it does, and the item under the pointer where it was @webkit`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)

#### Scenario: The GitHub repository origin pushes to
- **WHEN** the route is the merge of the pull request, `origin` pushes to the GitHub repository of the branches, and GitHub hasn't answered
- **THEN** "⋯" shows "Merge", disabled with "Waits for GitHub to tell what you may do in the repository", "Create Pull Request…" and "Request Review…", disabled with "Waits for GitHub to list the pull requests"
- **AND** once GitHub lists #7, "Merge #7" is in the place of "Merge", "Open Pull Request #7" in the place of "Create Pull Request…", and "Request Review…" is enabled in its place
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Merge, Create Pull Request… and Request Review… in their places while GitHub lists, the open pull request in the place of Create @webkit`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)

#### Scenario: A mirror origin doesn't push to
- **WHEN** the branches end up in a GitHub repository `origin` doesn't push to (`origin` a path, `branchdeck.githubRepository` set)
- **THEN** "⋯" never shows "Merge", before GitHub answers or after
- **AND** "Create Pull Request…" waits for the listing, then is enabled in its place, and "Request Review…" says "JIRA-456 has no open pull request: Create Pull Request… asks for its review"
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should never offer Merge on a mirror origin doesn't push to, and keep Create Pull Request… in its place while GitHub lists`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A local remote
- **WHEN** `origin` is a path and nothing names a GitHub repository
- **THEN** "⋯" shows no start and no pull request from the first view, and none after the answers
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should offer no start and no pull request on a local remote from the first view`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)
