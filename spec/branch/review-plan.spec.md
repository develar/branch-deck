---
name: Review Plan
description: Requirements for the plan of a review (experimental), what a reviewer of a branch needs first; its start is Mark Reviewed, the base and the tip of a local branch kept in `refs/branch-deck/reviewed/`, the interdiff of its next review (`brd review-plan`), the commits since then, and the API surface the reviewed commits change with its usages from accio, updated in the change or left outside, the facts of each file (its links to the other changed files, a mechanical change, a generated file, a repeated edit, a missing test), the dependency changes of the build files and the files in dependency order.
style: plain-1
targets:
  - ../../crates/review-plan/src/lib.rs
  - ../../crates/review-plan/src/mark_reviewed.rs
  - ../../crates/review-plan/src/interdiff.rs
  - ../../crates/review-plan/src/api.rs
  - ../../crates/review-plan/src/fan_in.rs
  - ../../crates/review-plan/src/files.rs
  - ../../crates/generated-files/src/header.rs
  - ../../crates/generated-files/src/attributes.rs
  - ../../crates/generated-files/src/generated_by.rs
  - ../../crates/review-plan/src/links.rs
  - ../../crates/review-plan/src/dependencies.rs
  - ../../crates/review-plan/src/registrations.rs
  - ../../crates/review-plan/src/order.rs
  - ../../crates/trial-check/src/declaration_checks.rs
  - ../../crates/code-symbols/src/kotlin/summary.rs
  - ../../crates/code-symbols/src/java/summary.rs
  - ../../crates/code-symbols/src/rust/summary.rs
  - ../../crates/trial-check/src/symbol_dependencies.rs
  - ../../crates/trial-check/src/symbol_scan.rs
  - ../../crates/file-diffs/src/accio.rs
  - ../../crates/file-diffs/src/outside_place.rs
  - ../../crates/file-diffs/src/service.rs
  - ../../src/lib/diff/rows.ts
  - ../../crates/app-api/src/reads/symbols.rs
  - ../../crates/git-ops/src/reviewed_refs.rs
  - ../../crates/git-ops/src/carried_review.rs
  - ../../crates/trial-check/src/diff_scan.rs
  - ../../crates/bd-rewrite/src/mark_reviewed_command.rs
  - ../../crates/bd-rewrite/src/review_plan_command.rs
  - ../../crates/bd-cli/src/lib.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/app-api/src/reads/review_plan.rs
  - ../../crates/app-api/src/reads/review_prefetch.rs
  - ../../crates/app-api/src/app/mark_reviewed.rs
  - ../../crates/app-api/src/session/windows.rs
  - ../../crates/sync-core/src/repository_config.rs
  - ../../src/components/settings/BranchesPage.tsx
  - ../../src/lib/review-plan.ts
  - ../../src/lib/range-review.ts
  - ../../src/lib/file-tree.ts
  - ../../src/lib/file-actions.ts
  - ../../src/hooks/useReviewPlan.ts
  - ../../src/lib/review-plan-answer.ts
  - ../../src/hooks/useBranchReview.ts
  - ../../src/hooks/useSelectReviewFile.ts
  - ../../src/lib/review-place.ts
  - ../../src/presenters/review-plan.ts
  - ../../src/components/review/ReviewPrologue.tsx
  - ../../src/components/review/MessageChangeView.tsx
  - ../../src/presenters/review-story.ts
  - ../../src/components/review/BranchReview.tsx
  - ../../src/components/commit/CommitReview.tsx
  - ../../src/components/files/FileTree.tsx
  - ../../src/components/files/FileRowMark.tsx
  - ../../src/components/diff/CommitReviewCode.tsx
---

# Review Plan

Status: Experimental
Date: 2026-10-29

## Summary

A review of a branch lists its files in tree order, and a branch that agents keep landing commits on is read whole
again after each round. The review plan shows what the reviewer needs first: what changed since the last review, the
public declarations the branch adds, removes or changes with the places that use them, the code changed without its
tests, and the files in the order of their dependencies. It is computed in Rust, once for `brd` and the app
(`crates/review-plan`), and printed by `brd` as JSON, so that it can be measured over many ranges before the review
windows show it behind a flag.

Its first parts are Mark Reviewed, the base and the tip of a local branch kept when the user says they reviewed it,
and Since Your Review: the commits of the branch paired with the reviewed ones, unchanged, reworded, rewritten, added or
dropped, how the message of a reworded or rewritten one changed, and what changed since the review as one diff (`brd review-plan`): the plain range of the commits added on
top, else the diff from the reviewed change carried onto the base of the branch to its tip, whatever the branch went
through. Then the API surface: the public
declarations the reviewed commits add, remove or change in form, ranked by their usages at the head of the range from
accio that the change left outside, and the facts of each changed file: which other changed files it uses and which use it, whether its change
is mechanical, the edit it shares with other files, the usages of the API it removes or changes, and whether it
changed without its tests. Then the dependency changes: the module, library, plugin, crate and package entries the
commits add to, remove from or change in the build files, and the build files they add or delete. Then the order of
the files for `brd`: a file that declares what another uses before it, a test right after its subject. The review
window keeps the tree of the files: each row shows its links and its risk, and the mechanical files fold into one
group at the end. The overview at the top of the document of the review, before the story of the diffs
([Review Tour](review-tour.spec.md#in-the-review-window)), says what can break and what changed since the review. The vision of the review, its progress and the rounds of polish:
[Change Review](../../docs/design/change-review.md).

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
- The owners of a file in its facts: the owner chips of the review already mark them.

## Requirements

### Mark Reviewed

- A mark must keep the branch's tip and its base, the merge base of the tip and the baseline, as
  `refs/branch-deck/reviewed/<sha256 of the branch's full name>/{base,tip}`, so that the pairs of a branch `a` and a
  branch `a/b` never clash; a mark of a branch that has a pair must replace both refs with compare-and-swap on the
  values it read, and name the pair it replaced; the same base and tip again must move no ref.
  [@test] ../../crates/git-ops/src/reviewed_refs_test.rs (`a_mark_writes_the_pair_of_the_branch_and_a_second_mark_replaces_it`, `the_same_mark_again_moves_no_ref`)
  [@test] ../../crates/review-plan/src/mark_reviewed_test.rs (`marks_the_virtual_branch_of_a_name_at_its_merge_base_with_the_baseline`)
- A branch stacked on another must be markable on the base its review shows, the head of its parent (`brd mark-reviewed
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
- The branch must be named as `brd` names branches: the virtual branch `{prefix}/virtual/<name>` first, else the local
  branch `<name>`, or exactly `refs/heads/<name>`; a name of no branch must be `unknown_branch` and write nothing.
  [@test] ../../crates/review-plan/src/mark_reviewed_test.rs (`marks_the_virtual_branch_of_a_name_at_its_merge_base_with_the_baseline`, `an_unknown_branch_is_its_own_error`)
  [@test] ../../crates/bd-rewrite/src/mark_reviewed_test.rs (`an_unknown_branch_is_refused`)
- `brd mark-reviewed <branch>` must print the branch, its ref, the base, the tip, the pair it replaced (`previous`,
  `null` the first time), `changed` and `pruned`, and its output must be in `brd schema`.
  [@test] ../../crates/bd-rewrite/src/mark_reviewed_test.rs (`marks_a_virtual_branch_reviewed_and_again_after_a_new_commit`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- A mark must run a fixed number of git processes: the baseline (only without a base), one `for-each-ref` of the
  reviewed refs with the branches, one `rev-parse --verify` of a base that names no branch, one `merge-base` and one
  `update-ref`; it takes no repository lock and writes no branch, index or working tree.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_mark_reviewed`, `a_mark_reviewed_on_a_base`)
- An agent must run it only when the user says they reviewed the branch (`brd help worktrees`).
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Since Your Review

- A branch never marked must have no interdiff (`null`), and no scope; a range has none either.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`a_branch_never_marked_has_no_interdiff`, `a_range_has_no_interdiff`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`a_branch_never_marked_and_a_range_have_no_interdiff`)
- Each commit of the branch (`<base>..<tip>`, the base as a mark finds it) must be paired one to one, oldest first,
  with a commit of the reviewed pair: the same commit, else the same identity (author email, author time, subject
  without the `(prefix)`), else the same changed lines (`patch-id --verbatim` of the diff without context). A pair
  is `unchanged` when both change the same lines and have the same message, `reworded` when they change the same
  lines and the messages differ, else `rewritten`; a commit left is `added`, a reviewed one left is `dropped`, listed
  after the commits of the branch.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`commits_added_after_the_mark_are_the_range_on_top`, `a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed`)
- A `reworded` commit, and a `rewritten` one whose message changed too, must have its `message_change`: the subject of
  its reviewed version without the `(prefix)` when that differs (`subjectBefore`, only of a reword by the lines), and
  the `hunks` of its body by the blocks of the grammar of a body ([Commit Row → Message
  body](../commit/commit-row.spec.md)), an LCS of whole blocks: each hunk the blocks `added` at `start` of the body now
  and the reviewed blocks `removed` right before it, with the code of their lines. A trailer added, a paragraph wrapped
  anew and a `(prefix)` changed say the same: no `message_change`, and a pair `unchanged`. The messages are those of
  the `rev-list` of each side: no process more.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`the_body_of_a_message_changes_by_its_blocks`, `a_new_trailer_or_a_rewrap_changes_no_message`, `a_commit_amended_with_a_new_body_is_rewritten_with_its_added_and_removed_blocks`)
