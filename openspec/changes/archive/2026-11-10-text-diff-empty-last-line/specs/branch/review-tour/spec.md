## MODIFIED Requirements

### Requirement: The Map: The lines of the shape are what git counts

The lines of the shape, of the change and of each row, MUST be those `git diff --numstat` of the range counts: git's
counts of each file, from the processes the plan runs anyway, with git's default algorithm whatever `diff.algorithm`
says, never the engine's, which aligns some blocks otherwise. A file that several commits change MUST count its pair
as the engine does, since git's count of the range would take a process of its own.

#### Scenario: A file one commit changes that the engine aligns otherwise
- **WHEN** the engine counts the lines of a file one commit changes otherwise than git: a call moved into the test
  above it, +3 −3 by the engine, which matches the call before the closing brace and the blank line, and +1 −1 by git
- **THEN** the shape counts +1 −1
- **AND** the tree and the header of its card count as the engine does, under the whitespace policy
  ([Commit Files](../../commit/commit-files/spec.md#requirement-reading-the-diffs-the-counts-of-a-file-are-the-engines)),
  and the header of a part of a file its own blocks as the engine counts them under `Default`
  [@test] crates/review-plan/src/modules_test.rs (`the_lines_of_a_module_are_what_git_counts_where_the_engine_aligns_otherwise`)
  [@test] crates/trial-check/src/diff_scan_test.rs (`read_commit_diffs_gives_git_counts_beside_the_engines`, `parses_git_counts_after_the_raw_part`)

#### Scenario: The diff config of the user
- **WHEN** the user's config chooses another diff algorithm (`diff.algorithm=histogram`)
- **THEN** git's counts are those of the default algorithm
  [@test] crates/trial-check/src/diff_scan_test.rs (`the_diff_config_of_the_user_changes_neither_the_hunks_nor_the_edges`)

#### Scenario: A file several commits change
- **WHEN** several commits of the range change a file
- **THEN** the shape counts the blocks of its pair as the engine counts them, without the empty line after the final
  line end that a block reaching the end holds, which can differ from git's count of the range
  [@test] crates/review-plan/src/modules_test.rs (`the_lines_of_a_file_several_commits_change_are_the_engines_count_of_its_pair`)

#### Scenario: No process of their own
- **WHEN** the plan reads the changes of its commits, or the diff of a carried scope
- **THEN** git's counts come with them, from the same `log --raw` and the same `diff-tree`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_review_plan_since_the_review`, `a_review_plan_of_a_range`)
