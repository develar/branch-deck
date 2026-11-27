## ADDED Requirements

### Requirement: The proof: The same changed lines of a set of commits are a candidate after a conflict

When the replay conflicts, the check MUST pair each baseline commit of the local authors that nothing took with a set
of local commits of its author that nothing took, whose changed lines without context, summed in order, equal its own
both ways: a commit with its fixups, or a pull request squashed under another identity. The set MUST be one candidate,
never a proof; a candidate by identity that a later commit completes becomes one by changed lines.

#### Scenario: A commit and its fixup squashed
- **GIVEN** a commit and the fixup that rewrote its line, squashed upstream under another identity
- **WHEN** the replay conflicts
- **THEN** the two are one candidate of the squash, and before the replay nothing pairs them
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_commit_and_its_fixup_squashed_under_another_identity_are_one_candidate_after_a_conflict`)

#### Scenario: A pull request whose later commit rewrote an earlier one
- **WHEN** a pull request of several commits, one of them rewriting the line of an earlier one, landed squashed
- **THEN** its commits are one candidate of the squash, also when one only adds a line that a later one rewrote, and
  also when it added a file and deleted it again or updated a binary file twice (the sum keeps the last version of a
  binary file and composes the changes of a mode)
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_pull_request_squashed_under_another_identity_is_one_candidate_of_its_commits`, `a_pure_addition_rewritten_by_a_later_commit_is_in_the_candidate_of_its_squash`, `a_pull_request_that_added_and_deleted_a_file_is_one_candidate`, `a_pull_request_that_updated_a_binary_file_twice_is_one_candidate`)

#### Scenario: A twin completed by a later commit
- **WHEN** a candidate by identity lacks what a later commit of its pull request changed
- **THEN** the later commit joins it, and the candidate is one by changed lines
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_key_candidate_takes_the_later_commit_that_completes_its_squash`)

#### Scenario: One diff, none without paths
- **WHEN** the check looks for the candidates by changed lines
- **THEN** it reads one `diff-tree -p -U0`, and none when the local commits that meet a baseline commit's paths do
  not cover them
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_candidates_by_changed_lines_read_one_diff_and_none_without_a_set_of_paths`)
  [@test] crates/update-core/src/process_budget_test.rs (`a_conflicting_update_of_a_squashed_pull_request_reads_its_changed_lines_once`)

### Requirement: The proof: A set by changed lines is the one smallest set equal to its baseline commit

The set of a baseline commit MUST be the smallest one whose sum equals it, among the local commits that change a line it
changes or one that another such commit changes, at most 16 of them in every subset (past that only those within its
paths, and past that again only all of them at once); two sets of that size MUST make no candidate. So a change and its
revert, and a commit or a fixup made after the merge, stay out of it, and a baseline commit with a line no set has pairs
with nothing.

#### Scenario: A change and its revert
- **WHEN** a pull request holds a change and its revert besides the squashed lines
- **THEN** the set leaves the two out
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_change_and_its_revert_stay_out_of_the_candidate_of_a_squash`)

#### Scenario: A commit made after the merge
- **WHEN** a later local commit, or a fixup, rewrote the squashed line again
- **THEN** it stays out of the set
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_commit_made_after_the_squash_stays_out_of_its_candidate`)

#### Scenario: A line no set has
- **WHEN** the squash has a line that no local commit changes, e.g. a suggestion of the review applied on GitHub
- **THEN** it pairs with nothing
  [@test] crates/git-ops/src/landed_pairing_test.rs (`an_upstream_commit_with_a_line_no_set_has_pairs_with_nothing`)

#### Scenario: Two sets of one size
- **WHEN** two sets of the same size sum to the baseline commit (A, its revert, A again)
- **THEN** neither is its candidate
  [@test] crates/git-ops/src/landed_pairing_test.rs (`two_sets_equal_to_one_squash_make_no_candidate`)

### Requirement: Detection: A gone commit of a candidate landed as its baseline commit

After a replay onto the upstream, a local commit of a candidate's unit that is gone MUST land as the candidate's baseline
commit, also a squash under another identity, in the preview, the update and the finish of a session.

#### Scenario: A squash is named
- **WHEN** the update leaves out a commit and its fixup that GitHub squashed
- **THEN** the preview and the update name the squash as the baseline commit of both
  [@test] crates/update-core/src/landed_test.rs (`a_commit_and_its_fixup_squashed_upstream_landed_as_the_squash`, `a_pull_request_squashed_upstream_landed_as_the_squash_when_a_later_commit_rewrote_an_earlier_one`)

#### Scenario: After `--landed`
- **WHEN** a session drops the commit with `--landed` and its fixup's replay is empty
- **THEN** both landed as the squash
  [@test] crates/bd-update/src/resolve_test.rs (`a_commit_squashed_upstream_with_its_fixup_drops_as_landed_in_its_round_while_another_commit_conflicts`)

