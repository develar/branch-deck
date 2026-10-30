---
name: Review Plan
description: Requirements for the plan of a review (experimental), what a reviewer of a branch needs first; its start is Mark Reviewed, the base and the tip of a local branch kept in `refs/branch-deck/reviewed/`, the interdiff of its next review (`bd review-plan`), the commits since then, and the API surface the reviewed commits change with its fan-in from accio, the facts of each file (its links to the other changed files, a mechanical change, a repeated edit, a missing test), the dependency changes of the build files and the files in dependency order.
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
  - ../../src/hooks/useReviewPlan.ts
  - ../../src/hooks/useBranchReview.ts
  - ../../src/presenters/review-plan.ts
  - ../../src/components/review/ReviewPlanCard.tsx
  - ../../src/components/review/BranchReview.tsx
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
dropped, and the plain range of the commits added on top (`bd review-plan`). Then the API surface: the public
declarations the reviewed commits add, remove or change in form, ranked by their usages at the head of the range from
accio, and the facts of each changed file: which other changed files it uses and which use it, whether its change
is mechanical, the edit it shares with other files, the usages of the API it removes or changes, and whether it
changed without its tests. Then the dependency changes: the module, library, plugin, crate and package entries the
commits add to, remove from or change in the build files, and the build files they add or delete. Then the order of
the files: a file that declares what another uses before it, a test right after its subject.

## Goals

- Remember, per local branch, the base and the tip the user reviewed, with one command and one write.
- Keep the reviewed commits reachable, so that a review after a rewrite or a `git gc` still reads them.
- Cost a fixed number of git processes, whatever the number of branches, of marks and of commits.
- Show only what changed since the review, also after the branch was rebased, synced, amended or reworded.
- Show the contract changes first, the one used most at the top, and never fail a plan because accio cannot answer.

## Non-goals

- Mark a pull request of GitHub: its "Since Your Review" stays the one of GitHub.
- Push or fetch the marks: they are local to the repository, shared by its worktrees.
- Mark a range that is no branch.
- Build an accio index: a repository without one gets no fan-in.
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
  nothing changed; none after a rewrite, a drop or a commit added below a reviewed one.
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`commits_added_after_the_mark_are_the_range_on_top`, `a_baseline_move_alone_leaves_every_commit_unchanged`, `a_reworded_commit_pairs_by_its_lines_and_a_dropped_one_is_listed`)
- `--since-reviewed` must list only the `added` and the `rewritten` commits; of a range, or with `--base` of a range,
  or a range rev that names no commit, it is `invalid_arguments` (exit 2).
  [@test] ../../crates/review-plan/src/interdiff_test.rs (`a_range_has_no_interdiff`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`, `a_branch_never_marked_and_a_range_have_no_interdiff`)
- `bd review-plan` must print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`,
  `commits` with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, and `scope`), with
  `"schema": 2`, and its output must be in `bd schema`.
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)
- The plan must read only and run a fixed number of git processes, whatever the number of commits: the listing and
  the base of a mark, one `rev-list` per side, one `diff-tree` with one `patch-id` for the commits that are not the
  same on both sides, one `diff-tree` for the rewritten ones; never `git range-diff`, never a process per commit.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`)

### API Surface

- The reviewed commits must be those of `base..head`; with `--since-reviewed` only the `added` and the `rewritten`
  ones, adjacent or not, each read against its own parent, so an unchanged commit between them adds nothing; a branch
  never marked has no reviewed commit, so all of its commits are read. The `api`, the `files`, the `links`, the
  `repeated_edits` and the `order` must be computed from these commits alone.
  [@test] ../../crates/review-plan/src/api_test.rs (`since_the_review_the_api_is_that_of_the_added_and_rewritten_commits`)
- `api` must list the public declarations (it and every declaration around it public, Java's `protected` too, none
  around it a function) that the commits add, remove or change in form, as the syntax summaries read them: the
  header, the parameters of a JVM function, the reach, `open`, the constructors of a type, what Java sees; a change
  counts when the declaration is public before or after it, so a narrowed reach is listed. A change of a private
  declaration or of a body alone must not be listed, nor a member of a type added or removed with it. A name of one
  or two characters must be listed too (the checks of a sync search only longer ones).
  [@test] ../../crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`, `a_rust_function_changes_by_its_header`, `a_public_declaration_of_a_short_name_is_listed`)