- A `reworded` commit changes the lines the reviewer read: the scope must treat it as `unchanged` (a reword below the
  commits added on top keeps their plain range), and the plan must read none of its lines.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`a_body_rewritten_alone_is_reworded_its_code_unread_and_the_scope_on_top`)
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
- `--since-reviewed` must list only the `added`, the `rewritten` and the `reworded` commits; of a range, or with `--base` of a range,
  or a range rev that names no commit, it is `invalid_arguments` (exit 2).
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`a_range_has_no_interdiff`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`, `a_branch_never_marked_and_a_range_have_no_interdiff`)
- `brd review-plan` must print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`,
  `commits` with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, `message_change`, and `scope`
  with `base`, `head` and `carried`), with
  `"schema": 25`, and its output must be in `brd schema`. Version 3 has `files` in place of `risk` and `usages` in place
  of `fan_in`, version 4 the `units` ([Review Units](review-units.spec.md)), version 5 the `generated` files with
  why, the lock files among them, version 6 a deletion as real work (no `mechanical: "deleted"`; the units
  `deletedFile` and `deletedFiles`), version 7 the `concerns` ([Review Concerns](review-concerns.spec.md)), version 8 their `elsewhere`, version 9 the name `file`, version 10 the `steps` and the `brief` ([Review Tour](review-tour.spec.md)), version 11 the `api` and `test` of a unit ([Review Units](review-units.spec.md)) in place of its `public`, version 12 the `still_bound` and `by_name` usages and `unbroken` in place of `updated`, version 13 the `reached` and `unreached` tests of a step ([Review Tour](review-tour.spec.md)) in place of `named` and `unnamed`, version 14 the story of each commit once (`steps.commits`, [Review Tour](review-tour.spec.md#the-story-of-a-commit)) in place of the `body` of a commit of a step, and the `newApi` and `moduleLinks` of a step, version 15 the `parts` of a story where they are `told` in place of its `toldIn`, the `joined` units and the `marks` of a stop, the `shownElsewhere` units of a step, the state `reworded`, the `message_change` of a commit, the `change` of a story and the `removals` of its parts, version 16 the `paragraphs` of an entry of the brief, whole and changed by word, with their `links` and `excerpt`, in place of its `added` and `removed`, and its `blocks` in place of its `block` ([Review Tour](review-tour.spec.md#brief)); version 19 added repeated replacements to the brief, version 20 a widening as an `added` entry with `widened`, the reach in the forms and the `verdict`, version 21 the form `row` of a paragraph of the brief, the `excerpt` of every changed paragraph of several sentences and no paragraph of front matter or metadata ([Review Tour](review-tour.spec.md#brief)), version 22 the `table` block of a body ([Commit Row](../commit/commit-row.spec.md#message-body)) and its `summary`, version 23 the `modules` of the changed files ([Modules](#modules)), the `module` of a file and the `moduleEdges` of a step in place of its `moduleLinks`, version 24 the `groups` of a large step ([Review Tour](review-tour.spec.md#groups-of-a-step)) in place of the `concern` of a stop, version 25 the `registrations` ([Registrations](#registrations)); the other outputs of `brd` keep `"schema": 2`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- The plan must write no ref and run a fixed number of git processes, whatever the number of commits: the listing and
  the base of a mark, one `rev-list` per side, one `diff-tree` with one `patch-id` for the commits that are not the
  same on both sides, one `diff-tree` for the rewritten ones, one `merge-tree` and one `fast-import` for a carry onto
  a base that moved; never `git range-diff`, never a process per commit. The plan of a commit shown alone with its
  reviewed commit (`ReviewSubject` `commit` with `reviewed`) must run one `rev-list` of both messages more than
  without it, and a reviewed commit git no longer has marks nothing.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`, `a_review_plan_since_the_review_on_a_moved_baseline`, `a_review_plan_of_a_commit_with_its_reviewed_one`)
  [@test] ../../crates/review-plan/src/story_test.rs (`a_changed_part_of_a_body_is_marked_and_unfolded_a_verification_too`)

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
  counts when the declaration is API or unstable API before or after it (below), so a narrowed reach is listed. A
  change of a private
  declaration or of a body alone must not be listed, nor a member of a type added or removed with it. A name of one
  or two characters must be listed too (the checks of a sync search only longer ones).
  [@test] ../../crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`, `a_rust_function_changes_by_its_header`, `a_public_declaration_of_a_short_name_is_listed`)
- Each public declaration must be, on each side of the commits, API, unstable API or no API, decided once
  (`declaration_checks::surface_of`) from the marks of the syntax summaries
  ([Symbol Dependencies](../commit/symbol-dependencies.spec.md)); a declaration that is not public is no API. Test
  code, an entry point and an override are no API whatever their other marks; else a declaration marked `unstable`, or
  inside a type marked so, is unstable API; else it is API.
  [@test] ../../crates/trial-check/src/declaration_checks_test.rs (`the_surface_of_a_kotlin_declaration`, `the_surface_of_a_kotlin_file_marked_internal`, `the_surface_of_a_java_declaration`, `the_surface_of_a_rust_declaration`)
- A member that the summary marks as an override (Kotlin `override`, Java `@Override`) must not be listed: its
  supertype declares the API. A change is of an override when the member overrides before and after it.
  [@test] ../../crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_place_and_an_override_is_no_entry`, `a_changed_override_in_kotlin_or_java_is_no_entry`)
- Test code must not be listed, added, removed or changed: a test function (`test`: `@Test` and the other annotations
  of a test framework, a JUnit 3 `testX()`, a Rust `#[test]`), a declaration a test function is declared in (a test
  class, a type around a nested test class, a Rust module that holds a test), and every declaration inside one (a
  `@BeforeEach` or `setUp`, a fixture field, a nested class); the climb from a test stops at a Rust module, so the
  module around a `mod tests` and its other items stay API. A public helper beside a test in the same file, outside
  the test class, is listed.
  [@test] ../../crates/review-plan/src/api_test.rs (`a_test_function_and_its_class_are_no_entry`)
  [@test] ../../crates/trial-check/src/declaration_checks_test.rs (`the_surface_of_a_kotlin_declaration`, `the_surface_of_a_rust_declaration`, `a_rust_public_inline_module_is_not_test_code`)
- An entry point must not be listed, added or changed: a Kotlin top-level `fun main` and a Java `public static void
  main(String[])` (`entry_point`).
  [@test] ../../crates/review-plan/src/api_test.rs (`a_main_function_is_no_entry`)
- Unstable API, marked `@ApiStatus.Internal`, `@ApiStatus.Experimental`, `@VisibleForTesting` or `@TestOnly` on the
  declaration, on a type around it or, in Kotlin, on its file (`@file:ApiStatus.Internal`), must not be listed as
  added: no other module is meant to start to depend on it. Its removal and its change must be listed, with their
  `usages`, because callers in other modules of the repository can break.
  [@test] ../../crates/review-plan/src/api_test.rs (`unstable_api_is_no_new_api_but_its_change_is_listed`)
  [@test] ../../crates/trial-check/src/declaration_checks_test.rs (`the_surface_of_a_kotlin_declaration`, `the_surface_of_a_kotlin_file_marked_internal`, `the_surface_of_a_java_declaration`)
