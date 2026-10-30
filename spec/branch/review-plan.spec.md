---
name: Review Plan
description: Requirements for the plan of a review (experimental), what a reviewer of a branch needs first; its start is Mark Reviewed, the base and the tip of a local branch kept in `refs/branch-deck/reviewed/`, the interdiff of its next review (`bd review-plan`), the commits since then, and the API surface the reviewed commits change with its usages from accio, updated in the change or left outside, the facts of each file (its links to the other changed files, a mechanical change, a repeated edit, a missing test), the dependency changes of the build files and the files in dependency order.
style: plain-1
targets:
  - ../../crates/review-plan/src/lib.rs
  - ../../crates/review-plan/src/mark_reviewed.rs
  - ../../crates/review-plan/src/interdiff.rs
  - ../../crates/review-plan/src/api.rs
  - ../../crates/review-plan/src/fan_in.rs
  - ../../crates/review-plan/src/files.rs
  - ../../crates/review-plan/src/links.rs
  - ../../crates/review-plan/src/dependencies.rs
  - ../../crates/review-plan/src/order.rs
  - ../../crates/trial-check/src/declaration_checks.rs
  - ../../crates/trial-check/src/symbol_dependencies.rs
  - ../../crates/trial-check/src/symbol_scan.rs
  - ../../crates/file-diffs/src/accio.rs
  - ../../crates/git-ops/src/reviewed_refs.rs
  - ../../crates/git-ops/src/carried_review.rs
  - ../../crates/trial-check/src/diff_scan.rs
  - ../../crates/bd-rewrite/src/mark_reviewed_command.rs
  - ../../crates/bd-rewrite/src/review_plan_command.rs
  - ../../crates/bd-cli/src/lib.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/app-api/src/reads/review_plan.rs
  - ../../crates/app-api/src/app/mark_reviewed.rs
  - ../../crates/app-api/src/session/windows.rs
  - ../../crates/sync-core/src/repository_config.rs
  - ../../src/components/settings/DiffsPage.tsx
  - ../../src/lib/review-plan.ts
  - ../../src/lib/range-review.ts
  - ../../src/lib/file-tree.ts
  - ../../src/lib/file-actions.ts
  - ../../src/hooks/useReviewPlan.ts
  - ../../src/hooks/useBranchReview.ts
  - ../../src/hooks/useSelectReviewFile.ts
  - ../../src/lib/review-place.ts
  - ../../src/presenters/review-plan.ts
  - ../../src/components/review/ReviewPlanCard.tsx
  - ../../src/components/review/BranchReview.tsx
  - ../../src/components/commit/CommitReview.tsx
  - ../../src/components/files/FileTree.tsx
  - ../../src/components/files/FileRowBadges.tsx
---

# Review Plan

Status: Experimental
Date: 2026-10-29

## Summary

A review of a branch lists its files in tree order, and a branch that agents keep landing commits on is read whole
again after each round. The review plan shows what the reviewer needs first: what changed since the last review, the
public declarations the branch adds, removes or changes with the places that use them, the code changed without its
tests, and the files in the order of their dependencies. It is computed in Rust, once for `bd` and the app
(`crates/review-plan`), and printed by `bd` as JSON, so that it can be measured over many ranges before the review
windows show it behind a flag.

Its first parts are Mark Reviewed, the base and the tip of a local branch kept when the user says they reviewed it,
and Since Your Review: the commits of the branch paired with the reviewed ones, unchanged, rewritten, added or
dropped, and what changed since the review as one diff (`bd review-plan`): the plain range of the commits added on
top, else the diff from the reviewed change carried onto the base of the branch to its tip, whatever the branch went
through. Then the API surface: the public
declarations the reviewed commits add, remove or change in form, ranked by their usages at the head of the range from
accio that the change left outside, and the facts of each changed file: which other changed files it uses and which use it, whether its change
is mechanical, the edit it shares with other files, the usages of the API it removes or changes, and whether it
changed without its tests. Then the dependency changes: the module, library, plugin, crate and package entries the
commits add to, remove from or change in the build files, and the build files they add or delete. Then the order of
the files for `bd`: a file that declares what another uses before it, a test right after its subject. The review
window keeps the tree of the files: each row shows its links and its risk, and the mechanical files fold into one
group at the end. Above the diffs, the Review Summary card leads with what can break, then the dependency changes,
the repeated edits and the new API.

## Goals

- Remember, per local branch, the base and the tip the user reviewed, with one command and one write.
- Keep the reviewed commits reachable, so that a review after a rewrite or a `git gc` still reads them.
- Cost a fixed number of git processes, whatever the number of branches, of marks and of commits.
- Show only what changed since the review, also after the branch was rebased, synced, amended or reworded.
- Show the contract changes first, the one used most outside the change at the top, and never fail a plan because
  accio cannot answer.

## Non-goals

- Mark a pull request of GitHub: its "Since Your Review" stays the one of GitHub.
- Push or fetch the marks: they are local to the repository, shared by its worktrees.
- Mark a range that is no branch.
- Build an accio index: a repository without one gets no usages.
- The owners of a file in its facts: the review already filters by owner.

## Requirements

### Mark Reviewed

- A mark must keep the branch's tip and its base, the merge base of the tip and the baseline, as
  `refs/branch-deck/reviewed/<sha256 of the branch's full name>/{base,tip}`, so that the pairs of a branch `a` and a
  branch `a/b` never clash; a mark of a branch that has a pair must replace both refs with compare-and-swap on the
  values it read, and name the pair it replaced; the same base and tip again must move no ref.
  [@test] ../../crates/git-ops/src/reviewed_refs_test.rs (`a_mark_writes_the_pair_of_the_branch_and_a_second_mark_replaces_it`, `the_same_mark_again_moves_no_ref`)
  [@test] ../../crates/review-plan/src/mark_reviewed_test.rs (`marks_the_virtual_branch_of_a_name_at_its_merge_base_with_the_baseline`)
