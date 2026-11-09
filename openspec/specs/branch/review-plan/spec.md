---
name: Review Plan
description: Requirements for the plan of a review (experimental), what a reviewer of a branch needs first; its start is Mark Reviewed, the base and the tip of a local branch kept in `refs/branch-deck/reviewed/`, the interdiff of its next review (`brd review-plan`), the commits since then, and the API surface the reviewed commits change with its usages from accio, updated in the change or left outside, the facts of each file (its links to the other changed files, a mechanical change, a generated file, a repeated edit, a missing test), the dependency changes of the build files and the files in dependency order.
style: plain-1
targets:
  - crates/review-plan/src/lib.rs
  - crates/review-plan/src/mark_reviewed.rs
  - crates/review-plan/src/interdiff.rs
  - crates/review-plan/src/api.rs
  - crates/review-plan/src/fan_in.rs
  - crates/review-plan/src/files.rs
  - crates/generated-files/src/header.rs
  - crates/generated-files/src/attributes.rs
  - crates/generated-files/src/generated_by.rs
  - crates/review-plan/src/links.rs
  - crates/review-plan/src/dependencies.rs
  - crates/review-plan/src/registrations.rs
  - crates/review-plan/src/order.rs
  - crates/review-plan/src/texts.rs
  - crates/trial-check/src/declaration_checks.rs
  - crates/code-symbols/src/kotlin/summary.rs
  - crates/code-symbols/src/java/summary.rs
  - crates/code-symbols/src/rust/summary.rs
  - crates/trial-check/src/symbol_dependencies.rs
  - crates/trial-check/src/symbol_scan.rs
  - crates/file-diffs/src/accio.rs
  - crates/file-diffs/src/outside_place.rs
  - crates/file-diffs/src/service.rs
  - src/lib/diff/rows.ts
  - crates/app-api/src/reads/symbols.rs
  - crates/git-ops/src/reviewed_refs.rs
  - crates/git-ops/src/carried_review.rs
  - crates/trial-check/src/diff_scan.rs
  - crates/bd-rewrite/src/mark_reviewed_command.rs
  - crates/bd-rewrite/src/review_plan_command.rs
  - crates/bd-cli/src/lib.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/app-api/src/reads/review_plan.rs
  - crates/app-api/src/reads/review_prefetch.rs
  - crates/app-api/src/app/mark_reviewed.rs
  - crates/app-api/src/session/windows.rs
  - crates/sync-core/src/repository_config.rs
  - src/components/settings/BranchesPage.tsx
  - src/lib/review-plan.ts
  - src/lib/range-review.ts
  - src/lib/file-tree.ts
  - src/lib/file-actions.ts
  - src/hooks/useReviewPlan.ts
  - src/lib/review-plan-answer.ts
  - src/hooks/useBranchReview.ts
  - src/hooks/useSelectReviewFile.ts
  - src/lib/review-place.ts
  - src/presenters/review-plan.ts
  - src/components/review/ReviewPrologue.tsx
  - src/components/review/MessageChangeView.tsx
  - src/presenters/review-story.ts
  - src/components/review/BranchReview.tsx
  - src/components/commit/CommitReview.tsx
  - src/components/files/FileTree.tsx
  - src/components/files/FileRowMark.tsx
  - src/components/diff/CommitReviewCode.tsx
---

# Review Plan

Status: Experimental
Date: 2026-10-29