- Per key, the entry must follow the surfaces before and after the commits: a new form that is API is `added`, one
  that is unstable or no API is no entry; a form gone that was API or unstable API is `removed`, one that was no API
  is no entry; a form that changed is `changed` when either side is API or unstable API, and no entry when both are no
  API. A form both sides have that becomes API from unstable API or no API (its `@ApiStatus.Internal` removed) is
  `added`, a promotion; one that stops being API in the same form is no entry.
  [@test] ../../crates/review-plan/src/api_test.rs (`a_promoted_declaration_is_new_api`, `unstable_api_is_no_new_api_but_its_change_is_listed`, `the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)
  [@test] ../../crates/review-plan/src/api_test.rs (`a_change_of_form_to_or_from_api_is_listed`)
- A change of the reach alone to a wider one (`Reach::wider_than`: a private member, private in its file, `pub(super)`,
  the module, public), of a declaration no subtype may override, must be `added` with `widened`, its form `before`
  kept and no `usages` asked: it breaks no use (`internal fun mediaDuration` → `fun mediaDuration`). Opened to unstable
  API it is no entry, as an addition of it is none. The reach of an overridable member (an `open` Kotlin member, a
  method of a Java class that is not final) must stay `changed`: an override must not weaken the access of what it
  overrides; so must an `open` member of a final Kotlin class, whose modality the summary does not read, and a widening
  with another change of the form (a parameter more).
  [@test] ../../crates/review-plan/src/api_test.rs (`a_widening_alone_is_an_addition_that_breaks_nothing`, `a_widening_of_an_open_member_stays_a_change`, `a_change_of_form_to_or_from_api_is_listed`)
  [@test] ../../crates/code-symbols/src/summary_test.rs (`a_reach_is_wider_than_every_one_before_it`)
- Each entry must give its file and line, its name, its kind (`removed`, `changed`, `added`), the parts that
  changed, and its form before and after: the header with the parameters after the name, a type by its constructors,
  written as code is (`fun hooks(): List<String>`, `fun open(path: String): Int`), never its tokens spaced apart; the
  type parameters of a declaration stand apart from its keyword (`fun <T> String.trim2(): T?`, `static <T> List<T> of()`),
  the parameters a header holds follow the name (`fn open() -> u8`) or its type parameters, and a TypeScript
  string or template literal reads as written (`` function pick<T>(mode: "a", prefix = `x-${name}`): T ``), a Rust
  header as rustfmt writes it (`fn get<'a>(&self, path: &'a str) -> Option<&'a [u8]>`, `x: *const u8`, `T: ?Sized`),
  an operator after an operand keeping its spaces (TypeScript's `A & B`). Each
  form must write its reach as the language of its file does, so two forms that differ in the reach alone read apart: Kotlin `internal` and `private`, Java
  `private` (its package none), Rust `pub(crate)` and `pub(super)` (private none), TypeScript `private` of a member,
  C# `internal`, `private` and `file`; a public reach (Java and C# `public`, Rust `pub`, a top-level TypeScript
  `export`; Kotlin none) only in the forms of a change whose reach differs, as the summary holds a `protected` member as
  public and the members of a Rust trait are public without `pub`. A type's constructors read as calls of it, a line
  each (`Event(text: String, start: Long = …)`, `private Event()`), each after the narrower of its reach and its
  type's (`internal Port(value: Int?)` of an internal class), never the name of a `Reach`; a parameter by its name and
  type, `= …` for a default, `vararg` before a vararg; a type of several names (its last names, the structure gone) is
  not shown, never as `List/Window`. Only what the form shows changes: Rust compares the forms as it holds them.
  The file and line are those after the commits, through every rename, also one that changes no declaration; of a
  removal, those before the first commit that touched the declaration, so `path:line` is of the same content: the old
  path when that commit also renamed the file. A range of no source file of a known language has an empty `api`.
  [@test] ../../crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`, `a_range_without_source_files_has_no_api`, `a_removal_after_a_rename_names_the_path_its_line_is_of`, `the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`)
  [@test] ../../crates/review-plan/src/api_test.rs (`a_form_of_type_parameters_reads_as_code`, `a_form_of_constructors_and_parameters_reads_as_code`, `a_widening_alone_is_an_addition_that_breaks_nothing`, `a_widening_of_an_open_member_stays_a_change`, `a_typescript_header_reads_as_code`, `a_rust_header_reads_as_code`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Fan-in

- Every removed or changed entry must get its `usages` at the head of the range from `accio references --at <place>
  --ref <head>`, every place of the plan in one process, with `complete: false` when accio cut the answer of a place
  at 100. An addition, a widening too, is never asked.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_usages_left_outside`, `a_cut_answer_is_not_complete`)
  [@test] ../../crates/file-diffs/src/accio_test.rs (`the_places_of_a_plan_are_asked_in_one_process`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_usages_at_the_head_rank_the_card`)
- The place of an entry must be the line of its name in the last commit that touched it, `<commit>:<path>:<line>`;
  of a removal, the line before the commit that first touched it, at that commit's parent. accio answers only that
  declaration: a member of its own type, a top-level function of its own module, a name every type declares too.
  Two entries of one place must share it. A place accio finds no declaration at (`unavailable`: a line it does not
  index, such as a Kotlin primary-constructor `val`, JetBrains/accio#189) must leave the entry's `usages` null,
  unknown, never "none left outside" (JetBrains/accio#215: `references --at` still answers it as complete with no hit).
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`an_entry_is_asked_by_its_place_once`, `a_place_with_no_declaration_leaves_its_usages_unknown`)
  [@test] ../../crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_place_and_an_override_is_no_entry`, `a_rust_constructor_is_asked_by_its_place`, `the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`)
- Only a hit in code that a type or an import ties to the declaration must count as a usage: not the declaration
  itself, an `import` or a `doc` hit (`NOT_USES`). A hit only the name ties (`resolved: name`: a receiver nothing
  typed, another language, a namesake's) must not count; those left outside are `by_name`, unknown, and keep the
  entry from being `unbroken`.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes`, `a_hit_only_the_name_ties_is_no_use_and_keeps_the_api_listed`)
  [@test] ../../crates/review-plan/src/api_test.rs (`a_rust_constructor_is_asked_by_its_place`)
- An `override` hit, an override of a changed member in a subtype, is a use: a changed parameter list breaks it.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`an_override_left_outside_can_break`)
- Of a type whose only change is its constructors, only a `call` hit (`Type(…)`, `new Type(…)`, a Kotlin
  `class Sub : Type(…)`) and a `supertype` hit (a Java `extends Type`, whose `super(…)` calls a constructor) must
  count. A use of the type as a type or an import still compiles. The usages are as `complete` as the answer of accio.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes`)
- A usage must be `inside` when a line of it at the head is one the reviewed commits changed, a call from its first
  line to its last (`end_line`): a line of the blocks of its file from the blob before the first commit that changes
  it to the blob after the last one, as the diffs of the range show them; any line of a file they add. This change
  wrote or rewrote it. Else it is left outside, a use on lines of a changed file that the change left as they were
  too.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`a_use_is_inside_only_on_a_line_the_commits_changed`, `a_call_is_inside_by_any_of_its_lines`)
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_usages_split_into_those_inside_the_change_and_those_left_outside`)
  [@test] ../../crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_place_and_an_override_is_no_entry`)
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
- A call left outside must be `still_bound`, no break, when the entry changed only its parameters or its
  constructors, accio says the call fits the new form (`fits`), and each argument binds (`parameter`) a parameter of
  the same name and type as before the commits: a named one by its name, a positional one by its position in every
  parameter list before (a vararg takes every position from its own), a trailing lambda as the last parameter. A
  positional argument that now binds another parameter, a spread, and a call of an entry whose header, reach or Java
  view changed too, can break. Every other use left outside is `outside`, and the first 5 of those must be listed as
  `outside_sites` by path and line.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`a_call_left_outside_that_binds_what_it_bound_before_still_binds`, `a_trailing_lambda_and_a_vararg_bind_by_their_place`, `a_constructor_call_left_outside_that_names_only_kept_parameters_still_binds`)
- `unbroken` must be true only when the usages are `complete`, none left outside can break and none is tied by the
  name alone: every use is updated in this change or still binds. An unknown or cut count never is, also at 0.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`an_api_is_unbroken_only_by_a_complete_count_with_no_use_left_outside_that_breaks`, `a_hit_only_the_name_ties_is_no_use_and_keeps_the_api_listed`)
- The plan must say once whether a use left outside breaks (`verdict`), over its removed and changed entries:
  `breaks` when one has a use left outside that can break, whatever the others; else `unknown` when the `usages` of one
  are null or not `unbroken` (unknown, cut, tied by the name alone); else `noUseBreaks`, also of an API of additions and
  widenings alone; `pending` in the plan the window gets while the index is asked.
  [@test] ../../crates/review-plan/src/api_test.rs (`the_verdict_says_whether_a_use_left_outside_breaks`, `a_widening_alone_is_an_addition_that_breaks_nothing`, `the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
- The entries must be sorted by their usages left outside that can break, the most first, then by those inside,
  those without `usages` last; then removals, changes and additions, then by file and line.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_usages_left_outside`, `the_usages_split_into_those_inside_the_change_and_those_left_outside`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_usages_at_the_head_rank_the_card`)
- Without accio, or without a usable index of the repository, the `usages` must be `null` and `index_note` must say
  why: "accio is not installed", else the reason accio gives ("no index of this repository", "accio is building the
  index of this repository"), or explain a format mismatch without the shard path: an index from a newer accio than
  `<executable>` says "the index of this repository is from a newer accio than <executable>: update accio", and an
  older one says "the index of this repository is from an older accio than <executable>: run accio index". A plan never builds, waits for or
  refreshes an index: every query runs with `--no-build`.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`without_an_index_the_usages_are_missing_and_the_note_says_why`)
  [@test] ../../crates/file-diffs/src/accio_test.rs (`an_index_accio_cannot_use_answers_no_index_with_its_reason`, `an_index_format_mismatch_says_which_accio_version_to_run`)
- A failing query must fail no plan: the entries keep no `usages` and `index_note` gives the failure; only a cancel
  of the caller (a newer request of the window) cancels the plan.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`a_failing_index_keeps_the_api`, `a_canceled_query_cancels_the_plan`)

### Files

- `files` must list every path the reviewed commits change exactly once, at its last path: a renamed file at its new
  path with the path before the first commit (`old_path`), a deleted one at its path, none that the commits add and
  delete again.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_source_file_changed_without_its_test_is_untested`, `a_renamed_file_is_listed_at_its_new_path`)
- A file must be `untested` when it is a source file of a known language, no test itself, not mechanical, no
  changed test names its stem, and the commits change no test code inside it (a unit that is `test`: a test of its
  Rust `#[cfg(test)] mod tests`, a fixture of it; or a block of a repeated edit inside such code, whose unit is of no
  declaration); a test it keeps while its code changes does not count.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_source_file_changed_without_its_test_is_untested`, `a_renamed_file_is_listed_at_its_new_path`)
  [@test] ../../crates/review-plan/src/files_test.rs (`a_rust_file_whose_inline_test_changed_is_tested`, `a_rust_file_whose_inline_test_a_repeated_edit_changed_is_tested`)
- `breaks` must sum the usages left outside the change of the API a file removes or changes, at its last path, also
  of a removal whose entry names the old path. A usage inside the change and an addition break nothing.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_usages_of_a_files_api_rank_it_first`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`, `a_member_is_asked_by_its_place_and_an_override_is_no_entry`)
- `breaks_complete` must be true only when every API the file removes or changes has `usages` that are `complete`.
  A name not asked, no index, a failed query, a cut answer or a constructor-only change makes `breaks` a lower
  bound, also at 0. A file without such an API is complete.
  [@test] ../../crates/review-plan/src/files_test.rs (`breaks_is_a_lower_bound_when_a_usage_count_of_its_api_is_unknown_or_cut`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)
- The files with the most `breaks` must come first, then the `untested` ones, then the tree order.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_usages_of_a_files_api_rank_it_first`)
- A test must be told by its path: a directory `test`, `tests`, `__tests__`, `testFixtures`, `testSrc` (of an IntelliJ module), or a name `*Test`,
  `*Tests`, `*_test`, `*.test`, `*.spec`, `test_*`; it names the stem without that mark, compared without case.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_test_names_its_subject_by_its_path`)
- `brd review-plan` must print `api`, `files`, `links`, `dependencies`, `repeated_edits` and `index_note`, in
  `brd schema`.
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
- A link must be `cross_module` when the nearest directories above its two files with a build file differ, and
  `new_between_modules` when it is also `added`: a new dependency of a module, which the review lists. The build files
  are those of the head reviewed, not of the checkout: a pull request is not checked out, and a module it adds is in no
  other tree.
  [@test] ../../crates/review-plan/src/links_test.rs (`a_link_between_two_modules_crosses_them`, `the_links_of_a_head_that_is_not_checked_out_cross_the_module_it_adds`)
  [@test] ../../crates/trial-check/src/module_roots_test.rs (`reads_the_directories_of_a_revision_that_is_not_checked_out`)
- The links must run no git process: they read the summaries the API read.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### Mechanical Changes

- A file must be `buildModel` when its name is of the build model: `*.iml`, `BUILD`, `BUILD.bazel`, a `*.lock` that
  is no lock file of a package manager, `.idea/modules.xml`.
  [@test] ../../crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`)
