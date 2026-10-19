---
name: Commit Files
description: Requirements for the changed files of a commit, the file and line counts on its meta line, the Files section with its directory tree and the diffs in the commit preview, the actions on a file, and Copy Changed Paths.
style: plain-1
targets:
  - ../../crates/git-ops/src/commit_files.rs
  - ../../crates/object-cache/src/commit_files.rs
  - ../../src-tauri/src/range_files.rs
  - ../../src-tauri/src/commands/commit_files.rs
  - ../../src-tauri/src/repository_state.rs
  - ../../src/stores/commit-files.ts
  - ../../src/hooks/useCommitFiles.ts
  - ../../src/lib/commit-files.ts
  - ../../src/components/commit/CommitFilesSection.tsx
  - ../../src/components/commit/CommitMetadata.tsx
  - ../../src/components/commit/CommitList.tsx
  - ../../src/components/commit/CommitPreviewDialog.tsx
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/commit/InlineCommitFileDiff.tsx
  - ../../src/components/files/FileTree.tsx
  - ../../src/components/files/FileContextMenu.tsx
  - ../../src/lib/file-actions.ts
  - ../../src/lib/file-tree.ts
  - ../../src-tauri/src/commands/review_requests.rs
  - ../../src/lib/diff/view-files.ts
  - ../../src/lib/diff/file-diff-model.ts
  - ../../src/lib/diff/rows.ts
  - ../../src/lib/diff/line-parts.ts
  - ../../src/components/diff/DiffView.tsx
  - ../../src/stores/file-diffs.ts
  - ../../src/stores/blob-texts.ts
  - ../../src/components/shared/DiffViewToggle.tsx
  - ../../src/stores/settings/diff-view-settings.ts
  - ../../src/components/settings/AppearancePage.tsx
  - ../../src/lib/preview-keys.ts
  - ../../crates/git-ops/src/cat_file_batch.rs
  - ../../crates/file-diffs/src/service.rs
  - ../../crates/file-diffs/src/worktree.rs
  - ../../crates/file-diffs/src/model.rs
  - ../../crates/text-diff/src/model.rs
  - ../../crates/object-cache/src/file_diff.rs
  - ../../src-tauri/src/commands/file_diffs.rs
  - ../../crates/test-utils/src/repo_template.rs
  - ../../src/components/conflict/CollapsibleFileHeader.tsx
  - ../../src/components/diff/LazyDiffViews.tsx
  - ../../src/lib/diff/file-languages.ts
  - ../../src/lib/diff/bundled-languages.ts
  - ../../vite.config.ts
  - ../../src/lib/diff/islands-themes.ts
  - ../../src/index.css
  - ../../src/hooks/useCommitCopyActions.ts
  - ../../tests/e2e/helpers/aria-snapshot-helpers.ts
---

# Commit Files

Status: Implemented (the typed file diffs: the backend; the views still show the patch)
Date: 2026-10-18

## Summary

A commit row says what is in the commit: its meta line ends with the number of changed files and the added and deleted
lines, "3 files · +42 −7".
The commit preview has a Files section after the message, closed at first ("› Files 4"): opened, it lists the files
grouped by directory as IntelliJ and Space show the changes of a commit. A file expands to its diff; the first one
read the patch of the whole commit with one git process.
The context menu of a commit copies the paths it changed.
Everything is read with read-only plumbing, one git process for a whole list, and cached by commit id.

## Goals

- See the size of a commit without opening it, in every commit list.
- Read the changes of a commit in the app, file by file, without a terminal or an IDE.
- Cost one git process for an expanded branch, and none for a commit seen before.

## Non-goals

- The row does not list the files; the preview does.
- The branch preview lists no files: its commits show the counts on their meta lines.
- No blame, no file history.
- The diffs of uncommitted changes stay in the amend card ([Amend Changes](amend.spec.md)).

## Requirements

### Reading the files

- The files of a commit must be its changes against its first parent, with renames detected. A root commit adds
  every file. A merge commit shows the changes against its first parent.
- Each file must have its status (added, modified, deleted, renamed, copied, type changed), its path, the path before
  a rename or copy, its added and deleted lines, whether git counts no lines (a binary file), and the blob ids of its
  two sides: none for a side the file does not exist on, and the commit of a submodule.