## Purpose

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
([Review Tour](../review-tour/spec.md#in-the-review-window)), says what can break and what changed since the review. The vision of the review, its progress and the rounds of polish:
[Change Review](../../../../docs/design/change-review.md).

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

## Concepts

### Modules

The shape of a change before any of its words: which modules it adds, changes or deletes, how large each part is,
and what each newly uses. The review opens on it ([Review Tour](../review-tour/spec.md)).

### Registrations

How a change plugs into the platform: what its plugin descriptors register, as the platform reads them. Every fact
comes from accio's port of the platform's reader; the plan parses no descriptor of its own for it.

### UI Texts

What the user of the change reads: the texts of the UI it adds, removes or changes, quoted, where the review showed
them only as cards of a diff.

## Requirements

### Requirement: Mark Reviewed: A mark keeps the tip and the base of the branch

A mark MUST keep the branch's tip and its base, the merge base of the tip and the baseline, as
`refs/branch-deck/reviewed/<sha256 of the branch's full name>/{base,tip}`, so that the pairs of a branch `a` and a
branch `a/b` never clash.

#### Scenario: The first mark of a branch
- **WHEN** a branch is marked reviewed
- **THEN** its tip and the merge base of the tip and the baseline are kept as `refs/branch-deck/reviewed/<sha256 of the branch's full name>/{base,tip}`
- **AND** the pairs of a branch `a` and a branch `a/b` never clash
  [@test] crates/git-ops/src/reviewed_refs_test.rs (`a_mark_writes_the_pair_of_the_branch_and_a_second_mark_replaces_it`)
  [@test] crates/review-plan/src/mark_reviewed_test.rs (`marks_the_virtual_branch_of_a_name_at_its_merge_base_with_the_baseline`)

### Requirement: Mark Reviewed: A new mark replaces the pair with compare-and-swap

A mark of a branch that has a pair MUST replace both refs with compare-and-swap on the values it read, and name the
pair it replaced; the same base and tip again MUST move no ref.

#### Scenario: A second mark replaces the pair
- **GIVEN** a branch that has a pair
- **WHEN** it is marked again
- **THEN** both refs are replaced with compare-and-swap on the values the mark read
- **AND** the mark names the pair it replaced
  [@test] crates/git-ops/src/reviewed_refs_test.rs (`a_mark_writes_the_pair_of_the_branch_and_a_second_mark_replaces_it`)

#### Scenario: The same base and tip again
- **WHEN** a mark has the same base and tip as the pair
- **THEN** it moves no ref
  [@test] crates/git-ops/src/reviewed_refs_test.rs (`the_same_mark_again_moves_no_ref`)

### Requirement: Mark Reviewed: A stacked branch is marked on the head of its parent

A branch stacked on another MUST be markable on the base its review shows, the head of its parent (`brd mark-reviewed
<branch> --base <parent>`, the parent named as a branch is, or any commit), so that its reviewed commits are its
own; the base is then the merge base of the tip and that commit, and no baseline is read.

#### Scenario: A mark on the base its caller names
- **GIVEN** a branch stacked on another
- **WHEN** `brd mark-reviewed <branch> --base <parent>` runs, the parent named as a branch is, or any commit
- **THEN** the base is the merge base of the tip and that commit, so that its reviewed commits are its own
- **AND** no baseline is read
  [@test] crates/review-plan/src/mark_reviewed_test.rs (`a_branch_stacked_on_another_is_marked_on_the_base_its_caller_names`)
  [@test] crates/bd-rewrite/src/mark_reviewed_test.rs (`a_stacked_branch_is_marked_on_its_parent`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_mark_reviewed_on_a_base`)

### Requirement: Mark Reviewed: A base that is no commit or shares no history fails before the write

A base that names no branch and no commit (one `rev-parse --verify`, never git's message), or that shares no history
with the tip, MUST fail before the write and leave every reviewed ref as it was; a base that starts with `-` MUST
never be read as an option of git.

#### Scenario: A base that is no commit or shares no history
- **WHEN** a base names no branch and no commit, or shares no history with the tip
- **THEN** the mark fails before the write and leaves every reviewed ref as it was: `invalid_arguments` (exit 2) for a `--base`, `no_baseline` for a baseline with no history in common
- **AND** a base that starts with `-` is never read as an option of git
  [@test] crates/review-plan/src/mark_reviewed_test.rs (`a_base_that_is_no_commit_or_shares_no_history_is_its_own_error_and_writes_nothing`)
  [@test] crates/bd-rewrite/src/mark_reviewed_test.rs (`a_base_that_is_no_commit_or_shares_no_history_is_refused`)

### Requirement: Mark Reviewed: Every ref of the mark is one write with compare-and-swap

Every ref of the mark MUST be written in one `update-ref --stdin` with compare-and-swap on the listing it was computed
from, a ref the mark keeps included (`verify`): a mark or a prune of another process meanwhile MUST fail the whole
write and leave the other one's pair.

#### Scenario: A concurrent mark
- **WHEN** another process marks or prunes between the listing and the write
- **THEN** the compare-and-swap fails the whole write
- **AND** the other one's pair is left
  [@test] crates/git-ops/src/reviewed_refs_test.rs (`a_concurrent_mark_fails_its_compare_and_swap`)

### Requirement: Mark Reviewed: The write prunes the pairs of gone branches

The same write MUST delete the pairs of the branches that are gone, and keep the others.

#### Scenario: A mark after a branch was deleted
- **GIVEN** pairs of branches that are gone and of branches that exist
- **WHEN** a mark is written
- **THEN** the same write deletes the pairs of the gone branches and keeps the others
  [@test] crates/git-ops/src/reviewed_refs_test.rs (`a_mark_prunes_the_pairs_of_gone_branches`)

### Requirement: Mark Reviewed: The branch is named as brd names branches

The branch MUST be named as `brd` names branches: the virtual branch `{prefix}/virtual/<name>` first, else the local
branch `<name>`, or exactly `refs/heads/<name>`; a name of no branch MUST be `unknown_branch` and write nothing.

#### Scenario: A name of a virtual branch
- **WHEN** the name is of a virtual branch and of a local branch
- **THEN** the virtual branch `{prefix}/virtual/<name>` is marked first, else the local branch `<name>`, or exactly `refs/heads/<name>`
  [@test] crates/review-plan/src/mark_reviewed_test.rs (`marks_the_virtual_branch_of_a_name_at_its_merge_base_with_the_baseline`)

#### Scenario: A name of no branch
- **WHEN** the name is of no branch
- **THEN** the mark is `unknown_branch` and writes nothing
  [@test] crates/review-plan/src/mark_reviewed_test.rs (`an_unknown_branch_is_its_own_error`)
  [@test] crates/bd-rewrite/src/mark_reviewed_test.rs (`an_unknown_branch_is_refused`)

### Requirement: Mark Reviewed: brd mark-reviewed prints the mark it wrote

`brd mark-reviewed <branch>` MUST print the branch, its ref, the base, the tip, the pair it replaced (`previous`,
`null` the first time), `changed` and `pruned`, and its output MUST be in `brd schema`.

#### Scenario: A mark and a mark after a new commit
- **WHEN** `brd mark-reviewed <branch>` marks a virtual branch, and again after a new commit
- **THEN** it prints the branch, its ref, the base, the tip, `previous` (`null` the first time), `changed` and `pruned`
  [@test] crates/bd-rewrite/src/mark_reviewed_test.rs (`marks_a_virtual_branch_reviewed_and_again_after_a_new_commit`)

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists the output of `brd mark-reviewed`, and the output matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

### Requirement: Mark Reviewed: A mark runs a fixed number of git processes

A mark MUST run a fixed number of git processes: the baseline (only without a base), one `for-each-ref` of the
reviewed refs with the branches, one `rev-parse --verify` of a base that names no branch, one `merge-base` and one
`update-ref`; it MUST take no repository lock and write no branch, index or working tree.

#### Scenario: The processes of a mark
- **WHEN** a mark runs, with a base or without
- **THEN** it runs the baseline (only without a base), one `for-each-ref`, one `rev-parse --verify` of a base that names no branch, one `merge-base` and one `update-ref`
- **AND** it takes no repository lock and writes no branch, index or working tree
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_mark_reviewed`, `a_mark_reviewed_on_a_base`)

### Requirement: Mark Reviewed: An agent marks only when the user says they reviewed

An agent MUST run it only when the user says they reviewed the branch (`brd help worktrees`).

#### Scenario: The agent help says when to mark
- **WHEN** an agent reads `brd help worktrees`
- **THEN** it says to run `brd mark-reviewed` only when the user says they reviewed the branch
  [@test] crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Requirement: Since Your Review: A branch never marked and a range have no interdiff

A branch never marked MUST have no interdiff (`null`), and no scope; a range MUST have none either.

#### Scenario: A branch never marked
- **WHEN** the plan of a branch never marked is read
- **THEN** it has no interdiff (`null`) and no scope
  [@test] crates/review-plan/src/interdiff_test.rs (`a_branch_never_marked_has_no_interdiff`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`a_branch_never_marked_and_a_range_have_no_interdiff`)

#### Scenario: A range
- **WHEN** the plan of a range is read
- **THEN** it has no interdiff either
  [@test] crates/review-plan/src/interdiff_test.rs (`a_range_has_no_interdiff`)

### Requirement: Since Your Review: Each commit pairs one to one with a reviewed commit

Each commit of the branch (`<base>..<tip>`, the base as a mark finds it) MUST be paired one to one, oldest first, with a
commit of the reviewed pair: the same commit, else the same identity (author email, author time, subject without the
`(prefix)`), else the same changed lines (`patch-id --verbatim` of the diff without context).

#### Scenario: The commits of the branch pair with the reviewed ones
- **GIVEN** a marked branch whose commits since the mark were added, reworded or dropped
- **WHEN** the plan is read
- **THEN** each commit of `<base>..<tip>` pairs, oldest first, with the same commit, else the one of the same identity, else the one of the same changed lines (`patch-id --verbatim` of the diff without context)
  [@test] crates/review-plan/src/interdiff_test.rs (`commits_added_after_the_mark_are_the_range_on_top`, `a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed`)

### Requirement: Since Your Review: A pair is unchanged, reworded or rewritten, the rest added or dropped

A pair MUST be `unchanged` when both change the same lines and have the same message, `reworded` when they change the
same lines and the messages differ, else `rewritten`; a commit left MUST be `added`, a reviewed one left `dropped`,
listed after the commits of the branch.

#### Scenario: The states of the commits
- **WHEN** the commits of the branch are paired with the reviewed ones
- **THEN** a pair of the same lines and message is `unchanged`, of the same lines and another message `reworded`, else `rewritten`
- **AND** a commit left is `added`, a reviewed one left is `dropped`, listed after the commits of the branch
  [@test] crates/review-plan/src/interdiff_test.rs (`commits_added_after_the_mark_are_the_range_on_top`, `a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed`)

### Requirement: Since Your Review: A changed message has its message change by blocks

A `reworded` commit, and a `rewritten` one whose message changed too, MUST have its `message_change`: the subject of its
reviewed version without the `(prefix)` when that differs (`subjectBefore`, only of a reword by the lines), and the
`hunks` of its body by the blocks of the grammar of a body ([Commit Row → Message
body](../../commit/commit-row/spec.md)), an LCS of whole blocks.

#### Scenario: The body of a message changes by its blocks
- **WHEN** the body of a reworded commit changed
- **THEN** each hunk is the blocks `added` at `start` of the body now and the reviewed blocks `removed` right before it, with the code of their lines
- **AND** a subject that differs without the `(prefix)` is its `subjectBefore`
  [@test] crates/review-plan/src/interdiff_test.rs (`the_body_of_a_message_changes_by_its_blocks`)

#### Scenario: A commit amended with a new body
- **WHEN** a commit was amended with a new body and new lines
- **THEN** it is `rewritten` with its `message_change`: its added and removed blocks
  [@test] crates/review-plan/src/interdiff_test.rs (`a_commit_amended_with_a_new_body_is_rewritten_with_its_added_and_removed_blocks`)

### Requirement: Since Your Review: A trailer, a rewrap or a prefix changes no message

A trailer added, a paragraph wrapped anew and a `(prefix)` changed MUST say the same: no `message_change`, and a pair
`unchanged`. The messages MUST be those of the `rev-list` of each side: no process more.

#### Scenario: A new trailer or a rewrap
- **WHEN** the only change of a message is a trailer added, a paragraph wrapped anew or a `(prefix)` changed
- **THEN** the commit has no `message_change` and its pair is `unchanged`
- **AND** the messages come from the `rev-list` of each side, no process more
  [@test] crates/review-plan/src/interdiff_test.rs (`a_new_trailer_or_a_rewrap_changes_no_message`)

### Requirement: Since Your Review: A reworded commit is unchanged for the scope

A `reworded` commit changes the same lines the reviewer read: the scope MUST treat it as `unchanged` (a reword below the
commits added on top keeps their plain range), and the plan MUST read none of its lines.

#### Scenario: A body rewritten alone
- **WHEN** a commit's body was rewritten alone, below commits added on top
- **THEN** it is `reworded`, the plan reads none of its lines
- **AND** the scope is the plain range of the commits added on top
  [@test] crates/review-plan/src/interdiff_test.rs (`a_body_rewritten_alone_is_reworded_its_code_unread_and_the_scope_on_top`)

### Requirement: Since Your Review: An amended commit is rewritten with its lines

A commit amended since the review MUST be `rewritten` with the lines it changes that its reviewed version did not, and
the reverse, per path.

#### Scenario: A commit amended since the review
- **WHEN** a commit was amended since the review
- **THEN** it is `rewritten` with the lines it changes that its reviewed version did not, and the reverse, per path
  [@test] crates/review-plan/src/interdiff_test.rs (`an_amended_commit_is_rewritten_with_its_lines`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

### Requirement: Since Your Review: A move of the baseline alone changes no commit

A move of the baseline alone (a rebase, a sync onto a new baseline) MUST leave every commit `unchanged`.

#### Scenario: A rebase onto a new baseline
- **WHEN** the branch only moved onto a new baseline (a rebase, a sync)
- **THEN** every commit is `unchanged`
  [@test] crates/review-plan/src/interdiff_test.rs (`a_baseline_move_alone_leaves_every_commit_unchanged`)

### Requirement: Since Your Review: Commits added on top are the plain range

When commits were only added on top, the scope MUST be the plain range `<first added>^..<tip>`; `<tip>..<tip>` when
nothing changed.

#### Scenario: Commits added after the mark
- **WHEN** commits were only added on top of the reviewed ones
- **THEN** the scope is `<first added>^..<tip>`
  [@test] crates/review-plan/src/interdiff_test.rs (`commits_added_after_the_mark_are_the_range_on_top`)

#### Scenario: Nothing changed
- **WHEN** nothing changed but the baseline
- **THEN** the scope is `<tip>..<tip>`
  [@test] crates/review-plan/src/interdiff_test.rs (`a_baseline_move_alone_leaves_every_commit_unchanged`)

### Requirement: Since Your Review: After a rewrite the scope starts at the carried review

After a rewrite, a drop or a commit added below a reviewed one, the scope MUST be `<carried>..<tip>`, `carried`: the
carried commit has the base of the branch as its parent and the tree of `merge-tree --merge-base <reviewed base>
<base> <reviewed tip>`, on no ref, in the name and at the author time of the reviewed tip, so the same carry writes
the same commit; it is the reviewed tip itself when the base did not move.

#### Scenario: An amend, a drop or a rebase with an amend
- **WHEN** a reviewed commit was amended or dropped, or the branch was rebased with an amend
- **THEN** the scope is `<carried>..<tip>`, and its diff is what changed since the review: after a rebase with an amend, the amend alone
  [@test] crates/review-plan/src/interdiff_test.rs (`an_amended_commit_is_rewritten_with_its_lines`, `after_a_rebase_with_an_amend_the_scope_holds_the_amend_alone`, `a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed`)

#### Scenario: The base did not move
- **WHEN** the base of the branch did not move
- **THEN** the carried commit is the reviewed tip itself
  [@test] crates/git-ops/src/carried_review_test.rs (`the_reviewed_tip_is_its_own_carry_on_the_same_base`)

#### Scenario: The base moved
- **WHEN** the base of the branch moved
- **THEN** the reviewed change is merged onto the base as a commit of it, on no ref, in the name and at the author time of the reviewed tip
  [@test] crates/git-ops/src/carried_review_test.rs (`the_reviewed_change_is_merged_onto_a_moved_base_as_a_commit_of_it`)

### Requirement: Since Your Review: A carry that conflicts gives no scope

A merge of the carry that conflicts MUST give no scope.

#### Scenario: A reviewed change that conflicts with the base
- **WHEN** the `merge-tree` of the reviewed change onto the base conflicts
- **THEN** nothing is carried and the interdiff has no scope
  [@test] crates/git-ops/src/carried_review_test.rs (`a_reviewed_change_that_conflicts_with_the_base_carries_nothing`)

### Requirement: Since Your Review: --since-reviewed lists only the changed commits

`--since-reviewed` MUST list only the `added`, the `rewritten` and the `reworded` commits; of a range, or with `--base` of
a range, or a range rev that names no commit, it MUST be `invalid_arguments` (exit 2).

#### Scenario: The commits since the review
- **WHEN** `brd review-plan` runs with `--since-reviewed` on a marked branch
- **THEN** it lists only the `added`, the `rewritten` and the `reworded` commits
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

#### Scenario: Since the review of a range
- **WHEN** `--since-reviewed` is given of a range, with `--base` of a range, or with a range rev that names no commit
- **THEN** it is `invalid_arguments` (exit 2)
  [@test] crates/review-plan/src/interdiff_test.rs (`a_range_has_no_interdiff`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`a_branch_never_marked_and_a_range_have_no_interdiff`)

### Requirement: Since Your Review: brd review-plan prints the interdiff in schema 28

`brd review-plan` MUST print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`, `commits`
with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, `message_change`, and `scope` with `base`,
`head` and `carried`), with `"schema": 28`, and its output MUST be in `brd schema`; the other outputs of `brd` keep
`"schema": 2`.

#### Scenario: The plan since the review
- **WHEN** `brd review-plan` runs on a marked branch
- **THEN** it prints the branch, `base`, `head` and the `interdiff` with `"schema": 28`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists the output of `brd review-plan`, and the output matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

#### Scenario: The versions of the schema
- **WHEN** a reader of an older plan reads the versions
- **THEN** version 3 has `files` in place of `risk` and `usages` in place of `fan_in`, version 4 the `units` ([Review Units](../review-units/spec.md)), version 5 the `generated` files with why, the lock files among them, version 6 a deletion as real work (no `mechanical: "deleted"`; the units `deletedFile` and `deletedFiles`), version 7 the `concerns` ([Review Concerns](../review-concerns/spec.md)), version 8 their `elsewhere`, version 9 the name `file`, version 10 the `steps` and the `brief` ([Review Tour](../review-tour/spec.md)), version 11 the `api` and `test` of a unit ([Review Units](../review-units/spec.md)) in place of its `public`, version 12 the `still_bound` and `by_name` usages and `unbroken` in place of `updated`, version 13 the `reached` and `unreached` tests of a step ([Review Tour](../review-tour/spec.md)) in place of `named` and `unnamed`, version 14 the story of each commit once (`steps.commits`, [Review Tour](../review-tour/spec.md#the-story-of-a-commit)) in place of the `body` of a commit of a step, and the `newApi` and `moduleLinks` of a step, version 15 the `parts` of a story where they are `told` in place of its `toldIn`, the `joined` units and the `marks` of a stop, the `shownElsewhere` units of a step, the state `reworded`, the `message_change` of a commit, the `change` of a story and the `removals` of its parts, version 16 the `paragraphs` of an entry of the brief, whole and changed by word, with their `links` and `excerpt`, in place of its `added` and `removed`, and its `blocks` in place of its `block` ([Review Tour](../review-tour/spec.md#brief)); version 19 added repeated replacements to the brief, version 20 a widening as an `added` entry with `widened`, the reach in the forms and the `verdict`, version 21 the form `row` of a paragraph of the brief, the `excerpt` of every changed paragraph of several sentences and no paragraph of front matter or metadata ([Review Tour](../review-tour/spec.md#brief)), version 22 the `table` block of a body ([Commit Row](../../commit/commit-row/spec.md#message-body)) and its `summary`, version 23 the `modules` of the changed files ([Modules](#modules)), the `module` of a file and the `moduleEdges` of a step in place of its `moduleLinks`, version 24 the `groups` of a large step ([Review Tour](../review-tour/spec.md#groups-of-a-step)) in place of the `concern` of a stop, version 25 the `registrations` ([Registrations](#registrations)), version 26 the `texts` of the UI ([UI Texts](#ui-texts)), version 27 the `folder` of each file ([Modules](#modules)), version 28 the `tested` of each group of a large step ([Groups of a Step](../review-tour/spec.md#groups-of-a-step))

### Requirement: Since Your Review: The plan runs a fixed number of git processes

The plan MUST write no ref and run a fixed number of git processes, whatever the number of commits: the listing and the
base of a mark, one `rev-list` per side, one `diff-tree` with one `patch-id` for the commits that are not the same on
both sides, one `diff-tree` for the rewritten ones, one `merge-tree` and one `fast-import` for a carry onto a base that
moved; never `git range-diff`, never a process per commit.

#### Scenario: A plan since the review
- **WHEN** the plan since the review is read, on the same baseline or on a moved one
- **THEN** it writes no ref and runs the listing and the base of a mark, one `rev-list` per side, one `diff-tree` with one `patch-id`, one `diff-tree` for the rewritten ones
- **AND** one `merge-tree` and one `fast-import` for a carry onto a base that moved, never `git range-diff`, never a process per commit
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`, `a_review_plan_since_the_review_on_a_moved_baseline`)

### Requirement: Since Your Review: The plan of a commit with its reviewed one reads both messages

The plan of a commit shown alone with its reviewed commit (`ReviewSubject` `commit` with `reviewed`) MUST run one
`rev-list` of both messages more than without it, and a reviewed commit git no longer has MUST mark nothing.

#### Scenario: A commit with its reviewed one
- **WHEN** the plan of a `ReviewSubject` `commit` with `reviewed` is read
- **THEN** it runs one `rev-list` of both messages more than without it
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_commit_with_its_reviewed_one`)

#### Scenario: The changed part of a body is marked
- **WHEN** the body of the commit changed since its reviewed one
- **THEN** the changed part is marked, and a reviewed commit git no longer has marks nothing
  [@test] crates/review-plan/src/story_test.rs (`a_changed_part_of_a_body_is_marked_and_unfolded_a_verification_too`)

### Requirement: API Surface: The reviewed commits are those of the range or of the scope

The reviewed commits MUST be those of `base..head`; with `--since-reviewed` the diff of the scope: the commits added on
top, or the one diff from the carried commit to the tip, so a rewritten commit counts against its reviewed version (a
declaration it changed is changed, not added again) and an unchanged commit between two new ones adds nothing.

#### Scenario: Since the review, the diff from the reviewed change
- **GIVEN** a marked branch whose reviewed commits were rewritten
- **WHEN** the plan runs with `--since-reviewed`
- **THEN** the `api` is that of the one diff from the carried commit to the tip: a declaration a rewritten commit changed is changed, not added again
  [@test] crates/review-plan/src/api_test.rs (`since_the_review_the_api_is_that_of_the_diff_from_the_reviewed_change`)

#### Scenario: An unchanged commit between two new ones
- **WHEN** an unchanged commit lies between two new ones of one file or of one manifest
- **THEN** it adds nothing to the `api` and no entry to the `dependencies`
  [@test] crates/review-plan/src/api_test.rs (`since_the_review_an_unchanged_commit_between_two_of_one_file_adds_nothing`)
  [@test] crates/review-plan/src/dependencies_test.rs (`since_the_review_an_unchanged_commit_between_two_of_one_manifest_adds_no_entry`)

### Requirement: API Surface: Without a scope each changed commit is read against its parent

Only without a scope (the carry conflicted) the reviewed commits MUST be the `added` and the `rewritten` commits, each
read against its own parent. A branch never marked has no reviewed commit, so all of its commits MUST be read. The
`api`, the `files`, the `links`, the `dependencies`, the `repeated_edits` and the `order` MUST be computed from these
alone.

#### Scenario: The carry conflicted
- **WHEN** the plan runs with `--since-reviewed` and the carry conflicted
- **THEN** the `added` and the `rewritten` commits are read, each against its own parent

#### Scenario: A branch never marked
- **WHEN** the plan of a branch never marked is read
- **THEN** all of its commits are read
- **AND** the `api`, the `files`, the `links`, the `dependencies`, the `repeated_edits` and the `order` are computed from the reviewed commits alone

### Requirement: API Surface: The api lists the public declarations added, removed or changed in form

`api` MUST list the public declarations (it and every declaration around it public, Java's `protected` too, none around
it a function) that the commits add, remove or change in form, as the syntax summaries read them: the header, the
parameters of a JVM function, the reach, `open`, the constructors of a type, what Java sees; a change counts when the
declaration is API or unstable API before or after it (below), so a narrowed reach is listed.

#### Scenario: The public declarations of a range
- **WHEN** the commits add, remove or change in form public declarations
- **THEN** the `api` lists them, a narrowed reach too
  [@test] crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)

#### Scenario: A Rust function changes by its header
- **WHEN** the header of a public Rust function changes
- **THEN** it is listed as changed
  [@test] crates/review-plan/src/api_test.rs (`a_rust_function_changes_by_its_header`)

### Requirement: API Surface: A private change, a body alone and a member of a new type are not listed

A change of a private declaration or of a body alone MUST NOT be listed, nor a member of a type added or removed with
it. A name of one or two characters MUST be listed too (the checks of a sync search only longer ones).

#### Scenario: Private changes and bodies
- **WHEN** the commits change a private declaration, a body alone, or add or remove a type with its members
- **THEN** none of them is listed, only the type
  [@test] crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)

#### Scenario: A short name
- **WHEN** a public declaration of a name of one or two characters changes
- **THEN** it is listed
  [@test] crates/review-plan/src/api_test.rs (`a_public_declaration_of_a_short_name_is_listed`)

### Requirement: API Surface: Each declaration is API, unstable API or no API on each side

Each public declaration MUST be, on each side of the commits, API, unstable API or no API, decided once
(`declaration_checks::surface_of`) from the marks of the syntax summaries
([Symbol Dependencies](../../commit/symbol-dependencies/spec.md)); a declaration that is not public is no API. Test code,
an entry point and an override are no API whatever their other marks; else a declaration marked `unstable`, or inside a
type marked so, is unstable API; else it is API.

#### Scenario: The surface of a declaration
- **WHEN** the surface of a Kotlin, Java or Rust declaration, or of a Kotlin file marked internal, is decided
- **THEN** test code, an entry point and an override are no API, a declaration marked `unstable` or inside a type marked so is unstable API, else it is API
  [@test] crates/trial-check/src/declaration_checks_test.rs (`the_surface_of_a_kotlin_declaration`, `the_surface_of_a_kotlin_file_marked_internal`, `the_surface_of_a_java_declaration`, `the_surface_of_a_rust_declaration`)

### Requirement: API Surface: An override is not listed

A member that the summary marks as an override (Kotlin `override`, Java `@Override`) MUST NOT be listed: its supertype
declares the API. A change is of an override when the member overrides before and after it.

#### Scenario: An override is no entry
- **WHEN** the commits add or change a member marked `override` or `@Override`
- **THEN** it is no entry, and a member is asked by its place
  [@test] crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_place_and_an_override_is_no_entry`, `a_changed_override_in_kotlin_or_java_is_no_entry`)

### Requirement: API Surface: Test code is not listed

Test code MUST NOT be listed, added, removed or changed: a test function (`test`: `@Test` and the other annotations of a
test framework, a JUnit 3 `testX()`, a Rust `#[test]`), a declaration a test function is declared in (a test class, a
type around a nested test class, a Rust module that holds a test), and every declaration inside one (a `@BeforeEach` or
`setUp`, a fixture field, a nested class).

#### Scenario: A test function and its class
- **WHEN** the commits add, remove or change a test function, its class, or a declaration inside one
- **THEN** none of them is an entry
  [@test] crates/review-plan/src/api_test.rs (`a_test_function_and_its_class_are_no_entry`)
  [@test] crates/trial-check/src/declaration_checks_test.rs (`the_surface_of_a_kotlin_declaration`, `the_surface_of_a_rust_declaration`)

### Requirement: API Surface: The climb from a test stops at a Rust module

The climb from a test MUST stop at a Rust module, so the module around a `mod tests` and its other items stay API. A
public helper beside a test in the same file, outside the test class, MUST be listed.

#### Scenario: A Rust public inline module
- **WHEN** a public Rust module holds a `mod tests` and other items
- **THEN** it and its other items stay API
  [@test] crates/trial-check/src/declaration_checks_test.rs (`a_rust_public_inline_module_is_not_test_code`)

#### Scenario: A public helper beside a test
- **WHEN** a public helper is declared beside a test in the same file, outside the test class
- **THEN** it is listed

### Requirement: API Surface: An entry point is not listed

An entry point MUST NOT be listed, added or changed: a Kotlin top-level `fun main` and a Java `public static void
main(String[])` (`entry_point`).

#### Scenario: A main function
- **WHEN** the commits add or change a Kotlin top-level `fun main` or a Java `main`
- **THEN** it is no entry
  [@test] crates/review-plan/src/api_test.rs (`a_main_function_is_no_entry`)

### Requirement: API Surface: Unstable API is no new API but its removal and change are listed

Unstable API, marked `@ApiStatus.Internal`, `@ApiStatus.Experimental`, `@VisibleForTesting` or `@TestOnly` on the
declaration, on a type around it or, in Kotlin, on its file (`@file:ApiStatus.Internal`), MUST NOT be listed as added:
no other module is meant to start to depend on it. Its removal and its change MUST be listed, with their `usages`,
because callers in other modules of the repository can break.

#### Scenario: Unstable API added, removed and changed
- **WHEN** the commits add, remove and change declarations marked unstable
- **THEN** the added one is no entry, the removed and the changed ones are listed with their `usages`
  [@test] crates/review-plan/src/api_test.rs (`unstable_api_is_no_new_api_but_its_change_is_listed`)
  [@test] crates/trial-check/src/declaration_checks_test.rs (`the_surface_of_a_kotlin_declaration`, `the_surface_of_a_kotlin_file_marked_internal`, `the_surface_of_a_java_declaration`)

### Requirement: API Surface: The entry follows the surfaces before and after the commits

Per key, the entry MUST follow the surfaces before and after the commits: a new form that is API is `added`, one that is
unstable or no API is no entry; a form gone that was API or unstable API is `removed`, one that was no API is no entry;
a form that changed is `changed` when either side is API or unstable API, and no entry when both are no API.

#### Scenario: Forms added, removed and changed
- **WHEN** the commits add, remove and change forms of declarations of each surface
- **THEN** the entries are `added`, `removed` and `changed` by the surfaces of both sides
  [@test] crates/review-plan/src/api_test.rs (`unstable_api_is_no_new_api_but_its_change_is_listed`, `the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)

#### Scenario: A change of form to or from API
- **WHEN** a form changes and the declaration becomes API or stops being API with it
- **THEN** it is listed
  [@test] crates/review-plan/src/api_test.rs (`a_change_of_form_to_or_from_api_is_listed`)

### Requirement: API Surface: A promotion to API is an addition

A form both sides have that becomes API from unstable API or no API (its `@ApiStatus.Internal` removed) MUST be `added`,
a promotion; one that stops being API in the same form MUST be no entry.

#### Scenario: A promoted declaration
- **WHEN** a declaration's `@ApiStatus.Internal` is removed and its form stays
- **THEN** it is `added`, a promotion
  [@test] crates/review-plan/src/api_test.rs (`a_promoted_declaration_is_new_api`)

#### Scenario: A declaration that stops being API in the same form
- **WHEN** a declaration stops being API and its form stays
- **THEN** it is no entry

### Requirement: API Surface: A widening alone is an addition that breaks nothing

A change of the reach alone to a wider one (`Reach::wider_than`: a private member, private in its file, `pub(super)`, the
module, public), of a declaration no subtype may override, MUST be `added` with `widened`, its form `before` kept and no
`usages` asked: it breaks no use (`internal fun mediaDuration` → `fun mediaDuration`). Opened to unstable API it MUST be
no entry, as an addition of it is none.

#### Scenario: A widening alone
- **WHEN** the reach of a declaration no subtype may override widens alone (`internal fun mediaDuration` → `fun mediaDuration`)
- **THEN** it is `added` with `widened`, its form `before` kept, and no `usages` are asked
  [@test] crates/review-plan/src/api_test.rs (`a_widening_alone_is_an_addition_that_breaks_nothing`)

#### Scenario: The order of the reaches
- **WHEN** two reaches are compared by `Reach::wider_than`
- **THEN** each is wider than every one before it: a private member, private in its file, `pub(super)`, the module, public
  [@test] crates/code-symbols/src/summary_test.rs (`a_reach_is_wider_than_every_one_before_it`)

#### Scenario: Opened to unstable API
- **WHEN** the reach widens and the declaration is unstable API
- **THEN** it is no entry

### Requirement: API Surface: The reach of an overridable member stays a change

The reach of an overridable member (an `open` Kotlin member, a method of a Java class that is not final) MUST stay
`changed`: an override MUST NOT weaken the access of what it overrides; so MUST an `open` member of a final Kotlin class,
whose modality the summary does not read, and a widening with another change of the form (a parameter more).

#### Scenario: A widening of an open member
- **WHEN** the reach of an `open` Kotlin member or of a method of a Java class that is not final widens
- **THEN** it stays `changed`
  [@test] crates/review-plan/src/api_test.rs (`a_widening_of_an_open_member_stays_a_change`)

#### Scenario: A widening with another change of the form
- **WHEN** the reach widens and the form changes otherwise too (a parameter more)
- **THEN** it stays `changed`
  [@test] crates/review-plan/src/api_test.rs (`a_change_of_form_to_or_from_api_is_listed`)

### Requirement: API Surface: Each entry gives its place, name, kind, parts and forms

Each entry MUST give its file and line, its name, its kind (`removed`, `changed`, `added`), the parts that changed, and
its form before and after: the header with the parameters after the name, a type by its constructors, written as code
is (`fun hooks(): List<String>`, `fun open(path: String): Int`), never its tokens spaced apart.

#### Scenario: The entries of a range
- **WHEN** the `api` of a range is printed
- **THEN** each entry gives its file and line, its name, its kind, the parts that changed and its forms before and after, written as code is (`fun hooks(): List<String>`, `fun open(path: String): Int`)
  [@test] crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Requirement: API Surface: A form reads as code is written in its language

The type parameters of a declaration MUST stand apart from its keyword, the parameters a header holds follow the name or
its type parameters, a TypeScript string or template literal MUST read as written, a Rust header as rustfmt writes it,
and an operator after an operand keeps its spaces (TypeScript's `A & B`).

#### Scenario: Type parameters
- **WHEN** a form has type parameters or the parameters a header holds
- **THEN** they read as code: `fun <T> String.trim2(): T?`, `static <T> List<T> of()`, `fn open() -> u8`
  [@test] crates/review-plan/src/api_test.rs (`a_form_of_type_parameters_reads_as_code`)

#### Scenario: A TypeScript header
- **WHEN** a TypeScript header holds a string or template literal, or an operator after an operand
- **THEN** it reads as written: `` function pick<T>(mode: "a", prefix = `x-${name}`): T ``, `A & B`
  [@test] crates/review-plan/src/api_test.rs (`a_typescript_header_reads_as_code`)

#### Scenario: A Rust header
- **WHEN** a Rust header is shown
- **THEN** it reads as rustfmt writes it: `fn get<'a>(&self, path: &'a str) -> Option<&'a [u8]>`, `x: *const u8`, `T: ?Sized`
  [@test] crates/review-plan/src/api_test.rs (`a_rust_header_reads_as_code`)

### Requirement: API Surface: Each form writes its reach as the language of its file does

Each form MUST write its reach as the language of its file does, so two forms that differ in the reach alone read apart;
a public reach only in the forms of a change whose reach differs, as the summary holds a `protected` member as public and
the members of a Rust trait are public without `pub`.

#### Scenario: The reaches of each language
- **WHEN** a form of each language is written
- **THEN** its reach reads as in its file: Kotlin `internal` and `private`, Java `private` (its package none), Rust `pub(crate)` and `pub(super)` (private none), TypeScript `private` of a member, C# `internal`, `private` and `file`
  [@test] crates/review-plan/src/api_test.rs (`a_widening_alone_is_an_addition_that_breaks_nothing`, `a_widening_of_an_open_member_stays_a_change`)

#### Scenario: A public reach
- **WHEN** a declaration is public (Java and C# `public`, Rust `pub`, a top-level TypeScript `export`; Kotlin none)
- **THEN** its forms write the public reach only in a change whose reach differs

### Requirement: API Surface: A type's constructors read as calls of it

A type's constructors MUST read as calls of it, a line each, each after the narrower of its reach and its type's, never
the name of a `Reach`; a parameter by its name and type, `= …` for a default, `vararg` before a vararg; a type of several
names (its last names, the structure gone) MUST NOT be shown, never as `List/Window`.

#### Scenario: The constructors and parameters of a type
- **WHEN** the form of a type with constructors is written
- **THEN** each constructor reads as a call, a line each: `Event(text: String, start: Long = …)`, `private Event()`
- **AND** each after the narrower of its reach and its type's (`internal Port(value: Int?)` of an internal class)
- **AND** a type of several names is not shown, never as `List/Window`
  [@test] crates/review-plan/src/api_test.rs (`a_form_of_constructors_and_parameters_reads_as_code`)

### Requirement: API Surface: Only what the form shows changes

Only what the form shows MUST count as a change: Rust compares the forms as it holds them.

#### Scenario: A change the form does not show
- **WHEN** a declaration changes in a way its form does not show
- **THEN** it is no change of its form; Rust compares the forms as it holds them

### Requirement: API Surface: The file and line of an entry follow every rename

The file and line MUST be those after the commits, through every rename, also one that changes no declaration; of a
removal, those before the first commit that touched the declaration, so `path:line` is of the same content: the old path
when that commit also renamed the file. A range of no source file of a known language MUST have an empty `api`.

#### Scenario: An API of a renamed file
- **WHEN** a file with a changed API was renamed
- **THEN** the entry and its usages count at its new path
  [@test] crates/review-plan/src/api_test.rs (`the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`)

#### Scenario: A removal after a rename
- **WHEN** the commit that first touched a removed declaration also renamed its file
- **THEN** its `path:line` is of the old path, before that commit
  [@test] crates/review-plan/src/api_test.rs (`a_removal_after_a_rename_names_the_path_its_line_is_of`)

#### Scenario: A range without source files
- **WHEN** a range changes no source file of a known language
- **THEN** its `api` is empty
  [@test] crates/review-plan/src/api_test.rs (`a_range_without_source_files_has_no_api`)

### Requirement: Fan-in: Every removed or changed entry gets its usages at the head

Every removed or changed entry MUST get its `usages` at the head of the range from `accio references --at <place>
--ref <head>`, every place of the plan in one process, with `complete: false` when accio cut the answer of a place at
100. An addition, a widening too, MUST never be asked.

#### Scenario: The usages at the head rank the card
- **WHEN** the plan has removed and changed entries
- **THEN** each gets its `usages` at the head of the range, and an addition, a widening too, is never asked
  [@test] crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_usages_left_outside`)
  [@test] crates/review-plan/src/api_test.rs (`the_usages_at_the_head_rank_the_card`)

#### Scenario: Every place in one process
- **WHEN** the plan asks the usages of several places
- **THEN** every place is asked in one process of `accio references --at <place> --ref <head>`
  [@test] crates/file-diffs/src/accio_test.rs (`the_places_of_a_plan_are_asked_in_one_process`)

#### Scenario: A cut answer
- **WHEN** accio cut the answer of a place at 100
- **THEN** its usages have `complete: false`
  [@test] crates/review-plan/src/fan_in_test.rs (`a_cut_answer_is_not_complete`)

### Requirement: Fan-in: An entry is asked by the line of its name in the last commit that touched it

The place of an entry MUST be the line of its name in the last commit that touched it, `<commit>:<path>:<line>`; of a
removal, the line before the commit that first touched it, at that commit's parent. accio answers only that
declaration: a member of its own type, a top-level function of its own module, a name every type declares too. Two
entries of one place MUST share it.

#### Scenario: An entry is asked by its place once
- **WHEN** two entries have one place
- **THEN** they share it, asked once as `<commit>:<path>:<line>`
  [@test] crates/review-plan/src/fan_in_test.rs (`an_entry_is_asked_by_its_place_once`)

#### Scenario: A member and a constructor are asked by their place
- **WHEN** the entry is a member of a type, or a Rust constructor
- **THEN** accio answers only that declaration, asked by its place
  [@test] crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_place_and_an_override_is_no_entry`, `a_rust_constructor_is_asked_by_its_place`)

#### Scenario: An entry of a renamed file
- **WHEN** the file of the entry was renamed
- **THEN** its place is at its new path in the last commit that touched it
  [@test] crates/review-plan/src/api_test.rs (`the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`)

### Requirement: Fan-in: A place with no declaration leaves its usages unknown

A place accio finds no declaration at (`unavailable`: a line it does not index, such as a Kotlin primary-constructor
`val`, JetBrains/accio#189) MUST leave the entry's `usages` null, unknown, never "none left outside"
(JetBrains/accio#215: `references --at` still answers it as complete with no hit).

#### Scenario: A place with no declaration
- **WHEN** accio answers `unavailable` for the place of an entry
- **THEN** its `usages` are null, unknown, never "none left outside"
  [@test] crates/review-plan/src/fan_in_test.rs (`a_place_with_no_declaration_leaves_its_usages_unknown`)

### Requirement: Fan-in: Only code a type or an import ties to the declaration counts

Only a hit in code that a type or an import ties to the declaration MUST count as a usage: not the declaration itself,
an `import` or a `doc` hit (`NOT_USES`). A hit only the name ties (`resolved: name`: a receiver nothing typed, another
language, a namesake's) MUST NOT count; those left outside are `by_name`, unknown, and keep the entry from being
`unbroken`.

#### Scenario: Only code hits count
- **WHEN** accio answers the declaration, an `import`, a `doc` hit and code hits
- **THEN** only the code hits count as usages
  [@test] crates/review-plan/src/fan_in_test.rs (`only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes`)
  [@test] crates/review-plan/src/api_test.rs (`a_rust_constructor_is_asked_by_its_place`)

#### Scenario: A hit only the name ties
- **WHEN** a hit is `resolved: name`
- **THEN** it is no use; left outside it is `by_name` and keeps the entry from being `unbroken`
  [@test] crates/review-plan/src/fan_in_test.rs (`a_hit_only_the_name_ties_is_no_use_and_keeps_the_api_listed`)

### Requirement: Fan-in: An override hit is a use

An `override` hit, an override of a changed member in a subtype, MUST count as a use: a changed parameter list breaks it.

#### Scenario: An override left outside
- **WHEN** a subtype outside the change overrides a member whose parameters changed
- **THEN** the `override` hit is a use that can break
  [@test] crates/review-plan/src/fan_in_test.rs (`an_override_left_outside_can_break`)

### Requirement: Fan-in: Of a constructor change only calls and subtypes count

Of a type whose only change is its constructors, only a `call` hit (`Type(…)`, `new Type(…)`, a Kotlin `class Sub :
Type(…)`) and a `supertype` hit (a Java `extends Type`, whose `super(…)` calls a constructor) MUST count. A use of the
type as a type or an import still compiles. The usages MUST be as `complete` as the answer of accio.

#### Scenario: A constructor change
- **WHEN** a type changes only its constructors
- **THEN** only its `call` and `supertype` hits count, not its uses as a type or an import
- **AND** the usages are as `complete` as the answer of accio
  [@test] crates/review-plan/src/fan_in_test.rs (`only_code_hits_count_and_of_a_constructor_change_only_calls_and_subtypes`)

### Requirement: Fan-in: A usage on a line the commits changed is inside

A usage MUST be `inside` when a line of it at the head is one the reviewed commits changed, a call from its first line to
its last (`end_line`): a line of the blocks of its file from the blob before the first commit that changes it to the
blob after the last one, as the diffs of the range show them; any line of a file they add. This change wrote or
rewrote it.

#### Scenario: A use on a changed line
- **WHEN** a use lies on a line the commits changed, or in a file they add
- **THEN** it is `inside`
  [@test] crates/review-plan/src/fan_in_test.rs (`a_use_is_inside_only_on_a_line_the_commits_changed`)
  [@test] crates/review-plan/src/api_test.rs (`a_member_is_asked_by_its_place_and_an_override_is_no_entry`)

#### Scenario: A call over several lines
- **WHEN** any line of a call from its first line to its `end_line` is one the commits changed
- **THEN** the call is `inside`
  [@test] crates/review-plan/src/fan_in_test.rs (`a_call_is_inside_by_any_of_its_lines`)

#### Scenario: The usages split
- **WHEN** an entry has uses inside the change and outside it
- **THEN** its usages split into those inside the change and those left outside
  [@test] crates/review-plan/src/fan_in_test.rs (`the_usages_split_into_those_inside_the_change_and_those_left_outside`)
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

### Requirement: Fan-in: Any other usage is left outside

A usage that is not `inside` MUST be left outside, a use on lines of a changed file that the change left as they were
too.

#### Scenario: A use on an unchanged line of a changed file
- **WHEN** a use lies on a line of a changed file that the change left as it was
- **THEN** it is left outside
  [@test] crates/review-plan/src/fan_in_test.rs (`a_use_is_inside_only_on_a_line_the_commits_changed`)

### Requirement: Fan-in: A call left outside that binds as before still binds

A call left outside MUST be `still_bound`, no break, when the entry changed only its parameters or its constructors,
accio says the call fits the new form (`fits`), and each argument binds (`parameter`) a parameter of the same name and
type as before the commits: a named one by its name, a positional one by its position in every parameter list before
(a vararg takes every position from its own), a trailing lambda as the last parameter.

#### Scenario: A call that binds what it bound before
- **WHEN** a call left outside `fits` the new form and each named or positional argument binds a parameter of the same name and type as before
- **THEN** it is `still_bound`
  [@test] crates/review-plan/src/fan_in_test.rs (`a_call_left_outside_that_binds_what_it_bound_before_still_binds`)

#### Scenario: A trailing lambda and a vararg
- **WHEN** a call left outside passes a trailing lambda or a vararg
- **THEN** the lambda binds the last parameter and the vararg takes every position from its own
  [@test] crates/review-plan/src/fan_in_test.rs (`a_trailing_lambda_and_a_vararg_bind_by_their_place`)

#### Scenario: A constructor call that names only kept parameters
- **WHEN** a constructor call left outside names only parameters the commits kept
- **THEN** it is `still_bound`
  [@test] crates/review-plan/src/fan_in_test.rs (`a_constructor_call_left_outside_that_names_only_kept_parameters_still_binds`)

### Requirement: Fan-in: A use left outside that is not still bound can break

A positional argument that now binds another parameter, a spread, and a call of an entry whose header, reach or Java
view changed too, MUST count as can break. Every other use left outside MUST be `outside`, and the first 5 of those MUST
be listed as `outside_sites` by path and line.

#### Scenario: A call that binds another parameter
- **WHEN** a positional argument of a call left outside now binds another parameter, or the call passes a spread
- **THEN** the call can break
  [@test] crates/review-plan/src/fan_in_test.rs (`a_call_left_outside_that_binds_what_it_bound_before_still_binds`)

#### Scenario: The sites left outside
- **WHEN** uses left outside are `outside`
- **THEN** the first 5 of them are listed as `outside_sites` by path and line

### Requirement: Fan-in: An API is unbroken only by a complete count with nothing that breaks

`unbroken` MUST be true only when the usages are `complete`, none left outside can break and none is tied by the name
alone: every use is updated in this change or still binds. An unknown or cut count MUST never be, also at 0.

#### Scenario: A complete count with no use that breaks
- **WHEN** the usages are `complete` and every use is updated in this change or still binds
- **THEN** the entry is `unbroken`
- **AND** an unknown or cut count never is, also at 0
  [@test] crates/review-plan/src/fan_in_test.rs (`an_api_is_unbroken_only_by_a_complete_count_with_no_use_left_outside_that_breaks`)

#### Scenario: A use tied by the name alone
- **WHEN** a use left outside is tied by the name alone
- **THEN** the entry is not `unbroken`
  [@test] crates/review-plan/src/fan_in_test.rs (`a_hit_only_the_name_ties_is_no_use_and_keeps_the_api_listed`)

### Requirement: Fan-in: The plan says once whether a use left outside breaks

The plan MUST say once whether a use left outside breaks (`verdict`), over its removed and changed entries: `breaks` when
one has a use left outside that can break, whatever the others; else `unknown` when the `usages` of one are null or not
`unbroken` (unknown, cut, tied by the name alone); else `noUseBreaks`; `pending` in the plan the window gets while the
index is asked.

#### Scenario: The verdict of a plan
- **WHEN** the plan has removed and changed entries
- **THEN** its `verdict` is `breaks` when one has a use left outside that can break, else `unknown` when the usages of one are unknown, cut or tied by the name alone, else `noUseBreaks`
  [@test] crates/review-plan/src/api_test.rs (`the_verdict_says_whether_a_use_left_outside_breaks`, `the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

#### Scenario: An API of additions and widenings alone
- **WHEN** the API of the plan holds only additions and widenings
- **THEN** the `verdict` is `noUseBreaks`
  [@test] crates/review-plan/src/api_test.rs (`a_widening_alone_is_an_addition_that_breaks_nothing`)

#### Scenario: While the index is asked
- **WHEN** the window gets the plan before the index answered
- **THEN** its `verdict` is `pending`

### Requirement: Fan-in: The entries are sorted by their usages left outside that can break

The entries MUST be sorted by their usages left outside that can break, the most first, then by those inside, those
without `usages` last; then removals, changes and additions, then by file and line.

#### Scenario: The order of the entries
- **WHEN** the entries have usages left outside, inside, and none
- **THEN** the most usages left outside that can break come first, then by those inside, those without `usages` last, then removals, changes and additions, then by file and line
  [@test] crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_usages_left_outside`, `the_usages_split_into_those_inside_the_change_and_those_left_outside`)
  [@test] crates/review-plan/src/api_test.rs (`the_usages_at_the_head_rank_the_card`)

### Requirement: Fan-in: Without a usable index the usages are null and the note says why

Without accio, or without a usable index of the repository, the `usages` MUST be `null` and `index_note` MUST say why:
"accio is not installed", else the reason accio gives, or explain a format mismatch without the shard path. A plan MUST
never build, wait for or refresh an index: every query runs with `--no-build`.

#### Scenario: Without an index
- **WHEN** accio is not installed or has no index of the repository
- **THEN** the `usages` are `null` and `index_note` says "accio is not installed", else the reason accio gives ("no index of this repository", "accio is building the index of this repository")
- **AND** every query runs with `--no-build`
  [@test] crates/review-plan/src/fan_in_test.rs (`without_an_index_the_usages_are_missing_and_the_note_says_why`)
  [@test] crates/file-diffs/src/accio_test.rs (`an_index_accio_cannot_use_answers_no_index_with_its_reason`)

#### Scenario: An index of another format
- **WHEN** the index of the repository is from another accio than `<executable>`
- **THEN** a newer one says "the index of this repository is from a newer accio than <executable>: update accio"
- **AND** an older one says "the index of this repository is from an older accio than <executable>: run accio index", without the shard path
  [@test] crates/file-diffs/src/accio_test.rs (`an_index_format_mismatch_says_which_accio_version_to_run`)

### Requirement: Fan-in: A failing query fails no plan

A failing query MUST fail no plan: the entries keep no `usages` and `index_note` gives the failure; only a cancel of the
caller (a newer request of the window) MUST cancel the plan.

#### Scenario: A failing index
- **WHEN** a query of the index fails
- **THEN** the plan keeps its API, the entries keep no `usages` and `index_note` gives the failure
  [@test] crates/review-plan/src/fan_in_test.rs (`a_failing_index_keeps_the_api`)

#### Scenario: A canceled query
- **WHEN** the caller cancels (a newer request of the window)
- **THEN** the plan is canceled
  [@test] crates/review-plan/src/fan_in_test.rs (`a_canceled_query_cancels_the_plan`)

### Requirement: Files: Every changed path is listed once at its last path

`files` MUST list every path the reviewed commits change exactly once, at its last path: a renamed file at its new
path with the path before the first commit (`old_path`), a deleted one at its path, none that the commits add and
delete again.

#### Scenario: A changed file is listed once
- **WHEN** the reviewed commits change a file
- **THEN** `files` lists it exactly once, at its last path
- **AND** a deleted one is listed at its path, and none that the commits add and delete again
  [@test] crates/review-plan/src/files_test.rs (`a_source_file_changed_without_its_test_is_untested`)

#### Scenario: A renamed file
- **WHEN** the reviewed commits rename a file
- **THEN** it is listed at its new path with the path before the first commit (`old_path`)
  [@test] crates/review-plan/src/files_test.rs (`a_renamed_file_is_listed_at_its_new_path`)

### Requirement: Files: A source file changed without its test is untested

A file MUST be `untested` when it is a source file of a known language, no test itself, not mechanical, no changed
test names its stem, and the commits change no test code inside it (a unit that is `test`: a test of its Rust
`#[cfg(test)] mod tests`, a fixture of it; or a block of a repeated edit inside such code, whose unit is of no
declaration); a test it keeps while its code changes MUST NOT count.

#### Scenario: A source file without a changed test
- **WHEN** a source file of a known language changes, and no changed test names its stem
- **THEN** it is `untested`, also at its new path after a rename
  [@test] crates/review-plan/src/files_test.rs (`a_source_file_changed_without_its_test_is_untested`, `a_renamed_file_is_listed_at_its_new_path`)

#### Scenario: A Rust file whose inline test changed
- **WHEN** the commits change a test of the Rust `#[cfg(test)] mod tests` of a file, directly or by a repeated edit inside it
- **THEN** the file is not `untested`
  [@test] crates/review-plan/src/files_test.rs (`a_rust_file_whose_inline_test_changed_is_tested`, `a_rust_file_whose_inline_test_a_repeated_edit_changed_is_tested`)

#### Scenario: A kept test does not count
- **WHEN** a file keeps its test while its code changes
- **THEN** that test does not count, and the file is `untested`

### Requirement: Files: breaks sums the usages left outside the change

`breaks` MUST sum the usages left outside the change of the API a file removes or changes, at its last path, also of
a removal whose entry names the old path. A usage inside the change and an addition MUST break nothing.

#### Scenario: The usages of a file's API
- **WHEN** a file removes or changes an API with usages left outside the change
- **THEN** its `breaks` sums them
- **AND** a usage inside the change and an addition break nothing
  [@test] crates/review-plan/src/files_test.rs (`the_usages_of_a_files_api_rank_it_first`)

#### Scenario: A renamed file and a member
- **WHEN** the API is of a renamed file, or a removal's entry names the old path
- **THEN** its usages count at the new path
- **AND** a member is asked by its place, and an override is no entry
  [@test] crates/review-plan/src/api_test.rs (`the_usages_of_an_api_of_a_renamed_file_count_at_its_new_path`, `a_member_is_asked_by_its_place_and_an_override_is_no_entry`)

### Requirement: Files: breaks_complete only when every usage count is complete

`breaks_complete` MUST be true only when every API the file removes or changes has `usages` that are `complete`. A
name not asked, no index, a failed query, a cut answer or a constructor-only change MUST make `breaks` a lower bound,
also at 0. A file without such an API is complete.

#### Scenario: A usage count unknown or cut
- **WHEN** a name was not asked, there is no index, a query failed, an answer was cut or the change is constructor-only
- **THEN** `breaks` is a lower bound, also at 0, and `breaks_complete` is false
  [@test] crates/review-plan/src/files_test.rs (`breaks_is_a_lower_bound_when_a_usage_count_of_its_api_is_unknown_or_cut`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

#### Scenario: A file without such an API
- **WHEN** a file removes or changes no API
- **THEN** it is complete

### Requirement: Files: The files with the most breaks come first

The files with the most `breaks` MUST come first, then the `untested` ones, then the tree order.

#### Scenario: The order of the files
- **WHEN** the files are listed
- **THEN** the ones with the most `breaks` come first, then the `untested` ones, then the tree order
  [@test] crates/review-plan/src/files_test.rs (`the_usages_of_a_files_api_rank_it_first`)

### Requirement: Files: A test is told by its path

A test MUST be told by its path: a directory `test`, `tests`, `__tests__`, `testFixtures`, `testSrc` (of an IntelliJ
module), or a name `*Test`, `*Tests`, `*_test`, `*.test`, `*.spec`, `test_*`; it names the stem without that mark,
compared without case.

#### Scenario: A test names its subject
- **WHEN** a path is in a directory `test`, `tests`, `__tests__`, `testFixtures`, `testSrc`, or its name is `*Test`, `*Tests`, `*_test`, `*.test`, `*.spec`, `test_*`
- **THEN** it is a test, and it names the stem without that mark, compared without case
  [@test] crates/review-plan/src/files_test.rs (`a_test_names_its_subject_by_its_path`)

### Requirement: Files: brd review-plan prints the parts of the plan

`brd review-plan` MUST print `api`, `files`, `links`, `dependencies`, `repeated_edits` and `index_note`, in
`brd schema`.

#### Scenario: The output of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `api`, `files`, `links`, `dependencies`, `repeated_edits` and `index_note`, in `brd schema`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_files_in_dependency_order`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)

### Requirement: Files: The API and the files add a fixed number of git processes

The API and the files MUST add a fixed number of git processes, whatever the number of commits: one `log` of the
changes of the reviewed commits, one `cat-file` of their blobs, and the reads of the syntax summaries; accio MUST run
no git.

#### Scenario: The processes of the API and the files
- **WHEN** a plan reads the API and the files, since the review or of a range
- **THEN** it adds one `log` of the changes of the reviewed commits, one `cat-file` of their blobs, and the reads of the syntax summaries, whatever the number of commits
- **AND** accio runs no git
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`, `a_review_plan_of_a_range`)

### Requirement: Files: The text of the hunks reads no blob twice

The text of the hunks MUST come from the memory of the diffs when they read the blobs in this process. Else the blobs
MUST join the `cat-file` of the summaries, which then reads them alone when it has no summary or manifest to read: one
process more only for a plan whose diffs and summaries come from the object cache.

#### Scenario: The diffs read the blobs in this process
- **WHEN** the diffs read the blobs in this process
- **THEN** the text of the hunks comes from their memory
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_modified_files`)

#### Scenario: The diffs and summaries come from the object cache
- **WHEN** the diffs and the summaries come from the object cache
- **THEN** the blobs join the `cat-file` of the summaries, which reads them alone when it has no summary or manifest to read: one process more

### Requirement: Links: A link names the file that uses and the file that declares

`links` MUST give, per pair of changed files, the file that uses (`from`) and the file that declares (`to`), at their
last paths, with each name that links them and the line of its first use, by the evidence of the dependency order.

#### Scenario: A new file another imports
- **WHEN** a changed file uses a name another changed file declares
- **THEN** `links` gives the pair, the file that uses (`from`) and the file that declares (`to`), at their last paths
- **AND** each name that links them, with the line of its first use, by the evidence of the dependency order
  [@test] crates/review-plan/src/links_test.rs (`a_new_file_that_another_imports_is_an_added_link`)
  [@test] crates/review-plan/src/order_test.rs (`the_order_returns_its_graph_with_the_names_and_lines`)

### Requirement: Links: A link is added, removed or kept

A link MUST be `added` when it exists only after the commits, `removed` when only before them, and `kept` when both; a
file before the commits links at its path then, named by its last path. The names of a kept link MUST be those after
the commits, of a removed one those before.

#### Scenario: A link only after the commits
- **WHEN** a link exists only after the commits
- **THEN** it is `added`
  [@test] crates/review-plan/src/links_test.rs (`a_new_file_that_another_imports_is_an_added_link`)

#### Scenario: A link before and after, and one only before
- **WHEN** a link exists before and after the commits, or only before them
- **THEN** it is `kept` with the names after the commits, or `removed` with the names before them
  [@test] crates/review-plan/src/links_test.rs (`a_link_before_and_after_is_kept_and_one_only_before_is_removed`)

#### Scenario: A renamed file
- **WHEN** a file before the commits had another path
- **THEN** it links at its path then, named by its last path
  [@test] crates/review-plan/src/links_test.rs (`a_renamed_file_links_at_its_new_path`)

### Requirement: Links: uses, used_by and new_links count the links of a file

`uses` and `used_by` of a file MUST count its `added` and `kept` links; `new_links` its `added` links as the file that
uses.

#### Scenario: The counts of a file
- **WHEN** a file has `added`, `kept` and `removed` links
- **THEN** `uses` and `used_by` count its `added` and `kept` links
- **AND** `new_links` counts its `added` links as the file that uses
  [@test] crates/review-plan/src/links_test.rs (`a_new_file_that_another_imports_is_an_added_link`, `a_link_before_and_after_is_kept_and_one_only_before_is_removed`)

### Requirement: Links: A link between modules is cross_module

A link MUST be `cross_module` when the nearest directories above its two files with a build file differ, and
`new_between_modules` when it is also `added`: a new dependency of a module, which the review lists.

#### Scenario: A link between two modules
- **WHEN** the nearest directories above the two files of a link with a build file differ
- **THEN** the link is `cross_module`
- **AND** `new_between_modules` when it is also `added`: a new dependency of a module, which the review lists
  [@test] crates/review-plan/src/links_test.rs (`a_link_between_two_modules_crosses_them`)

### Requirement: Links: The build files are those of the head reviewed

The build files MUST be those of the head reviewed, not of the checkout: a pull request is not checked out, and a
module it adds is in no other tree.

#### Scenario: A head that is not checked out
- **WHEN** the head reviewed is not checked out and adds a module
- **THEN** the directories of its build files are read from that revision
- **AND** the links cross the module it adds
  [@test] crates/review-plan/src/links_test.rs (`the_links_of_a_head_that_is_not_checked_out_cross_the_module_it_adds`)
  [@test] crates/trial-check/src/module_roots_test.rs (`reads_the_directories_of_a_revision_that_is_not_checked_out`)

### Requirement: Links: The links run no git process

The links MUST run no git process: they read the summaries the API read.

#### Scenario: The processes of the links
- **WHEN** a plan of a range computes its links
- **THEN** it runs no git process for them, reading the summaries the API read
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### Requirement: Mechanical Changes: A file of the build model is buildModel

A file MUST be `buildModel` when its name is of the build model: `*.iml`, `BUILD`, `BUILD.bazel`, a `*.lock` that is
no lock file of a package manager, `.idea/modules.xml`.

#### Scenario: A file named as the build model
- **WHEN** the commits change an `*.iml`, a `BUILD`, a `BUILD.bazel`, a `*.lock` that is no lock file of a package manager, or `.idea/modules.xml`
- **THEN** the file is `buildModel`
  [@test] crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`)

### Requirement: Mechanical Changes: A list of the modules the change adds is moduleList

A file of no language and of no module itself MUST be `moduleList` when every line the commits add or remove (one at
least) names whole a module they add or delete: the directory of its build file or the name of its `*.iml`, no
character of a name right before or after it. It is the wiring of that module, not the first thing of its step.

#### Scenario: A generated list of the module directories
- **WHEN** every line the commits add or remove in a file of no language names whole a module they add or delete (a generated list of the module directories, as `build/bazel-generated-file-list.txt` of a large monorepo, a list of module names, as a `*.dev-plan.json`)
- **THEN** the file is `moduleList`, the wiring of that module, not the first thing of its step
- **AND** a line of punctuation alone (`{`, `},` of a JSON list) frames an entry, as a blank line, and names nothing
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)

#### Scenario: A line that names a module the change did not add
- **WHEN** a line names a module the change did not add
- **THEN** it keeps the file real work
  [@test] crates/review-plan/src/modules_test.rs (`a_list_that_names_no_module_the_commits_add_is_real_work`)

### Requirement: Mechanical Changes: A deleted file stays real work

A file the commits delete MUST stay real work: the deletion is a decision of its author, as a new file is, and the
same change as the edits of the other files it goes with (a server dropped from five configs, and from the sixth that
held nothing else). A deleted file MUST never be `untested`.

#### Scenario: A deleted file
- **WHEN** the commits delete a file that is not generated and not of the build model by its name
- **THEN** it stays real work, and it is never `untested`
  [@test] crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`)
  [@test] crates/review-plan/src/units_test.rs (`a_deleted_file_is_a_unit_and_the_deleted_files_of_a_directory_are_one`)

### Requirement: Mechanical Changes: Only a generated or build-model deletion is mechanical

Only a deleted file that is generated (by its name, the attributes of the head, or the header of the blob the deletion
removed, read in the same batch) or of the build model by its name SHALL be mechanical.

#### Scenario: A deleted generated file
- **WHEN** the commits delete a file generated by its name, the attributes of the head, or the header of the blob the deletion removed, read in the same batch
- **THEN** the deleted file is mechanical
  [@test] crates/review-plan/src/files_test.rs (`a_generated_file_is_mechanical_by_its_name_its_attributes_or_its_header`)

#### Scenario: A deleted file of the build model
- **WHEN** the commits delete a file of the build model by its name
- **THEN** the deleted file is mechanical

### Requirement: Mechanical Changes: A generated file is told by its name or its attributes

A file MUST be `generated`, nobody writes it by hand, with why (`generated`, `GeneratedBy` of `generated-files`): by
its name, a lock file of a package manager (`Cargo.lock`, `pnpm-lock.yaml`, `go.sum`, …), a minified file
(`*.min.js`, `*.min.css`) or a source map (`*.map`); else by the attributes of the head of the range
(`linguist-generated`, `-diff`), one `check-attr --source=<head>` for the paths no name marks.

#### Scenario: A lock file, a minified file, a source map
- **WHEN** the commits change `Cargo.lock`, `pnpm-lock.yaml`, `go.sum`, a `*.min.js`, a `*.min.css` or a `*.map`
- **THEN** the file is `generated` by its name, with why (`GeneratedBy` of `generated-files`)
  [@test] crates/review-plan/src/files_test.rs (`a_generated_file_is_mechanical_by_its_name_its_attributes_or_its_header`)

#### Scenario: The attributes of the head
- **WHEN** the attributes of the head of the range mark a path `linguist-generated` or `-diff`
- **THEN** the file is `generated`, read by one `check-attr --source=<head>` for the paths no name marks
  [@test] crates/generated-files/src/attributes_test.rs (`the_attributes_of_the_revision_mark_generated_files_and_a_lock_file_is_one_by_its_name`)

### Requirement: Mechanical Changes: A generated file is told by its header

Else a file MUST be `generated` by its header: the leading block of comments of its new text (of a deleted file the
text its deletion removed), in the comment syntax of its language, within 5 comment lines after a front matter, a
shebang and blank lines, whose comment starts with the words of a generator or holds `@generated`; the line is kept for
the review. An added file too, and a change of code inside one.

#### Scenario: The leading comment of a generator
- **WHEN** the leading block of comments of a file's new text, in the comment syntax of its language (`<!--` in Markdown and XML, `#`, `//`, `/*`, `--`, `;`), within 5 comment lines after a front matter, a shebang and blank lines, starts with `Generated by`, `Generated from`, `Code generated by`, `This file is generated`, `Auto-generated`, … or holds `@generated`
- **THEN** the file is `generated`, and the line is kept for the review
- **AND** so is an added file, and a change of code inside one
  [@test] crates/review-plan/src/files_test.rs (`a_generated_file_is_mechanical_by_its_name_its_attributes_or_its_header`)
  [@test] crates/generated-files/src/header_test.rs (`the_leading_comment_of_a_generator_marks_a_file_in_the_comment_syntax_of_its_language`)

#### Scenario: A snapshot
- **WHEN** the commits change a snapshot (`*.snap`)
- **THEN** it stays real work, its review reads the expected output

### Requirement: Mechanical Changes: A file that only mentions a generator stays real work

A file that only mentions a generator MUST stay real work.

#### Scenario: A mention that is no header of a generator
- **WHEN** a file has `generated` in the middle of a sentence, a Markdown heading (`# Generated Files`), a marker past the fifth comment line or after the code, a comment of another syntax, or it is JSON (no comments)
- **THEN** it stays real work
- **AND** so do the ignore files, whose `# Generated by Cargo` names the tool that started a file people keep
  [@test] crates/generated-files/src/header_test.rs (`a_hand_kept_file_that_mentions_a_generator_is_no_generated_file`)

### Requirement: Mechanical Changes: The text of a file generated by its name is not read

The text of a file generated by its name or its attributes MUST NOT be read, and of the others the new blob MUST join
the batch the hunks are read in (no process more when the blobs are in memory); a blob too large for that batch has no
header, and its file starts collapsed for its size anyway.

#### Scenario: A file generated by its name or attributes
- **WHEN** a file is generated by its name or its attributes
- **THEN** its blob is not read
  [@test] crates/review-plan/src/files_test.rs (`a_file_generated_by_its_name_or_attributes_reads_no_blob`)

#### Scenario: The header of the other files
- **WHEN** the header of another file is read
- **THEN** its new blob joins the batch the hunks are read in, no process more when the blobs are in memory
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`, `a_review_plan_of_modified_files`)

#### Scenario: A blob too large for the batch
- **WHEN** a blob is too large for that batch
- **THEN** it has no header, and its file starts collapsed for its size anyway

### Requirement: Mechanical Changes: A manifest whose dependency entries alone changed is buildModel

A plugin descriptor, a `Cargo.toml` or a `package.json` MUST be `buildModel` when only its dependency entries changed
("Dependency Changes"). One that changed beside them, a new one, and one that a side does not parse MUST stay real
work. One whose entries and remainder did not change is read as any other file. An `*.iml` that does not parse MUST
stay real work too.

#### Scenario: Only the dependency entries changed
- **WHEN** only the dependency entries of an `*.iml`, a `Cargo.toml`, a `package.json` or a plugin descriptor changed
- **THEN** the file is `buildModel`
  [@test] crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`, `the_dependencies_of_a_package_json_are_its_dependency_changes`, `the_dependencies_and_the_content_of_a_plugin_descriptor_are_its_dependency_changes`)

#### Scenario: A manifest that changed beside its entries
- **WHEN** a manifest changed beside its dependency entries, is new, or a side does not parse, or an `*.iml` does not parse
- **THEN** it stays real work
- **AND** one whose entries and remainder did not change is read as any other file
  [@test] crates/review-plan/src/dependencies_test.rs (`a_new_version_with_new_features_keeps_the_manifest_in_the_tree`)

### Requirement: Mechanical Changes: A rename, whitespace or imports alone is mechanical

Else, from the text of every hunk of every commit that changes it, a file MUST be `renamed` when each change is a
rename with no line added or removed; `whitespace` when each hunk has the same text without its whitespace, line ends
included, so a line wrapped or joined too; `importsOnly` when, in a source file, each other hunk has only blank lines
and lines of import statements, at least one.

#### Scenario: A rename, a whitespace change, an imports change
- **WHEN** each change of a file is a rename with no line added or removed, or each hunk has the same text without its whitespace, or each other hunk of a source file has only blank lines and lines of import statements
- **THEN** it is `renamed`, `whitespace` or `importsOnly`
  [@test] crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`)

#### Scenario: A wrapped line
- **WHEN** a hunk only wraps or joins a line
- **THEN** it has the same text without its whitespace, line ends included, and the file is `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)

### Requirement: Mechanical Changes: An import statement runs until its brackets close

An import statement MUST start with an `import`, `use` or `package` line, a C# `using` directive or a `namespace`
line, and run until the brackets it opens close, read in the whole file. A `pub use` re-exports, so it MUST be none,
and a C# `using` statement (`using (var x = …)`, `using var x = …;`) MUST be code.

#### Scenario: A line inside an import block
- **WHEN** a line is inside a Rust `use a::{` block or a TypeScript `import {` block over several lines
- **THEN** it is a line of an import statement
  [@test] crates/review-plan/src/files_test.rs (`a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)

#### Scenario: A C# using directive and a using statement
- **WHEN** a C# file changes a `using` directive, or a `using` statement (`using (var x = …)`, `using var x = …;`)
- **THEN** the directive is an import and the statement is code
  [@test] crates/review-plan/src/files_test.rs (`a_csharp_using_directive_is_an_import_and_a_using_statement_code`)

#### Scenario: A pub use
- **WHEN** a hunk changes a `pub use`
- **THEN** it re-exports, so it is no import statement

### Requirement: Mechanical Changes: An unclosed bracket ends the import at its first line

A bracket inside a string, a backtick name or a comment of an import line MUST NOT count. A statement whose brackets
are still open at a `;`, at the next import line or at the end of the file MUST count as its first line only, so the
code after it stays real work.

#### Scenario: An unclosed bracket on an import line
- **WHEN** the brackets of an import statement are still open at a `;`, at the next import line or at the end of the file, or a bracket is inside a string, a backtick name or a comment
- **THEN** the statement counts as its first line only, or the bracket does not count
- **AND** the code after it stays real work
  [@test] crates/review-plan/src/files_test.rs (`an_unclosed_bracket_on_an_import_line_makes_no_import_of_the_code_after_it`)

### Requirement: Mechanical Changes: A meaningful indent is never whitespace

A file whose whitespace has a meaning MUST never be `whitespace`: Python (`*.py`, `*.pyi`), Starlark (`*.bzl`,
`*.star`, `*.bazel`, `BUILD`, `WORKSPACE`), YAML (`*.yml`, `*.yaml`) and Makefiles (`Makefile`, `GNUmakefile`,
`*.mk`). Its indent stays real work.

#### Scenario: An indent change of a Python file
- **WHEN** the commits only change the indent of a file whose whitespace has a meaning
- **THEN** it is no `whitespace` change, and its indent stays real work
  [@test] crates/review-plan/src/files_test.rs (`an_indent_where_the_indentation_has_a_meaning_is_no_whitespace_change`)

### Requirement: Mechanical Changes: A change of code keeps the file out of the mechanical ones

One hunk with an import line and a line of code, a commit that changes code, an added file, and a file without text
MUST keep the file out of the mechanical ones.

#### Scenario: An import line beside a line of code, an added file
- **WHEN** one hunk has an import line and a line of code, or the file is added, or has no text
- **THEN** the file is not mechanical
  [@test] crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`)

#### Scenario: One commit that changes code
- **WHEN** every hunk of one commit is mechanical but another commit changes code
- **THEN** the file is not mechanical
  [@test] crates/review-plan/src/files_test.rs (`a_file_is_mechanical_only_when_every_hunk_of_every_commit_is`)

### Requirement: Repeated Edits: The same hunk in 3 or more files is one group

`repeated_edits` MUST group the files whose commits make one hunk with the same removed and added text, without
whitespace and line ends: 3 or more files, the group of the most files first, a file in one group at most
(`repeated_edit`, the index of its group), with the first lines removed and added for its label.

#### Scenario: The same edit in three or more files
- **WHEN** the commits make one hunk with the same removed and added text, without whitespace and line ends, in 3 or more files
- **THEN** `repeated_edits` groups them, the group of the most files first, with the first lines removed and added for its label
- **AND** a file is in one group at most (`repeated_edit`, the index of its group)
  [@test] crates/review-plan/src/files_test.rs (`the_same_edit_in_three_or_more_files_is_one_group`)

### Requirement: Repeated Edits: A generated file is in no group

A generated file MUST be in no group: its tool made the edit, which the source it was rendered from shows. The source
and two renderings with the same hunk MUST make no group, and the source shows its own change.

#### Scenario: A source and two renderings with the same hunk
- **WHEN** a source and two generated renderings of it have the same hunk
- **THEN** they make no group, and the source shows its own change
  [@test] crates/review-plan/src/files_test.rs (`a_generated_file_makes_no_repeated_edit`)

### Requirement: Repeated Edits: A file whose every hunk is the edit is repeated_edit_only

A file of a group MUST be `repeated_edit_only` when every hunk of every commit that changes it is the edit of its
group, every change of it with the text of its hunks: the group says all of its change. One with another change beside
the edit, or with a change whose hunks are not read, MUST NOT be.

#### Scenario: Every hunk is the repeated edit
- **WHEN** every hunk of every commit that changes a file of a group is the edit of its group
- **THEN** the file is `repeated_edit_only`: the group says all of its change
- **AND** one with another change beside the edit, or with a change whose hunks are not read, is not
  [@test] crates/review-plan/src/files_test.rs (`a_file_whose_every_hunk_is_the_repeated_edit_is_only_that_edit`)

### Requirement: Repeated Edits: Whitespace alone or short lines make no group

A hunk of whitespace alone (a wrapped line too), and one without a line of 10 or more characters (a `}` alone), MUST
make no group.

#### Scenario: A hunk without a line of 10 characters
- **WHEN** the same hunk without a line of 10 or more characters (a `}` alone) is in 3 or more files
- **THEN** it makes no group
  [@test] crates/review-plan/src/files_test.rs (`the_same_edit_in_three_or_more_files_is_one_group`)

#### Scenario: A hunk of whitespace alone
- **WHEN** the same hunk of whitespace alone, a wrapped line too, is in 3 or more files
- **THEN** it makes no group
  [@test] crates/review-plan/src/files_test.rs (`a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)

### Requirement: Dependency Changes: The entries the commits change in the build files

`dependencies` MUST list the entries the commits add to, remove from or change in the build files, read before the
first commit that changes each file and after the last. They are sorted by build file in tree order, then by kind and
name. Each MUST have its build file (`manifest`), its `module`, its `scope`, and its versions before and after when it
has one.

#### Scenario: The dependency changes of a range
- **WHEN** the commits change the dependency entries of build files
- **THEN** `dependencies` lists them, read before the first commit that changes each file and after the last, sorted by build file in tree order, then by kind and name
- **AND** each has its `manifest`, its `module`, its `scope`, and its versions before and after when it has one
  [@test] crates/review-plan/src/dependencies_test.rs (`the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)

### Requirement: Dependency Changes: The entries of an iml

The entries of an `*.iml` MUST be its `orderEntry` of type `module` and `library`, and of type `module-library` with a
named `library`. The scope is `TEST`, `RUNTIME` or `PROVIDED`, else `main`. Its module is the name of the file.

#### Scenario: The order entries of an iml
- **WHEN** the commits change the `orderEntry` elements of an `*.iml`
- **THEN** its entries are those of type `module` and `library`, and of type `module-library` with a named `library`
- **AND** the scope is `TEST`, `RUNTIME` or `PROVIDED`, else `main`, and the module is the name of the file
  [@test] crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`)

### Requirement: Dependency Changes: The entries of a plugin descriptor

A plugin descriptor MUST be an `*.xml` named `plugin.xml`, or in a `META-INF` or `resources` directory, out of
`.idea`, whose root is `idea-plugin`. Its entries MUST be its `depends` and `dependencies` `plugin` (`plugin`), its
`dependencies` `module` (`module`) and its `content` `module` (`content`). Its module is its `id`, else the name of the
file. Another `*.xml` has no entry.

#### Scenario: The dependencies and the content of a plugin descriptor
- **WHEN** the commits change a `plugin.xml`, or an `*.xml` in a `META-INF` or `resources` directory out of `.idea`, whose root is `idea-plugin`
- **THEN** its entries are its `depends` and `dependencies` `plugin` (`plugin`), its `dependencies` `module` (`module`) and its `content` `module` (`content`)
- **AND** its module is its `id`, else the name of the file, and another `*.xml` has no entry
  [@test] crates/review-plan/src/dependencies_test.rs (`the_dependencies_and_the_content_of_a_plugin_descriptor_are_its_dependency_changes`)

### Requirement: Dependency Changes: The entries of a Cargo.toml

The entries of a `Cargo.toml` MUST be those of `dependencies`, `dev-dependencies` and `build-dependencies`, of the
same tables of each `target`, and of `workspace.dependencies`. The version is a string, or the `version` key of a
table. Its module is `package.name`, else the name of its directory.

#### Scenario: The dependency tables of a Cargo.toml
- **WHEN** the commits change the `dependencies`, `dev-dependencies` or `build-dependencies` of a `Cargo.toml`, of a `target`, or `workspace.dependencies`
- **THEN** those are its entries, the version a string or the `version` key of a table
- **AND** its module is `package.name`, else the name of its directory
  [@test] crates/review-plan/src/dependencies_test.rs (`the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)