- A file of no language and of no module itself must be `moduleList` when every line the commits add or remove (one
  at least) names whole a module they add or delete: the directory of its build file or the name of its `*.iml`, no
  character of a name right before or after it. It is the wiring of that module (a generated list of the module
  directories, as `build/bazel-generated-file-list.txt` of a large monorepo), not the first thing of its step; a line
  that names a module the change did not add keeps the file real work.
  [@test] ../../crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`, `a_list_that_names_no_module_the_commits_add_is_real_work`)
- A file the commits delete must stay real work: the deletion is a decision of its author, as a new file is, and the
  same change as the edits of the other files it goes with (a server dropped from five configs, and from the sixth
  that held nothing else). Only a deleted file that is generated (by its name, the attributes of the head, or the
  header of the blob the deletion removed, read in the same batch) or of the build model by its name is mechanical.
  A deleted file is never `untested`.
  [@test] ../../crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`, `a_generated_file_is_mechanical_by_its_name_its_attributes_or_its_header`)
  [@test] ../../crates/review-plan/src/units_test.rs (`a_deleted_file_is_a_unit_and_the_deleted_files_of_a_directory_are_one`)
- A file must be `generated`, nobody writes it by hand, with why (`generated`, `GeneratedBy` of
  `generated-files`): by its name, a lock file of a package manager (`Cargo.lock`, `pnpm-lock.yaml`, `go.sum`, …), a
  minified file (`*.min.js`, `*.min.css`) or a source map (`*.map`); else by the attributes of the head of the range
  (`linguist-generated`, `-diff`), one `check-attr --source=<head>` for the paths no name marks; else by its header:
  the leading block of comments of its new text (of a deleted file the text its deletion removed), in the comment syntax of its language (`<!--` in Markdown and XML,
  `#`, `//`, `/*`, `--`, `;`), within 5 comment lines after a front matter, a shebang and blank lines, whose comment
  starts with the words of a generator (`Generated by`, `Generated from`, `Code generated by`, `This file is
  generated`, `Auto-generated`, …) or holds `@generated`; the line is kept for the review. An added file too, and a
  change of code inside one. A snapshot (`*.snap`) stays real work, its review reads the expected output.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_generated_file_is_mechanical_by_its_name_its_attributes_or_its_header`)
  [@test] ../../crates/generated-files/src/header_test.rs (`the_leading_comment_of_a_generator_marks_a_file_in_the_comment_syntax_of_its_language`)
  [@test] ../../crates/generated-files/src/attributes_test.rs (`the_attributes_of_the_revision_mark_generated_files_and_a_lock_file_is_one_by_its_name`)
- A file that only mentions a generator must stay real work: `generated` in the middle of a sentence, a Markdown
  heading (`# Generated Files`), a marker past the fifth comment line or after the code, a comment of another syntax,
  JSON (no comments), and the ignore files, whose `# Generated by Cargo` names the tool that started a file people
  keep.
  [@test] ../../crates/generated-files/src/header_test.rs (`a_hand_kept_file_that_mentions_a_generator_is_no_generated_file`)
- The text of a file generated by its name or its attributes must not be read, and of the others the new blob joins
  the batch the hunks are read in (no process more when the blobs are in memory); a blob too large for that batch has
  no header, and its file starts collapsed for its size anyway.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_file_generated_by_its_name_or_attributes_reads_no_blob`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`, `a_review_plan_of_modified_files`)
- A plugin descriptor, a `Cargo.toml` or a `package.json` must be `buildModel` when only its dependency entries
  changed ("Dependency Changes"). One that changed beside them, a new one, and one that a side does not parse must
  stay real work. One whose entries and remainder did not change is read as any other file. An `*.iml` that does not
  parse must stay real work too.
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`, `the_dependencies_of_a_package_json_are_its_dependency_changes`, `the_dependencies_and_the_content_of_a_plugin_descriptor_are_its_dependency_changes`, `a_new_version_with_new_features_keeps_the_manifest_in_the_tree`)
- Else, from the text of every hunk of every commit that changes it: `renamed` when each change is a rename with no
  line added or removed; `whitespace` when each hunk has the same text without its whitespace, line ends included, so
  a line wrapped or joined too; `importsOnly` when, in a source file, each other hunk has only blank lines and lines
  of import statements, at least one.
  [@test] ../../crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`, `a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)
- An import statement must start with an `import`, `use` or `package` line, a C# `using` directive or a `namespace`
  line, and run until the brackets it opens close, read in the whole file: a line inside a Rust `use a::{` block or a
  TypeScript `import {` block over several lines is a line of an import statement. A `pub use` re-exports, so it is
  none, and a C# `using` statement (`using (var x = …)`, `using var x = …;`) is code.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`, `a_csharp_using_directive_is_an_import_and_a_using_statement_code`)
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
  [@test] ../../crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`, `a_file_is_mechanical_only_when_every_hunk_of_every_commit_is`)

### Repeated Edits

- `repeated_edits` must group the files whose commits make one hunk with the same removed and added text, without
  whitespace and line ends: 3 or more files, the group of the most files first, a file in one group at most
  (`repeated_edit`, the index of its group), with the first lines removed and added for its label.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_same_edit_in_three_or_more_files_is_one_group`)
- A generated file must be in no group: its tool made the edit, which the source it was rendered from shows. The
  source and two renderings with the same hunk make no group, and the source shows its own change.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_generated_file_makes_no_repeated_edit`)
- A file of a group must be `repeated_edit_only` when every hunk of every commit that changes it is the edit of its
  group, every change of it with the text of its hunks: the group says all of its change. One with another change
  beside the edit, or with a change whose hunks are not read, is not.
  [@test] ../../crates/review-plan/src/files_test.rs (`a_file_whose_every_hunk_is_the_repeated_edit_is_only_that_edit`)
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

### Modules

The shape of a change before any of its words: which modules it adds, changes or deletes, how large each part is,
and what each newly uses. The review opens on it ([Review Tour](review-tour.spec.md)).
- `modules` must list the module of every changed file once: the nearest directory above the file with a build file
  at the head reviewed (`ModuleRoots::root_of`: an `*.iml`, a `*.csproj`, a Bazel, Gradle or Maven build file, a
  `Cargo.toml`, a `package.json`), or of a module the commits delete (a build file they delete there, which the head
  lacks), else the root of the repository (`""`, named and labelled `(root)`). `files[].module` names it by its place.
  [@test] ../../crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`, `the_files_of_a_module_the_commits_delete_stay_in_it`)
- A module must be `added` when the commits add a build file at its root, `deleted` when they delete one and the head
  has no module there, else `changed`. Its `name` is that of its `*.iml` (the shortest stem beside a `*.tests.iml`), of
  its crate, of the module a dependency change gives a build file at its root, else of its directory. Its `label` is
  the shortest suffix of its root no other module of the review shares, of two directories at least when its last
  has no `.`, `-` or `_` (`shared/code`, `view.gui.media`, `review-plan`).
  [@test] ../../crates/review-plan/src/modules_test.rs (`a_label_is_the_shortest_suffix_no_other_module_shares`, `the_files_of_a_module_the_commits_delete_stay_in_it`)
