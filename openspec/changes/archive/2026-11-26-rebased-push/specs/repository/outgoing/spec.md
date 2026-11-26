## ADDED Requirements

### Requirement: What is outgoing: A branch only rebuilt on the baseline tip is on an older base

A branch up to date by content whose remote branch has its commits on another base, its own the baseline tip, MUST be
on an older base (`RemoteStatusUpdate::older_base`): the oldest commits of both sides of the walk of the commits ahead
have different parents (`--boundary`, no other process). It stays up to date and not outgoing. A fast-forward, a remote
at the tip, the same base under other hashes and a branch behind the fetched baseline MUST NOT be.

#### Scenario: Rebuilt on a newer baseline after its push
- **GIVEN** a pushed branch, then Update Project rebuilt it on a newer baseline
- **WHEN** its remote status is computed
- **THEN** it is up to date, no fast-forward, on an older base; once pushed it is on none
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_branch_rebuilt_on_a_newer_baseline_after_its_push_is_up_to_date_on_an_older_base`)
  [@test] crates/domain-types/src/remote_status_test.rs (`a_branch_only_rebuilt_on_another_base_is_on_an_older_base_and_one_with_work_or_at_the_tip_is_not`)

#### Scenario: A stack rebuilt with its parent
- **GIVEN** a stack rebuilt with its parent on a newer baseline after their push
- **WHEN** their remote statuses are computed
- **THEN** both are on an older base
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_stacked_branch_rebuilt_with_its_parent_on_a_newer_baseline_is_on_an_older_base`)

#### Scenario: The same base, a newer remote or a branch behind the baseline
- **WHEN** the remote has the commits under other hashes on the same base, or on a newer baseline than the branch, or the branch is behind the fetched baseline
- **THEN** the branch is on no older base
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_branch_rebuilt_under_other_hashes_on_the_same_baseline_is_not_on_an_older_base`, `a_remote_branch_on_a_newer_baseline_than_the_branch_is_not_on_an_older_base`, `a_branch_behind_the_fetched_baseline_is_not_on_an_older_base`)