### Requirement: Dependency Changes: The entries of a package.json

The entries of a `package.json` MUST be those of `dependencies`, `devDependencies` and `peerDependencies`. Its module
is its `name`, else the name of its directory.

#### Scenario: The dependencies of a package.json
- **WHEN** the commits change the `dependencies`, `devDependencies` or `peerDependencies` of a `package.json`
- **THEN** those are its entries, and its module is its `name`, else the name of its directory
  [@test] crates/review-plan/src/dependencies_test.rs (`the_dependencies_of_a_package_json_are_its_dependency_changes`)

### Requirement: Dependency Changes: A root manifest without a name is the module (root)

A `Cargo.toml` or a `package.json` at the root of the repository without a name MUST have the module `(root)`. A root
workspace manifest is the usual case. No crate or package name has parentheses, so the label is unique.

#### Scenario: A root workspace manifest
- **WHEN** the commits change a `Cargo.toml` or a `package.json` at the root of the repository without a name
- **THEN** its module is `(root)`
  [@test] crates/review-plan/src/dependencies_test.rs (`a_new_version_with_new_features_keeps_the_manifest_in_the_tree`)

### Requirement: Dependency Changes: Entries pair by their table, then in their order

Before and after, the entries of one kind and name MUST pair first by their table, then in their order. A pair whose
version or scope differs MUST be one `changed` entry with both versions. It has `scope_before` when the scope differs.
An entry without a pair is `added` or `removed`.

