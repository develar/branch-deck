## MODIFIED Requirements

### Requirement: Detection: A commit the replay drops landed without a counterpart

Where the replay onto the upstream is known, a commit it drops without a key match, a proof or a candidate MUST count
as landed: as the one baseline commit of its author that nothing else names whose changed lines the smallest set of
such commits sums to, unless the set of another baseline commit shares a commit with it, else without a baseline commit,
in the preview, the update and the finish of a session. A commit the user dropped MUST take no part.

#### Scenario: A branch squashed upstream
- **GIVEN** the upstream has the commits of a branch squashed into one commit of another key
- **WHEN** the replay onto the upstream drops them
- **THEN** they landed as the squash, in the preview and the update
  [@test] crates/update-core/src/landed_test.rs (`a_branch_squashed_upstream_landed_as_the_squash_after_a_clean_replay`)
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_gone_commits_that_sum_to_an_upstream_commit_of_their_author_name_it`)

#### Scenario: An upstream change split over two commits
- **WHEN** the upstream made the change of a dropped commit in two commits of other changes
- **THEN** it landed without a baseline commit
  [@test] crates/update-core/src/landed_test.rs (`a_change_split_over_two_upstream_commits_lands_without_an_upstream_commit`)
  [@test] crates/bd-update/src/update_test.rs (`a_commit_that_the_replay_leaves_without_a_change_is_dropped_as_empty`)

#### Scenario: A change made twice
- **WHEN** two sets of the dropped commits of one size sum to a baseline commit, or one set sums to two of them (the
  upstream made the change, reverted it and made it again)
- **THEN** none of them is named
  [@test] crates/git-ops/src/landed_pairing_test.rs (`two_sets_of_gone_commits_equal_to_one_squash_name_nothing`, `a_set_equal_to_two_upstream_commits_names_neither`)

#### Scenario: A fixup of a named commit
- **WHEN** the replay drops a fixup whose lines are none of the squash its target landed as
- **THEN** it landed with its target as the squash
  [@test] crates/update-core/src/landed_test.rs (`a_fixup_of_a_named_commit_lands_as_its_squash`)

#### Scenario: The finish of a session
- **WHEN** a session finishes whose replay left commits of a squash empty
- **THEN** they landed as the squash, and a commit the user dropped (`--drop-empty`) takes no part in the sum
  [@test] crates/bd-update/src/resolve_test.rs (`a_session_finish_names_the_squash_of_its_commits_that_went_empty`)
  [@test] crates/update-core/src/landed_test.rs (`a_commit_dropped_by_the_user_in_a_session_names_no_squash`)

### Requirement: The proof: Detection runs a fixed set of read-only processes

Detection MUST cost three read-only `log` processes, one more for a proven commit of another author, and one
`diff-tree --name-only` of the local commits and the baseline commits of their authors; the squash of the commits a
replay dropped one `diff-tree -p -U0` only when a baseline commit's paths are theirs, a session finish one
`diff-tree --name-only` more. The check inside a sync MUST reuse them for an unchanged state, and a failure MUST NOT
fail the check or the update.

#### Scenario: The processes of a detection
- **WHEN** the check detects what landed
- **THEN** it runs three read-only `log` processes, one more for a proven commit of another author, and one
  `diff-tree --name-only`

#### Scenario: One diff-tree for the paths of the commits
- **WHEN** the check looks for the same patch of an author under another key
- **THEN** it reads the changed paths of the local commits and the baseline commits of their authors in one
  `diff-tree` process
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_patch_of_the_author_is_looked_for_only_among_its_sets_of_paths`)

#### Scenario: The squash of the dropped commits
- **WHEN** the replay dropped commits without a counterpart
- **THEN** one `diff-tree -p -U0` reads them with the baseline commits of their authors whose paths they cover, none
  without such a commit, and only a session finish reads their paths first
  [@test] crates/update-core/src/process_budget_test.rs (`a_clean_update_whose_commits_went_empty_reads_their_changed_lines_once`)
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_gone_commits_read_no_diff_without_a_baseline_commit_of_their_author`)

#### Scenario: A sync of an unchanged state
- **WHEN** the check inside a sync runs again for an unchanged state
- **THEN** it reuses what it read

#### Scenario: Detection fails
- **WHEN** the detection fails
- **THEN** the check and the update do not fail