- A branch stacked on another must be markable on the base its review shows, the head of its parent (`bd mark-reviewed
  <branch> --base <parent>`, the parent named as a branch is, or any commit), so that its reviewed commits are its
  own; the base is then the merge base of the tip and that commit, and no baseline is read.
  [@test] ../../crates/review-plan/src/mark_reviewed_test.rs (`a_branch_stacked_on_another_is_marked_on_the_base_its_caller_names`)
  [@test] ../../crates/bd-rewrite/src/mark_reviewed_test.rs (`a_stacked_branch_is_marked_on_its_parent`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_mark_reviewed_on_a_base`)
- A base that names no branch and no commit (one `rev-parse --verify`, never git's message), or that shares no
  history with the tip, must fail before the write and leave every reviewed ref as it was: `invalid_arguments` (exit
  2) for a `--base`, `no_baseline` for a baseline with no history in common; a base that starts with `-` is never read
  as an option of git.
  [@test] ../../crates/review-plan/src/mark_reviewed_test.rs (`a_base_that_is_no_commit_or_shares_no_history_is_its_own_error_and_writes_nothing`)
  [@test] ../../crates/bd-rewrite/src/mark_reviewed_test.rs (`a_base_that_is_no_commit_or_shares_no_history_is_refused`)
- Every ref of the mark must be written in one `update-ref --stdin` with compare-and-swap on the listing it was
  computed from, a ref the mark keeps included (`verify`): a mark or a prune of another process meanwhile must fail the
  whole write and leave the other one's pair.
  [@test] ../../crates/git-ops/src/reviewed_refs_test.rs (`a_concurrent_mark_fails_its_compare_and_swap`)
- The same write must delete the pairs of the branches that are gone, and keep the others.
  [@test] ../../crates/git-ops/src/reviewed_refs_test.rs (`a_mark_prunes_the_pairs_of_gone_branches`)
- The branch must be named as `bd` names branches: the virtual branch `{prefix}/virtual/<name>` first, else the local
  branch `<name>`, or exactly `refs/heads/<name>`; a name of no branch must be `unknown_branch` and write nothing.
  [@test] ../../crates/review-plan/src/mark_reviewed_test.rs (`marks_the_virtual_branch_of_a_name_at_its_merge_base_with_the_baseline`, `an_unknown_branch_is_its_own_error`)
  [@test] ../../crates/bd-rewrite/src/mark_reviewed_test.rs (`an_unknown_branch_is_refused`)
- `bd mark-reviewed <branch>` must print the branch, its ref, the base, the tip, the pair it replaced (`previous`,
  `null` the first time), `changed` and `pruned`, and its output must be in `bd schema`.
  [@test] ../../crates/bd-rewrite/src/mark_reviewed_test.rs (`marks_a_virtual_branch_reviewed_and_again_after_a_new_commit`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- A mark must run a fixed number of git processes: the baseline (only without a base), one `for-each-ref` of the
  reviewed refs with the branches, one `rev-parse --verify` of a base that names no branch, one `merge-base` and one
  `update-ref`; it takes no repository lock and writes no branch, index or working tree.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_mark_reviewed`, `a_mark_reviewed_on_a_base`)
- An agent must run it only when the user says they reviewed the branch (`bd help worktrees`).
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Since Your Review

- A branch never marked must have no interdiff (`null`), and no scope; a range has none either.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`a_branch_never_marked_has_no_interdiff`, `a_range_has_no_interdiff`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`a_branch_never_marked_and_a_range_have_no_interdiff`)
- Each commit of the branch (`<base>..<tip>`, the base as a mark finds it) must be paired one to one, oldest first,
  with a commit of the reviewed pair: the same commit, else the same identity (author email, author time, subject
  without the `(prefix)`), else the same changed lines (`patch-id --verbatim` of the diff without context). A pair
  is `unchanged` when both change the same lines, else `rewritten`; a commit left is `added`, a reviewed one left is
  `dropped`, listed after the commits of the branch.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`commits_added_after_the_mark_are_the_range_on_top`, `a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed`)
- A commit amended since the review must be `rewritten` with the lines it changes that its reviewed version did not,
  and the reverse, per path.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`an_amended_commit_is_rewritten_with_its_lines`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)
- A move of the baseline alone (a rebase, a sync onto a new baseline) must leave every commit `unchanged`.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`a_baseline_move_alone_leaves_every_commit_unchanged`)
- When commits were only added on top, the scope must be the plain range `<first added>^..<tip>`; `<tip>..<tip>` when
  nothing changed.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`commits_added_after_the_mark_are_the_range_on_top`, `a_baseline_move_alone_leaves_every_commit_unchanged`)
- After a rewrite, a drop or a commit added below a reviewed one, the scope must be `<carried>..<tip>`, `carried`: the
  carried commit has the base of the branch as its parent and the tree of `merge-tree --merge-base <reviewed base>
  <base> <reviewed tip>`, on no ref, in the name and at the author time of the reviewed tip, so the same carry writes
  the same commit; it is the reviewed tip itself when the base did not move. Its diff to the tip is what changed since
  the review: after a rebase with an amend, the amend alone. A merge that conflicts must give no scope.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`an_amended_commit_is_rewritten_with_its_lines`, `after_a_rebase_with_an_amend_the_scope_holds_the_amend_alone`, `a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed`)
  [@test] ../../crates/git-ops/src/carried_review_test.rs (`the_reviewed_tip_is_its_own_carry_on_the_same_base`, `the_reviewed_change_is_merged_onto_a_moved_base_as_a_commit_of_it`, `a_reviewed_change_that_conflicts_with_the_base_carries_nothing`)