#### Scenario: A changed version, an added and a removed entry
- **WHEN** an entry of one kind and name is in the build file before and after the commits with another version or scope, or only on one side
- **THEN** the pair is one `changed` entry with both versions, and `scope_before` when the scope differs
- **AND** an entry without a pair is `added` or `removed`
  [@test] crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`, `the_dependencies_of_a_package_json_are_its_dependency_changes`)

### Requirement: Dependency Changes: A change of form or table is a change beside the dependencies

A pair MUST count as a change beside the dependencies when its form without the version and the scope differs, or when
it moves to another table of the same scope. This applies also to a `changed` pair.

#### Scenario: A new version together with new features
- **WHEN** a crate gets a new version together with new features
- **THEN** it is a `changed` entry, and its build file stays real work
  [@test] crates/review-plan/src/dependencies_test.rs (`a_new_version_with_new_features_keeps_the_manifest_in_the_tree`)

#### Scenario: An entry that moves to another table of the same scope
- **WHEN** an entry moves to another table of the same scope
- **THEN** it counts as a change beside the dependencies, and its build file stays real work
  [@test] crates/review-plan/src/dependencies_test.rs (`an_entry_that_moves_to_another_table_of_the_same_scope_keeps_the_manifest_in_the_tree`)

### Requirement: Dependency Changes: An added or deleted build file is one newModule entry

A build file the commits add MUST be one `newModule` entry `added`, named by its module. A build file the commits
delete MUST be one `newModule` entry `removed`.

#### Scenario: A new build file and a deleted one
- **WHEN** the commits add a build file, or delete one
- **THEN** it is one `newModule` entry, `added` and named by its module, or `removed`
  [@test] crates/review-plan/src/dependencies_test.rs (`the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)

