## MODIFIED Requirements

### Requirement: Safe Push of a reviewed pull request: Rust decides it on the branch Patronus pushes into

Rust MUST decide it ([Reviews](../reviews/spec.md#the-actions-of-a-pull-request),
`domain_policy::actions::review_actions`) on the branch Patronus pushes into for the baseline, which the sync read
(`TableHeader.targetBranch`): enabled only for a pull request into it, since Patronus refuses a base no protected group
names before the start. The row's menu and the review MUST decide alike, by the same actions.

#### Scenario: A pull request into the Patronus target
- **WHEN** the pull request is into the branch Patronus pushes into
- **THEN** Approve and Safe Push… is enabled, and an approval of the head leaves the Safe Push alone
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] src/lib/review-request-actions.test.ts (`offers Approve and Safe Push where Rust offers it, disabled with its reason, the Safe Push alone once approved`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve and Safe Push a pull request into the Patronus target with a plain /patronus merge, after a confirmation`)

#### Scenario: Another base
- **WHEN** the pull request is into another base
- **THEN** it is disabled with "Patronus starts a Safe Push only from a pull request into master: #103 goes into release/253"
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve and Safe Push a pull request into the Patronus target with a plain /patronus merge, after a confirmation`)

#### Scenario: A layer of a stack
- **WHEN** the review window shows a layer of a stack into the branch of the one below, where Patronus merges into master
- **THEN** its Safe Push is disabled with "Patronus starts a Safe Push only from a pull request into master: #302 goes into user-name/virtual/auth", and its Approve stays, counting once it goes into master
- **AND** the bottom of the stack, into master, offers its Safe Push
  [@test] crates/domain-policy/src/actions_test.rs (`a_layer_into_the_branch_below_is_offered_no_safe_push_and_its_approval_counts_once_it_goes_into_the_target`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should offer no Safe Push of a layer into the branch below where Patronus merges, and the Safe Push of the bottom`)

#### Scenario: Before the first sync, a baseline that is no branch of origin
- **WHEN** the sync has not read the target yet, or the baseline has none: no branch of origin or of the GitHub repository of the branches
- **THEN** it is disabled with "Reading the branch Patronus pushes into…" before the first sync, or "Patronus pushes into a branch of origin: the baseline is none"
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)

#### Scenario: Before Patronus answers
- **GIVEN** git names a repository Patronus knows (`LandingRemote.patronus`)
- **WHEN** Patronus has not answered yet
- **THEN** Approve and Safe Push… stands in its place in the row's menu, disabled with "Waits for the answer of Patronus",
  as Request Review… does before GitHub answers
- **AND** once Patronus answers it is decided as above in that place, in a menu open meanwhile too, and the next open
  has the same items; enabled, it opens the review window with its confirmation
- **AND** where git names no repository Patronus knows, it is absent until Patronus says it merges there
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep Approve and Safe Push… in the row's menu, waiting for Patronus, until it answers, and open the review for it then`)