## MODIFIED Requirements

### Requirement: The proof: The same patch of the author under another key is paired

Before the replay too, the check MUST pair the local commits of the same patch id under another key by the same author
(`patch_id`: a reworded one, one landed at another author time, the squash of a pull request of one commit), among the
baseline commits of that author with the same set of changed paths, as git compares.

#### Scenario: The same patch under another key of its author
- **WHEN** a baseline commit of the same author has the same patch id under another key
- **THEN** the pair is proven before the replay, `patch_id`
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_patch_under_another_identity_of_its_author_is_proven_before_the_replay`)

#### Scenario: Only among the author's sets of paths
- **WHEN** no baseline commit of the author has the same set of changed paths
- **THEN** no patch id is compared and the commit stays unpaired
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_patch_of_the_author_is_looked_for_only_among_its_sets_of_paths`)

#### Scenario: The same lines under another key
- **WHEN** a baseline commit under another key has the same changed lines but another patch id
- **THEN** before the replay the local commit pairs with nothing; after a conflict it is a candidate by changed lines
  ("The same changed lines of a set of commits are a candidate after a conflict"), never a proof
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_lines_under_another_identity_are_no_proof_but_a_candidate_after_a_conflict`)


### Requirement: The proof: The pairing is one to one

The pairing MUST be one to one: each baseline commit proves at most one local commit, the oldest first, and is the
candidate of at most one local commit or one set of them, each local commit in at most one. Merge commits MUST NOT pair.

#### Scenario: A change made twice locally with one copy upstream
- **GIVEN** a change made twice locally (A, a revert of A, A again) with one copy upstream
- **WHEN** the check pairs them
- **THEN** only the first A is left out
  [@test] crates/git-ops/src/landed_pairing_test.rs (`one_upstream_commit_proves_one_local_commit`)

#### Scenario: A merge commit
- **WHEN** a merge commit is on either side
- **THEN** it never pairs

#### Scenario: A squash of several commits
- **WHEN** a pull request of several commits landed squashed
- **THEN** its commits are one candidate of the squash, none of them a candidate of another baseline commit
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_pull_request_squashed_under_another_identity_is_one_candidate_of_its_commits`)


### Requirement: The proof: A candidate is landed only by a decision

A candidate MUST be landed only by a decision: `brd update --continue --landed` in its round drops it as its baseline
commit, at once when the baseline commit has every line it changes (and, for a candidate by changed lines, the upstream
has the change of its unit as it is), else after the user's yes (`--user`).

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

### Requirement: Update: A replay that conflicts again leaves out the candidates

If the replay without the commits of the same patch id by another author conflicts too, the check MUST leave out the
candidates together with the commits they landed with, then the candidates by identity alone with those by changed lines
whole (only their whole set equals their baseline commit).

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

### Requirement: Update: A result without the candidates counts only if it keeps their change

A result that leaves out candidates MUST count only if it keeps the change of each candidate: the merge of the change
from the candidate's parent to the last commit it landed with gives the tree it merges into, either the result's, or the
upstream's when the replay leaves out the candidate with every commit it landed with, so the commits replayed after it
build on it there. A set of candidates one of which fails MUST be tried once more without that one. So a commit amended
after the push, or a fixup made after it, MUST NOT be left out, and does not keep the other candidates in; the update
conflicts as before only when nothing gives a clean result.

#### Scenario: A fixup made after the push
- **WHEN** a fixup of a candidate was made after the push
- **THEN** it is never left out
  [@test] crates/update-core/src/landed_test.rs (`fixup_made_after_the_push_stays_local`)

#### Scenario: A commit amended after the push
- **WHEN** a candidate was amended after the push
- **THEN** it is never left out, and the update conflicts as before
  [@test] crates/update-core/src/landed_test.rs (`commit_amended_after_the_push_is_never_left_out`)

#### Scenario: An amended candidate beside a squash
- **WHEN** a candidate amended after the push and a squashed pull request are left out together
- **THEN** the set is tried once more without the amended one: the squashed commits drop, the amended one stays
  [@test] crates/update-core/src/landed_test.rs (`a_candidate_amended_after_the_push_keeps_only_itself_local_while_the_other_squashes_drop`)

#### Scenario: A later commit of the squashed file
- **WHEN** a local commit after the squashed ones changes another line of their file, or rewrote their line again
- **THEN** the squashed ones drop and the later commit stays, replayed onto the upstream
  [@test] crates/update-core/src/landed_test.rs (`a_later_commit_of_the_squashed_file_stays_local`, `a_local_commit_that_rewrote_the_squashed_lines_further_stays_while_the_squashed_ones_drop`)