### Requirement: Dependency Changes: A build file without text or that does not parse has none

A build file that has no text or does not parse, before or after the commits, MUST have no dependency change.

#### Scenario: A build file that does not parse
- **WHEN** a build file has no text or does not parse, before or after the commits
- **THEN** it has no dependency change
  [@test] crates/review-plan/src/dependencies_test.rs (`the_entries_of_an_iml_are_its_dependency_changes`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)

### Requirement: Dependency Changes: The texts of the build files come with the hunks

The texts of the build files MUST come with the text of the hunks: from the memory of the diffs, else in the
`cat-file` of the summaries. A plan of build files alone MUST run one `cat-file`, with the diffs read in this process
or from the object cache.

#### Scenario: A plan of build files alone
- **WHEN** a plan reads only build files, with the diffs read in this process or from the object cache
- **THEN** it runs one `cat-file`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)

### Requirement: Modules: Every changed file has its module

`modules` MUST list the module of every changed file once: the nearest directory above the file with a build file at
the head reviewed (`ModuleRoots::root_of`: an `*.iml`, a `*.csproj`, a Bazel, Gradle or Maven build file, a
`Cargo.toml`, a `package.json`), or of a module the commits delete (a build file they delete there, which the head
lacks), else the root of the repository (`""`, named and labelled `(root)`). `files[].module` names it by its place.

#### Scenario: The module of a changed file
- **WHEN** a file changes below a directory with a build file at the head reviewed
- **THEN** `modules` lists that module once, and `files[].module` names it by its place
- **AND** a file under no such directory is of the root of the repository (`""`, named and labelled `(root)`)
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)

#### Scenario: A module the commits delete
- **WHEN** the commits delete a build file, which the head lacks, and files below it
- **THEN** those files stay in that module
  [@test] crates/review-plan/src/modules_test.rs (`the_files_of_a_module_the_commits_delete_stay_in_it`)

### Requirement: Modules: A module is added, deleted or changed

A module MUST be `added` when the commits add a build file at its root, `deleted` when they delete one and the head has
no module there, else `changed`. Its `name` is that of its `*.iml` (the shortest stem beside a `*.tests.iml`), of its
crate, of the module a dependency change gives a build file at its root, else of its directory.

#### Scenario: A deleted module
- **WHEN** the commits delete the build file at the root of a module and the head has no module there
- **THEN** the module is `deleted`
  [@test] crates/review-plan/src/modules_test.rs (`the_files_of_a_module_the_commits_delete_stay_in_it`)

#### Scenario: An added or a changed module
- **WHEN** the commits add a build file at the root of a module, or change files of a module without either
- **THEN** the module is `added`, else `changed`
- **AND** its `name` is that of its `*.iml` (the shortest stem beside a `*.tests.iml`), of its crate, of the module a dependency change gives a build file at its root, else of its directory

### Requirement: Modules: The label is the shortest suffix no other module shares

The `label` of a module MUST be the shortest suffix of its root no other module of the review shares, of two
directories at least when its last has no `.`, `-` or `_` (`shared/code`, `view.gui.media`, `review-plan`).

#### Scenario: The labels of the modules of a review
- **WHEN** the modules of a review are labelled
- **THEN** each label is the shortest suffix of its root no other module shares, of two directories at least when its last has no `.`, `-` or `_` (`shared/code`, `view.gui.media`, `review-plan`)
  [@test] crates/review-plan/src/modules_test.rs (`a_label_is_the_shortest_suffix_no_other_module_shares`)

### Requirement: Modules: A module counts its files by role and its lines

A module MUST count its changed `files` by role: `build` (the build model, a manifest, a `moduleList`), `prose`
(Markdown, reStructuredText, AsciiDoc, text), `tests` (the paths of [Files](#files)), `code` (a language the summaries
parse), the rest `resources`; its lines `additions` and `deletions` over the range, by the blocks of each file, of a
file the range adds or deletes the lines it leaves or had, as the diff of the range counts them.

#### Scenario: The files by role
- **WHEN** the commits change files of a module
- **THEN** it counts them as `build`, `prose`, `tests`, `code` and `resources`
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)

#### Scenario: A file the range adds
- **WHEN** the range adds or deletes a file
- **THEN** the `additions` and `deletions` of its module count the lines it leaves or had, as the diff of the range counts them
  [@test] crates/review-plan/src/modules_test.rs (`a_file_the_range_adds_counts_the_lines_its_commits_leave`)

### Requirement: Modules: The directories of a module start below its packages

A module MUST list its `directories` below the root without a leading source directory (`src`, `testSrc`, `main`,
`kotlin`, `java`, `resources`, …) and the folders its code and its tests share, its resources apart, a file right in
its root and under no source directory (its build file, its `*.iml`) taking no part, by their first two levels, the
most files first.

#### Scenario: The folders its code and tests share
- **WHEN** a module of Java or Kotlin changes files of the packages `com/example/player` under `src` and under `testSrc` alike
- **THEN** its `directories` start below those packages, by their first two levels, the most files first
- **AND** its resources stand apart (a bundle under `resources/messages` shares none of them), and its build file and its `*.iml` take no part
  [@test] crates/review-plan/src/modules_test.rs (`the_folders_of_a_module_start_below_the_packages_its_files_share`)

### Requirement: Modules: Each file has its folder by the same rule

Each file MUST have its `folder` by the same rule, all of its levels, "" in the folder its kind shares: the tree names
it after the file, so a test reads in the folder of its code.

#### Scenario: A test reads in the folder of its code
- **WHEN** the commits change `src/ui/embeds/View.kt` and `testSrc/ui/embeds/ViewTest.kt`
- **THEN** both have the `folder` `ui/embeds`, all of its levels, "" in the folder its kind shares
  [@test] crates/review-plan/src/modules_test.rs (`the_folders_of_a_module_start_below_the_packages_its_files_share`)

### Requirement: Modules: A module lists the modules it newly uses

A module MUST list the modules of the review it newly `uses`: by the links the commits add from its files to theirs
(`new_between_modules`), and `declared` when a dependency entry its build files add, or one a build file the commits
add declares, names that module; `usedBy` the reverse; `contentOf` the plugin modules whose descriptors the commits make
hold it as content; `newDependencies` the entries its build files add that name no module of the review.

#### Scenario: A new link and a new dependency entry
- **WHEN** the commits add links from the files of a module to those of another (`new_between_modules`), or a dependency entry that names it
- **THEN** the module `uses` the other, `declared` when an entry names it, and the other lists it in `usedBy`

#### Scenario: Content of a plugin and dependencies out of the review
- **WHEN** the commits make a plugin descriptor hold the module as content, or its build files add entries that name no module of the review
- **THEN** `contentOf` names the plugin modules, and `newDependencies` the entries

### Requirement: Modules: The modules come in the order of the review

The modules MUST come in the order of the review: one the commits add first, then the most changed lines of code and
tests, then tree order.

#### Scenario: An added module comes first
- **WHEN** the commits add a module and change others
- **THEN** the added one comes first, then the most changed lines of code and tests, then tree order
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)

### Requirement: Modules: A step names the new uses between modules it adds

A step MUST name the new uses between modules it adds (`moduleEdges`: `from`, `to` and the `links` of the step between
their files), each link in the step whose stop shows its first use, never a list of the pairs of files.

#### Scenario: A step with new links between modules
- **WHEN** a step adds links between the files of two modules
- **THEN** it names them as "view.gui.media → view.gui (38 links)", never a list of the pairs of files
- **AND** each link is in the step whose stop shows its first use
  [@test] crates/review-plan/src/story_test.rs (`a_step_lists_the_new_api_and_the_new_links_between_modules_it_holds`)

### Requirement: Modules: The modules read the trees in the batch of the summaries

The modules MUST read the trees above every changed file in the `cat-file --batch` of the summaries: a plan of build
files alone runs that batch too.

#### Scenario: A plan of build files alone
- **WHEN** a plan reads only build files
- **THEN** it reads the trees above them in the `cat-file --batch` of the summaries, which it runs too
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_build_files`)

### Requirement: Registrations: Every fact comes from accio's reader

Every fact of the registrations MUST come from accio's port of the platform's reader; the plan MUST parse no
descriptor of its own for it.

#### Scenario: The descriptors are read by accio
- **WHEN** the commits change plugin descriptors
- **THEN** what they register comes from accio's port of the platform's reader, as the platform reads them
- **AND** the plan parses no descriptor of its own for it

### Requirement: Registrations: The registrations the changed descriptors add, remove or change

`registrations` MUST list each service, extension point, listener, extension, registry key, action or group, and
content module that the changed plugin descriptors add, remove or change between the base and the head. A changed file
can be a descriptor by its path as in [Dependency Changes](#dependency-changes); only accio's answer of its root
(`idea-plugin`) MUST make it one.

#### Scenario: The registrations between the base and the head
- **WHEN** the commits change plugin descriptors
- **THEN** `registrations` lists what they add, remove or change between the base and the head
- **AND** only accio's answer of the root (`idea-plugin`) of a file makes it a descriptor
  [@test] crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)

### Requirement: Registrations: Each side is read by one accio process

Each side MUST be read by one `accio parse --descriptor --resolve --no-build --ref <side>` with the paths on its
standard input: at the base the files the commits do not add, at their first path, at the head those they do not
delete. The two MUST run at once, beside the usages and the tests of the plan.

#### Scenario: The descriptors of a side
- **WHEN** the plan reads the descriptors of the base and of the head
- **THEN** each side is one `accio parse --descriptor --resolve --no-build --ref <side>` with the paths on its standard input
- **AND** at the base the files the commits do not add, at their first path, at the head those they do not delete, the two at once beside the usages and the tests
  [@test] crates/file-diffs/src/accio_test.rs (`the_descriptors_of_a_side_are_read_in_one_process`)

### Requirement: Registrations: A registration is known by what identifies it to the platform

A registration MUST be known by what identifies it to the platform: a service by its area (`app`, `project`,
`module`), its interface (else its implementation), its client and its OS; an extension point by its area and its
qualified name (else its name); a listener by its area, its topic and its class; an action or a group by its `id`, else
its class, nested in a group too.

#### Scenario: A service by its interface and client
- **WHEN** a descriptor registers a service by its area (`app`, `project`, `module`), interface, client and OS
- **THEN** the service is known by them, its implementation only without an interface
  [@test] crates/review-plan/src/registrations_test.rs (`a_refused_descriptor_registers_nothing_and_a_service_is_known_by_its_interface_and_client`)

#### Scenario: Extension points, listeners, actions and groups
- **WHEN** a descriptor registers an extension point, a listener, an action or a group
- **THEN** the extension point is known by its area and its qualified name (else its name), the listener by its area, its topic and its class, the action or group by its `id`, else its class, nested in a group too
  [@test] crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)

### Requirement: Registrations: Registry keys, extensions and content modules are known by their key

An extension of `com.intellij.registryKey` MUST be known by its key (`registryKey`, its `defaultValue` the option
marked `*` of a key of `options`, its `description`); another extension by its point and its implementation, else its
`id`, else its whole element; a content module by its name (its `loading`, the `reviewModule` of that name).

#### Scenario: A registry key, an extension, a content module
- **WHEN** a descriptor registers a registry key, another extension or a content module
- **THEN** the key is known by `registryKey`, with its `defaultValue` and `description`, the extension by its point and its implementation, else its `id`, else its whole element, the content module by its name with its `loading`
  [@test] crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)

### Requirement: Registrations: Registrations of one key pair in their order

The registrations of one key MUST pair in their order. A pair whose other fields differ MUST be `changed`, with what it
was `before` and the fields it `changed`: an attribute by its name (`serviceImplementation`, `defaultValue`,
`loading`), a child element by its tag (`<add-to-group>`). The rest are `added` or `removed`; a descriptor the commits
add or delete adds or removes all it registers.

#### Scenario: A changed attribute and a changed child element
- **WHEN** a registration of one key has other fields after the commits
- **THEN** it is `changed`, with what it was `before` and the fields it `changed`, as `serviceImplementation`, `defaultValue`, `loading` or `<add-to-group>`
  [@test] crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)

#### Scenario: Added and removed registrations
- **WHEN** a registration has no pair, or a descriptor is added or deleted
- **THEN** it is `added` or `removed`, and the added or deleted descriptor adds or removes all it registers
  [@test] crates/review-plan/src/registrations_test.rs (`a_refused_descriptor_registers_nothing_and_a_service_is_known_by_its_interface_and_client`)

### Requirement: Registrations: A refused descriptor registers nothing

A side whose root is no `idea-plugin` MUST register nothing. A descriptor the platform's reader refuses on either side
MUST have no registration.

#### Scenario: A refused descriptor
- **WHEN** the platform's reader refuses a descriptor on either side, or a side's root is no `idea-plugin`
- **THEN** it has no registration
  [@test] crates/review-plan/src/registrations_test.rs (`a_refused_descriptor_registers_nothing_and_a_service_is_known_by_its_interface_and_client`)

### Requirement: Registrations: A registration names its descriptor, module and classes

Each registration MUST name its `descriptor` at its last path, the `module` of the descriptor, and the `classes` it
names, each as accio resolved it at the head (of a removed one, at the base): `declared` with its `path` and `line`,
`ambiguous` with the `count`, `notFound`.

#### Scenario: The classes as accio resolved them
- **WHEN** a registration names classes
- **THEN** each is `declared` with its `path` and `line`, `ambiguous` with the `count`, or `notFound`, as accio resolved it at the head (of a removed one, at the base)
  [@test] crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)

### Requirement: Registrations: Without an index or without accio the plan says why

Without a usable index accio MUST still read the descriptors: their classes are unknown (`null`), never `notFound`, and
the reason joins `index_note`. Without accio, or when accio fails, there MUST be no registration and `index_note` says
why.

#### Scenario: Without an index
- **WHEN** accio has no usable index
- **THEN** the registrations are listed with their classes unknown (`null`), never `notFound`, and the reason joins `index_note`
  [@test] crates/review-plan/src/registrations_test.rs (`without_an_index_the_registrations_are_listed_and_their_classes_unknown`)

#### Scenario: Without accio
- **WHEN** there is no accio, or accio fails
- **THEN** the plan lists no registration and `index_note` says why
  [@test] crates/review-plan/src/registrations_test.rs (`without_accio_a_plan_lists_no_registration_and_says_why`)

### Requirement: Registrations: The registrations come in the order of the modules

The registrations MUST come by the module of the descriptor in the order of `modules`, then the descriptor in tree
order, then the kind (service, extension point, listener, extension, registry key, action, content module), then the
order of the head, the removed ones last in the order of the base.

#### Scenario: The order of the registrations
- **WHEN** the plan lists registrations of several descriptors and kinds
- **THEN** they come by the module in the order of `modules`, the descriptor in tree order, the kind, then the order of the head, the removed ones last in the order of the base
  [@test] crates/review-plan/src/registrations_test.rs (`the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head`)

### Requirement: Registrations: Since Your Review compares the two ends of its scope

Since Your Review MUST compare the two ends of its scope, never what the reviewer already read; when its carry
conflicts, its commits are read one at a time, which no two revisions hold, and it MUST list no registration.

#### Scenario: The registrations of the commits added on top
- **WHEN** a plan since the review has commits added on top
- **THEN** its registrations are those between the two ends of its scope, never what the reviewer already read
  [@test] crates/review-plan/src/registrations_test.rs (`since_the_review_the_registrations_are_those_of_the_commits_added_on_top`)

#### Scenario: The carry conflicts
- **WHEN** the carry of Since Your Review conflicts and its commits are read one at a time
- **THEN** it lists no registration

### Requirement: Registrations: The plan sent while the index is asked has no registration

The plan sent while the index is asked MUST have no registration; the plan that follows has them. A change of
descriptors alone is a query to ask.

#### Scenario: The plan while the index is asked
- **WHEN** the plan is sent while the index is asked, also for a change of descriptors alone
- **THEN** it has no registration, and the plan that follows has them
  [@test] crates/review-plan/src/registrations_test.rs (`the_plan_sent_while_the_index_is_asked_has_no_registration`)

### Requirement: UI Texts: The keys the commits change in the message bundles

`texts` MUST list the keys the commits add to, remove from or change in the message bundles: a `*.properties` file in a
`messages` directory, or whose name without the extension ends with `Bundle` or `Messages`, at its path before or after
the commits. Another `*.properties` file (`gradle.properties`, the keys of a registry) MUST hold none.

#### Scenario: The keys of a bundle
- **WHEN** the commits change a `*.properties` file in a `messages` directory, or whose name without the extension ends with `Bundle` or `Messages`
- **THEN** `texts` lists the keys they add, remove or change
  [@test] crates/review-plan/src/texts_test.rs (`the_keys_a_bundle_adds_removes_or_changes_are_its_texts`)

#### Scenario: Another properties file
- **WHEN** the commits change another `*.properties` file (`gradle.properties`, the keys of a registry)
- **THEN** it holds no text
  [@test] crates/review-plan/src/texts_test.rs (`an_added_bundle_has_every_key_added_and_another_properties_file_none`)

### Requirement: UI Texts: A bundle is read as Java reads a properties file

A bundle MUST be read as Java reads a properties file: an entry per logical line, a line that ends with an odd number
of `\` going on in the next without its leading whitespace; a comment (`#`, `!`) and a blank line are no entry; the key
up to the first `=`, `:` or whitespace not escaped, the value after the separator and the whitespace around it; `\t`,
`\n`, `\r`, `\f` and `\uXXXX` (a surrogate pair too) unescaped; the last of a key given twice.

