## MODIFIED Requirements

### Requirement: The actions of a pull request: Approve and Safe Push waits for the target

Approve and Safe Push MUST be disabled while Patronus has not answered where git names a repository it knows
(`LandingRemote.patronus`; absent where git names none), while the target is not known, for a baseline that is no
branch of origin, and for another base than the target. The target MUST be what the sync read of the baseline
(`TableHeader.targetBranch`), no git of its own.

#### Scenario: Patronus has not answered
- **GIVEN** git names a repository Patronus knows
- **WHEN** Patronus has not answered yet
- **THEN** Approve and Safe Push is disabled with the wait for the answer of Patronus, and an approval counts for no
  Safe Push yet
- **AND** where git names no repository Patronus knows, it is absent
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] crates/domain-policy/src/approval_landing_test.rs (`an_approval_counts_for_the_safe_push_of_its_own_pull_request_into_the_target_and_a_layer_above_once_it_goes_there`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for`)

#### Scenario: The target is not known or no branch of origin
- **WHEN** the target is not known, or the baseline is no branch of origin
- **THEN** Approve and Safe Push is disabled
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)

#### Scenario: Another base than the target
- **GIVEN** the target is what the sync read of the baseline (`TableHeader.targetBranch`)
- **WHEN** the pull request has another base than the target
- **THEN** Approve and Safe Push is disabled
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for`)

