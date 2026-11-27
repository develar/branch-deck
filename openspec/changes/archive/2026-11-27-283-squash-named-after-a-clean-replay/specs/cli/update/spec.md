## MODIFIED Requirements

### Requirement: Update: Every local commit the update dropped is in `dropped`

Every local commit the update dropped MUST be in one list, `dropped`: `commit`, `subject`, the baseline commit it
landed as (`upstream`, when one is known, also the squash that a commit its replay left empty landed as) and its
`proof`. `dropped_count` MUST count them.

#### Scenario: A commit the pairing left out
- **WHEN** the pairing left a local commit out
- **THEN** it is in `dropped` with its `proof`: `patch_id`, `replayed` or `changed_lines` (the pairing left it out)
  [@test] crates/bd-update/src/resolve_test.rs (`twins_in_another_order_are_left_out_by_their_proof_and_land`)

#### Scenario: A commit that its replay left without a change
- **WHEN** the replay of a local commit leaves it without a change
- **THEN** it is in `dropped` with the `proof` `empty` (its replay left it without a change, as `git rebase` drops it)
  [@test] crates/bd-update/src/update_test.rs (`a_commit_that_the_replay_leaves_without_a_change_is_dropped_as_empty`)

#### Scenario: A commit its replay left empty in a squash
- **WHEN** the replay leaves the commits of a pull request empty that the baseline squashed under another key, at once
  or in a session
- **THEN** each is in `dropped` with the `proof` `empty` and the squash as its `upstream`
  [@test] crates/bd-update/src/update_test.rs (`a_commit_that_its_replay_leaves_empty_names_the_squash_it_landed_as`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_session_finish_names_the_squash_of_its_commits_that_went_empty`)

#### Scenario: A commit dropped as landed
- **WHEN** the agent drops a round's commit with `--landed`
- **THEN** it is in `dropped` with the `proof` `landed` (`--landed`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

#### Scenario: A commit the agent drops as empty
- **WHEN** the agent drops a round's commit with `--drop-empty`
- **THEN** it is in `dropped` with the `proof` `user` (`--drop-empty`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)