- Each entry must give its file and line, its name, its kind (`removed`, `changed`, `added`), the parts that
  changed, and its form before and after: the header with the parameters after the name, a type by its constructors.
  The file and line are those after the commits, through every rename, also one that changes no declaration; of a
  removal, those before the first commit that touched the declaration, so `path:line` is of the same content: the old
  path when that commit also renamed the file. A range of no source file of a known language has an empty `api`.
  [@test] ../../crates/review-plan/src/api_test.rs (`the_card_lists_the_public_declarations_the_range_adds_removes_or_changes`, `a_range_without_source_files_has_no_api`, `a_removal_after_a_rename_names_the_path_its_line_is_of`, `the_fan_in_of_an_api_of_a_renamed_file_counts_at_its_new_path`)
  [@test] ../../crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`)

### Fan-in

- Every removed or changed entry, the first 20 of the card (`MAX_FANIN_QUERIES`), must get its usages at the head of
  the range from `accio references <name> --ref <head>`, the declaration itself not counted, with `complete: false`
  when accio cut its answer at 100; the queries run in parallel; an addition is never asked.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_fan_in`, `a_cut_answer_is_not_complete`, `at_most_twenty_queries_per_plan`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_fan_in_at_the_head_ranks_the_card`)
- The entries must be sorted by their fan-in, the most first and those without one last, then removals, changes and
  additions, then by file and line.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`the_entries_are_sorted_by_their_fan_in`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_fan_in_at_the_head_ranks_the_card`)
- Without accio, or without its index of the repository, the fan-in must be `null` and `index_note` must say why
  ("accio is not installed", "accio has no index of this repository"); a plan never builds an index.
  [@test] ../../crates/review-plan/src/fan_in_test.rs (`without_an_index_the_fan_in_is_missing_and_the_note_says_why`)
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
- `breaks` must sum the usages at the head of the API a file removes or changes, at its last path, also of a removal
  whose entry names the old path; an addition breaks nothing.
  [@test] ../../crates/review-plan/src/files_test.rs (`the_usages_of_a_files_api_rank_it_first`)
  [@test] ../../crates/review-plan/src/api_test.rs (`the_fan_in_of_an_api_of_a_renamed_file_counts_at_its_new_path`)
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
  [@test] ../../crates/review-plan/src/dependencies_test.rs (`a_new_version_with_new_features_keeps_the_manifest_in_the_tree`, `the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes`)
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
  and the fan-in comes from the index of the host (the fixture `.git/branch-deck-accio.json` in the e2e server).
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_fan_in`)
- An unknown branch must answer `unknown_branch`; a base that names no commit, a branch that shares no history with
  its base, an unknown revision of a range and `sinceReviewed` of a range must answer `invalid_arguments`.
  [@test] ../../crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_fan_in`)
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
  a commit added or rewritten since; each such commit says "New" or "Rewritten". Chosen, it shows the range of the
  commits added on top when the plan has that scope; otherwise the column lists only the added and rewritten commits,
  adjacent or not, and shows the newest of them. "All Changes", another part or a mark leaves it.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, with what it changes against its reviewed version`)
  [@test] ../../src/lib/range-review.test.ts (`offers no scope for a branch never marked, nor when nothing was added or rewritten`, `marks the added and the rewritten commits, and lists only them, adjacent or not, under the scope`, `keeps the plain range of the commits added on top, and the column of one new commit`)
- The API & Risk card must head the diffs of a scope, and in "Since Your Review" also a shown commit above its heading
  (a rewritten commit shows its whole diff, the card what it changes against its reviewed version): the summary, what changed since the review (a rewritten commit
  with the lines per path it changes against its reviewed version), the API in the order Rust ranked it with its
  kind, `path:line`, the parts that changed, the forms before and after of a removal or a change and the usages, the
  files with used API removed or changed or `untested`, with why, and the `index_note` when the usages are missing.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show the API & Risk card with the fan-in, and order the files by their dependencies, kept for every review`)
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show a reviewed commit amended since as Rewritten, with what it changes against its reviewed version`)
  [@test] ../../src/presenters/review-plan.test.ts (`counts what changed since the review, and nothing when only the baseline moved`, `says per path what a rewritten commit changes that its reviewed version did not, and the reverse`, `words the usages, a cut answer as more, none without a fan-in`, `keeps the order Rust ranked, with the kind, the place, the parts that changed and the forms of a removal or a change`, `raises only the risky files, in Rust's order, with the usages and the missing test`, `sums the card up in its title`)