- The paths must be the bytes git stores: spaces, tabs and non-ASCII letters stay as they are.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`lists_the_status_paths_and_line_counts_of_each_file`, `each_file_has_the_blob_ids_of_its_sides`, `a_submodule_has_the_ids_of_its_commits`, `lists_a_merge_against_its_first_parent_and_nothing_for_an_empty_commit`, `parses_numstat_entries_in_another_order_than_the_raw_entries`, `rejects_output_it_does_not_understand`)
- The files of many commits must be read by one `diff-tree --stdin --raw --numstat -z` process.
- The files must be cached by commit id in the object cache of the repository, which the app shares with `bd`.
  A commit read before must cost no git process.
- A commit git lists nothing for must not be cached: it has no changes, or it is not in the repository yet.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`reads_through_the_cache_once_per_commit`)
  [@test] ../../crates/object-cache/src/cache_test.rs (`round_trip`)
- The object cache must live in the common git directory of the repository, which the app resolves once per
  repository. Outside a repository the cache is disabled, and the files are still read.
  [@test] ../../src-tauri/src/repository_state_test.rs (`object_cache_lives_in_the_common_dir_of_the_repository`)
- A commit id must be a full object id: an abbreviation, a ref or an option is refused.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`refuses_what_is_not_an_object_id`)
- Nothing must write to a ref, the index or the working tree.
- The files of a range `base..head` (the review of a pull request,
  [Review Requests](../repository/review-requests.spec.md#review-window), or of a branch) are one
  `diff-tree --raw --numstat -z` of the two commits (never `--stdin`, so a range never lands in the disk cache entry
  of its head), kept in memory only, at most 16 MiB by their paths, whatever the number of ranges: the reads of a
  review and its prefetch run one process. Its id is `<base>..<head>`, both ids validated; the app keys it by that id
  and drops it when the review closes.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`range_files_are_the_changes_of_all_its_commits`, `a_range_refuses_what_is_not_an_object_id`)
  [@test] ../../src-tauri/src/range_files_test.rs (`the_files_of_a_range_stay_in_memory`)
  [@test] ../../src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)

### Reading the diffs

