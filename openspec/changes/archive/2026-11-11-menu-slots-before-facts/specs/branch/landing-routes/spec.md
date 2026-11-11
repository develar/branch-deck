## MODIFIED Requirements

### Requirement: Gates: A mirror or a repository off GitHub has no merge

A pull request of a mirror `origin` doesn't push to MUST be refused, as it merges nothing into its target
(`gitHubMirror`), and off GitHub there is none (`notOnGitHub`). Both MUST be decided on what git names before GitHub
answers (`LandingRemote::github`), so no merge of them is offered while GitHub is asked, nor taken back after.

#### Scenario: A mirror and off GitHub
- **WHEN** the route is the pull request and its pull requests are of a mirror `origin` doesn't push to
- **THEN** the merge is refused (`gitHubMirror`)
- **AND** off GitHub it is refused (`notOnGitHub`)
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_merge_needs_write_access_to_the_repository_origin_pushes_to`)

#### Scenario: Before GitHub answers
- **WHEN** GitHub hasn't answered, and git names no GitHub repository of the branches, or one `origin` doesn't push to
- **THEN** the merge is refused at once (`notOnGitHub`, `gitHubMirror`)
- **AND** for the repository `origin` pushes to it waits for the permission
  [@test] crates/domain-policy/src/landing_route_test.rs (`a_merge_is_refused_before_any_answer_where_git_names_no_repository_origin_pushes_to`)

### Requirement: Merge: The start merges on GitHub only while no robot lands the repository

The start MUST merge on GitHub only while no robot of Patronus lands the repository and a merge there lands into its
target (`land::merges_on_github`: Patronus answers that the repository is not its, or, before it answers, `origin` is
not on Space (`land::patronus_lands`); the gate refuses neither a mirror nor a repository off GitHub); otherwise the
start stays the robot's, as before the routes.

#### Scenario: The step by what GitHub and Patronus tell
- **WHEN** Patronus answers that the repository is not its and the gate refuses neither a mirror nor a repository off GitHub
- **THEN** the next step merges the pull request
- **AND** while Patronus lands the repository the start stays the robot's
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merge_of_the_pull_request_is_the_step_of_its_route_by_what_github_tells`)

#### Scenario: Before Patronus answers
- **WHEN** Patronus hasn't answered and `origin` is not on Space
- **THEN** the merge is the start already, waiting for GitHub, as it stays after the answer
  [@test] crates/domain-policy/src/actions_test.rs (`the_slots_of_a_branch_are_decided_on_its_remote_before_any_answer_and_never_leave`)