- A module must count its changed `files` by role: `build` (the build model, a manifest, a `moduleList`), `prose`
  (Markdown, reStructuredText, AsciiDoc, text), `tests` (the paths of [Files](#files)), `code` (a language the summaries
  parse), the rest `resources`; its lines `additions` and `deletions` over the range, by the blocks of each file, of a
  file the range adds or deletes the lines it leaves or had, as the diff of the range counts them; its
  `directories` below the root without a leading source directory (`src`, `testSrc`, `main`, `kotlin`, `java`,
  `resources`, …) and the folders the files of each source directory share (the packages `com/example/player` of a
  module of Java or Kotlin, under `src` and under `testSrc` alike; a bundle under `resources/messages` shares none of
  them), by their first two levels, the most files first.
  [@test] ../../crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`, `a_file_the_range_adds_counts_the_lines_its_commits_leave`, `the_folders_of_a_module_start_below_the_packages_its_files_share`)
- A module must list the modules of the review it newly `uses`: by the links the commits add from its files to theirs
  (`new_between_modules`), and `declared` when a dependency entry its build files add, or one a build file the commits
  add declares, names that module; `usedBy` the reverse; `contentOf` the plugin modules whose descriptors the commits
  make hold it as content; `newDependencies` the entries its build files add that name no module of the review.
- The modules must come in the order of the review: one the commits add first, then the most changed lines of code
  and tests, then tree order.
  [@test] ../../crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)
- A step must name the new uses between modules it adds (`moduleEdges`: `from`, `to` and the `links` of the step between
  their files), each link in the step whose stop shows its first use: "view.gui.media → view.gui (38 links)", never a
  list of the pairs of files.
  [@test] ../../crates/review-plan/src/story_test.rs (`a_step_lists_the_new_api_and_the_new_links_between_modules_it_holds`)
- The modules must read the trees above every changed file in the `cat-file --batch` of the summaries: a plan of
  build files alone runs that batch too.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)

### Registrations

How a change plugs into the platform: what its plugin descriptors register, as the platform reads them. Every fact
comes from accio's port of the platform's reader; the plan parses no descriptor of its own for it.
- `registrations` must list each service, extension point, listener, extension, registry key, action or group, and
  content module that the changed plugin descriptors add, remove or change between the base and the head. A changed
  file can be a descriptor by its path as in [Dependency Changes](#dependency-changes); only accio's answer of its root
  (`idea-plugin`) makes it one. Each side is read by one `accio parse --descriptor --resolve --no-build --ref <side>`
  with the paths on its standard input: at the base the files the commits do not add, at their first path, at the head
  those they do not delete. The two run at once, beside the usages and the tests of the plan.
  [@test] ../../crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)
  [@test] ../../crates/file-diffs/src/accio_test.rs (`the_descriptors_of_a_side_are_read_in_one_process`)
- A registration must be known by what identifies it to the platform: a service by its area (`app`, `project`,
  `module`), its interface (else its implementation), its client and its OS; an extension point by its area and its
  qualified name (else its name); a listener by its area, its topic and its class; an extension of
  `com.intellij.registryKey` by its key (`registryKey`, its `defaultValue` the option marked `*` of a key of `options`,
  its `description`); another extension by its point and its implementation, else its `id`, else its whole element; an
  action or a group by its `id`, else its class, nested in a group too; a content module by its name (its `loading`,
  the `reviewModule` of that name).
  [@test] ../../crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`, `a_refused_descriptor_registers_nothing_and_a_service_is_known_by_its_interface_and_client`)
- The registrations of one key must pair in their order. A pair whose other fields differ must be `changed`, with
  what it was `before` and the fields it `changed`: an attribute by its name (`serviceImplementation`, `defaultValue`,
  `loading`), a child element by its tag (`<add-to-group>`). The rest are `added` or `removed`; a descriptor the commits
  add or delete adds or removes all it registers.
  [@test] ../../crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`, `a_refused_descriptor_registers_nothing_and_a_service_is_known_by_its_interface_and_client`)
- A side whose root is no `idea-plugin` must register nothing. A descriptor the platform's reader refuses on either
  side must have no registration.
  [@test] ../../crates/review-plan/src/registrations_test.rs (`a_refused_descriptor_registers_nothing_and_a_service_is_known_by_its_interface_and_client`)
- Each registration must name its `descriptor` at its last path, the `module` of the descriptor, and the `classes` it
  names, each as accio resolved it at the head (of a removed one, at the base): `declared` with its `path` and `line`,
  `ambiguous` with the `count`, `notFound`. Without a usable index accio still reads the descriptors: their classes are
  unknown (`null`), never `notFound`, and the reason joins `index_note`. Without accio, or when accio fails, there is no
  registration and `index_note` says why.
  [@test] ../../crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`, `without_an_index_the_registrations_are_listed_and_their_classes_unknown`, `without_accio_a_plan_lists_no_registration_and_says_why`)
- The registrations must come by the module of the descriptor in the order of `modules`, then the descriptor in tree
  order, then the kind (service, extension point, listener, extension, registry key, action, content module), then
  the order of the head, the removed ones last in the order of the base.
  [@test] ../../crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)
- Since Your Review must compare the two ends of its scope, never what the reviewer already read; when its carry
  conflicts, its commits are read one at a time, which no two revisions hold, and it must list no registration.
  [@test] ../../crates/review-plan/src/registrations_test.rs (`since_the_review_the_registrations_are_those_of_the_commits_added_on_top`)
- The plan sent while the index is asked must have no registration; the plan that follows has them. A change of
  descriptors alone is a query to ask.
  [@test] ../../crates/review-plan/src/registrations_test.rs (`the_plan_sent_while_the_index_is_asked_has_no_registration`)

### Dependency Order

- `order` must list every path the reviewed commits change exactly once, at its last path: a renamed file at its new
  path, a deleted one at its path (the review still shows it), none that the commits add and delete again.
  [@test] ../../crates/review-plan/src/order_test.rs (`files_of_no_known_language_keep_the_tree_order`)
- A file that declares what another file of the order uses or imports must come before it, read from the files as the
  last reviewed commit that changes each leaves it, in Kotlin, Java, C#, Rust and TypeScript.
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
- `brd review-plan` must print `order`, in `brd schema`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_files_in_dependency_order`)
- The order must run no git process: it reads the summaries the API read.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### In the App

- The app must read the plan with `get_review_plan` and mark a branch with `mark_branch_reviewed`, through the same
  `review_plan_core` and `mark_reviewed_core` as `brd`, the branch named with the prefix the app resolves for the
  repository: after a mark and a commit on top, the plan since the review lists that commit as `added` with its scope,
  and the usages come from the index of the host (the fixture `.git/branch-deck-accio.json` in the e2e server).
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
- `get_review_plan` must send the plan on its channel: the plan without the usages of its API (`pending`, no file
  counting `breaks`) before the index is asked, when there is a query to ask, then the plan with them; the window
  replaces what it shows with each and words an API without usages "Finding the uses…" and the summary "Finding
  possible breaks" while `pending`. A repository whose `branchdeck.reviewExperimental` is off must get `disabled` and
  no plan.
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
  [@test] ../../src/presenters/review-plan.test.ts (`says the uses are being found while Rust asks the index`)
- A plan whose every query answered (`index_note` null) must be kept in the memory of the app by what it is of (the
  repository, the prefix, the branch, `base`, `head`, the reviewed pair and `sinceReviewed`), at most 64 MiB of its
  JSON and 10 minutes unread: asked again, it comes whole in one event, with no git past the resolve of its subject
  and no query of the index.
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
- An unknown branch must answer `unknown_branch`; a base that names no commit, a branch that shares no history with
  its base, an unknown revision of a range and `sinceReviewed` of a range must answer `invalid_arguments`.
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)
- A window must read one plan at a time per slot, the plan of a scope (a branch, a range) and the plan of a commit
  (`ReviewSubject::Commit`): a newer plan of the slot and the close of the window must supersede the plan in flight,
  kill its queries of accio and answer `canceled`; the two slots, and a symbol search of the window, must never
  supersede each other.
  [@test] ../../crates/app-api/src/session/windows_test.rs (`a_review_plan_and_a_symbol_search_of_a_window_never_supersede_each_other`, `a_closed_window_supersedes_its_request`, `the_plan_of_a_commit_and_the_plan_of_its_scope_never_supersede_each_other`)
- A plan must be read once, whoever asks it: a window that asks the plan another window or the read ahead reads now
  must wait for that reading and get its plan whole, in one event, without the plan of its pending usages; when that
  reading ends without a plan (its window asked another, a newer commit superseded it), the window must read the plan
  itself and still get it.
  [@test] ../../crates/app-api/src/reads/review_prefetch_test.rs (`a_window_that_asks_the_plan_being_read_ahead_joins_its_reading`, `a_window_that_waited_for_a_superseded_read_ahead_still_gets_its_plan`)
- Once a sync commits the table of a repository (also one that moved nothing), the app must read ahead, in the
  background and one at a time, the most recently asked first, the plan of each branch it watches (a review window
  asked it since the app started, among the last 4, below) whose head the table moved and that no open window shows
  (that window reads its own plan once it sees the new head): the branch by the name the window asked it, on the stack parent whose head the window named as its
  base, else on the baseline of the table, "Since Your Review" as asked. The window that opens the review again then
  finds the plan whole in the memory of the plans and asks the index nothing, and a commit that leaves the head there
  reads nothing again. A branch the table no longer has, or that no longer resolves, is forgotten; a range, a commit
  and a kept branch are never read ahead; nothing is without `branchdeck.reviewExperimental` or an index on the
  machine, since a plan without its usages is not kept. No memory of its own beside the memory of the plans: the speed
  comes from accio ([Change Review](../../docs/design/change-review.md#prefetch-after-a-rewrite)).
  [@test] ../../crates/app-api/src/reads/review_prefetch_test.rs (`a_commit_that_moved_a_branch_a_window_asked_reads_its_plan_ahead_once_below_the_app`, `only_a_branch_a_window_asked_and_no_window_shows_is_read_ahead`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_host_hears_each_table_a_sync_commits`)
  [@test] ../../crates/review-plan/src/mark_reviewed_test.rs (`the_base_of_a_stacked_branch_resolves_to_its_parent_by_name`)
- The read ahead must watch at most the 4 branches of a repository a window asked the plan of last: a branch asked
  again becomes the newest, and asking a fifth one forgets the one asked longest ago, which no commit reads ahead any
  more. `brd update` moves every branch at once, and the reviews opened again are mostly the recent ones.
  [@test] ../../crates/app-api/src/reads/review_prefetch_test.rs (`a_fifth_asked_branch_forgets_the_one_asked_longest_ago`)
