## ADDED Requirements

### Requirement: In the Review Window: A card of a file in parts names its part and its own lines

A card of a file the story shows in two sections or more MUST say which part of the file it is, in the order of the
story ("part 1 of 3"), and the lines its own blocks add and remove as the engine counts them under `Default` (Rust's
count of each block), in place of the counts of the file; a part of deletions alone too ("+0 −4").

#### Scenario: A file in three parts
- **WHEN** the story shows a file in three sections: a new declaration, a removal and an import
- **THEN** their headers say "part 1 of 3 +5 −0", "part 2 of 3 +0 −4" and "part 3 of 3 +1 −0"
- **AND** a file the story shows in one section keeps the counts of the file and names no part
  [@test] src/presenters/review-units.test.ts (`numbers the parts of a file the story shows in several sections, each with the lines of its own blocks, a part of deletions alone too`, `names no part of a file the story shows in one section`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: The lines of a block
- **WHEN** Rust counts the lines of a block that reaches the end of a file and holds the empty line after its final
  line end
- **THEN** the count leaves that line out, so the blocks of the file sum to the engine's count of it
  [@test] crates/review-plan/src/units_test.rs (`each_block_counts_its_lines_as_the_engine_does`)

### Requirement: In the Review Window: The footer names the part of a file at the top

While a card of a file in parts is at the top, the footer MUST name its part after the file ("Step 1 · File 2 / 7 ·
part 2 of 3"), as J and K walk the parts.

#### Scenario: J through the parts of a file
- **WHEN** J walks from the first part of a file to its second and third
- **THEN** the footer says "part 1 of 3", "part 2 of 3" and "part 3 of 3" after the file, nothing at an opening
  between them
  [@test] src/lib/review-position.test.ts (`names the part at the top of a file the story shows in several sections after the file`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

### Requirement: In the Review Window: A fold names the changes another section shows

A fold MUST count only the unchanged lines it hides and name after them each place that shows a block it hides, with
the count of those blocks ("⋯ 9 unchanged lines · 1 change in part 2", "in Other changes" of another concern), a
click there showing that section; it MUST name the lines of a block the whitespace policy ignores ("· 1 line hidden by
Whitespace").

#### Scenario: A fold over the change of another part
- **WHEN** a fold of a section hides a block another section shows: an import the group of the imports shows
- **THEN** it counts the unchanged lines alone, 66 where the old side holds 66 and the new one 67
- **AND** it says "1 change in part 2", and a click there shows that section, expanded
  [@test] src/lib/diff/rows.test.ts (`names in a fold of a section the blocks another section shows that it hides, and counts only its unchanged lines`)
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A fold over a line the policy ignores
- **WHEN** a fold hides blocks the whitespace policy ignores: a deleted blank line, a line indented again
- **THEN** it says "24 unchanged lines · 2 lines hidden by Whitespace"
  [@test] src/lib/diff/rows.test.ts (`counts the unchanged lines a fold hides, apart from the lines of the blocks the policy ignores there`)

## MODIFIED Requirements

### Requirement: In the Review Window: A file is seen when shown on purpose or scrolled through

A file MUST be seen once the window showed it on purpose (its row, J and K at its card, N and P, a place of a mark or
of the document) or its card passed entirely through the viewport of the diffs, its top and its bottom each in view at
some time, its diff read (`DiffLayout.passedCards`: the cards in view at each scroll, no work per frame); a file in
parts once each of its parts was.

#### Scenario: A card scrolled entirely through the diffs
- **WHEN** the top and the bottom of a card were each in view, at once or one scroll after the other
- **THEN** its file is seen, never a card whose diff is not read
- **AND** the card at the top as the diffs scroll is no file shown on purpose
  [@test] src/lib/diff/layout.test.ts (`reports a card once both its top and its bottom were in view, at once or one scroll after the other, and never a card whose diff is not read`)

#### Scenario: A file shown on purpose
- **WHEN** the window shows a file on purpose: its row, J and K at its card, N and P, a place of a mark or of the
  document
- **THEN** the file is seen
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should count the files of a group the reader saw: a file shown, a card scrolled entirely through the diffs, and say in the bar above the diffs where the reader is`)

#### Scenario: A file in parts
- **WHEN** the reader saw the first part of a file the story shows in three, then the two others
- **THEN** the file is seen only after the third part, and a new head counts its parts from zero
  [@test] src/stores/review-seen.test.ts (`sees a file of one card at its card, a file the story shows in parts once each of its parts was seen`)

### Requirement: The Map: The lines of the shape are what git counts

The lines of the shape, of the change and of each row, MUST be those `git diff --numstat` of the range counts: git's
counts of each file, from the processes the plan runs anyway, with git's default algorithm whatever `diff.algorithm`
says, never the engine's, which aligns some blocks otherwise. A file that several commits change MUST count its pair
as the engine does, since git's count of the range would take a process of its own.

#### Scenario: A file one commit changes that the engine aligns otherwise
- **WHEN** the engine counts the lines of a file one commit changes otherwise than git: a test appended after the
  blank line before the closing brace of a class, +8 −0 by the engine and +9 −0 by git
- **THEN** the shape counts +9 −0
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