- Order: Tree / Dependencies beside the header of the files must be kept for every review (`reviewFileOrder` of the
  app settings); Dependencies shows the files as one flat list, each by its path, in the plan's `order`, a renamed file
  also by its old path, a file the order lacks after them in tree order; the diffs, J and K follow it.
  [@test] ../../tests/e2e/branch-review/review-plan.spec.ts (`should show the API & Risk card with the fan-in, and order the files by their dependencies, kept for every review`)
  [@test] ../../src/lib/file-tree.test.ts (`lists the files flat in the order of the review plan, a rename by either path, the files it lacks after in tree order`)

## User Experience

- `bd mark-reviewed auth` → `{"branch": "jane/virtual/auth", "ref": "refs/heads/jane/virtual/auth", "base": …,
  "tip": …, "previous": null, "changed": true, "pruned": 0}`.
- `bd mark-reviewed auth --base nope` → `{"error": {"code": "invalid_arguments", "message": "No branch or commit \"nope\"
  to review the branch on"}}`, exit 2.
- `bd review-plan auth --since-reviewed` after one commit on top → `{"branch": "jane/virtual/auth", "base": …, "head":
  …, "interdiff": {"reviewed_base": …, "reviewed_tip": …, "commits": [{"id": …, "reviewed_id": null, "subject": "Add
  logout", "state": "added"}], "scope": {"base": <reviewed tip>, "head": <tip>}}}`.
- `bd review-plan base..head` of a new parameter → `"api": [{"path": "src/a/Api.kt", "line": 3, "name": "open",
  "kind": "changed", "changes": ["parameters"], "before": "fun open(path:String) : Int", "after": "fun
  open(path:String, mode:Int) : Int", "fan_in": {"usages": 12, "complete": true}}]`, `"files": [{"path": "src/a/Api.kt",
  "old_path": null, "mechanical": null, "uses": 0, "used_by": 0, "new_links": 0, "breaks": 12, "untested": true,
  "repeated_edit": null}]`, `"index_note": null`.
- `bd review-plan base..head` of a new `Store` that `User` imports, with its test → `"order": ["src/b/Store.kt",
  "src/test/b/StoreTest.kt", "src/a/User.kt", "README.md"]`, `"links": [{"from": "src/a/User.kt", "to":
  "src/b/Store.kt", "symbols": [{"name": "Store", "line": 3}], "state": "added", "cross_module": false}]`.
- The review window, flag on: **Mark Reviewed** in the title bar → toast "Marked plan reviewed" ("plan was already
  marked reviewed at this commit" when nothing moved); the column: "All Changes 3 commits", "Since Your Review 1
  commit", "read the cache twice  New".
- The card **API & Risk**, "3 API changes · 3 risky files": `Changed helper src/Store.kt:36 parameters 3 usages`, under
  it `fun helper(value:Int) → fun helper(value:Int, scale:Int)`; `Added Cache src/b/Cache.kt:3`; risky `src/Store.kt
  An API used in 3 places · No test changed`; "Since your review: 1 new · 1 rewritten", under it "Rewritten scale the helper: src/Store.kt +1 −1"; "No usages: accio has no index
  of this repository"; "Cannot read the review plan: …".
- **Order: Tree | Dependencies** beside the header of the files.

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
- A rewritten commit is read against its own parent, not against its reviewed version: a declaration it adds shows as
  added even when its reviewed version added it too.
- The fan-in counts the whole-word usages of a name, so another declaration of the same name counts too; a removal's
  usages at the head are those left behind, or of a namesake.
- The module roots of the summaries are read at the checked out HEAD, not at the head of the range.
- A key gets one entry: when an overload changed and another was added in the same commits, only the change is
  listed.
- `untested` sees only test files the commits change; a Rust test module inside its file does not count.
- `breaks` counts the usages of a name at the head, a namesake and a use the commits already updated included.
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
  `target` table, or a `depends` that becomes a `dependencies` `plugin`. Its build file stays real work.
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
- The diffs show a commit or a range only: "Since Your Review" of commits that are not one range lists them in the
  column and shows one at a time, never their changes together.
- A part of several branches of a stack reads the plan of the reviewed branch on the base of the part, but offers no
  "Since Your Review" and its card no "since": the reviewed pair is on the base of the branch alone.
- The card heads the diffs of a scope, and a commit only in "Since Your Review": a commit chosen under "All Changes"
  shows its own heading alone.