#### Scenario: The entries of a properties file
- **WHEN** a bundle has continued lines, comments (`#`, `!`), blank lines, escapes (`\t`, `\n`, `\r`, `\f`, `\uXXXX`, a surrogate pair too) and a key given twice
- **THEN** its entries are read as Java reads them, the last of a key given twice
  [@test] crates/review-plan/src/texts_test.rs (`the_entries_are_read_as_java_reads_a_properties_file`, `the_keys_a_bundle_adds_removes_or_changes_are_its_texts`)

### Requirement: UI Texts: A text is added, removed or changed between the ends

A text MUST be `added` (a key only after the commits), `removed` (only before them) or `changed` (its value differs),
read before the first commit that changes the bundle and after the last: a change of a comment, of a blank line or of
the lines a value is broken over alone is none, and neither is a value changed and changed back.

#### Scenario: Added, removed and changed keys
- **WHEN** the commits add a key, remove one, change the value of one, or change only a comment, a blank line or the lines a value is broken over
- **THEN** the texts are `added`, `removed` and `changed`, and the comment, the blank line and the broken lines are none
  [@test] crates/review-plan/src/texts_test.rs (`the_keys_a_bundle_adds_removes_or_changes_are_its_texts`)

#### Scenario: A value changed and changed back
- **WHEN** one commit changes a value and a later one changes it back
- **THEN** it is no text, as the bundle is read before the first commit that changes it and after the last
  [@test] crates/review-plan/src/texts_test.rs (`a_bundle_is_read_before_the_first_commit_and_after_the_last`)

### Requirement: UI Texts: An added bundle has every key added

A bundle the commits add MUST have every key `added`, one they delete every key `removed`.

#### Scenario: An added or deleted bundle
- **WHEN** the commits add a bundle, or delete one
- **THEN** every key is `added`, or every key `removed`
  [@test] crates/review-plan/src/texts_test.rs (`an_added_bundle_has_every_key_added_and_another_properties_file_none`)

### Requirement: UI Texts: Each text has its bundle, key, values, line and module

Each text MUST have its bundle (`path`, at its last path), its `key`, its values `before` and `after` (`null` on the
side without the key), its `line` (from 1, of its entry after the commits, before them for a removal) and the `module`
of its bundle by its place in `modules`; the bundles in tree order, then by line, then by key.

#### Scenario: The fields and the order of the texts
- **WHEN** the plan lists texts of several bundles
- **THEN** each has its `path`, `key`, `before` and `after` (`null` on the side without the key), `line` and `module`
- **AND** the bundles come in tree order, then by line, then by key
  [@test] crates/review-plan/src/texts_test.rs (`the_keys_a_bundle_adds_removes_or_changes_are_its_texts`, `an_added_bundle_has_every_key_added_and_another_properties_file_none`)

### Requirement: UI Texts: A bundle without its text on a side has no texts

A bundle without its text on a side (binary, too large) MUST have no texts, never every key removed.

#### Scenario: A bundle binary on a side
- **WHEN** a bundle is binary or too large on a side
- **THEN** it has no texts, never every key removed
  [@test] crates/review-plan/src/texts_test.rs (`a_bundle_binary_on_a_side_has_no_texts`)

### Requirement: UI Texts: brd review-plan prints texts

`brd review-plan` MUST print `texts`, in `brd schema`.

#### Scenario: The texts of the UI in the output
- **WHEN** `brd review-plan` runs on a range that changes a bundle
- **THEN** it prints `texts`, in `brd schema`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_texts_of_the_ui`)

### Requirement: UI Texts: The texts of the bundles come with the hunks

The texts of the bundles before and after the commits MUST come with the text of the hunks: from the memory of the
diffs, else in the `cat-file` of the summaries; the texts of the UI MUST run no process more.

#### Scenario: A plan of UI texts
- **WHEN** a plan reads the texts of the bundles
- **THEN** they come from the memory of the diffs, else in the `cat-file` of the summaries, and run no process more
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_ui_texts`)

### Requirement: Dependency Order: Every changed path is in the order once

`order` MUST list every path the reviewed commits change exactly once, at its last path: a renamed file at its new
path, a deleted one at its path (the review still shows it), none that the commits add and delete again.

#### Scenario: The paths of the order
- **WHEN** the reviewed commits change, rename, delete, or add and delete again files
- **THEN** `order` lists each changed path exactly once at its last path, a deleted one at its path, none that the commits add and delete again
  [@test] crates/review-plan/src/order_test.rs (`files_of_no_known_language_keep_the_tree_order`)

### Requirement: Dependency Order: A declaration comes before the file that uses it

A file that declares what another file of the order uses or imports MUST come before it, read from the files as the
last reviewed commit that changes each leaves it, in Kotlin, Java, C#, Rust and TypeScript.

#### Scenario: A file uses what another declares
- **WHEN** a file uses or imports what another file of the order declares, in Kotlin, Java, C#, Rust or TypeScript
- **THEN** the file that declares comes before it, read as the last reviewed commit that changes each leaves it
  [@test] crates/review-plan/src/order_test.rs (`a_declaration_comes_before_the_file_that_uses_it`, `a_cycle_keeps_its_tree_order`)

### Requirement: Dependency Order: A use links only with the evidence of the symbol dependencies

A use MUST link to a declaration only with the evidence the symbol dependencies ask (an import, a qualified name, the
same package or module): of two modules that declare one name, a file follows only the one it imports.

#### Scenario: A name of two modules
- **WHEN** two modules declare one name and a file imports one of them
- **THEN** the file follows only the one it imports
  [@test] crates/review-plan/src/order_test.rs (`a_name_of_two_modules_links_only_the_one_imported`)

### Requirement: Dependency Order: Files without an order between them keep the tree order

Files that use each other MUST keep their tree order among themselves, after what they use and before what uses them;
files with no edge between them MUST keep their tree order: at each level the directories before the files, then the
names without case, a number by its value.

#### Scenario: A cycle
- **WHEN** files use each other
- **THEN** they keep their tree order among themselves, after what they use and before what uses them
  [@test] crates/review-plan/src/order_test.rs (`a_cycle_keeps_its_tree_order`, `the_order_of_a_graph`)

#### Scenario: Files with no edge between them
- **WHEN** files have no edge between them
- **THEN** they keep their tree order: at each level the directories before the files, then the names without case, a number by its value
  [@test] crates/review-plan/src/order_test.rs (`the_tree_order_of_paths`)

### Requirement: Dependency Order: A test comes right after its subject

A test (as `files` tells one) MUST come right after its subject when the commits change it, also inside a cycle,
several in tree order; of two subjects of its stem the one it uses, else the one nearest in the tree. A test whose
subject the commits don't change is a file like any other.

#### Scenario: A test and its subject
- **WHEN** the commits change a test and its subject, also inside a cycle, several tests in tree order
- **THEN** the test comes right after its subject, of two subjects of its stem the one it uses, else the one nearest in the tree
- **AND** a test whose subject the commits don't change is a file like any other
  [@test] crates/review-plan/src/order_test.rs (`a_test_comes_right_after_its_subject`, `the_order_of_a_graph`)

### Requirement: Dependency Order: Files the summaries cannot read have no edge

A deleted file, a file of no language the summaries read and a degraded one MUST have no edge: a range of such files is
in tree order.

#### Scenario: Files of no known language
- **WHEN** a range has only deleted files, files of no language the summaries read and degraded ones
- **THEN** they have no edge and keep the tree order
  [@test] crates/review-plan/src/order_test.rs (`files_of_no_known_language_keep_the_tree_order`)

### Requirement: Dependency Order: brd review-plan prints order

`brd review-plan` MUST print `order`, in `brd schema`.

#### Scenario: The files in dependency order in the output
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `order`, in `brd schema`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_files_in_dependency_order`)

### Requirement: Dependency Order: The order runs no git process

The order MUST run no git process: it reads the summaries the API read.

#### Scenario: The processes of the order
- **WHEN** a plan of a range computes its order
- **THEN** it runs no git process for it, reading the summaries the API read
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_of_a_range`)

### Requirement: In the App: The app reads the plan and marks through the cores of brd

The app MUST read the plan with `get_review_plan` and mark a branch with `mark_branch_reviewed`, through the same
`review_plan_core` and `mark_reviewed_core` as `brd`, the branch named with the prefix the app resolves for the
repository.

#### Scenario: A plan since the review after a mark and a commit on top
- **GIVEN** a branch marked with `mark_branch_reviewed` and a commit on top
- **WHEN** the app reads its plan since the review with `get_review_plan`
- **THEN** the plan lists that commit as `added` with its scope
- **AND** the usages come from the index of the host (the fixture `.git/branch-deck-accio.json` in the e2e server)
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

### Requirement: In the App: The plan comes first without its usages, then with them

`get_review_plan` MUST send the plan on its channel: the plan without the usages of its API (`pending`, no file
counting `breaks`) before the index is asked, when there is a query to ask, then the plan with them; the window
replaces what it shows with each.

#### Scenario: A plan with a query to ask
- **WHEN** `get_review_plan` reads a plan with a query of the index to ask
- **THEN** it first sends the plan without the usages of its API (`pending`, no file counting `breaks`)
- **AND** then the plan with them, and the window replaces what it shows with each
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

#### Scenario: The words while the usages are pending
- **WHEN** the window shows a plan that is `pending`
- **THEN** it words an API without usages "Finding the uses…" and the summary "Finding possible breaks"
  [@test] src/presenters/review-plan.test.ts (`says the uses are being found while Rust asks the index`)

### Requirement: In the App: A repository with the flag off gets no plan

A repository whose `branchdeck.reviewExperimental` is off MUST get `disabled` and no plan from `get_review_plan`.

#### Scenario: The flag is off
- **GIVEN** a repository whose `branchdeck.reviewExperimental` is off
- **WHEN** the window asks `get_review_plan`
- **THEN** it gets `disabled` and no plan
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

### Requirement: In the App: A plan whose every query answered is kept in memory

A plan whose every query answered (`index_note` null) MUST be kept in the memory of the app by what it is of (the
repository, the prefix, the branch, `base`, `head`, the reviewed pair and `sinceReviewed`), at most 64 MiB of its
JSON and 10 minutes unread.

#### Scenario: The same plan asked again
- **GIVEN** a kept plan, read less than 10 minutes ago
- **WHEN** it is asked again
- **THEN** it comes whole in one event, with no git past the resolve of its subject and no query of the index
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

### Requirement: In the App: A subject that does not resolve is its own error

An unknown branch MUST answer `unknown_branch`; a base that names no commit, a branch that shares no history with its
base, an unknown revision of a range and `sinceReviewed` of a range MUST answer `invalid_arguments`.

#### Scenario: An unknown branch
- **WHEN** the plan of an unknown branch is asked
- **THEN** it answers `unknown_branch`
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

#### Scenario: A base, a history or a range that does not resolve
- **WHEN** a base names no commit, the branch shares no history with its base, a revision of a range is unknown, or a range is asked with `sinceReviewed`
- **THEN** it answers `invalid_arguments`

### Requirement: In the App: A window reads one plan at a time per slot

A window MUST read one plan at a time per slot, the plan of a scope (a branch, a range) and the plan of a commit
(`ReviewSubject::Commit`): a newer plan of the slot and the close of the window MUST supersede the plan in flight,
kill its queries of accio and answer `canceled`; the two slots, and a symbol search of the window, MUST never
supersede each other.

#### Scenario: The window closes
- **WHEN** the window closes while its plan is in flight
- **THEN** the plan is superseded, its queries of accio are killed and it answers `canceled`
  [@test] crates/app-api/src/session/windows_test.rs (`a_closed_window_supersedes_its_request`)

#### Scenario: The plan of a commit and the plan of its scope
- **WHEN** the window reads the plan of a commit (`ReviewSubject::Commit`) while the plan of its scope is in flight
- **THEN** neither supersedes the other
  [@test] crates/app-api/src/session/windows_test.rs (`the_plan_of_a_commit_and_the_plan_of_its_scope_never_supersede_each_other`)

#### Scenario: A plan and a symbol search
- **WHEN** the window runs a symbol search while its plan is in flight
- **THEN** neither supersedes the other
  [@test] crates/app-api/src/session/windows_test.rs (`a_review_plan_and_a_symbol_search_of_a_window_never_supersede_each_other`)

### Requirement: In the App: A plan is read once whoever asks it

A plan MUST be read once, whoever asks it: a window that asks the plan another window or the read ahead reads now
MUST wait for that reading and get its plan whole, in one event, without the plan of its pending usages; when that
reading ends without a plan, the window MUST read the plan itself and still get it.

#### Scenario: A window asks the plan being read ahead
- **WHEN** a window asks the plan another window or the read ahead reads now
- **THEN** it waits for that reading and gets its plan whole, in one event, without the plan of its pending usages
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_window_that_asks_the_plan_being_read_ahead_joins_its_reading`)

#### Scenario: The reading it waited for ends without a plan
- **WHEN** that reading ends without a plan (its window asked another, a newer commit superseded it)
- **THEN** the window reads the plan itself and still gets it
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_window_that_waited_for_a_superseded_read_ahead_still_gets_its_plan`)

### Requirement: In the App: A sync reads ahead the plans of the branches it moved

Once a sync commits the table of a repository (also one that moved nothing), the app MUST read ahead, in the
background and one at a time, the most recently asked first, the plan of each branch it watches (a review window asked
it since the app started, among the last 4, below) whose head the table moved and that no open window shows (that
window reads its own plan once it sees the new head).

#### Scenario: A commit moved a branch a window asked
- **GIVEN** a branch a review window asked the plan of, which no open window shows
- **WHEN** a sync commits a table that moved its head
- **THEN** the app reads its plan ahead once, in the background, the most recently asked first
- **AND** the window that opens the review again then finds the plan whole in the memory of the plans and asks the index nothing
- **AND** a commit that leaves the head there reads nothing again
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_commit_that_moved_a_branch_a_window_asked_reads_its_plan_ahead_once_below_the_app`)

#### Scenario: Each table a sync commits reaches the host
- **WHEN** a sync commits the table of a repository, also one that moved nothing
- **THEN** the host hears it
  [@test] crates/repository-model/src/session_test.rs (`the_host_hears_each_table_a_sync_commits`)

#### Scenario: A branch an open window shows, or that no window asked
- **WHEN** the table moves a branch that an open window shows, or that no window asked
- **THEN** it is not read ahead; the open window reads its own plan once it sees the new head
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`only_a_branch_a_window_asked_and_no_window_shows_is_read_ahead`)

### Requirement: In the App: The read ahead reads the plan as the window asked it

The read ahead MUST read the branch by the name the window asked it, on the stack parent whose head the window named as
its base, else on the baseline of the table, "Since Your Review" as asked.

#### Scenario: A stacked branch read ahead on its parent
- **GIVEN** a window that asked the plan of a stacked branch on the head of its parent
- **WHEN** the plan is read ahead
- **THEN** the base resolves to its parent by name, else to the baseline of the table, and "Since Your Review" is as asked
  [@test] crates/review-plan/src/mark_reviewed_test.rs (`the_base_of_a_stacked_branch_resolves_to_its_parent_by_name`)

### Requirement: In the App: What is never read ahead

A branch the table no longer has, or that no longer resolves, MUST be forgotten; a range, a commit and a kept branch
MUST never be read ahead; nothing is without `branchdeck.reviewExperimental` or an index on the machine, since a plan
without its usages is not kept. The read ahead SHALL have no memory of its own beside the memory of the plans: the
speed comes from accio ([Change Review](../../../../docs/design/change-review.md#prefetch-after-a-rewrite)).

#### Scenario: A gone branch, a range, a commit, a kept branch
- **WHEN** a watched branch is no longer in the table or no longer resolves, or the window asked a range, a commit or a kept branch
- **THEN** the gone branch is forgotten, and the range, the commit and the kept branch are never read ahead

#### Scenario: No flag or no index
- **WHEN** `branchdeck.reviewExperimental` is off or the machine has no index
- **THEN** nothing is read ahead, since a plan without its usages is not kept

### Requirement: In the App: The read ahead watches the 4 branches asked last

The read ahead MUST watch at most the 4 branches of a repository a window asked the plan of last: a branch asked again
becomes the newest, and asking a fifth one forgets the one asked longest ago, which no commit reads ahead any more.
`brd update` moves every branch at once, and the reviews opened again are mostly the recent ones.

#### Scenario: A fifth branch is asked
- **GIVEN** 4 watched branches
- **WHEN** a window asks the plan of a fifth one
- **THEN** the one asked longest ago is forgotten, and no commit reads it ahead any more
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_fifth_asked_branch_forgets_the_one_asked_longest_ago`)

#### Scenario: A watched branch asked again
- **WHEN** a watched branch is asked again
- **THEN** it becomes the newest

### Requirement: In the App: The queries of a plan read ahead run below the app

The queries of accio of a plan read ahead MUST run below the app, at nice 19 on Unix and the below-normal priority
class on Windows, which the processes accio starts inherit; those of a plan a window asks MUST run at the priority of
the app.

#### Scenario: A child in the background
- **WHEN** a child process starts in the background
- **THEN** it runs below the app, at nice 19 on Unix and the below-normal priority class on Windows
  [@test] crates/sync-utils/src/child_process_test.rs (`a_background_child_runs_below_the_app`)

#### Scenario: The queries of a plan read ahead
- **WHEN** a plan is read ahead
- **THEN** its queries of accio run at the lowest priority, and the processes accio starts inherit it
- **AND** those of a plan a window asks run at the priority of the app
  [@test] crates/file-diffs/src/accio_test.rs (`a_search_in_the_background_runs_its_queries_at_the_lowest_priority`)
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_commit_that_moved_a_branch_a_window_asked_reads_its_plan_ahead_once_below_the_app`)

### Requirement: In the App: A newer commit supersedes the read ahead in flight

A newer commit of the table of the same repository MUST supersede the read ahead in flight: its queries of accio are
killed, and the newer one reads what is left to read.

#### Scenario: A commit while a plan is read ahead
- **WHEN** a newer commit of the table of the same repository comes while a read ahead is in flight
- **THEN** its queries of accio are killed, and the newer one reads what is left to read
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_newer_commit_supersedes_the_read_ahead_in_flight`)

### Requirement: In the App: The review flag is on unless git config turns it off

`branchdeck.reviewExperimental` MUST be on unless git config turns it off (Settings → Branches → Review), read and
written like the other keys of Settings; a value git refuses MUST be the default, on, with its reason, and fail no
sync.