- The views must read the diffs of every file of a commit or a range with one `get_file_diffs` under the whitespace
  policy: typed blocks of the diff engine, no patch text. The diffs stream ([File diffs](#file-diffs)): a file shows
  once its content arrives and "Loading…" before, the files in view are asked for first (`priorityPaths`, the file a
  review opens at), and the late scopes of a text name its folds without losing the lines it expanded. The app keeps
  the diffs of the last 8 subjects per policy, reads a subject once while it is being read, tries a failed read
  again, shows the loaded uncommitted changes until their new read ends, and drops the ranges of a closed review.
  [@test] ../../src/stores/file-diffs.test.ts (`reads a subject once per policy, the files in view first`, `shows each file as its content arrives, and the late scopes of a text keep the lines it expanded`, `reads the uncommitted changes again on a refresh, and keeps the loaded ones until the new read ends`, `keeps the last subjects only, and forgets the subjects of a closed review under every policy`)
- The texts of a diff must be read by git blob id (`get_blob_texts`), shared by every commit, range and window, the new
  side of an uncommitted change by its path too: the ids of one task in one call (64 at most), each id once, a failed read tried again, the least recently used texts
  dropped over 64 Mi UTF-16 units, never the ones just read. A file's diff is built from its blocks and both texts,
  complete: the unchanged lines between hunks expand without a request. The same diff gives the same model, so the
  lines it expanded stay expanded.
  [@test] ../../src/stores/blob-texts.test.ts (`reads the ids of one task in one call, and each id once`, `reads many ids in batches`, `keeps a failed read as an error and reads it again on the next request`, `drops the least recently requested texts over the length, never the ones just read`)
  [@test] ../../src/lib/diff/rows.test.ts (`finds each row from its segment, and keeps the fold levels of a diff object`)
  [@test] ../../src/lib/diff/view-files.test.ts (`says why a file shows no lines: binary, too large, a submodule, a blob git cannot read, a text that cannot be read, no changed lines`)
  [@test] ../../crates/git-ops/src/cat_file_batch_test.rs (`the_content_of_an_object_over_the_limit_is_not_buffered`)
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)
- The `+N −M` of a file and a directory in the tree, in the header of a diff and in the uncommitted card must be the
  counts of the engine under the whitespace policy, and show nothing until its diffs are read, never git's count
  first. The meta line of a commit row keeps git's numstat, which reads no blob, so the two can differ.
  [@test] ../../src/lib/diff/view-files.test.ts (`takes the line counts of the engine, and none before its diff is read`)

### File diffs

The views show the typed diff of each file instead of the patch ([Reading the diffs](#reading-the-diffs)).

- `get_file_diffs` must give the diff of each changed file of a commit, a range or the working tree (`DiffSource`)
  under a whitespace policy, in the order of git: its path, the path before a rename or copy, its status, the blob ids
  of both sides, and its content. The content is a `TextDiff` of the engine
  ([Text Diff Engine](../../docs/design/text-diff.md), "Model"), `Binary`, `TooLarge` with the sizes, `Submodule` with
  the commit of each side ("Subproject commit a → b"), or `Missing` for a blob git cannot read. No patch text carries
  it.
  [@test] ../../crates/file-diffs/src/service_test.rs (`a_commit_has_a_typed_diff_of_each_kind_of_file`, `a_range_has_the_diffs_of_all_its_commits`, `the_model_is_json_with_a_kind`, `a_submodule_names_its_commits`, `a_blob_git_cannot_read_is_missing_and_not_cached`)
  [@test] ../../crates/test-server/src/commit_files_test.rs (`commit_files_routes_read_the_fixture_repository`)
- The diffs must stream: the files first, then the content of each file as it is known, the files in view
  (`priorityPaths`) first and the others in the order of the tree, then the scopes of a text that the caches did not
  have. The result must not depend on the load of the machine: only a pathological file (100 ms for its inner ranges,
  1 s for its lines) is compared by line.
  [@test] ../../crates/file-diffs/src/service_test.rs (`the_stream_lists_the_files_then_the_files_in_view_first_and_their_scopes_after_their_blocks`, `the_work_takes_the_files_in_view_then_the_tree_order_with_directories_first_and_numbers_as_numbers`)
  [@test] ../../crates/test-server/src/commit_files_test.rs (`commit_files_routes_read_the_fixture_repository`)
- The diff of a commit must cost one file list and one `cat-file --batch`, and nothing when the caches have it. The
  caches key a diff by its blob pair, the policy and the languages of the scopes, so two commits with the same blob
  pair share one diff. The store never keeps a diff that the time cut short.
  [@test] ../../crates/file-diffs/src/service_test.rs (`a_diff_costs_one_list_and_one_cat_file_then_nothing`, `two_commits_with_the_same_blob_pair_share_its_diff`, `the_policy_keys_its_own_diff`, `a_diff_out_of_time_is_by_line_and_not_stored`)
  [@test] ../../crates/object-cache/src/cache_test.rs (`file_diffs_are_read_by_their_blob_pair_policy_and_version`)
- The working tree must be compared with `HEAD`: staged, unstaged and untracked files, from one `status`. The new side
  is the file on the disk, with the blob id that `git hash-object --no-filters` gives its bytes.
  [@test] ../../crates/file-diffs/src/service_test.rs (`the_working_tree_is_compared_with_head_by_the_ids_git_gives_its_files`)
- `get_blob_texts` must give the texts by blob id, from the blobs that the diffs read when it can. A file of the working
  tree is read by its path, and only when its bytes still have the id; a path outside the repository is refused.
  [@test] ../../crates/file-diffs/src/service_test.rs (`the_texts_of_a_diff_come_from_memory`, `a_working_tree_text_is_read_by_its_path_and_checked_against_its_id`)
- The hunk headers must name the declaration around each hunk, `Outer › inner`, from the scopes of the diff.
  [@test] ../../crates/file-diffs/src/scopes_test.rs (`a_member_is_labelled_with_its_containers`, `a_rust_impl_member_is_labelled_with_its_type`)
- The views must build the context, the hunks and the expansion from the blocks and the texts: the tints of IntelliJ
  (below), an `Ignored` block as unchanged and skipped by N and P, the inline pairs of a block in one row in the
  Inline view, and a notice when the files differ in their line ends only.
  [@test] ../../src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`, `makes each similar pair of a block one row only in the Inline view, also at the end of a file`)
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`, `Inline shows a lightly edited line as one row`)

### Meta line

- A list must load the files of its commits with one call when it is shown or gets other commits, and only for the
  commits not known yet. A second request for a commit must wait for the one in flight.
- A list must be `aria-busy` while the files of its commits are read, so that a snapshot waits for the counts.
- A failed load must be logged, not shown; the rows go without counts, and the next load tries again.
  [@test] ../../src/stores/commit-files.test.ts (`loads the commits of a list with one call, and only the ones not known yet`, `waits for a load in flight instead of asking again`, `tries again after a failed load`)
- Once known, the meta line of a commit row must end with the file count and the line counts: `1 file · +1 −0`.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should show the file count and the line counts in the meta line of each commit row`)
- A commit without changed lines (only renames, mode changes or binary files) must show the file count only. A
  commit without files must show neither.
- The commits of the branch preview must show the same meta line.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should show the meta line with the files in the commits of the branch preview`)
- The meta line of the commit preview must show the counts too.

### Files section

- The commit preview must show a "Files" section after the message, unless the message is being edited. It starts
  closed: its header ("Files 4", the count once a list read the files of the commit) with a chevron, no tree, and
  nothing of the files is read or rendered for it until it opens. A commit without files says "No changed files".
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`, `should open and close the files with their header, F and the footer button`)
- The files must be a tree grouped by directory. A directory that holds nothing but one directory must be one row
  with the joined name (`plugins/air/backend/session`). Directories come before files, names in a human order
  (`file2` before `file10`, case-insensitive). A directory row shows the sums of the line counts of its subtree.
  [@test] ../../src/lib/file-tree.test.ts (`merges a chain of directories with one subdirectory into one row`, `sums the files and the line counts of each directory over its subtree`, `puts directories first and sorts names as a person reads them`, `keeps the files of a directory with one subdirectory apart from it`, `takes a path with spaces and letters beyond ASCII as it is`, `lists the files in the order of the tree`)
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)
- Every directory starts open; in a commit with more than 100 files the directories below the top level start
  collapsed. A collapsed directory hides its rows.
  [@test] ../../src/lib/file-tree.test.ts (`hides the rows below a collapsed directory`, `collapses the directories below the top level of a large commit only`, `opens the directories above a row to reveal it, and only those`)
- A file row must show its name in the colour of its status, where a renamed or copied file came from (relative to
  the directory both paths share), its line counts and the status letter (M, A, D, R, C, T); nothing else takes width
  from the name. Its hover shows the path (`old → new` for a rename) and, in a review, the code owner. Its accessible
  name is the file name, its description the status and the counts. A deleted file is struck through, its status
  letter never, in the tree and in the header of its diff.
  [@test] ../../src/lib/file-tree.test.ts (`names a file by its name in its directory`, `names where a renamed file came from, relative to the directory both paths share`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should strike a deleted file through, never its status letter, in the tree and above its diff`)