- `--since-reviewed` must list only the `added` and the `rewritten` commits; of a range, or with `--base` of a range,
  or a range rev that names no commit, it is `invalid_arguments` (exit 2).
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`a_range_has_no_interdiff`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`, `a_branch_never_marked_and_a_range_have_no_interdiff`)
- `bd review-plan` must print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`,
  `commits` with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, and `scope` with `base`, `head`
  and `carried`), with
  `"schema": 3`, and its output must be in `bd schema`. Version 3 has `files` in place of `risk` and `usages` in place
  of `fan_in`; the other outputs of `bd` keep `"schema": 2`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- The plan must write no ref and run a fixed number of git processes, whatever the number of commits: the listing and
  the base of a mark, one `rev-list` per side, one `diff-tree` with one `patch-id` for the commits that are not the
  same on both sides, one `diff-tree` for the rewritten ones, one `merge-tree` and one `fast-import` for a carry onto
  a base that moved; never `git range-diff`, never a process per commit.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`, `a_review_plan_since_the_review_on_a_moved_baseline`)

### API Surface

- The reviewed commits must be those of `base..head`; with `--since-reviewed` the diff of the scope: the commits added
  on top, or the one diff from the carried commit to the tip, so a rewritten commit counts against its reviewed
  version (a declaration it changed is changed, not added again) and an unchanged commit between two new ones adds
  nothing; only without a scope (the carry conflicted) the `added` and the `rewritten` commits, each read against its
  own parent. A branch never marked has no reviewed commit, so all of its commits are read. The `api`, the `files`,
  the `links`, the `dependencies`, the `repeated_edits` and the `order` must be computed from these alone.
  [@test] ../../crates/review-plan/src/api_test.rs (`since_the_review_the_api_is_that_of_the_diff_from_the_reviewed_change`, `since_the_review_an_unchanged_commit_between_two_of_one_file_adds_nothing`)
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`since_the_review_an_unchanged_commit_between_two_of_one_manifest_adds_no_entry`)
- `api` must list the public declarations (it and every declaration around it public, Java's `protected` too, none
  around it a function) that the commits add, remove or change in form, as the syntax summaries read them: the
  header, the parameters of a JVM function, the reach, `open`, the constructors of a type, what Java sees; a change
  counts when the declaration is public before or after it, so a narrowed reach is listed. A change of a private
  declaration or of a body alone must not be listed, nor a member of a type added or removed with it. A name of one
  or two characters must be listed too (the checks of a sync search only longer ones).
  [@test] ../../crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`, `a_rust_function_changes_by_its_header`, `a_public_declaration_of_a_short_name_is_listed`)
- A member that the summary marks as an override (Kotlin `override`, Java `@Override`) must not be listed: its
  supertype declares the API. A change is of an override when the member overrides before and after it.
  [@test] ../../crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_type_and_an_override_is_no_entry`, `a_changed_override_in_kotlin_or_java_is_no_entry`)
- Each entry must give its file and line, its name, its kind (`removed`, `changed`, `added`), the parts that
  changed, and its form before and after: the header with the parameters after the name, a type by its constructors.
  The file and line are those after the commits, through every rename, also one that changes no declaration; of a
  removal, those before the first commit that touched the declaration, so `path:line` is of the same content: the old
  path when that commit also renamed the file. A range of no source file of a known language has an empty `api`.
  [@test] ../../crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`, `a_range_without_source_files_has_no_api`, `a_removal_after_a_rename_names_the_path_its_line_is_of`, `the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Fan-in

- Every removed or changed entry must get its `usages` at the head of the range from `accio references <query>
  --ref <head>`, with `complete: false` when accio cut its answer at 100. An addition is never asked.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_usages_left_outside`, `a_cut_answer_is_not_complete`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_usages_at_the_head_rank_the_card`)
- The query of a member must be `Owner.name`, so that a use on another type drops out. The owner is the type around
  it that is not a Kotlin `Companion`: `ProjectManager.getInstance`, not `Companion.getInstance`. The query of a
  top-level declaration of a Kotlin or Java file in a package must be `package.name`; else it is the bare name.
  [@test] ../../crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_type_and_an_override_is_no_entry`, `the_usages_at_the_head_rank_the_card`)
- Only a hit in code must count as a usage: not the declaration itself, an `import` or a `doc` hit (`NOT_USES`).
  A hit in a file of another family of languages than the declaration (JVM, Rust, script) must not count: it is a
  namesake.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes`)
- Of a type whose only change is its constructors, only a `call` hit (`Type(…)`, `new Type(…)`, a Kotlin
  `class Sub : Type(…)`) and a `supertype` hit (a Java `extends Type`, whose `super(…)` calls a constructor) must
  count. A use of the type as a type or an import still compiles. The usages are as `complete` as the answer of accio.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes`)
- A name that nearly every type declares (`STOP_NAMES`: `equals`, `hashCode`, `toString`, `get`, `set`, `run`,
  `invoke`, `close`, `new` and others) must never be asked without an owner, bare or after a package: its `usages`
  stay `null`, unknown, not a wrong count. Two entries of the same query must share one query.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_entries_are_asked_by_their_query_and_never_by_a_bare_name_of_the_stop_list`)
- A member of such a name must be asked by `Owner.name` (a Rust `Store::new` as `Store.new`). accio also answers
  each call of the name on a receiver that it cannot type, so only a hit whose line names the owner as a word must
  count, and the usages must be `complete: false`, a lower bound.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`a_member_of_a_name_of_the_stop_list_counts_the_lines_that_name_its_owner`)
  [@test] ../../crates/review-plan/src/api_test.rs (`a_rust_constructor_is_asked_by_its_type`)
- The plan must ask at most 20 queries (`MAX_USAGE_QUERIES`), for the first entries of the card, in parallel.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`at_most_twenty_queries_per_plan`)
- A usage must be `inside` when its file is one that the reviewed commits change and still exists after them: this
  change updated it. Else it is `outside`, and the first 5 outside usages must be listed as `outside_sites` by path
  and line.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_usages_split_into_those_inside_the_change_and_those_left_outside`)
  [@test] ../../crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_type_and_an_override_is_no_entry`)
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
- The entries must be sorted by their usages left outside, the most first, then by those inside, those without
  `usages` last; then removals, changes and additions, then by file and line.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_usages_left_outside`, `the_usages_split_into_those_inside_the_change_and_those_left_outside`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_usages_at_the_head_rank_the_card`)
- Without accio, or without a usable index of the repository, the `usages` must be `null` and `index_note` must say
  why: "accio is not installed", else the reason accio gives ("no index of this repository", "accio is building the
  index of this repository", "the index was written by another version of accio"). A plan never builds, waits for or
  refreshes an index: every query runs with `--no-build`.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`without_an_index_the_usages_are_missing_and_the_note_says_why`)
  [@test] ../../crates/file-diffs/src/accio_test.rs (`an_index_accio_cannot_use_answers_no_index_with_its_reason`)
- A failing query must fail no plan: the others keep their answers and `index_note` gives the failure; only a
  cancel of the caller (a newer request of the window) cancels the plan.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`a_failing_index_keeps_the_api`, `a_canceled_query_cancels_the_plan`)

### Files

- `files` must list every path the reviewed commits change exactly once, at its last path: a renamed file at its new
  path with the path before the first commit (`old_path`), a deleted one at its path, none that the commits add and
  delete again.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_source_file_changed_without_its_test_is_untested`, `a_renamed_file_is_listed_at_its_new_path`)
