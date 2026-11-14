## ADDED Requirements

### Requirement: Review window: Approve of a layer says when Patronus counts it

On the route of Patronus, Approve MUST say what the approval counts for in landing, before the groups it counts for, as
Rust decides it of the base of the pull request and the target (`RepositoryView.approvalLanding` of `pull:<number>`,
`domain_policy::approval_landing`). Patronus starts a Safe Push of a pull request into its base, counts the approvals
of that pull request alone, once at its start, on the paths of all it lands, and takes the latest review of each
reviewer whatever commit it was on. A pull request into another base than the target (a layer of a stack goes into the
head branch of the one below it) MUST say "Counts once it goes into master: Patronus counts the approvals of the pull
request a Safe Push starts from"; the bottom of a stack, into the target, "Counts for the Safe Push of #301 alone, not
of the layers above it". Approve stays: the approval waits on the pull request and counts once it goes into the target,
then for all its Safe Push lands. A pull request alone into the target, an own one, and one off the route of Patronus
say nothing of it.

#### Scenario: A layer above the bottom
- **WHEN** the window reviews a stack of three on the route of Patronus, the bottom into master, and shows its second layer
- **THEN** Approve says "Counts once it goes into master: Patronus counts the approvals of the pull request a Safe Push starts from"
- **AND** Approve approves that pull request alone, on its head
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)
  [@test] crates/domain-policy/src/approval_landing_test.rs (`an_approval_counts_for_the_safe_push_of_its_own_pull_request_into_the_target_and_a_layer_above_once_it_goes_there`)
  [@test] src/presenters/reviews.test.ts (`says a layer into another base counts once it goes into the target, the bottom of a stack for its own Safe Push alone`)

#### Scenario: The bottom of a stack
- **WHEN** the window shows the bottom of the stack, into master
- **THEN** Approve says "Counts for the Safe Push of #301 alone, not of the layers above it"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)
  [@test] src/presenters/reviews.test.ts (`says a layer into another base counts once it goes into the target, the bottom of a stack for its own Safe Push alone`)

#### Scenario: Another route, an own pull request
- **WHEN** the repository lands by a merge on GitHub or a push, the target is not known yet, or the pull request is the user's own
- **THEN** the view has nothing of it for the pull request, and Approve says only the groups it counts for
  [@test] crates/domain-policy/src/approval_landing_test.rs (`an_approval_counts_for_the_safe_push_of_its_own_pull_request_into_the_target_and_a_layer_above_once_it_goes_there`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for`)