- A file must expand to its diff under its row.
- The tree must be one tab stop (roving focus): ↑/↓ move between the rows and never leave the tree, Home and End go
  to the first and the last row, → opens a row (a directory, a file's diff) or goes to the first child, ← closes it or
  goes to the parent, Space toggles it, Enter toggles a directory and opens a file in the review window at that
  file ([Commit Review](commit-review.spec.md)). ⌥↑/⌥↓ switch the commit from anywhere in the preview; the letters (F, E,
  D) keep working in the tree.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should move through the tree with the keyboard and switch commits with Alt`)
- A binary file must show "Binary file", and a file without changed lines "No changed lines", instead of a diff; a
  file over the limits of the engine shows "Too large to show: 12.3 MB". The diff under a row is virtualized in the
  scroller of the preview, so a large file shows at once, without a click.
- The diff must mark the changed words of a changed line, and highlight the code as the IntelliJ editor does, in the
  light and the dark scheme. Kotlin, Gradle (`.gradle`, `.gradle.kts`), Rust, TypeScript, Java and the other common
  languages must be read by their grammar; a file of a language the app does not bundle (`BUNDLED_LANGUAGE_IDS`: 84
  90 of Shiki's grammars, no Emacs Lisp, COBOL or Wolfram) and an unknown file are plain text, never a failed diff.
  [@test] ../../src/lib/diff/bundled-languages.test.ts (`keeps only the bundled entries of Shiki's language list, so the other grammars leave the build`, `fails the build for an id Shiki does not have`, `highlights %s with a bundled grammar`, `maps the extension table and our custom extensions to bundled grammars only`, `shows a file of a grammar the build leaves out as plain text`)
  [@test] ../../src/lib/diff/file-languages.test.ts (`maps our custom extensions to grammars Shiki has`, `highlights %s as %s`, `shows a file without a known name as plain text`)
  [@test] ../../src/lib/diff/highlight/engine.test.ts (`gives each token the role of the Islands themes, in runs that change only with the style`, `gives Rust and TypeScript tokens the roles of the Islands themes too`)
- A diff must show the blocks of the diff engine with 4 lines of context (IntelliJ's), changes whose context touches
  without a fold between them, and the changed ranges of the engine instead of a word diff, also on a changed line
  without a line beside it. The tints are IntelliJ's: a block with inner ranges draws its lines pale and its ranges
  in the full colour; a block without them (line-level only, or ranges dropped) draws its lines in the full colour; a
  whitespace-only block draws its lines pale with a faded bar. The line indices are the engine's: the empty line
  after a final line end is a line, and it shows only when a block touches it.
  [@test] ../../src/lib/diff/rows.test.ts (`leaves out the empty line after a final line end, unless a block touches it`, `draws the lines of a block with inner ranges pale and the lines of a block without them in the full tint, as IntelliJ`, `keeps 4 lines of context next to a change, as IntelliJ, and folds the others`, `skips a level that shows no more lines, and folds no fewer than 2 lines`, `cuts the context at both ends of the file`, `fills the shorter side of a block in the split view`, `shows a new file and a deleted file as one block of one side`, `splits lines as IntelliJ: LF, CRLF and a lone CR end a line, and a text may end without one`)
  [@test] ../../src/lib/diff/line-parts.test.ts (`cuts a line at each run of the tokens and each bound of a changed range`)
- A file whose line ends changed must say so under its header ("Line endings: LF → CRLF"), and "Only line endings
  changed: …" when that is its only change.
  [@test] ../../src/lib/diff/view-files.test.ts (`names a change of the line ends, and says so when it is the only change`)
- A fold of unchanged lines must name the innermost declaration around its lines (the scopes of the engine, such as
  `fun main() {`, the new side first) after their count. The folds nest as IntelliJ's: a click on a fold shows 8
  lines next to each change, the next one 16, then all. "Expand All" of a file header in a review, and ⌥-click (Alt) on
  a fold, show all the lines of the file.
  [@test] ../../src/lib/diff/rows.test.ts (`labels a fold with the innermost scope around its lines, the new side first`, `nests the folds at 4, 8 and 16 lines of context, then shows all lines`)
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)
- Highlighting must not block the window: it runs in a worker, which starts with the first diff shown and lives as
  long as the window, and a row shows plain text until its tokens arrive. When the workers cannot start, the diff is
  highlighted on the main thread in idle slices.
  [@test] ../../src/lib/diff/highlight/engine.test.ts (`answers a page far into a file first as provisional, then exact, and confirms equal runs without arrays`)
  [@test] ../../src/lib/diff/highlight/client.test.ts (`keeps the pages the engine on the main thread answers, a provisional one confirmed exact`)
- The section must offer the split, the unified and the inline view. The choice is one setting of every diff view (the
  section, the reviews, the uncommitted changes, the conflicts and the missing commits), also in Settings →
  Appearance, "Diffs". Split is the default, as in IntelliJ; a stored choice keeps winning over it.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should keep a stored diff view mode over the Split default`)
- Inline must show each pair of a block that the diff engine finds similar as one row: the new line with the deleted
  parts of the old line before the inserted parts, each part in the syntax colours of its own side, the deleted parts
  left out of a copy, the line number of the new line. A line of a pair without a range of its own (a pure insertion
  or deletion) has an empty one where the other line changes. The other blocks show as in Unified, and so does a pair
  at the end of a file without a newline at its end.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`Inline shows a lightly edited line as one row`)
  [@test] ../../src/lib/diff/line-parts.test.ts (`puts the deleted part of the old line before the inserted part, each with the runs of its side`, `aligns a pure insertion: the old line without a range gets an empty one, so the row is the new line`)
  [@test] ../../src/lib/diff/rows.test.ts (`makes each similar pair of a block one row only in the Inline view, also at the end of a file`)
- Split must scroll its two sides together: a horizontal scroll of one side moves the other to the same place, as
  IntelliJ's diff does.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`Split scrolls both sides together`)
- Another commit (↑, ↓) must show its own files, collapsed.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)
- The header of the section, F and the `F Files` button of the footer must open or close it. The choice must hold
  for the next preview, until the app restarts.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should open and close the files with their header, F and the footer button`)