- A file must be `untested` when it is a source file of a known language, no test itself, not mechanical, and no
  changed test names its stem.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_source_file_changed_without_its_test_is_untested`, `a_renamed_file_is_listed_at_its_new_path`)
- `breaks` must sum the usages left outside the change of the API a file removes or changes, at its last path, also
  of a removal whose entry names the old path. A usage inside the change and an addition break nothing.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_usages_of_a_files_api_rank_it_first`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`, `a_member_is_asked_by_its_type_and_an_override_is_no_entry`)
- `breaks_complete` must be true only when every API the file removes or changes has `usages` that are `complete`.
  A name not asked, no index, a failed query, a cut answer or a constructor-only change makes `breaks` a lower
  bound, also at 0. A file without such an API is complete.
  [@test] ../../crates/review-plan/src/files_test.rs (`breaks_is_a_lower_bound_when_a_usage_count_of_its_api_is_unknown_or_cut`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
- The files with the most `breaks` must come first, then the `untested` ones, then the tree order.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_usages_of_a_files_api_rank_it_first`)
- A test must be told by its path: a directory `test`, `tests`, `__tests__`, `testFixtures`, or a name `*Test`,
  `*Tests`, `*_test`, `*.test`, `*.spec`, `test_*`; it names the stem without that mark, compared without case.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_test_names_its_subject_by_its_path`)
- `bd review-plan` must print `api`, `files`, `links`, `dependencies`, `repeated_edits` and `index_note`, in
  `bd schema`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_files_in_dependency_order`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)
- The API and the files must add a fixed number of git processes, whatever the number of commits: one `log` of the
  changes of the reviewed commits, one `cat-file` of their blobs, and the reads of the syntax summaries; accio runs
  no git.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`, `a_review_plan_of_a_range`)
- The text of the hunks must come from the memory of the diffs when they read the blobs in this process. Else the
  blobs must join the `cat-file` of the summaries, which then reads them alone when it has no summary or manifest to
  read: one process more only for a plan whose diffs and summaries come from the object cache.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_modified_files`)

### Links

- `links` must give, per pair of changed files, the file that uses (`from`) and the file that declares (`to`), at
  their last paths, with each name that links them and the line of its first use, by the evidence of the dependency
  order.
  [@test] ../../crates/review-plan/src/links_test.rs (`a_new_file_that_another_imports_is_an_added_link`)
  [@test] ../../crates/review-plan/src/order_test.rs (`the_order_returns_its_graph_with_the_names_and_lines`)
- A link must be `added` when it exists only after the commits, `removed` when only before them, and `kept` when
  both; a file before the commits links at its path then, named by its last path. The names of a kept link are those
  after the commits, of a removed one those before.
  [@test] ../../crates/review-plan/src/links_test.rs (`a_new_file_that_another_imports_is_an_added_link`, `a_link_before_and_after_is_kept_and_one_only_before_is_removed`, `a_renamed_file_links_at_its_new_path`)
- `uses` and `used_by` of a file must count its `added` and `kept` links; `new_links` its `added` links as the file
  that uses.
  [@test] ../../crates/review-plan/src/links_test.rs (`a_new_file_that_another_imports_is_an_added_link`, `a_link_before_and_after_is_kept_and_one_only_before_is_removed`)
- A link must be `cross_module` when the nearest directories above its two files with a build file differ.
  [@test] ../../crates/review-plan/src/links_test.rs (`a_link_between_two_modules_crosses_them`)
- The links must run no git process: they read the summaries the API read.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### Mechanical Changes

- A file must be `deleted` when the commits delete it, and `buildModel` when its name is of the build model: `*.iml`,
  `BUILD`, `BUILD.bazel`, `*.lock`, `pnpm-lock.yaml`, `.idea/modules.xml`.
  [@test] ../../crates/review-plan/src/files_test.rs (`imports_whitespace_renames_deletions_and_the_build_model_are_mechanical`)
- A plugin descriptor, a `Cargo.toml` or a `package.json` must be `buildModel` when only its dependency entries
  changed ("Dependency Changes"). One that changed beside them, a new one, and one that a side does not parse must
  stay real work. One whose entries and remainder did not change is read as any other file. An `*.iml` that does not
  parse must stay real work too.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`, `the_dependencies_of_a_package_json_are_its_dependency_changes`, `the_dependencies_and_the_content_of_a_plugin_descriptor_are_its_dependency_changes`, `a_new_version_with_new_features_keeps_the_manifest_in_the_tree`)
- Else, from the text of every hunk of every commit that changes it: `renamed` when each change is a rename with no
  line added or removed; `whitespace` when each hunk has the same text without its whitespace, line ends included, so
  a line wrapped or joined too; `importsOnly` when, in a source file, each other hunk has only blank lines and lines
  of import statements, at least one.
  [@test] ../../crates/review-plan/src/files_test.rs (`imports_whitespace_renames_deletions_and_the_build_model_are_mechanical`, `a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)
- An import statement must start with an `import`, `use` or `package` line and run until the brackets it opens
  close, read in the whole file: a line inside a Rust `use a::{` block or a TypeScript `import {` block over several
  lines is a line of an import statement. A `pub use` re-exports, so it is none.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)
- A bracket inside a string, a backtick name or a comment of an import line must not count. A statement whose
  brackets are still open at a `;`, at the next import line or at the end of the file must count as its first line
  only, so the code after it stays real work.
  [@test] ../../crates/review-plan/src/files_test.rs (`an_unclosed_bracket_on_an_import_line_makes_no_import_of_the_code_after_it`)
- A file whose whitespace has a meaning must never be `whitespace`: Python (`*.py`, `*.pyi`), Starlark (`*.bzl`,
  `*.star`, `*.bazel`, `BUILD`, `WORKSPACE`), YAML (`*.yml`, `*.yaml`) and Makefiles (`Makefile`, `GNUmakefile`,
  `*.mk`). Its indent stays real work.
  [@test] ../../crates/review-plan/src/files_test.rs (`an_indent_where_the_indentation_has_a_meaning_is_no_whitespace_change`)