#### Scenario: Nothing set, or the config turns it off
- **WHEN** the config sets nothing, or turns `branchdeck.reviewExperimental` off
- **THEN** it is on when nothing is set, and off when the config turns it off
  [@test] crates/sync-core/src/repository_config_test.rs (`the_review_experiments_are_on_until_the_config_turns_them_off`, `nothing_set_is_the_defaults`)
  [@test] crates/sync-core/src/config_snapshot_test.rs (`the_review_experiments_are_on_unless_the_config_turns_them_off`)

#### Scenario: A value git refuses
- **WHEN** the value is one git refuses
- **THEN** it is the default, on, with its reason, and fails no sync

### Requirement: In the App: The Settings switch turns the review plan on and off

The switch "Review plan (experimental)" of Settings → Branches → Review MUST turn it on and off for every repository
in the global git config on "All repositories", and the repository's own `.git/config` value, written on its tab, MUST
win over it, as `branchdeck.resolveByWords` does (openspec/specs/repository/settings/spec.md).

#### Scenario: On and off for every repository
- **WHEN** the user turns "Review plan (experimental)" on or off on "All repositories"
- **THEN** it is written to the global git config for every repository
- **AND** the repository's own `.git/config` value, written on its tab, wins over it
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should turn the review plan on and off for every repository from Branches`)

### Requirement: In the App: The flag reaches the view through each sync

The flag MUST reach the view through each sync (`TableHeader.reviewExperimental`), and a change of it from Settings
MUST sync; with the flag off the review windows MUST look as they do without the review plan.

#### Scenario: The header of each grouping
- **WHEN** a sync groups the commits
- **THEN** `TableHeader.reviewExperimental` takes the flag, and a change of it from Settings syncs
  [@test] crates/repository-model/src/table_test.rs (`the_header_takes_the_review_experiments_of_each_grouping`)

#### Scenario: The flag is off
- **WHEN** the flag is off
- **THEN** the review windows look as they do without the review plan
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should look as it does without the review plan while the flag is off`)

### Requirement: In the App: The commands work without the flag

`brd review-plan` and `brd mark-reviewed` MUST work without the flag, and so MUST the two commands of the app.

#### Scenario: The flag is off
- **WHEN** the flag is off
- **THEN** `brd review-plan`, `brd mark-reviewed`, `get_review_plan` and `mark_branch_reviewed` still work

### Requirement: In the Review Window: With the flag off the window reads no plan

With the flag off, the branch review window MUST offer none of what follows and read no plan.

#### Scenario: The flag is off
- **GIVEN** a repository whose flag is off
- **WHEN** the branch review window opens
- **THEN** it offers none of the review plan and reads no plan
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should look as it does without the review plan while the flag is off`)

### Requirement: In the Review Window: The window reads the plan of the shown part

With the flag on, the window MUST read the plan of the shown part (`get_review_plan`): the reviewed branch on the base
of the part, the whole stack as its range; again when the head of the part moves, when "Since Your Review" is chosen
or left, and after Mark Reviewed; an answer `canceled` (a newer plan of the window) is dropped, and the plan of the
same branch stays while the next one is read.

#### Scenario: The plan is read again after Mark Reviewed and with Since Your Review
- **WHEN** the head of the part moves, "Since Your Review" is chosen or left, or Mark Reviewed runs
- **THEN** the window reads the plan of the shown part again with `get_review_plan`, the reviewed branch on the base of the part
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)

#### Scenario: The next plan of the same branch is read
- **WHEN** the window reads the next plan of the same branch, or gets an answer `canceled` (a newer plan of the window)
- **THEN** the `canceled` answer is dropped, and the plan of the same branch stays while the next one is read
  [@test] src/lib/review-plan-answer.test.ts (`replaces a plan without its usages by the next read's, and starts a read of another subject without a plan`)

### Requirement: In the Review Window: The plan of the same changes stays whole until the next one

The plan of the same changes, read again after Mark Reviewed, MUST stay whole until the next one comes with its
usages: the steps of the tree, its commit rows and the story of the diffs stay as they were; a plan of another head or
scope MUST give the tree and the diffs none of its facts.

#### Scenario: Mark Reviewed reads the plan again
- **WHEN** Mark Reviewed reads the plan of the same changes again
- **THEN** the steps of the tree, its commit rows and the story of the diffs stay as they were until the next plan comes with its usages
  [@test] src/lib/review-plan-answer.test.ts (`keeps the whole plan of the same changes while Mark Reviewed reads it again, until the next one has its usages`)

#### Scenario: The plan of another head or scope
- **WHEN** the plan of another head or scope is read
- **THEN** the tree and the diffs keep none of the facts of the plan before while the next one is read, but the plan of the branch for its card
  [@test] src/lib/review-plan-answer.test.ts (`keeps no facts of a plan of another head or scope while the next one is read, but the plan of the branch for its card`)

### Requirement: In the Review Window: The interdiff counts only for one branch at the head on screen

Its interdiff MUST count only while the part is one branch (the reviewed pair is on the base of that branch alone) and
the plan is of the head on screen: a part of several branches, or the plan of the head before a sync, MUST show no
"since" and offer no "Since Your Review".

#### Scenario: A part of several branches, or a plan of the head before a sync
- **WHEN** the part is of several branches, or the plan is of the head before a sync
- **THEN** the window shows no "since" and offers no "Since Your Review"
  [@test] src/lib/range-review.test.ts (`takes the interdiff only of the part's own branch, and only of a plan of the head on screen`)

### Requirement: In the Review Window: Mark Reviewed marks the branch of the part on its own base

Mark Reviewed of the title bar MUST mark the reviewed branch of the part on the base of its own branch, as
`brd mark-reviewed <branch> --base <parent>` does, say so, and read the plan again: nothing is new after it. The whole
stack MUST have no Mark Reviewed.

#### Scenario: Mark Reviewed of the title bar
- **WHEN** the user chooses Mark Reviewed of the title bar
- **THEN** the reviewed branch of the part is marked on the base of its own branch, as `brd mark-reviewed <branch> --base <parent>` does, and the window says so
- **AND** the plan is read again, and nothing is new after it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)

#### Scenario: The whole stack
- **WHEN** the review shows the whole stack
- **THEN** it has no Mark Reviewed

### Requirement: In the Review Window: Since Your Review is offered for one marked branch with news

"Since Your Review" MUST follow "All Changes" in the commit column only while the part is one branch, marked, with a
commit added, rewritten or reworded since (a message changed alone too); each such commit says "New", "Rewritten" or
"Reworded", on its row in the steps of the tree ([Review Tour](../review-tour/spec.md#in-the-review-window)) and in the
column of a commit shown alone.

#### Scenario: A branch never marked, or nothing added or rewritten
- **WHEN** the branch was never marked, or nothing was added or rewritten since the mark
- **THEN** "Since Your Review" is not offered
  [@test] src/lib/range-review.test.ts (`offers no scope for a branch never marked, nor when nothing was added or rewritten`)

#### Scenario: Added, rewritten and reworded commits
- **GIVEN** a marked branch with a commit added, one amended and one whose message alone changed since
- **WHEN** the window shows it
- **THEN** "Since Your Review" follows "All Changes" in the commit column
- **AND** the commits say "New", "Rewritten" and "Reworded", on their rows in the steps of the tree and in the column of a commit shown alone
  [@test] src/lib/range-review.test.ts (`marks the added and the rewritten commits, and lists only them, adjacent or not, under the scope`, `offers Since Your Review for a commit whose message alone changed, Reworded, and reads its plan with its reviewed commit`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit whose message alone changed as Reworded, what its body added and removed in its opening, the overview and its heading alone`)

### Requirement: In the Review Window: Since Your Review lists the new commits and shows the scope

Chosen, "Since Your Review" MUST make the tree list only the added and the rewritten commits, adjacent or not, and the
diffs show the scope: the commits added on top, or the diff from the carried commit, a rewrite against its reviewed
version and new commits apart together; without a scope (the carry conflicted) the newest of them. "All Changes",
another part or a mark MUST leave it.

#### Scenario: Commits added on top
- **GIVEN** a marked branch with commits added on top since
- **WHEN** the user chooses "Since Your Review"
- **THEN** the tree lists only those commits and the diffs show the plain range of the commits added on top
- **AND** a single new commit keeps its column
  [@test] src/lib/range-review.test.ts (`keeps the plain range of the commits added on top, and the column of one new commit`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)

#### Scenario: A rewritten commit and new commits apart
- **GIVEN** a marked branch whose reviewed commit was amended since
- **WHEN** the user chooses "Since Your Review"
- **THEN** the tree lists only the added and the rewritten commits, adjacent or not
- **AND** the diffs show the diff from the carried commit, a rewrite against its reviewed version and new commits apart together
  [@test] src/lib/range-review.test.ts (`marks the added and the rewritten commits, and lists only them, adjacent or not, under the scope`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)

#### Scenario: The carry conflicted
- **WHEN** the plan has no scope (the carry conflicted)
- **THEN** the diffs show the newest of the added and rewritten commits

#### Scenario: Leaving Since Your Review
- **WHEN** the user chooses "All Changes" or another part, or marks the branch
- **THEN** "Since Your Review" is left

### Requirement: In the Review Window: The overview heads the document of a scope and of a commit

The overview MUST head the document of a scope, and of a commit shown alone below its heading (a rewritten commit shows
its whole diff, the overview what it changes against its reviewed version); a commit shown alone is told by its own
plan, its steps, its story and its facts, under "Since Your Review" with what changed since of the part.

#### Scenario: The overview of a scope
- **WHEN** the review of a scope opens
- **THEN** the overview heads its document
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)

#### Scenario: A rewritten commit shown alone
- **WHEN** a rewritten commit is shown alone under "Since Your Review"
- **THEN** it shows its whole diff below its heading, the overview what it changes against its reviewed version
- **AND** it is told by its own plan, its steps, its story and its facts, with what changed since of the part
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)

### Requirement: In the Review Window: The overview keeps its place and says its state

The overview MUST keep a block above the headers and the messages while the diffs are read. While the plan is read it
says "Reading the review plan…", while Rust asks the index "Finding the uses left outside…", and "Cannot read the
review plan: …" when the read failed, in the review of a pull request too: no overview that looks whole and then moves
as the plan comes.

#### Scenario: The overview while the plan is read
- **WHEN** the plan is read, Rust asks the index, or the read failed
- **THEN** the overview keeps its block above the headers and the messages and says "Reading the review plan…", "Finding the uses left outside…" or "Cannot read the review plan: …"
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)

#### Scenario: The review of a pull request
- **WHEN** the review of a pull request shows the plan once the repository turns it on
- **THEN** its overview stands above its changes in the same way, its commits in the tree
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree`)

### Requirement: In the Review Window: The overview shows Possible Breakage, then Since your review

After the steps, the overview MUST show "Possible Breakage" in the warning tone, then "Since your review" as before (a
rewritten commit with the lines per path it changes against its reviewed version).

#### Scenario: The sections after the steps
- **WHEN** the overview lists its steps
- **THEN** "Possible Breakage" follows in the warning tone, then "Since your review", a rewritten commit with the lines per path it changes against its reviewed version
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)

### Requirement: In the Review Window: Since your review says what changed in each commit

"Since your review" MUST list each rewritten and each reworded commit with what changed: "Rewritten scale the helper:
src/Store.kt +1 −1", then " · message changed" when Rust says its message changed (`message_change`), "Reworded scale
the helper: message changed", a rewritten commit whose lines per path are the same "the same lines in another order";
a commit whose message did not change MUST say nothing of it.

#### Scenario: A rewritten commit
- **WHEN** a commit was rewritten since the review
- **THEN** its line says "Rewritten scale the helper: src/Store.kt +1 −1", per path what it changes that its reviewed version did not, and the reverse
- **AND** " · message changed" follows only when Rust says its message changed (`message_change`)
- **AND** a rewritten commit whose lines per path are the same says "the same lines in another order"
  [@test] src/presenters/review-plan.test.ts (`says per path what a rewritten commit changes that its reviewed version did not, and the reverse, and that its message changed only when Rust says so`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version`)

#### Scenario: A reworded commit
- **WHEN** only the message of a commit changed since the review
- **THEN** its line says "Reworded scale the helper: message changed"
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit whose message alone changed as Reworded, what its body added and removed in its opening, the overview and its heading alone`)

### Requirement: In the Review Window: A changed message opens to what Rust compared

A line with a change of the message MUST be a disclosure that opens to it, as Rust compared it: the subject it had
("Was …", struck through), then each hunk of its body in its order, its removed blocks muted and struck through with a
bar in the tone of a removal, then its added blocks with a bar in the tone of an addition. This is where Since Your
Review as one diff, which tells no opening, shows the message.

#### Scenario: The disclosure of a changed message
- **WHEN** the user opens the line of a commit whose message changed
- **THEN** it shows the subject it had ("Was …", struck through), then each hunk of its body in its order
- **AND** the removed blocks are muted and struck through with a bar in the tone of a removal, then the added blocks with a bar in the tone of an addition
  [@test] src/presenters/review-story.test.ts (`says in the overview how the message of a rewritten or a reworded commit changed, the blocks Rust compared under its line`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit whose message alone changed as Reworded, what its body added and removed in its opening, the overview and its heading alone`)

### Requirement: In the Review Window: The story marks a message changed since the review

Where the document tells the story of a commit whose message changed since the review (the openings of All Changes of
a marked branch, the heading of a commit shown alone with its reviewed commit, Since Your Review whose carry
conflicted), the story MUST mark it as Rust placed it ([Review Tour](../review-tour/spec.md#the-story-of-a-commit)). A
story whose message did not change MUST mark nothing.

#### Scenario: The marks of a changed body
- **WHEN** the story of a commit whose message changed since the review is told
- **THEN** an added block has the bar of an addition and "Added since your review" for a screen reader, a section whose label is new marked once whole
- **AND** the removed blocks are muted, struck through, with the bar of a removal and "Removed since your review", where they stood
- **AND** the subject it had stands below its title ("Was …")
- **AND** the bars stand in the gutter, so the text of a marked block stays at the edge of the others
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit whose message alone changed as Reworded, what its body added and removed in its opening, the overview and its heading alone`)
  [@test] src/presenters/review-story.test.ts (`marks what changed in a body since the review where Rust placed it: the added blocks, the removed ones before the block they stood before or at the end of their part, the subject it had`)

#### Scenario: A commit shown alone under Since Your Review
- **WHEN** a commit is shown alone under Since Your Review
- **THEN** it is told by its own plan: its story, its steps and the marks back to it
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell a commit shown alone under Since Your Review by its own plan: its story, its steps and the marks back to it`)

#### Scenario: A message that did not change
- **WHEN** the message of a commit did not change, or only the baseline moved
- **THEN** the story marks nothing, and nothing is counted as changed since the review
  [@test] src/presenters/review-plan.test.ts (`counts what changed since the review, and nothing when only the baseline moved`, `says per path what a rewritten commit changes that its reviewed version did not, and the reverse, and that its message changed only when Rust says so`)

### Requirement: In the Review Window: Possible Breakage lists each API whose uses can break

"Possible Breakage" MUST list each removed or changed API, in the order Rust ranked it, unless no use left outside
breaks: its kind and its whole name, wrapping at its dots; under them `path:line` (a long path losing its start), the
parts that changed ("parameters and visibility changed") and the uses left outside that can break ("3 uses left
outside").

#### Scenario: A removed or changed API with uses left outside
- **WHEN** the plan has a removed or changed API with uses left outside that can break
- **THEN** "Possible Breakage" lists it in Rust's order: its kind and its whole name, wrapping at its dots
- **AND** under them `path:line` (a long path losing its start), the parts that changed ("parameters and visibility changed") and the uses left outside that can break ("3 uses left outside")
  [@test] src/presenters/review-plan.test.ts (`lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)

### Requirement: In the Review Window: An entry shows its forms as code reads

An entry MUST show the form before and the form after on lines of their own, wrapping, never cut, as code reads: a
constructor as a call of its type, a line each, one not public after its visibility; a parameter whose type has
several names without its type, which Rust keeps only as those names; never the name of a `Reach` nor `List/Window`.

#### Scenario: The forms of a constructor
- **WHEN** an entry is a changed constructor
- **THEN** its form before and its form after stand on lines of their own, wrapping, never cut, as a call of its type, a line each: `Event(text: String, styles, start: Long = …)`
- **AND** one not public stands after its visibility
- **AND** a parameter whose type has several names stands without its type, and never the name of a `Reach` nor `List/Window`
  [@test] src/presenters/review-plan.test.ts (`lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites`)

### Requirement: In the Review Window: An entry lists its first sites and the rest in one line

After its forms, an entry MUST list the first sites a line each, its file name and line with its directory muted after
it and its `path:line` in its hover, and in one line the rest: the uses updated in this change, the calls left outside
that still fit the new form ("4 calls left outside still fit the new form") and the hits only the name ties ("2
matches by name only, not checked").

#### Scenario: The sites of an entry
- **WHEN** an entry has uses left outside
- **THEN** its first sites stand a line each, the file name and line with the directory muted after it, its `path:line` in its hover
- **AND** one line says the rest: the uses updated in this change, "4 calls left outside still fit the new form" and "2 matches by name only, not checked"
  [@test] src/presenters/review-plan.test.ts (`lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites`)

### Requirement: In the Review Window: An unknown or incomplete count is a lower bound

An entry with `usages` null MUST say "The uses are unknown". An entry whose count is not `complete` MUST say "At least N
uses left outside", or "No use left outside found, the search stopped early" at 0. An unknown, incomplete or by-name
count MUST never read as safe.

#### Scenario: The uses are unknown
- **WHEN** an entry has `usages` null, as accio did not answer
- **THEN** it stays under Possible breakage and says "The uses are unknown"
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep an API whose uses are unknown under Possible breakage, and say once above the map why accio did not answer`)
  [@test] src/presenters/review-plan.test.ts (`words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown`)

#### Scenario: A count that is not complete
- **WHEN** the count of an entry is not `complete`
- **THEN** it says "At least N uses left outside", or "No use left outside found, the search stopped early" at 0
- **AND** it stays listed, never folded as safe, also at 0
  [@test] src/presenters/review-plan.test.ts (`words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown`, `folds only the API none of whose uses left outside breaks, and keeps an unknown, incomplete or by-name count listed, also at 0`)

### Requirement: In the Review Window: What breaks nothing folds into one muted line

Only the entries Rust says are `unbroken` and the widenings (`widened`, the badge "Opens wider") MUST fold into one
collapsed line, muted, never in the warning tone: "No use left outside breaks · 12 API changes" where the `verdict` of
the plan is `noUseBreaks`, the API line of the map then ([The Map](../review-tour/spec.md#the-map)), else "12 API
changes, no use left outside breaks" under the entries listed. Another addition MUST not be listed there.

#### Scenario: The verdict that no use left outside breaks
- **WHEN** the `verdict` of the plan is `noUseBreaks`
- **THEN** the folded line leads with the verdict, "No use left outside breaks · 12 API changes", the API line of the map then
  [@test] src/presenters/review-plan.test.ts (`folds a widening with what breaks nothing, and leads the line with the verdict when no use left outside breaks`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)

#### Scenario: Entries listed and some folded
- **WHEN** some entries are listed and others are `unbroken` or `widened`
- **THEN** the others fold into one collapsed, muted line under the entries listed, "12 API changes, no use left outside breaks", never in the warning tone
- **AND** only `unbroken` entries and widenings fold; another addition is not listed there
  [@test] src/presenters/review-plan.test.ts (`folds only the API none of whose uses left outside breaks, and keeps an unknown, incomplete or by-name count listed, also at 0`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)

### Requirement: In the Review Window: A site of the uses opens its file at the head of the review

A click on a site of the uses left outside MUST open its file at the head of the review over the diffs, as Go to
Declaration opens a declaration outside the review, at the line of the use with the name of the API selected
(`get_outside_place`: one `cat-file --batch-check` and one `cat-file --batch`, no index). The file covers the
document, the overview too, until Back.

#### Scenario: A click on a site
- **WHEN** the user clicks a site of the uses left outside
- **THEN** its file opens at the head of the review over the diffs, at the line of the use with the name of the API selected
- **AND** `get_outside_place` reads it with one `cat-file --batch-check` and one `cat-file --batch`, no index
- **AND** the file covers the document, the overview too, until Back
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] crates/file-diffs/src/outside_place_test.rs (`a_use_outside_the_review_opens_at_its_line_with_its_name_selected`)

#### Scenario: A file or a line the head lacks
- **WHEN** the head lacks the file or the line of the site
- **THEN** the window says "Cannot open src/main/App.kt:9" with "The head of the review has no such line"

### Requirement: In the Review Window: The overview lists no dependency change and no repeated edit

The overview MUST list no dependency change and no repeated edit: they are on the rows of the tree, the mark of a row
and the divider "Same edit in 3 files" of the story.

#### Scenario: A change with dependency changes and a repeated edit
- **WHEN** the commits change dependencies and repeat an edit
- **THEN** the overview lists neither; they are on the rows of the tree, the mark of a row and the divider "Same edit in 3 files" of the story
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)
  [@test] src/presenters/review-story.test.ts (`words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

### Requirement: In the Review Window: The new API, the widenings and the new links are facts of a step

The new API (each added declaration), the declarations opened wider ("Opens wider: format", no new API) and the links
the commits added between the files of two modules (`new_between_modules`, `Reader.kt → Cache.kt` at its first use)
MUST be facts of the step that holds them, each name at its place, of the range when the overview lists no step.

#### Scenario: The facts of a step
- **WHEN** a step holds an added declaration and a new link between the files of two modules
- **THEN** they are facts of that step: the new API and `new_between_modules`, `Reader.kt → Cache.kt` at its first use, each name at its place
  [@test] src/presenters/review-story.test.ts (`opens a step of the commits with the body of the commit that starts it, its follow-ups and the commits of no story, its facts with the new API and the new links between modules at their places`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)

#### Scenario: A declaration opened wider
- **WHEN** a step opens a declaration wider
- **THEN** its fact says "Opens wider: format", no new API, and the name opens at its declaration
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)

