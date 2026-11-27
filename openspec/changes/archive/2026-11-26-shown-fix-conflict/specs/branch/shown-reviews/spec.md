## MODIFIED Requirements

### Requirement: Extend: A fix that landed goes onto the same review

Once the Safe Push of a fix merged, the open shown review MUST take it: its commits past the target replayed onto the
head the remote has (signed per `commit.gpgsign`), pushed leased on it, the reviewers asked again once GitHub shows it,
then the body records it. A commit that conflicts MUST take its conflicting files as master has them after the fix; a
fix master has under no identity of its commit MUST leave the head, named in a comment once. A repeat MUST add nothing.

#### Scenario: The fix on the head, its reviewers asked again
- **WHEN** a fix of auth merged after alice requested changes
- **THEN** the head of the review has the fix on its commits, alice is asked again, and its body records the fix
  [@test] crates/operations/src/shown_test.rs (`an_extend_adds_the_fixes_asks_the_reviewers_again_and_records_them_last`, `the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_landed_goes_onto_the_head_and_a_repeat_adds_nothing`, `a_fix_on_the_head_is_signed_as_commit_gpgsign_says`, `a_head_the_repository_lacks_is_fetched_first`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_extend_of_a_shown_review_by_a_fix_that_landed`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again`)

#### Scenario: A fix that conflicts takes the files as master has them
- **WHEN** master changed the lines of the review after it landed, and a fix of that change merged
- **THEN** the commit that conflicts takes the files git cannot merge as the commit that landed it on master has them
  (paired by the author, the author time and the subject; a rename and a delete too, the `<path>~<side>` copies of a
  file/directory conflict deleted), fetching the target first when it lacks that commit, and the next fix applies on it
- **AND** a comment names the fix and the files once ("Landed a follow-up as <sha>; `auth.rs` also shows what master
  changed in it after this review: <url>"), a repeat adds no commit and no comment
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_conflicts_takes_the_files_of_what_landed_it_the_next_fix_applies_and_a_repeat_adds_nothing`, `a_conflicting_commit_takes_a_rename_and_a_delete_as_master_landed_them`, `a_fix_that_conflicts_finds_what_landed_it_after_the_last_fetch_by_a_fetch_of_the_target`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_extend_of_a_shown_review_by_a_fix_that_conflicts`)
  [@test] crates/operations/src/shown_test.rs (`the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] crates/domain-policy/src/shown_test.rs (`the_comment_of_a_fix_that_took_files_names_them_as_a_list_of_names_does`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should take the file of a fix that conflicts on the commits of the review as master has it after the fix, and name it in a comment once`)

#### Scenario: A fix that doesn't apply
- **WHEN** the commit of a fix that conflicts has no commit of its identity on master, fetched (an autosquash folded
  it, a reword renamed it)
- **THEN** the head stays where the fix found it, the fixes after it apply all the same, and a comment names the fix
  once ("Landed a follow-up as <sha>; it does not apply on this review's commits: <url>"), a repeat comments nothing
  again
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_master_has_under_no_identity_of_its_commit_does_not_apply_and_the_next_fix_still_does`)
  [@test] crates/operations/src/shown_test.rs (`the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should comment once on a fix that master has under no identity of its commit, the head as it was`)