- One hunk with an import line and a line of code, a commit that changes code, an added file, and a file without
  text must keep the file out of the mechanical ones.
  [@test] ../../crates/review-plan/src/files_test.rs (`imports_whitespace_renames_deletions_and_the_build_model_are_mechanical`, `a_file_is_mechanical_only_when_every_hunk_of_every_commit_is`)

### Repeated Edits

- `repeated_edits` must group the files whose commits make one hunk with the same removed and added text, without
  whitespace and line ends: 3 or more files, the group of the most files first, a file in one group at most
  (`repeated_edit`, the index of its group), with the first lines removed and added for its label.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_same_edit_in_three_or_more_files_is_one_group`)
- A hunk of whitespace alone (a wrapped line too), and one without a line of 10 or more characters (a `}` alone),
  must make no group.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_same_edit_in_three_or_more_files_is_one_group`, `a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)

### Dependency Changes

- `dependencies` must list the entries the commits add to, remove from or change in the build files, read before the
  first commit that changes each file and after the last. They are sorted by build file in tree order, then by kind
  and name. Each has its build file (`manifest`), its `module`, its `scope`, and its versions before and after when it
  has one.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)
- The entries of an `*.iml` must be its `orderEntry` of type `module` and `library`, and of type `module-library`
  with a named `library`. The scope is `TEST`, `RUNTIME` or `PROVIDED`, else `main`. Its module is the name of the
  file.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`)
- A plugin descriptor must be an `*.xml` named `plugin.xml`, or in a `META-INF` or `resources` directory, out of
  `.idea`, whose root is `idea-plugin`. Its entries must be its `depends` and `dependencies` `plugin` (`plugin`), its
  `dependencies` `module` (`module`) and its `content` `module` (`content`). Its module is its `id`, else the name of
  the file. Another `*.xml` has no entry.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_dependencies_and_the_content_of_a_plugin_descriptor_are_its_dependency_changes`)
- The entries of a `Cargo.toml` must be those of `dependencies`, `dev-dependencies` and `build-dependencies`, of the
  same tables of each `target`, and of `workspace.dependencies`. The version is a string, or the `version` key of a
  table. Its module is `package.name`, else the name of its directory.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)
- The entries of a `package.json` must be those of `dependencies`, `devDependencies` and `peerDependencies`. Its
  module is its `name`, else the name of its directory.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_dependencies_of_a_package_json_are_its_dependency_changes`)
- A `Cargo.toml` or a `package.json` at the root of the repository without a name must have the module `(root)`. A
  root workspace manifest is the usual case. No crate or package name has parentheses, so the label is unique.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`a_new_version_with_new_features_keeps_the_manifest_in_the_tree`)
- Before and after, the entries of one kind and name must pair first by their table, then in their order. A pair
  whose version or scope differs must be one `changed` entry with both versions. It has `scope_before` when the scope
  differs. An entry without a pair is `added` or `removed`.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`, `the_dependencies_of_a_package_json_are_its_dependency_changes`)
- A pair must count as a change beside the dependencies when its form without the version and the scope differs, or
  when it moves to another table of the same scope. This applies also to a `changed` pair: a new version together
  with new features of a crate is a `changed` entry, and its build file stays real work.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`a_new_version_with_new_features_keeps_the_manifest_in_the_tree`, `an_entry_that_moves_to_another_table_of_the_same_scope_keeps_the_manifest_in_the_tree`)
- A build file the commits add must be one `newModule` entry `added`, named by its module. A build file the commits
  delete must be one `newModule` entry `removed`.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)
- A build file that has no text or does not parse, before or after the commits, must have no dependency change.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)
- The texts of the build files must come with the text of the hunks: from the memory of the diffs, else in the
  `cat-file` of the summaries. A plan of build files alone must run one `cat-file`, with the diffs read in this process
  or from the object cache.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)

### Dependency Order

- `order` must list every path the reviewed commits change exactly once, at its last path: a renamed file at its new
  path, a deleted one at its path (the review still shows it), none that the commits add and delete again.
  [@test] ../../crates/review-plan/src/order_test.rs (`files_of_no_known_language_keep_the_tree_order`)
- A file that declares what another file of the order uses or imports must come before it, read from the files as the
  last reviewed commit that changes each leaves it, in Kotlin, Java, Rust and TypeScript.
  [@test] ../../crates/review-plan/src/order_test.rs (`a_declaration_comes_before_the_file_that_uses_it`, `a_cycle_keeps_its_tree_order`)
- A use must link to a declaration only with the evidence the symbol dependencies ask (an import, a qualified name,
  the same package or module): of two modules that declare one name, a file follows only the one it imports.
  [@test] ../../crates/review-plan/src/order_test.rs (`a_name_of_two_modules_links_only_the_one_imported`)
- Files that use each other must keep their tree order among themselves, after what they use and before what uses
  them; files with no edge between them keep their tree order: at each level the directories before the files, then
  the names without case, a number by its value.
  [@test] ../../crates/review-plan/src/order_test.rs (`a_cycle_keeps_its_tree_order`, `the_order_of_a_graph`, `the_tree_order_of_paths`)
- A test (as `files` tells one) must come right after its subject when the commits change it, also inside a cycle,
  several in tree order; of two subjects of its stem the one it uses, else the one nearest in the tree. A test whose
  subject the commits don't change is a file like any other.
  [@test] ../../crates/review-plan/src/order_test.rs (`a_test_comes_right_after_its_subject`, `the_order_of_a_graph`)
- A deleted file, a file of no language the summaries read and a degraded one must have no edge: a range of such files
  is in tree order.
  [@test] ../../crates/review-plan/src/order_test.rs (`files_of_no_known_language_keep_the_tree_order`)
- `bd review-plan` must print `order`, in `bd schema`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_files_in_dependency_order`)
- The order must run no git process: it reads the summaries the API read.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### In the App

- The app must read the plan with `get_review_plan` and mark a branch with `mark_branch_reviewed`, through the same
  `review_plan_core` and `mark_reviewed_core` as `bd`, the branch named with the prefix the app resolves for the
  repository: after a mark and a commit on top, the plan since the review lists that commit as `added` with its scope,
  and the usages come from the index of the host (the fixture `.git/branch-deck-accio.json` in the e2e server).
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
- An unknown branch must answer `unknown_branch`; a base that names no commit, a branch that shares no history with
  its base, an unknown revision of a range and `sinceReviewed` of a range must answer `invalid_arguments`.
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
- A window must read one plan at a time: a newer plan of the window and its close must supersede the plan in flight,
  kill its queries of accio and answer `canceled`; a symbol search of the window and a plan must never supersede each
  other.
  [@test] ../../crates/app-api/src/session/windows_test.rs (`a_review_plan_and_a_symbol_search_of_a_window_never_supersede_each_other`, `a_closed_window_supersedes_its_request`)