- The queries of accio of a plan read ahead must run below the app, at nice 19 on Unix and the below-normal priority
  class on Windows, which the processes accio starts inherit; those of a plan a window asks run at the priority of the
  app.
  [@test] ../../crates/sync-utils/src/child_process_test.rs (`a_background_child_runs_below_the_app`)
  [@test] ../../crates/file-diffs/src/accio_test.rs (`a_search_in_the_background_runs_its_queries_at_the_lowest_priority`)
  [@test] ../../crates/app-api/src/reads/review_prefetch_test.rs (`a_commit_that_moved_a_branch_a_window_asked_reads_its_plan_ahead_once_below_the_app`)
- A newer commit of the table of the same repository must supersede the read ahead in flight: its queries of accio are
  killed, and the newer one reads what is left to read.
  [@test] ../../crates/app-api/src/reads/review_prefetch_test.rs (`a_newer_commit_supersedes_the_read_ahead_in_flight`)
- `branchdeck.reviewExperimental` must be off unless git config turns it on (Settings → Branches → Review), read
  and written like the other keys of Settings; a value git refuses must be off with its reason and fail no sync: an
  experiment never turns on from a broken value.
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_review_experiments_are_off_until_the_config_turns_them_on`, `nothing_set_is_the_defaults`)
  [@test] ../../crates/sync-core/src/config_snapshot_test.rs (`the_review_experiments_are_off_unless_the_config_turns_them_on`)
- The switch "Review plan (experimental)" of Settings → Branches → Review must turn it on and off for every
  repository in the global git config on "All repositories", and the repository's own `.git/config` value, written on
  its tab, must win over it, as `branchdeck.resolveByWords` does (spec/repository/settings.spec.md).
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should turn the review plan on and off for every repository from Branches`)
- The flag must reach the view through each sync (`TableHeader.reviewExperimental`), and a change of it from Settings
  must sync; with the flag off the review windows must look as they do without the review plan. `brd review-plan` and
  `brd mark-reviewed` must work without it, and so must the two commands of the app.
  [@test] ../../crates/repository-model/src/table_test.rs (`the_header_takes_the_review_experiments_of_each_grouping`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should look as it does without the review plan while the flag is off`)

### In the Review Window

- With the flag off, the branch review window must offer none of what follows and read no plan.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should look as it does without the review plan while the flag is off`)
- With it on, the window must read the plan of the shown part (`get_review_plan`): the reviewed branch on the base of
  the part, the whole stack as its range; again when the head of the part moves, when "Since Your Review" is chosen
  or left, and after Mark Reviewed; an answer `canceled` (a newer plan of the window) is dropped, and the plan of the
  same branch stays while the next one is read. The plan of the same changes, read again after Mark Reviewed, must
  stay whole until the next one comes with its usages: the steps of the tree, its commit rows and the story of the
  diffs stay as they were; a plan of another head or scope gives the tree and the diffs none of its facts. Its
  interdiff counts only while the part is one branch (the reviewed pair is on the base of that branch alone) and the
  plan is of the head on screen: a part of several branches, or the plan of the head before a sync, shows no "since"
  and offers no "Since Your Review".
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)
  [@test] ../../src/lib/range-review.test.ts (`takes the interdiff only of the part's own branch, and only of a plan of the head on screen`)
  [@test] ../../src/lib/review-plan-answer.test.ts (`keeps the whole plan of the same changes while Mark Reviewed reads it again, until the next one has its usages`, `keeps no facts of a plan of another head or scope while the next one is read, but the plan of the branch for its card`, `replaces a plan without its usages by the next read's, and starts a read of another subject without a plan`)
- Mark Reviewed of the title bar must mark the reviewed branch of the part on the base of its own branch, as
  `brd mark-reviewed <branch> --base <parent>` does, say so, and read the plan again: nothing is new after it. The
  whole stack has no Mark Reviewed.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)
- "Since Your Review" must follow "All Changes" in the commit column only while the part is one branch, marked, with
  a commit added, rewritten or reworded since (a message changed alone too); each such commit says "New",
  "Rewritten" or "Reworded", on its row in the steps of the tree
  ([Review Tour](review-tour.spec.md#in-the-review-window)) and in the column of a commit shown alone. Chosen, the
  tree lists only the added and the rewritten commits, adjacent or not, and the diffs show the scope: the commits
  added on top, or the diff from the carried commit, a rewrite against its reviewed version and new commits apart together; without a
  scope (the carry conflicted) the newest of them. "All Changes", another part or a mark leaves it.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] ../../src/lib/range-review.test.ts (`offers no scope for a branch never marked, nor when nothing was added or rewritten`, `marks the added and the rewritten commits, and lists only them, adjacent or not, under the scope`, `keeps the plain range of the commits added on top, and the column of one new commit`, `offers Since Your Review for a commit whose message alone changed, Reworded, and reads its plan with its reviewed commit`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit whose message alone changed as Reworded, what its body added and removed in its opening, the overview and its heading alone`)
- The overview must head the document of a scope, and of a commit shown alone below its heading (a rewritten commit
  shows its whole diff, the overview what it changes against its reviewed version); a commit shown alone is told by
  its own plan, its steps, its story and its facts, under "Since Your Review" with what changed since of the part. It
  must keep a block above the headers
  and the messages while the diffs are read. While the plan is read it says "Reading the review plan…", while Rust asks
  the index "Finding the uses left outside…", and "Cannot read the review plan: …" when the read failed, in the review
  of a pull request too: no overview that looks whole and then moves as the plan comes. After the
  steps, "Possible Breakage" in the warning tone, then "Since your review" as before (a rewritten commit with the lines per path it changes against its reviewed version).
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`)
- "Since your review" must list each rewritten and each reworded commit with what changed: "Rewritten scale the
  helper: src/Store.kt +1 −1", then " · message changed" when Rust says its message changed (`message_change`),
  "Reworded scale the helper: message changed", a rewritten commit whose lines per path are the same "the same lines
  in another order"; a commit whose message did not change says nothing of it. A line with a change of the message
  must be a disclosure that opens to it, as Rust compared it: the subject it had ("Was …", struck through), then each
  hunk of its body in its order, its removed blocks muted and struck through with a bar in the tone of a removal, then
  its added blocks with a bar in the tone of an addition. This is where Since Your Review as one diff, which tells no
  opening, shows the message.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit whose message alone changed as Reworded, what its body added and removed in its opening, the overview and its heading alone`, `should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] ../../src/presenters/review-plan.test.ts (`says per path what a rewritten commit changes that its reviewed version did not, and the reverse, and that its message changed only when Rust says so`)
  [@test] ../../src/presenters/review-story.test.ts (`says in the overview how the message of a rewritten or a reworded commit changed, the blocks Rust compared under its line`)
- Where the document tells the story of a commit whose message changed since the review (the openings of All
  Changes of a marked branch, the heading of a commit shown alone with its reviewed commit, Since Your Review whose
  carry conflicted), the story must mark it as Rust placed it ([Review Tour](review-tour.spec.md#the-story-of-a-commit)):
  an added block with the bar of an addition and "Added since your review" for a screen reader, a section whose label
  is new marked once whole; the removed blocks muted, struck through, with the bar of a removal and "Removed since your
  review", where they stood; the subject it had below its title ("Was …"). The bars stand in the gutter, so the text
  of a marked block stays at the edge of the others. A story whose message did not change marks nothing.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit whose message alone changed as Reworded, what its body added and removed in its opening, the overview and its heading alone`)
  [@test] ../../src/presenters/review-story.test.ts (`marks what changed in a body since the review where Rust placed it: the added blocks, the removed ones before the block they stood before or at the end of their part, the subject it had`)
  [@test] ../../tests/e2e/branch-review/review-tour.spec.ts (`should tell a commit shown alone under Since Your Review by its own plan: its story, its steps and the marks back to it`)
  [@test] ../../src/presenters/review-plan.test.ts (`counts what changed since the review, and nothing when only the baseline moved`, `says per path what a rewritten commit changes that its reviewed version did not, and the reverse, and that its message changed only when Rust says so`)
- "Possible Breakage" must list each removed or changed API, in the order Rust ranked it, unless no use left outside
  breaks: its kind and its whole name, wrapping at its dots; under them `path:line` (a long path losing its start),
  the parts that changed ("parameters and visibility changed") and the uses left outside that can break ("3 uses left
  outside"); the form before and the form after on lines of their own, wrapping, never cut, as code reads (a
  constructor as a call of its type, a line each: `Event(text: String, styles, start: Long = …)`, one not public
  after its visibility; a parameter whose type has several names without its type, which Rust keeps only as those
  names; never the name of a `Reach` nor `List/Window`); then the first sites a line each, its file name and line
  with its directory muted after it and its `path:line` in its hover, and in one line the rest: the uses updated in
  this change, the calls left outside that still fit the new form ("4 calls left outside still fit the new form") and
  the hits only the name ties ("2 matches by name only, not checked"). An entry with `usages` null says "The uses are
  unknown". An entry whose count is not `complete` says "At least N uses left outside", or "No use left outside found,
  the search stopped early" at 0. Only the entries Rust says are `unbroken` and the widenings (`widened`, the badge
  "Opens wider") fold into one collapsed line, muted, never in the warning tone: "No use left outside breaks · 12 API
  changes" where the `verdict` of the plan is `noUseBreaks`, the API line of the map then
  ([The Map](review-tour.spec.md#the-map)), else "12 API changes, no use left outside breaks" under the entries
  listed; an unknown, incomplete or by-name count never reads as safe. Another addition is not listed there.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)
  [@test] ../../src/presenters/review-plan.test.ts (`folds a widening with what breaks nothing, and leads the line with the verdict when no use left outside breaks`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should keep an API whose uses are unknown under Possible breakage, and say once above the map why accio did not answer`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/presenters/review-plan.test.ts (`words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown`, `lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites`, `folds only the API none of whose uses left outside breaks, and keeps an unknown, incomplete or by-name count listed, also at 0`)
- A click on a site of the uses left outside must open its file at the head of the review over the diffs, as Go to
  Declaration opens a declaration outside the review, at the line of the use with the name of the API selected
  (`get_outside_place`: one `cat-file --batch-check` and one `cat-file --batch`, no index); a file or a line the head
  lacks says "Cannot open src/main/App.kt:9" with "The head of the review has no such line". The file covers the
  document, the overview too, until Back.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../crates/file-diffs/src/outside_place_test.rs (`a_use_outside_the_review_opens_at_its_line_with_its_name_selected`)
- The overview lists no dependency change and no repeated edit: they are on the rows of the tree, the mark of a row
  and the divider "Same edit in 3 files" of the story. The new API (each added declaration), the declarations opened
  wider ("Opens wider: format", no new API) and the links the commits added between the files of two modules
  (`new_between_modules`, `Reader.kt → Cache.kt` at its first use) are facts of the step that holds them, each name at
  its place, of the range when the overview lists no step.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)
  [@test] ../../src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)
