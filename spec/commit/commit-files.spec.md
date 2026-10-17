---
name: Commit Files
description: Requirements for the changed files of a commit, the file and line counts on its meta line, the Files section with its directory tree and the diffs in the commit preview, the actions on a file, and Copy Changed Paths.
style: plain-1
targets:
  - ../../crates/git-ops/src/commit_files.rs
  - ../../crates/object-cache/src/commit_files.rs
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
  - ../../src/stores/commit-patch.ts
  - ../../src-tauri/src/commands/review_requests.rs
  - ../../src/lib/diff/commit-patch.ts
  - ../../src/lib/preview-keys.ts
  - ../../crates/git-ops/src/cat_file_batch.rs
  - ../../crates/test-utils/src/repo_template.rs
  - ../../src/components/conflict/CollapsibleFileHeader.tsx
  - ../../src/components/diff/FileDiffView.tsx
  - ../../src/components/diff/LazyDiffViews.tsx
  - ../../src/lib/diff/file-diff.ts
  - ../../src/lib/diff/diff-engine.ts
  - ../../src/lib/diff/diff-languages.ts
  - ../../src/lib/diff/bundled-languages.ts
  - ../../vite.config.ts
  - ../../src/lib/diff/islands-themes.ts
  - ../../src/index.css
  - ../../src/hooks/useCommitCopyActions.ts
  - ../../tests/e2e/helpers/aria-snapshot-helpers.ts
---

# Commit Files

Status: Implemented
Date: 2026-10-14

## Summary

A commit row says what is in the commit: its meta line ends with the number of changed files and the added and deleted
lines, "3 files · +42 −7".
The commit preview lists the files after the message, grouped by directory as IntelliJ and Space show the changes of a
commit. A file expands to its diff; the first one read the patch of the whole commit with one git process.
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
  a rename or copy, its added and deleted lines, and whether git counts no lines (a binary file).
- The paths must be the bytes git stores: spaces, tabs and non-ASCII letters stay as they are.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`lists_the_status_paths_and_line_counts_of_each_file`, `lists_a_merge_against_its_first_parent_and_nothing_for_an_empty_commit`, `parses_numstat_entries_in_another_order_than_the_raw_entries`, `rejects_output_it_does_not_understand`)
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
  [Review Requests](../repository/review-requests.spec.md#review-dialog)) are one `diff-tree --raw --numstat -z` of the
  two commits (never `--stdin`, so a range never lands in the disk cache entry of its head), its id `<base>..<head>`,
  both ids validated; the app keys it by that id and drops it when the review closes.
  [@test] ../../src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)

### Reading the diffs

- The patch of a commit must be read whole, by one `diff-tree -p --full-index` process with the options of the files
  (first parent, renames, no external diff or textconv), when the preview first shows a diff of it, never before.
  The patch must be cut into a section per file at its `diff --git` lines, as it streams, and kept untrimmed.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`commit_patch_has_a_section_per_file_in_git_order`, `commit_patch_of_a_rename_shows_the_changed_lines_with_full_blob_ids`, `patch_splitter_cuts_at_diff_headers_across_chunk_boundaries`)
- A file over 1 MiB or 10 000 lines of patch, and every file after the first 16 MiB of a commit, must keep only its
  header and the count of the lines left out: a huge commit costs the limits, not its size.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`commit_patch_keeps_only_the_header_of_a_file_over_a_limit`, `commit_patch_leaves_out_the_hunks_after_the_total_limit`)
- The patch of a range is one `diff-tree -p --full-index` of the two commits (`get_range_patch`), kept as a commit's
  under `<base>..<head>`.
  [@test] ../../src/stores/commit-patch.test.ts (`reads the range of a review with its own command, keyed base..head, until its review drops it`)
- The app must keep the patches of the last 8 commits it showed, at most 32 MiB of text, and read a commit once while
  it is being read; a failed read is tried again with the next diff shown.
  [@test] ../../src/stores/commit-patch.test.ts (`reads a commit once, also when asked again while it loads`, `keeps a failure as an error and tries again on the next load`, `evicts the least recently loaded patch over the limit`)
- A file's diff is its section of the patch, found by its path after the commit; the same file gives the same parsed
  diff, so lines it expanded stay expanded.
  [@test] ../../src/lib/diff/commit-patch.test.ts (`finds the section of each file by its path after the commit`, `parses a patch once: the same diff for the same file, which an expansion fills in place`, `has no diff for a file the patch does not list`)
- The unchanged lines between hunks must expand: both versions of the file are read by their blob ids from the
  patch's `index` line, with one `cat-file --batch` process, at most 16 blobs. A binary blob, a blob over 8 MiB and an
  id that is not a blob are told apart, never sent as text; the side a file does not exist on is not asked for.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`blob_contents_tells_text_from_binary_missing_and_too_many`)
  [@test] ../../src/lib/diff/commit-patch.test.ts (`reads both sides by the blob ids of the patch`, `does not ask for the side a file does not exist on`, `shows why a side cannot be read, and fails the expansion`)
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)

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

- The commit preview must show a "Files" section after the message, unless the message is being edited.
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
  name is the file name, its description the status and the counts.
  [@test] ../../src/lib/file-tree.test.ts (`names a file by its name in its directory`, `names where a renamed file came from, relative to the directory both paths share`)