- `branchdeck.reviewExperimental` must be on unless git config turns it off, read and written like the other keys of
  Settings; a value git refuses must be off with its reason and fail no sync: an experiment never turns on from a
  broken value.
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_review_experiments_are_on_until_the_config_turns_them_off`, `nothing_set_is_the_defaults`)
  [@test] ../../crates/sync-core/src/config_snapshot_test.rs (`the_review_experiments_are_on_unless_the_config_turns_them_off`)
- The switch "Review plan (experimental)" of Settings → Diffs & Merge → Review must turn it on and off for every
  repository in the global git config, and a repository's own `.git/config` value must win over it, as
  `branchdeck.resolveByWords` does (spec/repository/settings.spec.md).
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should turn the review plan on and off for every repository from Diffs & Merge`)
- The flag must reach the view through each sync (`TableHeader.reviewExperimental`), and a change of it from Settings
  must sync; with the flag off the review windows must look as they do without the review plan. `bd review-plan` and
  `bd mark-reviewed` must work without it, and so must the two commands of the app.
  [@test] ../../crates/repository-model/src/table_test.rs (`the_header_takes_the_review_experiments_of_each_grouping`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should look as it does without the review plan while the flag is off`)

### In the Review Window

- With the flag off, the branch review window must offer none of what follows and read no plan.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should look as it does without the review plan while the flag is off`)
- With it on, the window must read the plan of the shown part (`get_review_plan`): the reviewed branch on the base of
  the part, the whole stack as its range; again when the head of the part moves, when "Since Your Review" is chosen
  or left, and after Mark Reviewed; an answer `canceled` (a newer plan of the window) is dropped, and the plan of the
  same branch stays while the next one is read. Its interdiff counts only while the part is one branch (the reviewed
  pair is on the base of that branch alone) and the plan is of the head on screen: a part of several branches, or the
  plan of the head before a sync, shows no "since" and offers no "Since Your Review".
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)
  [@test] ../../src/lib/range-review.test.ts (`takes the interdiff only of the part's own branch, and only of a plan of the head on screen`)
- Mark Reviewed of the title bar must mark the reviewed branch of the part on the base of its own branch, as
  `bd mark-reviewed <branch> --base <parent>` does, say so, and read the plan again: nothing is new after it. The
  whole stack has no Mark Reviewed.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)
- "Since Your Review" must follow "All Changes" in the commit column only while the part is one branch, marked, with
  a commit added or rewritten since; each such commit says "New" or "Rewritten". Chosen, the column lists only the
  added and the rewritten commits, adjacent or not, and the diffs show the scope: the commits added on top, or the
  diff from the carried commit, a rewrite against its reviewed version and new commits apart together; without a
  scope (the carry conflicted) the newest of them. "All Changes", another part or a mark leaves it.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] ../../src/lib/range-review.test.ts (`offers no scope for a branch never marked, nor when nothing was added or rewritten`, `marks the added and the rewritten commits, and lists only them, adjacent or not, under the scope`, `keeps the plain range of the commits added on top, and the column of one new commit`)
- The Review Summary card must head the diffs of a scope, and in "Since Your Review" also a shown commit above its
  heading (a rewritten commit shows its whole diff, the card what it changes against its reviewed version). Its
  subtitle must give the counts, what can break first: "1 possible break · 1 dependency change · 1 new link between
  modules · 1 repeated edit · 2 new declarations", or "No possible break". Then "Since your review" as before (a
  rewritten commit with the lines per path it changes against its reviewed version), and the `index_note` last when
  the usages are missing.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should lead the Review Summary with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] ../../src/presenters/review-plan.test.ts (`counts what changed since the review, and nothing when only the baseline moved`, `says per path what a rewritten commit changes that its reviewed version did not, and the reverse`, `sums the card up in its title, what can break first`)
- "Possible Breakage" must list each removed or changed API, in the order Rust ranked it, unless every use is
  updated in this change: its kind, its name, `path:line`, the parts that changed, the form before and the form after
  on lines of their own, the uses left outside ("3 uses left outside") with the first sites as `path:line`, and the
  uses updated in this change. An entry with `usages` null says "The uses are unknown". An entry whose count is not
  `complete` says "At least N uses left outside", or "No use found outside, the count is incomplete" at 0. Only the
  entries with a `complete` count and no use outside fold into one collapsed line, "12 API changes, every use updated
  in this change": an unknown or incomplete count never reads as safe. An addition is not listed there.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should lead the Review Summary with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should keep an API whose uses are unknown under Possible breakage, and say why`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/presenters/review-plan.test.ts (`words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown`, `lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites`, `folds only the API whose every use is updated, and keeps an unknown or incomplete count listed, also at 0`)
- "Dependencies" must list the dependency changes per build file in Rust's order, under the module and the path of
  the build file: a changed version as `1.0.1 → 1.0.2`, a new or deleted module. Under them it must list the links
  the commits added between the files of two modules (`Reader.kt → Cache.kt`, with the names and the line of the
  first use). "Repeated Edits" must list one line per group, "Same edit in 3 files:" with the edit and its files, and
  "New API" the added declarations per file. Both start collapsed.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should lead the Review Summary with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../src/presenters/review-plan.test.ts (`groups the dependency changes by build file in Rust's order, and lists only the links the commits added between modules`, `words each repeated edit with its files, and the new API per file in the order of its first declaration`)
- A click on an entry of the card must select its file in the review and show its place in the diffs, its name
  selected, as a link of a badge does: the declaration of an API (a removal on the side before the commits, by its old
  path too), the first use of a link between modules in the file that uses, the first declaration of the new API of a
  file; the build file of a module and a file of a repeated edit at its header. A filter that hides the file is
  dropped as for a badge, also when the filters hide every file, and the place is shown once the file is.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should lead the Review Summary with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should select the file of a card entry while a filter hides every file, and drop only the filter that hides it`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
- The files must stay in the tree, in tree order, in All Changes and in "Since Your Review" as one range: the plan
  adds badges and the Mechanical group, and offers no other order. A commit of the column shows its plain tree.
  The facts apply once the plan of the shown scope is read, not while the next one is read.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should lead the Review Summary with what can break, show the declaration of an entry, and keep the files in the tree`)