- A preview of a commit no list loaded must load its files alone (`get_commit_files`).
  [@test] ../../src/stores/commit-files.test.ts (`loads the one commit a preview shows with the single command`)
- A failed read must show "Cannot read the files of the commit" in the section.

### Soft Wraps

- "Soft wraps" in Settings → Appearance, "Diffs", must break a long line at the edge of the diff. It is off by
  default, as IntelliJ's `USE_SOFT_WRAPS`: a long line keeps its shape and scrolls sideways. One setting for every
  diff view.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should break a long line with Soft Wraps on in Settings, and scroll it with them off`)

### Whitespace

- The Whitespace menu beside the layout switch (also in Settings → Appearance, "Diffs") must choose what the diff of
  every view ignores: Do Not Ignore (the default, as IntelliJ), Trim Whitespace, Ignore Whitespace, Ignore Whitespace
  and Blank Lines. The button is marked while it ignores anything. The choice goes to the backend with every read of
  the diffs of the files.
  Trim Whitespace hides a change of indentation. Under Ignore Whitespace a blank line added holds no character the
  engine compares: it is a whitespace-only change, pale, counted, and N and P stop at it, as in IntelliJ. Ignore
  Whitespace and Blank Lines ignores it. The counts of the file follow the choice.
  Under Do Not Ignore a change of indentation is a change with inner ranges, as in IntelliJ: its lines are pale, and
  the indentation is in the full colour.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`Ignore Whitespace hides an indentation-only change`)
- A block that only the choice ignores must show as unchanged lines: the unified and the inline view show its new
  lines as context and leave its old lines out (the old line numbers jump, as in IntelliJ), the split view shows both
  sides plain, the shorter one filled. N and P skip it. A block that differs only in whitespace is drawn pale; it
  counts, and N and P stop at it.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`N and P skip a block the whitespace policy ignores`)
  [@test] ../../src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`, `shows no rows for a file whose only blocks the policy ignores`, `folds the lines an ignored block holds, with their own count on each side`, `starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)

### The actions on a file

- A file and a directory have one set of actions (`fileMenuItems`), the same wherever they show: the rows of the tree,
  the headers of the diffs of a review, the headers of the conflict and missing-commits accordions, the files of a
  pull request. A file offers "Copy Path"; a directory "Copy Path" and "Copy Changed Paths" (the files under it, one
  per line).
- The right click on a row or a header opens them as its menu, one menu per row or header: never on a diff, whose text
  keeps its own selection and native menu. A right click on a row focuses it. ⌘C (Ctrl+C off macOS, the key of the
  layout) on a focused row (of the tree, of the files of a pull request) copies its path, unless text is selected; the
  menu of a row shows the key, the menu of a header, where the key does nothing, does not.
  [@test] ../../src/lib/file-actions.test.ts (`copies the path of a file, with the key only where the key works`, `copies the path of a directory, or the changed paths under it`)
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should copy the path of a file or the changed paths of a directory from the menu of its row, and with ⌘C`)
- The headers keep their copy button on hover: they have the room. A tree row has none.