#### Scenario: An overview with no step
- **WHEN** the overview lists no step
- **THEN** these facts are of the range

### Requirement: In the Review Window: An entry of the overview shows its place in the diffs

A click on an entry of the overview MUST select its file in the review and show its place in the diffs, its name
selected, as a link of the card of a row's mark does: the declaration of an API (a removal on the side before the
commits, by its old path too), the first use of a link between modules in the file that uses, a declaration of the new
API; a file of What Changes at its header.

#### Scenario: A click on an entry
- **WHEN** the user clicks an API, a link between modules, a new declaration or a file of What Changes in the overview
- **THEN** its file is selected in the review and its place shown in the diffs, its name selected
- **AND** a removal shows on the side before the commits, by its old path too; a file of What Changes at its header
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)

### Requirement: In the Review Window: An entry of the overview reveals a file a filter hides

A search that hides the file of an entry MUST be cleared as for a link of a mark, also when it hides every file, and
the place shown once the file is; an owner chip filters the files, and a link to a file it hides MUST clear the chip
before selecting that file ([Branch Review](../branch-review/spec.md#code-owners)). A chip that already keeps the
linked file MUST stay selected.

#### Scenario: A text search hides the file, the owner chip keeps it
- **GIVEN** a text search that hides the file of an entry and an owner chip that keeps it
- **WHEN** the user clicks the entry
- **THEN** the search is cleared, the place is shown once the file is, and the chip stays selected
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should reveal an overview file under a text search while retaining its owner chip`)

#### Scenario: An owner chip hides the file
- **WHEN** an owner chip hides the file of an entry the user clicks
- **THEN** the chip is cleared before that file is selected

### Requirement: In the Review Window: The tree lists the concerns of the plan

The tree MUST list the concerns of the plan, in All Changes and in "Since Your Review" as one range, and the story of
the diffs follow them ([Review Concerns](../review-concerns/spec.md#in-the-review-window)); with no concern but the
rest, the files stay in tree order, the plan adds the marks of the rows and the Mechanical group. There SHALL be no
switch of the order: one tree tells the finished review.

#### Scenario: The tree of a plan
- **WHEN** the plan of the shown scope is read
- **THEN** the tree lists its concerns and the story of the diffs follows them, the files kept in the tree
- **AND** with no concern but the rest, the files stay in tree order with the marks of the rows and the Mechanical group
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree`)

### Requirement: In the Review Window: The commits of a scope stand in its steps

The tree of a scope MUST list the commits in its steps, the column above it only the scopes; a commit shown alone shows
the tree of its own plan, the column every commit. The facts MUST apply once the plan of the shown scope is read, not
while the next one is read: until then the column lists every commit.

#### Scenario: A scope and a commit shown alone
- **WHEN** the review shows a scope, then a commit alone
- **THEN** the tree of the scope lists its commits in its steps and the column only the scopes
- **AND** the commit shown alone shows the tree of its own plan and the column every commit

#### Scenario: While the next plan is read
- **WHEN** the plan of the shown scope is not read yet
- **THEN** the facts do not apply and the column lists every commit

### Requirement: In the Review Window: A row shows at most one mark, warning only when it breaks

A row of the tree MUST show at most one small mark, where its facts are: those of its file and the dependency changes
the row carries. The mark MUST be in the warning tone when the file breaks uses left outside the change (`breaks N` or
`breaks N+`), and show always; else muted, shown only on the row under the pointer, the selected and the focused row and
while its card is open, its place kept so no name moves.

#### Scenario: A file that breaks uses outside
- **WHEN** the file of a row breaks uses left outside the change
- **THEN** its one mark is in the warning tone (`breaks N` or `breaks N+`) and shows always
  [@test] src/presenters/review-plan.test.ts (`merges the facts of a row into one mark, in the warning tone only when it breaks uses outside, and its card sections what can break first`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)

#### Scenario: Other facts and the dependency changes of a row
- **WHEN** a row has other facts, or carries dependency changes
- **THEN** they merge into the same muted mark, after the facts of its file, shown only on the row under the pointer, the selected and the focused row and while its card is open, its place kept so no name moves
- **AND** a mark on most rows of a change would tell nothing: the warning is the one signal of the tree
  [@test] src/presenters/review-plan.test.ts (`puts the dependency changes a row carries into the same mark, after the facts of its file`)

### Requirement: In the Review Window: The hover of a mark opens its card

Details MUST be on demand: the hover of the mark (300 ms, only after a move of the pointer) MUST open its card, with
the sections that apply in this order: Breaks, Uses, Used by, Same edit, Dependencies. The mark is no tab stop: the
description of the row MUST say every fact in words (`uses 1, +1 new, breaks 3, new module`).

#### Scenario: The card of a mark
- **WHEN** the pointer moves onto a mark and stays 300 ms
- **THEN** its card opens with the sections that apply in this order: Breaks, Uses, Used by, Same edit, Dependencies
  [@test] src/presenters/review-plan.test.ts (`merges the facts of a row into one mark, in the warning tone only when it breaks uses outside, and its card sections what can break first`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)

#### Scenario: The description of the row
- **WHEN** a screen reader reads a row with a mark
- **THEN** the description of the row says every fact in words (`uses 1, +1 new, breaks 3, new module`), as the mark is no tab stop

### Requirement: In the Review Window: The keyboard opens the card of a mark by I

The keyboard MUST reach the card: `I` on a focused row with a mark MUST open its card with the focus on its first file,
on the card when it lists none; Enter on a file selects it as a click does. Escape MUST close the card alone, not the
window or the review, and give the focus back to the row, as a file chosen from a card opened by `I` does. The key
hints MUST list `I` while a row of the tree has the focus.

#### Scenario: I, Enter and Escape on a card
- **GIVEN** a focused row with a mark
- **WHEN** the user presses `I`
- **THEN** its card opens with the focus on its first file, on the card when it lists none
- **AND** Enter on a file selects it as a click does, and the focus goes back to the row
- **AND** Escape closes the card alone, not the window or the review, and gives the focus back to the row
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the card of a row's mark by I on the focused row, the focus on its first file, Enter selecting it and Escape giving the focus back to the row`)

#### Scenario: The key hints
- **WHEN** a row of the tree has the focus
- **THEN** the key hints list `I`

### Requirement: In the Review Window: A card opened by I that goes otherwise gives the focus back

A card opened by `I` that goes otherwise, the hover of another mark replacing it or the pointer leaving its mark, MUST
give the focus back to the row too, unless the focus went elsewhere (a click, Tab out); a card goes with its row.

#### Scenario: Another mark's hover or the pointer leaving
- **GIVEN** a card opened by `I`
- **WHEN** the hover of another mark replaces it, or the pointer leaves its mark
- **THEN** the focus goes back to the row, unless it went elsewhere (a click, Tab out)
- **AND** Escape closes only the card
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should give the focus back to the row when a card opened by I goes by the hover of another mark or the pointer leaving, and close only the card on Escape`)

#### Scenario: The row goes
- **WHEN** the row of an open card goes
- **THEN** the card goes with it

### Requirement: In the Review Window: The card of a file lists its links

The card of a file MUST say `Uses N · +K new` and `Used by M`, the links among the changed files after the commits,
`+K new` for the links it uses that the commits added. It MUST list each link it uses, then each link that uses it, a
removed one too: the other file, the names and the line of the first use (`Cache, get · Reader.kt:3`), and the state
Added, Removed or Kept.

#### Scenario: The links of a file
- **WHEN** the card of a file with links among the changed files opens
- **THEN** it says `Uses N · +K new` and `Used by M`, `+K new` for the links it uses that the commits added
- **AND** it lists each link it uses, then each link that uses it, a removed one too, with the other file, the names and the line of the first use (`Cache, get · Reader.kt:3`) and the state Added, Removed or Kept
  [@test] src/presenters/review-plan.test.ts (`counts the links among the changed files, the new ones apart, and lists each with its names, its line, its state and its place`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)

### Requirement: In the Review Window: A link of a card shows the linked file

A click on a link MUST select that file, close the card and show the file: a file that uses this one at its first use,
the name selected, a file it uses at its header.

#### Scenario: A click on a link
- **WHEN** the user clicks a link in the card of a file
- **THEN** that file is selected, the card closes and the file is shown: a file that uses this one at its first use, the name selected, a file it uses at its header
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)

### Requirement: In the Review Window: A file of a card is selected also under a filter

A click on a file in a card MUST select it also when a filter hides it: a text search is cleared only when it hides the
file, and an owner chip is cleared only when the file belongs to another owner. A filter that keeps the file stays.
The click selects the file once: a later change of the owner chip or the search MUST not select it again.

#### Scenario: A file of another owner under a search that hides it
- **GIVEN** an owner chip of another owner and a text search that hides the linked file
- **WHEN** the user clicks the file in a card
- **THEN** the chip and the search are cleared and the file is selected
- **AND** a later change of the owner chip or the search does not select it again
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should reveal a linked file from another owner by clearing its chip and clear a search that hides the target`)

#### Scenario: A filter that keeps the file
- **GIVEN** an owner chip that keeps the file and a text search that hides it
- **WHEN** the file is revealed
- **THEN** the search is cleared and the chip stays
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should reveal an overview file under a text search while retaining its owner chip`)
  [@test] src/presenters/review-plan.test.ts (`counts the links among the changed files, the new ones apart, and lists each with its names, its line, its state and its place`)

### Requirement: In the Review Window: The card says what breaks, as a lower bound when cut

The card of a file MUST say under Breaks `breaks N` in the warning tone and why, for the uses left outside the change
of the API it removes or changes. When `breaks_complete` is false it MUST say `breaks N+` ("At least 3 uses left
outside the change: the search stopped early"), or a quiet `breaks ?` at 0, whose mark is muted: a lower bound MUST
never read as safe.

#### Scenario: A file that breaks uses outside
- **WHEN** the file removes or changes an API with uses left outside the change
- **THEN** its card says under Breaks `breaks N` in the warning tone and why
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use`)
  [@test] src/presenters/review-plan.test.ts (`says the uses an API leaves outside, as a lower bound where Rust says the count is, never a safe 0`)

#### Scenario: A count that is not complete
- **WHEN** `breaks_complete` is false
- **THEN** the card says `breaks N+` ("At least 3 uses left outside the change: the search stopped early"), or a quiet `breaks ?` at 0, whose mark is muted
  [@test] src/presenters/review-plan.test.ts (`says the uses an API leaves outside, as a lower bound where Rust says the count is, never a safe 0`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep an API whose uses are unknown under Possible breakage, and say once above the map why accio did not answer`)

### Requirement: In the Review Window: A row has no no-test mark

A row MUST have no `no test`: the tests that name the code of a change are those of its step
([Review Tour](../review-tour/spec.md#tests-that-name-it)); `untested` stays in `brd review-plan`.

#### Scenario: A file changed without its tests
- **WHEN** a file of the tree changed without its tests
- **THEN** its row shows no `no test` and lists no badge for it; `untested` stays in `brd review-plan`
  [@test] src/presenters/review-plan.test.ts (`lists no file without a badge`)

### Requirement: In the Review Window: The card of a repeated edit names the edit and its files

The card of a file of a repeated edit MUST say `Same edit ×N`, name the edit (the first line removed and the first
added, `a → b`) and list the other files of the group; a click selects one.

#### Scenario: A file of a repeated edit
- **WHEN** the card of a file of a repeated edit opens
- **THEN** it says `Same edit ×N`, names the edit, `a → b`, and lists the other files of the group
- **AND** a click on one selects it
  [@test] src/presenters/review-plan.test.ts (`names the same edit of a group of files and the other files that get it`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)

### Requirement: In the Review Window: The mechanical files fold into one group at the end

The mechanical files MUST leave the tree for one group at its end, `Mechanical (N)` (a generated file that follows its
source in the group of its concern, [Review Concerns](../review-concerns/spec.md#in-the-review-window)), with a group
per kind in the order Build files, Imports only, Whitespace only, Renamed, Generated (the order of the mechanical
units, which the page takes from the plan), each file by its path.

#### Scenario: The Mechanical group
- **WHEN** the plan has mechanical files
- **THEN** they leave the tree for one group at its end, `Mechanical (N)`, a group per kind, each file by its path
- **AND** a generated file that follows its source stays in the group of its concern
  [@test] src/lib/file-tree.test.ts (`folds the mechanical files into one group at the end, a group per kind, each file by its path; J and K reach them last`)
  [@test] src/presenters/review-plan.test.ts (`names the Mechanical group and its groups by kind`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)

#### Scenario: The order of the kinds
- **WHEN** the group holds files of several kinds
- **THEN** the kinds stand in the order Build files, Imports only, Whitespace only, Renamed, Generated, the order of the units of the plan
  [@test] src/lib/file-tree.test.ts (`orders the kinds as the units of the plan, the order Rust gave the story`)

### Requirement: In the Review Window: The Mechanical group starts collapsed

The group MUST start collapsed, also when the plan comes after the files, unless it holds the selected file. A file
selected only as the first of the tree without the plan, which may be a generated one, MUST give way to the first file
of the tree with the plan.

#### Scenario: The plan comes after the files
- **WHEN** the plan comes after the files
- **THEN** the Mechanical group starts collapsed unless it holds the selected file, and it opens to reveal a file in it
- **AND** a file selected only as the first of the tree without the plan gives way to the first file of the tree with the plan
  [@test] src/lib/file-tree.test.ts (`starts with the Mechanical group collapsed, and opens it to reveal a file in it`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)

### Requirement: In the Review Window: The diffs, J and K reach the mechanical files last

The diffs, J and K MUST reach these files last, and a file selected in the group opens it. Its menu and ⌘C MUST copy
its changed paths.

#### Scenario: J and K
- **WHEN** the user moves through the files with J and K
- **THEN** the files of the Mechanical group are reached last, and a file selected in it opens the group
  [@test] src/lib/file-tree.test.ts (`folds the mechanical files into one group at the end, a group per kind, each file by its path; J and K reach them last`)

#### Scenario: Copy the paths of the group
- **WHEN** the user copies the group from its menu or with ⌘C
- **THEN** its changed paths are copied, as the group has no path of its own
  [@test] src/lib/file-tree.test.ts (`copies the paths of a group, which has no path of its own`)

### Requirement: In the Review Window: A mechanical change starts collapsed and says why

The diffs MUST be the story of the units ([Review Units](../review-units/spec.md#in-the-review-window)): a mechanical
change starts collapsed, its header saying why ("Build file", "Imports only", "Whitespace only", "Renamed only"; a
generated file what its name makes it, "Lock file", "Minified", "Source map", "Generated (.gitattributes)", or the line
its tool wrote, cut at 100 characters). The chevron of the header opens it.

#### Scenario: A generated file
- **WHEN** the diffs show a generated file
- **THEN** it starts collapsed, its header saying why: what its name makes it ("Lock file", "Minified", "Source map"), "Generated (.gitattributes)", or the line its tool wrote, "Generated by tools/render.mjs; edit src/guide.md", cut at 100 characters
- **AND** a rendering stands at the end of the concern of its source
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)
  [@test] src/presenters/review-units.test.ts (`fold each with why it is generated: its name, its attributes, the line its tool wrote`)
  [@test] src/presenters/review-plan.test.ts (`says why a generated file is folded: its name, its attributes, the line its tool wrote, a long one cut`)

#### Scenario: Another mechanical change
- **WHEN** the diffs show a build file, a change of the imports or of whitespace only, or a rename
- **THEN** it starts collapsed, its header saying "Build file", "Imports only", "Whitespace only" or "Renamed only"
- **AND** the chevron of the header opens it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)

### Requirement: In the Review Window: A repeated edit shows once

A repeated edit MUST show once in the diffs, the other files of it collapsed, "Same edit as Alpha.kt"; the chevron of
the header opens either.

#### Scenario: The files of a repeated edit
- **WHEN** the diffs show the files of a repeated edit
- **THEN** the edit shows once under its group, the other files of it collapsed, "Same edit as Alpha.kt", the chevron of the header opening either
- **AND** a file's own change stands in its section with a row to the edit
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)

### Requirement: In the Review Window: A block of the imports alone is played down

A block whose every changed line of each side is an import or a package statement or blank (`TextDiff::import_blocks`,
from the syntax of the texts) MUST be pale, as a change of whitespace alone, and N and P MUST pass it; in the story it
is in the unit of the imports, and the section of its file shows one row for it. A block with a line of code beside
them is a change as any.

#### Scenario: A block of the imports alone
- **WHEN** every changed line of each side of a block is an import or a package statement or blank
- **THEN** the block is pale and N and P pass it; elsewhere than a review it is a change as any
- **AND** in the story it is in the unit of the imports, and the section of its file shows one row that opens it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)
  [@test] src/lib/diff/rows.test.ts (`plays down a block of the imports alone in a review: pale, and N and P pass it; elsewhere it is a change as any`)

#### Scenario: The statements that are imports
- **WHEN** `TextDiff::import_blocks` reads the syntax of the texts
- **THEN** the import and package statements of Kotlin, Java, Rust and TypeScript and the `using` directives and the `namespace` line of C# are imports
- **AND** a Rust `pub use` and a TypeScript `export … from` are none
  [@test] crates/file-diffs/src/service_test.rs (`a_block_of_import_statements_alone_is_marked_by_the_syntax`)
  [@test] crates/code-symbols/src/summary_test.rs (`the_import_lines_are_the_import_and_package_statements`)

#### Scenario: A line of code beside the imports
- **WHEN** a block has a line of code beside the imports
- **THEN** it is a change as any

### Requirement: In the Review Window: The dependency changes show on the row that holds the build file

The dependency changes of a build file MUST show on the row of the concern that holds the build file (it follows the
code that needs the dependency, [Review Concerns](../review-concerns/spec.md#the-rest)), else on the directory row whose
chain holds the directory of the build file, else on the row of the build file (one at the root), which the Mechanical
group stands for while it folds the build file, so that a collapsed group hides no dependency change.

#### Scenario: The row of the dependency changes
- **WHEN** a build file changes dependencies
- **THEN** they show on the row of its concern, else on the directory row of its module, else on the row of the build file, the Mechanical group that folds it
  [@test] src/lib/file-tree.test.ts (`shows the dependency changes of a build file on the row of its concern, else on the directory row of its module, else on the Mechanical group that folds it`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module`)

### Requirement: In the Review Window: The card of the dependency changes lists each change

The mark of that row MUST have a card that says `Dependencies · +N deps −M ~K` (`new module`, `deleted module`) and
lists each change, a changed version as `1.2 → 1.3` and a changed scope as `test → main`.

#### Scenario: The card of a module's dependency changes
- **WHEN** the card of the row with the dependency changes opens
- **THEN** it says `Dependencies · +N deps −M ~K` (`new module`, `deleted module`) and lists each change
- **AND** a changed version is one changed entry with both versions, `1.2 → 1.3`, and a changed scope `test → main`
  [@test] src/presenters/review-plan.test.ts (`counts the dependency changes of a module, a version change as one changed entry with both versions`, `words a version, a scope and a new module`)

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