- A file must expand to its diff under its row.
- The tree must be one tab stop (roving focus): ↑/↓ move between the rows and never leave the tree, Home and End go
  to the first and the last row, → opens a row (a directory, a file's diff) or goes to the first child, ← closes it or
  goes to the parent, Space toggles it, Enter toggles a directory and opens a file in the review at that file
  ([Commit Review](commit-review.spec.md)). ⌥↑/⌥↓ switch the commit from anywhere in the preview; the letters (F, E,
  D) keep working in the tree.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should move through the tree with the keyboard and switch commits with Alt`)
- A binary file must show "Binary file", and a file without changed lines "No changed lines", instead of a diff; a
  file the patch left out shows "Too large to show: 12,345 changed lines", and a file with more than 2,000 changed
  lines waits for `Show Diff`, as its diff is not virtualized under its row.
- The diff must mark the changed words of a changed line, and highlight the code as the IntelliJ editor does, in the
  light and the dark scheme. Kotlin, Gradle (`.gradle`, `.gradle.kts`), Rust, TypeScript, Java and the other common
  languages must be read by their grammar; a file of a language the app does not bundle (`BUNDLED_LANGUAGE_IDS`: 84
  90 of Shiki's grammars, no Emacs Lisp, COBOL or Wolfram) and an unknown file are plain text, never a failed diff.
  [@test] ../../src/lib/diff/bundled-languages.test.ts (`keeps only the bundled entries of Shiki's language list, so the other grammars leave the build`, `fails the build for an id Shiki does not have`, `highlights %s with a bundled grammar`, `maps our custom extensions to bundled grammars`, `shows a file of a grammar the build leaves out as plain text`)
  [@test] ../../src/lib/diff/file-diff.test.ts (`keeps a whole git patch as it is, also its last context line of a blank line`, `parses a git patch with its object ids, partial without the contents`, `has no hunks for a file without hunks`)
  [@test] ../../src/lib/diff/diff-languages.test.ts (`maps our custom extensions to grammars Shiki has`, `highlights %s as %s`)
  [@test] ../../src/lib/diff/islands-themes.test.ts (`colour Kotlin as the IntelliJ Light scheme does`, `colour Rust and TypeScript with the dark scheme`)
- Highlighting must not block the window: it runs in a worker pool, which starts with the first diff shown and lives
  as long as the window. When the workers cannot start, the diff is highlighted on the main thread.
- The section must offer the unified and the split view; the choice is the one of the conflict viewer.
- Another commit (↑, ↓) must show its own files, collapsed.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)
- F must show or hide the section, and so must the `F Files` button of the footer. The choice must hold for the
  next preview, until the app restarts.
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should show and hide the files with F and the footer button`)
- A preview of a commit no list loaded must load its files alone (`get_commit_files`).
  [@test] ../../src/stores/commit-files.test.ts (`loads the one commit a preview shows with the single command`)
- A failed read must show "Cannot read the files of the commit" in the section.

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
- In a row, the line counts are muted like the rest of the meta line. In the header of the commit preview, `+42` is
  `text-success` and `−7` is `text-destructive`.

### Files section

- A `border-border` rule above the section separates it from the message. The section title "Files" is
  `text-sm font-medium`, with the `Unified`/`Split` toggle on its right.
- The tree rows are `h-6 rounded-md text-sm`, `hover:bg-muted`, indented by 0.75rem per level, a `ring-2 ring-ring/50`
  inset on the focused row. A row starts with a `ChevronRightIcon` (`size-3.5` muted, turned down when open). A 1px
  `border` guide per level runs under the chevron of each parent, on the rows only (not on a diff under a row).
- A directory shows `FolderIcon` (`FolderOpenIcon` when open, `size-4` muted), its name (truncated, the full path in
  its title) and its counts muted in `text-xs`.
- A file shows its name in its status colour, IntelliJ's: added `text-success`, deleted muted and struck through, the
  others `text-primary`. A rename adds `← api/Old.kt` muted in `text-xs`, which gives way first. On the right: the
  coloured `+/−` counts in `text-xs` and the status letter in `font-mono text-xs` in the status colour. A long name is
  cut with "…" only once nothing else gives way; the counts never wrap or shrink. A zero count is not coloured.
- The diff of an expanded file is a `rounded-lg border` panel under its row, at the row's indent.
- The diff is JetBrains Mono at 12px on the `card` background, added lines `success`, deleted `destructive`, a bar in
  the gutter, the changed words a stronger tint ([UI Theme](../../docs/design/ui-theme.md) "Diffs").
- Loading: an inline spinner and "Loading files…" in muted `text-sm`; a file's diff shows "Loading diff…". A failed
  read of the patch is a destructive callout "Cannot read the changes of the commit" in the panel.
- The footer shows the ghost `xs` buttons `D Review Changes` and `F Files` (pressed while the section is shown) before
  `E Edit`.

## Testing / Local Run

- `pnpm test:backend`: `crates/git-ops/src/commit_files_test.rs`, `crates/object-cache/src/cache_test.rs`,
  `src-tauri/src/repository_state_test.rs`.
- `pnpm test:unit`: `src/lib/commit-files.test.ts`, `src/lib/file-tree.test.ts`, `src/stores/commit-files.test.ts`,
  `src/stores/commit-patch.test.ts`, `src/lib/diff/*.test.ts`.
- `pnpm e2e`: `tests/e2e/commit-files`, on the `issue_links` and `commit_tree` templates (nested directories, a Kotlin
  file with two hunks). The test server runs the commands on the fixture repositories without the object cache.

## Open Questions / Risks

- A commit with thousands of files renders a row per file in the preview, its directories collapsed; the tree is not
  virtualized.
- The patch of a commit is read whole for its first diff. One `diff-tree -p` of a large commit costs more than the
  patch of one file did, but every other file of the commit costs nothing.
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