### Copy Changed Paths

- The context menu of a commit row must offer "Copy Changed Paths": the paths the commit changed, one per line,
  a renamed file by its new path.
- For a selection it must be "Copy Changed Paths of 3 Commits": each path once, in the order of the commits.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should copy the changed paths of the commits from the context menu`)
  [@test] ../../src/lib/commit-files.test.ts (`lists each path once, in the order of the commits, a renamed file by its new path`)
- The paths of the commits the list loaded must be copied in the click, without waiting for git.
- When the files cannot be read, the copy must show an error toast and copy nothing.

## User Experience

### Meta line

- The counts follow the time: `a1b2c3d · 2 hours ago · 3 files · +42 −7`, with the `·` separator of the meta line.
- The counts are `tabular-nums`. The minus is `−` (U+2212), not a hyphen.
- In a row, the line counts are muted like the rest of the meta line. Where the commit is a heading (the header of the
  commit preview, a commit of the branch preview, the commit above its diffs in a review), `+42` is `text-success` and
  `−7` is `text-destructive`.

### Files section

- A `border-border` rule above the section separates it from the message. Its header is a button: a `ChevronRightIcon`
  (`size-3.5` muted, turned down when open, as a tree row's) and the label "Files 4", muted `text-xs font-medium`
  (`SectionLabel` with the count, as the files of a review), `hover:bg-muted rounded-md`, `aria-expanded`; the
  `Split`/`Unified`/`Inline` toggle on its right while it is open.
- The tree rows are `h-6 rounded-md text-sm`, `hover:bg-muted`, indented by 0.75rem per level, a `ring-2 ring-ring/50`
  inset on the focused row. A row starts with a `ChevronRightIcon` (`size-3.5` muted, turned down when open). A 1px
  `border` guide per level runs under the chevron of each parent, on the rows only (not on a diff under a row).
- A directory shows `FolderIcon` (`FolderOpenIcon` when open, `size-4` muted), its name (truncated, the full path in
  its title) and its counts muted in `text-xs`.
- A file shows its status letter in `font-mono text-xs` in the status colour, where a directory has its folder icon,
  then its name in its status colour, IntelliJ's: added `text-success`, deleted muted and struck through (the letter
  never), the others `text-primary` (`FilePathLabel`). A rename adds `← api/Old.kt` muted, which gives way first. On
  the right: the coloured `+/−` counts in `text-xs`. A long name is cut with "…" only once nothing else gives way; the
  counts never wrap or shrink. A zero count is not coloured.
- The diff of an expanded file is a `rounded-lg border` panel under its row, at the row's indent.
- The diff is JetBrains Mono at 13px on the `card` background, added lines `success`, deleted `destructive`, a bar in
  the gutter, the changed words a stronger tint ([UI Theme](../../docs/design/ui-theme.md) "Diffs"). The rows of a
  whitespace-only block are paler, their gutter bar faded. The scope of a hunk follows the count of its separator in
  `foreground`, cut with "…" when it is long.
- Loading: an inline spinner and "Loading files…" in muted `text-sm`; a file's diff shows "Loading diff…". A failed
  read of the patch is a destructive callout "Cannot read the changes of the commit" in the panel.
- The footer shows the ghost `xs` buttons `D Review Changes` and `F Files` (pressed while the section is open) before
  `E Edit`.

## Testing / Local Run

- `pnpm test:backend`: `crates/git-ops/src/commit_files_test.rs`, `crates/object-cache/src/cache_test.rs`,
  `src-tauri/src/repository_state_test.rs`, `src-tauri/src/range_files_test.rs`.
- `pnpm test:unit`: `src/lib/commit-files.test.ts`, `src/lib/file-tree.test.ts`, `src/stores/commit-files.test.ts`,
  `src/stores/file-diffs.test.ts`, `src/stores/blob-texts.test.ts`, `src/lib/diff/*.test.ts`.
- `pnpm e2e`: `tests/e2e/commit-files`, on the `issue_links` and `commit_tree` templates (nested directories, a Kotlin
  file with two hunks). The test server runs the commands on the fixture repositories without the object cache, the
  memory cache of patches and the warm: each read of a patch runs git.

## Open Questions / Risks

- A commit with thousands of files renders a row per file in the preview, its directories collapsed; the tree is not
  virtualized.
- Nothing warms the diffs of the listed commits ahead of their first view: with the files of the list in the object
  cache, a first view costs one `cat-file` and the comparison. Measured on the last 100 commits, the first content
  came after 16 ms (p50) and 39 ms (max) in Branch Deck, 8 ms and 29 ms in the monorepo; the whole commit 27–31 ms
  (p50) and 59–274 ms (max). The old patch warm saved a `diff-tree -p`, 146 ms (p50) in the monorepo.
- The counts are those of the original commit. A virtual copy that git merged differently may differ slightly.
- No e2e test covers a rename, a binary file or a failed read: the templates have none.
- The Activity entry of a push does not count files yet.

## References

- [Git Operations](../../docs/design/git-operations.md) ("Commit files", "Object cache")
- [Diff View](../../docs/design/diff-view.md)
- [Commit Review](commit-review.spec.md)
- [Commit Row](commit-row.spec.md)
- [Branch Preview](../branch/branch-preview.spec.md)
- GitHub and GitLab show `+42 −7` per commit and per file, with the added lines green and the deleted ones red.