- A file row must show `uses N` and `used by M`, the links among the changed files after the commits, with `+K new`
  for the links it uses that the commits added. Its popover must list each link it uses, then each link that uses it,
  a removed one too: the other file, the names and the line of the first use (`Cache, get · Reader.kt:3`), and the
  state Added, Removed or Kept. A click on a link must select that file, close the popover and show the file: a file
  that uses this one at its first use, the name selected, a file it uses at its header.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should badge each file with its links, the uses it breaks and no test, and show a linked file from its popover, a file that uses at its first use`)
- A click on a file in a popover must select it also when a filter hides it. The owner filter then goes back to All
  when it hides the file, and the search is cleared when it hides the file. A filter that shows the file stays.
  The click selects the file once: a later change of the owner filter or the search must not select it again.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should select a linked file that a filter hides, and drop only the filter that hides it`)
  [@test] ../../src/presenters/review-plan.test.ts (`counts the links among the changed files, the new ones apart, and lists each with its names, its line, its state and its place`)
- A file row must show `breaks N` in the warning tone for the uses left outside the change of the API it removes or
  changes. When `breaks_complete` is false it must show `breaks N+`, or a quiet `breaks ?` at 0: a lower bound never
  reads as safe. `no test` must be a quiet text of an `untested` file.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should badge each file with its links, the uses it breaks and no test, and show a linked file from its popover, a file that uses at its first use`)
  [@test] ../../src/presenters/review-plan.test.ts (`says the uses an API leaves outside, as a lower bound where Rust says the count is, never a safe 0`, `lists no file without a badge`)
- A file of a repeated edit must show `same edit ×N`. Its popover must name the edit (the first line removed and the
  first added, `a → b`) and list the other files of the group; a click selects one.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/presenters/review-plan.test.ts (`names the same edit of a group of files and the other files that get it`)
- The mechanical files must leave the tree for one group at its end, `Mechanical (N)`, with a group per kind in the
  order Build files, Imports only, Whitespace only, Renamed, Deleted, each file by its path. The group must start
  collapsed, also when the plan comes after the files, unless it holds the selected file. The diffs, J and K must
  reach these files last, and a file selected in it opens it. Its menu and ⌘C copy its changed paths.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/lib/file-tree.test.ts (`folds the mechanical files into one group at the end, a group per kind, each file by its path; J and K reach them last`, `starts with the Mechanical group collapsed, and opens it to reveal a file in it`, `copies the paths of a group, which has no path of its own`)
  [@test] ../../src/presenters/review-plan.test.ts (`names the Mechanical group and its groups by kind`)
- The dependency changes of a build file must show on the directory row whose chain holds the directory of the build
  file, else on the row of the build file (one at the root, or a module whose only change is its build file):
  `+N deps −M ~K`, `new module`, `deleted module`. Its tooltip must list each change, a changed version as
  `1.2 → 1.3` and a changed scope as `test → main`.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/lib/file-tree.test.ts (`shows the dependency changes of a build file on the directory row of its module, else on the row of the build file`)
  [@test] ../../src/presenters/review-plan.test.ts (`counts the dependency changes of a module, a version change as one changed entry with both versions`, `words a version, a scope and a new module`)

## User Experience

- `bd mark-reviewed auth` → `{"branch": "jane/virtual/auth", "ref": "refs/heads/jane/virtual/auth", "base": …,
  "tip": …, "previous": null, "changed": true, "pruned": 0}`.
- `bd mark-reviewed auth --base nope` → `{"error": {"code": "invalid_arguments", "message": "No branch or commit \"nope\"
  to review the branch on"}}`, exit 2.
- `bd review-plan auth --since-reviewed` after one commit on top → `{"branch": "jane/virtual/auth", "base": …, "head":
  …, "interdiff": {"reviewed_base": …, "reviewed_tip": …, "commits": [{"id": …, "reviewed_id": null, "subject": "Add
  logout", "state": "added"}], "scope": {"base": <reviewed tip>, "head": <tip>, "carried": false}}}`; after an amend
  of a reviewed commit, `"scope": {"base": <carried>, "head": <tip>, "carried": true}`.
- `bd review-plan base..head` of a new parameter → `"api": [{"path": "src/a/Api.kt", "line": 3, "name": "open",
  "kind": "changed", "changes": ["parameters"], "before": "fun open(path:String) : Int", "after": "fun
  open(path:String, mode:Int) : Int", "usages": {"inside": 2, "outside": 12, "outside_sites": [{"path":
  "src/b/Client.kt", "line": 8}, …], "complete": true}}]`, `"files": [{"path": "src/a/Api.kt",
  "old_path": null, "mechanical": null, "uses": 0, "used_by": 0, "new_links": 0, "breaks": 12, "breaks_complete":
  true, "untested": true, "repeated_edit": null}]`, `"index_note": null`.
- `bd review-plan base..head` of a new `Store` that `User` imports, with its test → `"order": ["src/b/Store.kt",
  "src/test/b/StoreTest.kt", "src/a/User.kt", "README.md"]`, `"links": [{"from": "src/a/User.kt", "to":
  "src/b/Store.kt", "symbols": [{"name": "Store", "line": 3}], "state": "added", "cross_module": false}]`.
- The review window, flag on: **Mark Reviewed** in the title bar → toast "Marked plan reviewed" ("plan was already
  marked reviewed at this commit" when nothing moved); the column: "All Changes 3 commits", "Since Your Review 1
  commit", "read the cache twice  New".
- The card **Review Summary**, "1 possible break · 2 new declarations": "Since your review: 1 new · 1 rewritten", under
  it "Rewritten scale the helper: src/Store.kt +1 −1"; under Possible Breakage `Changed helper src/Store.kt:36
  parameters 3 uses left outside`, under it `− fun helper(value:Int)`, `+ fun helper(value:Int, scale:Int)` and
  "src/Base.kt:1 · src/Uses.kt:4 · src/main/App.kt:9"; without an index "The uses are unknown"; "1 API change, every
  use updated in this change"; under Dependencies `b src/b/b.iml` "New module b" and "New links between modules
  Reader.kt → Cache.kt"; collapsed "Repeated Edits 1" and "New API 2"; "No usages: no index of this
  repository"; "Cannot read the review plan: …".
- The tree, flag on: `Reader.kt` `uses 1 +1 new` `same edit ×3` `no test`; its popover "Uses: Cache.kt Added, Cache,
  get · Reader.kt:3"; `Store.kt` `breaks 2`, or `breaks 2+` and `breaks ?` for a lower bound; the directory `b`
  `new module`, its tooltip "New module b"; last, the collapsed `Mechanical (2)` with `Build files (1)`
  and `Whitespace only (1)`.

## Measurements

<!-- The corpus runs of `bd review-plan` and what they found go here. -->

## Open Questions / Risks

- A branch deleted and created again under the same name keeps the pair of the old one, unless a mark pruned it while
  the branch was gone; the interdiff then compares it with the old branch.
- An unapplied (`{prefix}/unapplied/<name>`) or archived virtual branch has no `{prefix}/virtual/<name>`, so a mark of
  another branch meanwhile prunes its pair as gone; after a reapply it has no interdiff until it is marked again.
- Without `--base`, `bd` takes the baseline of the checked out branch for the merge base, so a stacked branch is
  marked with the commits of its parents; the app passes the base of the review's segment as `subject.base` and as
  the `base` of `mark_branch_reviewed`.
- A commit both reworded and amended has neither its identity nor its changed lines: it shows as `added`, and its
  reviewed version as `dropped`, which errs toward showing too much.
- Two commits of one author with the same subject in the same second share an identity; they pair in order.
- A carry that conflicts with the moved base (the baseline changed the lines the reviewed change changes) gives no
  scope: the commits are then read one at a time, a rewritten one against its own parent, so a declaration it adds
  shows as added even when its reviewed version added it too.
- The carried commit is on no ref: a `git gc` may prune it once it is old enough, and the next plan writes it again.
- A drop alone, no commit added or rewritten, offers no "Since Your Review", though the diff from the carried commit
  would show the removal.
- The usages of a top-level Rust or TypeScript declaration, and of a Kotlin or Java one in no package, are asked by
  the bare name, so a namesake of the same family of languages counts too. accio resolves `Owner.name` by the text
  and the types it knows, and it keeps a call on a receiver that it cannot type: a namesake can count there too. A
  removal's usages at the head are those left behind, or of a namesake; `outside_sites` lets the reader judge.
- A `string` hit counts: a class name in a plugin descriptor or a reflective call can break. A string that only
  holds the name counts too.
- Of a constructor-only change, every call of the type counts, also one that the change left valid, such as a call
  with named arguments that skips a new default parameter.
- An override of a changed open member in a subclass outside the change is missed: accio answers neither
  `references Owner.member` nor `implementations Owner.member` with it, and calls its answer complete
  (JetBrains/accio#71). A changed parameter list of an open member can therefore read as "every use updated".
- accio keeps a call of `Owner.member` whose receiver it cannot type and does not tell it from a typed one
  (JetBrains/accio#72); the owner check of `STOP_NAMES` stays until it does.
- A member of `STOP_NAMES` counts only the lines that name its owner: `store.close()` through a variable is missed.
- A usage is inside when its file is changed, not when its line is: a use in a changed file that the change left as
  it was counts as updated. With `--since-reviewed`, a use that an earlier reviewed commit of the branch updated
  counts as outside.
- An answer cut at 100 hits splits only those 100: `outside` is then a lower bound, and `complete` is false.
- The module roots of the summaries are read at the checked out HEAD, not at the head of the range.
- A key gets one entry: when an overload changed and another was added in the same commits, only the change is
  listed.
- `untested` sees only test files the commits change; a Rust test module inside its file does not count.
- `breaks` counts the usages left outside of the first 20 queries only; a name of `STOP_NAMES` without an owner
  breaks nothing known.
- An import statement ends where its brackets close. A Kotlin or Java import has no brackets, so a statement over
  several lines without brackets is not seen. A string or a comment over several lines is not seen either.
- `whitespace` removes the line ends too. In Kotlin, TypeScript or Go, a joined line can change the meaning, and a
  change of whitespace inside a string literal changes the value. Only the indentation-sensitive files are left out.
- The text of the hunks is read only up to the size limit of the summaries: a larger file is never `whitespace` or
  `importsOnly`. A Python or Go file is of no known language, so it is never `importsOnly`.
- An `*.iml` that parses is `buildModel` by its name, also when more than its entries changed. Its dependency
  changes still show.
- A dependency change that is no version or scope stays out of `dependencies`, such as the features of a crate,
  `exported` or `optional`. Its build file then stays real work, also when the same entry gets a new version.
- An entry that moves to another table of the same scope gives no entry, such as a crate that moves into a
  `target` table, or a `depends` that becomes a `dependencies` `plugin`. Its build file stays real work, also
  beside another dependency change of the same file.
- An entry that moves to another table and to another scope is one `changed` entry of the scope. The entry does not
  tell the new table, such as the condition of a `target` table, and its build file can be `buildModel`.
- `optionalDependencies` of a `package.json`, `[patch]` of a `Cargo.toml` and an unnamed module library of an
  `*.iml` are part of the remainder: a change of them keeps the build file real work, with no entry.
- A plugin descriptor is told by its path and its root: an `idea-plugin` elsewhere is no build file.
- Git may see a build file deleted and another added with a like text as a rename: then neither is a `newModule`.
- The links before the commits read the module roots at the checked out HEAD too: a module the commits add or delete
  may count a link before them across modules wrongly.
- The order reads each file as the last reviewed commit leaves it: with `--since-reviewed`, a later unreviewed commit
  may change its uses. A call of a function of the same package without an import is no evidence, so it orders
  nothing, as in the symbol dependencies.
- The tree order of `order` (`compare_tree_paths`, as the work order of the diffs) approximates the collator of the
  page's file tree for ASCII names; a name with accents may sort elsewhere.
- A part of several branches of a stack reads the plan of the reviewed branch on the base of the part, but offers no
  "Since Your Review" and its card no "since": the reviewed pair is on the base of the branch alone.
- The card heads the diffs of a scope, and a commit only in "Since Your Review": a commit chosen under "All Changes"
  shows its own heading alone.