- A click on an entry of the overview must select its file in the review and show its place in the diffs, its name
  selected, as a link of the card of a row's mark does: the declaration of an API (a removal on the side before the
  commits, by its old path too), the first use of a link between modules in the file that uses, a declaration of the
  new API; a file of What Changes at its header. A search that
  hides the file is cleared as for a link of a mark, also when it hides every file, and the place is shown once the
  file is; an owner chip filters the files, and a link to a file it hides clears the chip before selecting that file
  ([Branch Review](branch-review.spec.md#code-owners)). A chip that already keeps the linked file stays selected.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should reveal an overview file under a text search while retaining its owner chip`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
- The tree must list the concerns of the plan, in All Changes and in "Since Your Review" as one range, and the story
  of the diffs follow them ([Review Concerns](review-concerns.spec.md#in-the-review-window)); with no concern but the
  rest, the files stay in tree order, the plan adds the marks of the rows and the Mechanical group. There is no switch of the
  order: one tree tells the finished review. The tree of a scope lists the commits in its steps, the column above it
  only the scopes; a commit shown alone shows the tree of its own plan, the column every commit. The facts apply once
  the plan of the shown scope is read, not while the next one is read: until then the column lists every commit.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
- A row of the tree must show at most one small mark, where its facts are: those of its file and the dependency
  changes the row carries. The mark must be in the warning tone when the file breaks uses left outside the change
  (`breaks N` or `breaks N+`), and show always; else muted, shown only on the row under the pointer, the selected and
  the focused row and while its card is open, its place kept so no name moves: a mark on most rows of a change would
  tell nothing, and the warning is the one signal of the tree. Details are on demand: the hover of the mark (300 ms, only after a move of
  the pointer) must open its card, with the sections that apply in this order: Breaks, Uses, Used by, Same edit,
  Dependencies. The mark is no tab stop: the description of the row must say every fact in words (`uses 1, +1 new,
  breaks 3, new module`).
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)
  [@test] ../../src/presenters/review-plan.test.ts (`merges the facts of a row into one mark, in the warning tone only when it breaks uses outside, and its card sections what can break first`, `puts the dependency changes a row carries into the same mark, after the facts of its file`)
- The keyboard must reach the card: `I` on a focused row with a mark must open its card with the focus on its first
  file, on the card when it lists none; Enter on a file selects it as a click does. Escape must close the card alone,
  not the window or the review, and give the focus back to the row, as a file chosen from a card opened by `I` does.
  A card opened by `I` that goes otherwise, the hover of another mark replacing it or the pointer leaving its mark,
  must give the focus back to the row too, unless the focus went elsewhere (a click, Tab out); a card goes with its
  row. The key hints must list `I` while a row of the tree has the focus.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should open the card of a row's mark by I on the focused row, the focus on its first file, Enter selecting it and Escape giving the focus back to the row`, `should give the focus back to the row when a card opened by I goes by the hover of another mark or the pointer leaving, and close only the card on Escape`)
- The card of a file must say `Uses N · +K new` and `Used by M`, the links among the changed files after the commits,
  `+K new` for the links it uses that the commits added. It must list each link it uses, then each link that uses it,
  a removed one too: the other file, the names and the line of the first use (`Cache, get · Reader.kt:3`), and the
  state Added, Removed or Kept. A click on a link must select that file, close the card and show the file: a file
  that uses this one at its first use, the name selected, a file it uses at its header.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)
- A click on a file in a card must select it also when a filter hides it: a text search is cleared only when it hides the
  file, and an owner chip is cleared only when the file belongs to another owner. A filter that keeps the file stays.
  The click selects the file once: a later change of the owner chip or the search must not select it again.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should reveal a linked file from another owner by clearing its chip and clear a search that hides the target`, `should reveal an overview file under a text search while retaining its owner chip`)
  [@test] ../../src/presenters/review-plan.test.ts (`counts the links among the changed files, the new ones apart, and lists each with its names, its line, its state and its place`)
- The card of a file must say under Breaks `breaks N` in the warning tone and why, for the uses left outside the
  change of the API it removes or changes. When `breaks_complete` is false it must say `breaks N+` ("At least 3 uses
  left outside the change: the search stopped early"), or a quiet `breaks ?` at 0, whose mark is muted: a lower bound
  never reads as safe. A row has no `no test`: the tests that name the code of a change are those of its step
  ([Review Tour](review-tour.spec.md#tests-that-name-it)); `untested` stays in `brd review-plan`.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`, `should keep an API whose uses are unknown under Possible breakage, and say once above the map why accio did not answer`)
  [@test] ../../src/presenters/review-plan.test.ts (`says the uses an API leaves outside, as a lower bound where Rust says the count is, never a safe 0`, `lists no file without a badge`)
- The card of a file of a repeated edit must say `Same edit ×N`, name the edit (the first line removed and the first
  added, `a → b`) and list the other files of the group; a click selects one.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/presenters/review-plan.test.ts (`names the same edit of a group of files and the other files that get it`)
- The mechanical files must leave the tree for one group at its end, `Mechanical (N)` (a generated file that follows
  its source in the group of its concern, [Review Concerns](review-concerns.spec.md#in-the-review-window)), with a group per kind in the
  order Build files, Imports only, Whitespace only, Renamed, Generated (the order of the mechanical units,
  which the page takes from the plan), each file by its path. The group must start
  collapsed, also when the plan comes after the files, unless it holds the selected file. A file selected only as the
  first of the tree without the plan, which may be a generated one, must give way to the first file of the tree with
  the plan. The diffs, J and K must reach these files last, and a file selected in it opens it. Its menu and ⌘C copy
  its changed paths.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)
- The diffs must be the story of the units ([Review Units](review-units.spec.md#in-the-review-window)): a mechanical
  change starts collapsed, its header saying why ("Build file", "Imports only", "Whitespace only", "Renamed only"; a
  generated file what its name makes it, "Lock file", "Minified", "Source map", "Generated
  (.gitattributes)", or the line its tool wrote, "Generated by tools/render.mjs; edit src/guide.md", cut at 100
  characters), and a repeated edit shows once, the other files of it collapsed, "Same edit as Alpha.kt". The chevron
  of the header opens either.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)
  [@test] ../../src/presenters/review-units.test.ts (`fold each with why it is generated: its name, its attributes, the line its tool wrote`)
  [@test] ../../src/presenters/review-plan.test.ts (`says why a generated file is folded: its name, its attributes, the line its tool wrote, a long one cut`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`, `should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/lib/file-tree.test.ts (`folds the mechanical files into one group at the end, a group per kind, each file by its path; J and K reach them last`, `orders the kinds as the units of the plan, the order Rust gave the story`, `starts with the Mechanical group collapsed, and opens it to reveal a file in it`, `copies the paths of a group, which has no path of its own`)
  [@test] ../../src/presenters/review-plan.test.ts (`names the Mechanical group and its groups by kind`)
- A block whose every changed line of each side is an import or a package statement or blank
  (`TextDiff::import_blocks`, from the syntax of the texts: the import and package statements of Kotlin, Java, Rust and
  TypeScript, the `using` directives and the `namespace` line of C#, a Rust `pub use` and a TypeScript `export … from`
  none) must be pale, as a change of whitespace alone,
  and N and P must pass it; in the story it is in the unit of the imports, and the section of its file shows one row
  for it. A block with a line of code beside them is a change as any.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)
  [@test] ../../src/lib/diff/rows.test.ts (`plays down a block of the imports alone in a review: pale, and N and P pass it; elsewhere it is a change as any`)
  [@test] ../../crates/file-diffs/src/service_test.rs (`a_block_of_import_statements_alone_is_marked_by_the_syntax`)
  [@test] ../../crates/code-symbols/src/summary_test.rs (`the_import_lines_are_the_import_and_package_statements`)
- The dependency changes of a build file must show on the row of the concern that holds the build file (it follows the
  code that needs the dependency, [Review Concerns](review-concerns.spec.md#the-rest)), else on the directory row whose
  chain holds the directory of the build file, else on the row of the build file (one at the root), which the
  Mechanical group stands for while it folds the build file, so that a collapsed group hides no dependency change:
  the mark of that row, whose card says `Dependencies · +N deps −M ~K` (`new module`, `deleted module`) and lists
  each change, a changed version as `1.2 → 1.3` and a changed scope as `test → main`.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] ../../src/lib/file-tree.test.ts (`shows the dependency changes of a build file on the row of its concern, else on the directory row of its module, else on the Mechanical group that folds it`)
  [@test] ../../src/presenters/review-plan.test.ts (`counts the dependency changes of a module, a version change as one changed entry with both versions`, `words a version, a scope and a new module`)

## User Experience

- `brd mark-reviewed auth` → `{"branch": "jane/virtual/auth", "ref": "refs/heads/jane/virtual/auth", "base": …,
  "tip": …, "previous": null, "changed": true, "pruned": 0}`.
- `brd mark-reviewed auth --base nope` → `{"error": {"code": "invalid_arguments", "message": "No branch or commit \"nope\"
  to review the branch on"}}`, exit 2.
- `brd review-plan auth --since-reviewed` after a body written anew → `"state": "reworded", "message_change":
  {"subjectBefore": null, "hunks": [{"start": 0, "added": {"blocks": [{"type": "paragraph", "lines": ["The logout
  clears the session of the user."]}], "code": []}, "removed": {"blocks": [], "code": []}}]}`.
- `brd review-plan auth --since-reviewed` after one commit on top → `{"branch": "jane/virtual/auth", "base": …, "head":
  …, "interdiff": {"reviewed_base": …, "reviewed_tip": …, "commits": [{"id": …, "reviewed_id": null, "subject": "Add
  logout", "state": "added"}], "scope": {"base": <reviewed tip>, "head": <tip>, "carried": false}}}`; after an amend
  of a reviewed commit, `"scope": {"base": <carried>, "head": <tip>, "carried": true}`.
- `brd review-plan base..head` of a new parameter → `"api": [{"path": "src/a/Api.kt", "line": 3, "name": "open",
  "kind": "changed", "changes": ["parameters"], "before": "fun open(path:String) : Int", "after": "fun
  open(path:String, mode:Int) : Int", "usages": {"inside": 2, "outside": 12, "outside_sites": [{"path":
  "src/b/Client.kt", "line": 8}, …], "still_bound": 3, "by_name": 0, "complete": true, "unbroken": false}}]`, `"files": [{"path": "src/a/Api.kt",
  "old_path": null, "mechanical": null, "generated": null, "uses": 0, "used_by": 0, "new_links": 0, "breaks": 12,
  "breaks_complete": true, "untested": true, "repeated_edit": null, "repeated_edit_only": false}]`, `"index_note":
  null`; a rendered `AGENTS.md` → `"mechanical": "generated", "generated": {"kind": "header", "line": "Generated by
  tools/render.mjs; edit src/guide.md"}`, a lock file → `"generated": {"kind": "name", "name": "lockFile"}`.
- `brd review-plan base..head` of a new `Store` that `User` imports, with its test → `"order": ["src/b/Store.kt",
  "src/test/b/StoreTest.kt", "src/a/User.kt", "README.md"]`, `"links": [{"from": "src/a/User.kt", "to":
  "src/b/Store.kt", "symbols": [{"name": "Store", "line": 3}], "state": "added", "cross_module": false,
  "new_between_modules": false}]`.
- The review window, flag on: **Mark Reviewed** in the title bar → toast "Marked plan reviewed" ("plan was already
  marked reviewed at this commit" when nothing moved); the column: "All Changes 3 commits", "Since Your Review 1
  commit"; the tree ends with "Commits": "read the cache twice  New", "add the cache", "scale the helper".
- The **overview** at the top of the document: "Since your review: 1 new · 1 rewritten", under it "Rewritten scale the
  helper: src/Store.kt +1 −1"; after a body rewritten alone "Since your review: 1 reworded" and the disclosure
  "Reworded scale the helper: message changed", opened: "Why" and "the store kept one scale for all of them." struck
  through, "Look closely" and "a scale of zero empties the store." with the bar of an addition; under Possible Breakage `Changed helper`, under it
  `src/Store.kt:36 · parameters changed · 3 uses left outside`, `− fun helper(value: Int)`, `+ fun helper(value: Int,
  scale: Int)`, "Base.kt:1 src", "Uses.kt:4 src", "App.kt:9 src/main" and "1 match by name only, not checked"; without an index "The uses are
  unknown"; "1 API change, no use left outside breaks", "No use left outside breaks · 1 API change" and the badge "Opens
  wider"; "No usages: no index of this repository"; among the facts "New API: Reader, Cache", "Opens wider: format"
  and "New use between modules: (root) → src/b"; "Cannot read the review plan: …".
- The tree, flag on: `Reader.kt` with a muted mark, its card "Uses 1 · +1 new: Cache.kt Added, Cache, get ·
  Reader.kt:3" and "Same edit ×3"; `Store.kt` with a warning mark, its card "Breaks: breaks 2 · 2 uses left outside
  the change", or `breaks 2+` and a muted `breaks ?` for a lower bound; the directory `b` with a muted mark, its card
  "Dependencies · new module: New module b"; `I` on the focused row of `Reader.kt` opens the same card, the focus on
  `Cache.kt`, and the footer says `I Details` while a row has the focus; last, the collapsed `Mechanical (2)` with `Build files (1)`
  and `Whitespace only (1)`; a branch with rendered files ends with `Generated (5)`, the header of a rendering "Generated
  by tools/render.mjs; edit src/guide.md", of a lock file "Lock file".

## Measurements

<!-- The corpus runs of `brd review-plan` and what they found go here. -->

## Open Questions / Risks

- A branch deleted and created again under the same name keeps the pair of the old one, unless a mark pruned it while
  the branch was gone; the interdiff then compares it with the old branch.
- An unapplied (`{prefix}/unapplied/<name>`) or archived virtual branch has no `{prefix}/virtual/<name>`, so a mark of
  another branch meanwhile prunes its pair as gone; after a reapply it has no interdiff until it is marked again.
- Without `--base`, `brd` takes the baseline of the checked out branch for the merge base, so a stacked branch is
  marked with the commits of its parents; the app passes the base of the review's segment as `subject.base` and as
  the `base` of `mark_branch_reviewed`.
- A commit whose subject was reworded and whose lines were amended has neither its identity nor its changed lines: it
  shows as `added`, and its reviewed version as `dropped`, which errs toward showing too much. A body rewritten with
  an amend keeps the identity: `rewritten` with its `message_change`.
- A paragraph of a body edited is one hunk of the paragraph removed and the paragraph added: the change is by whole
  blocks, not by words.
- Since Your Review of a commit reworded alone is the overview with its message change: the scope is empty (no line
  changed), so no opening tells the commit there; All Changes and the commit shown alone tell it with its marks.
- Two commits of one author with the same subject in the same second share an identity; they pair in order.
- A carry that conflicts with the moved base (the baseline changed the lines the reviewed change changes) gives no
  scope: the commits are then read one at a time, a rewritten one against its own parent, so a declaration it adds
  shows as added even when its reviewed version added it too.
- The carried commit is on no ref: a `git gc` may prune it once it is old enough, and the next plan writes it again.
- A drop alone, no commit added or rewritten, offers no "Since Your Review", though the diff from the carried commit
  would show the removal.
- A removal's usages at the head are those left behind that accio ties to the declaration at its place before the
  commit; `outside_sites` lets the reader judge.
- A `string` or `config` hit counts only when accio ties it by the qualified name of the declaration (a class in a
  plugin descriptor, a `Class.forName`); a simple name in a string is `by_name`.
- A Rust or TypeScript function holds its parameters in its header (`FormChange::Header`, which also holds the
  return type): its calls left outside are never `still_bound`.
- The `path:line` of a change is the line of its name in the last commit that touched it, where the plan asks accio;
  a later commit that moves the lines of the file without touching the declaration leaves the card's line behind.
- With `--since-reviewed`, a use that an earlier reviewed commit of the branch updated counts as outside.
- An answer cut at 100 hits splits only those 100: `outside` is then a lower bound, and `complete` is false.
- The module roots of the summaries are read at the checked out HEAD, not at the head of the range.
- A key gets one entry: when an overload changed and another was added in the same commits, only the change is
  listed.
- `untested` sees a test inside a file only by its changed lines: a commit that changes the code of a Rust file and
  only a fixture of its `mod tests` that no test runs directly still counts as tested.
- An abstract test base class whose `@Test` methods its subclasses run is test code, and so are its helpers: its
  contract is for the test subclasses. A class named `…Test` without a test function stays API, as a public helper
  of a test source set (`testFramework`, `testSrc`) does: only a test function marks a class.
- A JVM class that holds a nested test class is test code with all its members, a production helper among them.
  A Rust module that holds a `#[test]` directly is test code with all its items, `pub` or not and without
  `#[cfg(test)]` too; the module around it and its siblings stay API.
- `@ApiStatus.Internal` on a package (`package-info.java`) and an annotation meta-annotated as such are not seen:
  their declarations stay API.
- A promotion is seen only on the lines the commits change: a type whose `@ApiStatus.Internal` is removed is `added`,
  its members are the type's entry; the top-level declarations of a Kotlin file whose `@file:ApiStatus.Internal` is
  removed are not listed, nor a declaration whose removed annotation line git's diff pairs with a like line added
  elsewhere in the file. A declaration that becomes unstable or test code in the same form is no entry.
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
  "Since Your Review" and its overview no "since": the reviewed pair is on the base of the branch alone.
- The overview of the plan heads the document of a scope, and of a commit only in "Since Your Review": a commit chosen
  under "All Changes" shows its own heading alone.
