## ADDED Requirements

### Requirement: The proof: A merged pull request is a candidate of the commits it pushed after a conflict

When the replay conflicts, after the candidates by changed lines, the local commits nothing took that a merged pull
request pushed (its head's commits the upstream lacks, by id, identity or mapping note) MUST be one candidate of its
merge commit, never a proof, whatever its lines and author. The merge commit MUST be untaken, in the upstream since the
merge base and no twin of a local commit. Asked only then.

#### Scenario: A squash with an edit of the review
- **GIVEN** GitHub squashed the pull request of two commits with an edit of the review of its own
- **WHEN** the replay conflicts
- **THEN** the two are one candidate of the squash by its pull request
  [@test] crates/git-ops/src/landed_by_pull_request_test.rs (`a_pull_request_squashed_with_a_review_edit_is_a_candidate_of_the_commits_it_pushed`)

#### Scenario: A squash under a noreply address
- **WHEN** the squash is of the noreply address of the user
- **THEN** one `log` of that address in the upstream since the merge base finds it, and the commits are its candidate
  [@test] crates/git-ops/src/landed_by_pull_request_test.rs (`a_pull_request_squashed_under_a_noreply_address_is_a_candidate_of_the_commits_it_pushed`)

#### Scenario: What the pull request pushed
- **WHEN** a local commit was made after the push, or the pull request of a later layer of a stack pushed the earlier
  layer too
- **THEN** the commit stays out of the candidate, and the earlier layer is in it, whatever their groups
  [@test] crates/git-ops/src/landed_by_pull_request_test.rs (`a_commit_never_pushed_stays_out_of_its_pull_request`, `a_stacked_layer_squashed_with_its_base_brings_both`)

#### Scenario: No exact fact
- **WHEN** the repository lacks the head, the merge commit is not in the upstream since the merge base, or it is a twin
  of a local commit (a rebase merge)
- **THEN** the pull request pairs nothing, and the keys pair a rebase merge
  [@test] crates/git-ops/src/landed_by_pull_request_test.rs (`a_pull_request_whose_head_is_missing_pairs_nothing`, `a_merge_commit_outside_the_upstream_since_the_merge_base_pairs_nothing`, `a_rebase_merged_pull_request_is_left_to_the_keys`)

#### Scenario: The changed lines first
- **WHEN** the local commits sum to the squash of their pull request
- **THEN** they are its candidate by changed lines
  [@test] crates/git-ops/src/landed_by_pull_request_test.rs (`the_changed_lines_take_a_squash_before_its_pull_request`)

#### Scenario: Asked only after a conflict
- **WHEN** the replay is clean
- **THEN** no merged pull request is asked, and the update runs the processes it ran before
  [@test] crates/git-ops/src/landed_by_pull_request_test.rs (`the_merged_pull_requests_are_asked_only_after_a_conflict`)
  [@test] crates/update-core/src/process_budget_test.rs (`a_clean_update_asks_no_merged_pull_requests`)

#### Scenario: Never under the repository lock
- **WHEN** an update asks the merged pull requests
- **THEN** its check asks them before the repository lock
  [@test] crates/update-core/src/landed_test.rs (`the_merged_pull_requests_are_asked_without_the_repository_lock`)

## MODIFIED Requirements

### Requirement: The proof: A candidate is landed only by a decision

A candidate MUST be landed only by a decision: `brd update --continue --landed` in its round drops it as its baseline
commit, at once when the baseline commit has every line it changes (and, for a candidate by changed lines or by pull
request, the upstream has the change of its unit as it is), else after the user's yes (`--user`).

#### Scenario: A twin that differs
- **WHEN** the baseline commit lacks a line the candidate changes
- **THEN** `--landed` asks the user and drops it as its baseline commit only after the yes (`--user`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

#### Scenario: A twin with every line of the commit
- **WHEN** the baseline commit has every line the candidate changes
- **THEN** `--landed` drops it as its baseline commit at once, without a question
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_with_every_line_of_the_commit_drops_without_a_question`)

#### Scenario: A squash rewritten since
- **WHEN** the upstream rewrote the line of a squash whose candidate by changed lines opens its round
- **THEN** the round asks the user, and `--landed` drops it, and then its fixup in its own round, only with `--user`
  [@test] crates/bd-update/src/resolve_test.rs (`landed_of_a_candidate_by_changed_lines_whose_squash_was_rewritten_asks_the_user`)

#### Scenario: A squash with a review edit
- **WHEN** the round opens of a commit of a pull request whose squash edited its lines in the review
- **THEN** its candidate is the merge commit, its interdiff shows the edit, and a drop needs the user's yes
  [@test] crates/update-core/src/landed_test.rs (`the_round_of_a_pull_request_squashed_with_a_review_edit_names_its_merge_commit_and_asks_the_user`)

### Requirement: Update: A replay that conflicts again leaves out the candidates

If the replay without the commits of the same patch id by another author conflicts too, the check MUST leave out the
candidates together with the commits they landed with, then the candidates by identity alone with those by changed lines
and by pull request whole (only their whole unit is their baseline commit).

#### Scenario: The robot folded the fixups into their target
- **GIVEN** a robot folded the fixups of a candidate into their target
- **WHEN** the replay conflicts
- **THEN** the check leaves out the candidate together with its fixups, and the update no longer conflicts
  [@test] crates/update-core/src/landed_test.rs (`fixups_folded_upstream_landed_with_their_target`)

#### Scenario: A squashed pull request
- **GIVEN** a pull request squashed under another identity, whose later commit rewrote an earlier one
- **WHEN** the replay conflicts
- **THEN** the check leaves out its commits, and the update no longer conflicts
  [@test] crates/update-core/src/landed_test.rs (`a_commit_and_its_fixup_squashed_upstream_landed_as_the_squash`, `a_pull_request_squashed_upstream_landed_as_the_squash_when_a_later_commit_rewrote_an_earlier_one`)

#### Scenario: A pull request squashed under a noreply address
- **GIVEN** GitHub squashed a pull request under the noreply address of the user
- **WHEN** the replay conflicts
- **THEN** the preview and the update leave out what it pushed and land it as its merge commit, in the app too; with an
  edit of the review of its own lines the app's update still conflicts
  [@test] crates/update-core/src/landed_test.rs (`a_pull_request_squashed_under_a_noreply_address_lands_as_its_merge_commit`, `a_squash_with_a_review_edit_of_its_own_lines_still_conflicts_in_the_app`)
  [@test] crates/operations/src/update_test.rs (`update_project_pairs_with_the_merged_pull_requests_of_the_session`)
  [@test] tests/e2e/update-project/landed.spec.ts (`should land a pull request squashed on GitHub under a noreply address as its merge commit`)

### Requirement: Update: The preview of a sync decides as the update does

The preview of every sync and the update MUST make the same decision, so the header shows such an update as clean. A
cached check that asked the merged pull requests MUST hold only while they answer alike, and a listing with other ones
MUST sync again when the last check asked them.

#### Scenario: An update clean by leaving out landed commits
- **WHEN** an update is clean only by leaving out the proven commits or the candidates
- **THEN** the preview of the sync makes the same decision and the header shows the update as clean

#### Scenario: Other merged pull requests
- **WHEN** the merged pull requests a cached check asked answer otherwise, or a listing brings others after a check
  that asked them
- **THEN** the check runs again, and the session syncs again once; a check that asked nothing syncs nothing
  [@test] crates/git-ops/src/landed_by_pull_request_test.rs (`a_cached_preview_that_asked_is_checked_again_with_other_facts`)
  [@test] crates/repository-model/src/session_test.rs (`a_listing_with_other_merged_pull_requests_syncs_again_when_the_last_check_asked_them`)

### Requirement: The proof: Detection runs a fixed set of read-only processes

Detection MUST cost three read-only `log` processes, one more for a proven commit of another author, and one
`diff-tree --name-only`; the squash of the commits a replay dropped one `diff-tree -p -U0` only when a baseline commit's
paths are theirs, a session finish one `diff-tree --name-only` more; after a conflict the merged pull requests one
`rev-list` of their heads and one `log` of other authors. A sync MUST reuse them for an unchanged state; a failure MUST
NOT fail the check or the update.

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

#### Scenario: The merged pull requests after a conflict
- **WHEN** a conflicting update pairs a pull request squashed under a noreply address
- **THEN** it asks them once, with one `log` of the merge commit's author and one `rev-list` of the head
  [@test] crates/update-core/src/process_budget_test.rs (`a_conflicting_update_of_a_pull_request_squashed_under_a_noreply_address_reads_its_merge_commit_once`)

#### Scenario: A sync of an unchanged state
- **WHEN** the check inside a sync runs again for an unchanged state
- **THEN** it reuses what it read

#### Scenario: Detection fails
- **WHEN** the detection fails
- **THEN** the check and the update do not fail
