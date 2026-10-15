---
name: Commit Files
description: Requirements for the changed files of a commit, the file and line counts on its meta line, the Files section with the diffs in the commit preview, and Copy Changed Paths.
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
  - ../../src/components/conflict/OnDemandFileDiffList.tsx
  - ../../src/components/conflict/CollapsibleFileHeader.tsx
  - ../../src/components/diff/FileDiffView.tsx
  - ../../src/components/diff/LazyDiffViews.tsx
  - ../../src/lib/diff/file-diff.ts
  - ../../src/lib/diff/diff-engine.ts
  - ../../src/lib/diff/diff-languages.ts
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
The commit preview lists the files after the message. A file expands to its diff, read when it is first expanded.
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
- Each file must show its path (`old → new` for a rename or a copy), its line counts and its status badge.
- A file must expand to its diff. The diff must be read when the file is first expanded, never before.
- The diff of a renamed file must show the changed lines, not a new file.
- A path must be taken literally, never as a pattern.
  [@test] ../../crates/git-ops/src/commit_files_test.rs (`file_diff_of_a_rename_shows_the_changed_lines`, `file_diff_takes_the_path_literally`)
- A binary file must show "Binary file", and a file without changed lines "No changed lines", instead of a diff.
- The diff must mark the changed words of a changed line, and highlight the code as the IntelliJ editor does, in the
  light and the dark scheme. Kotlin, Gradle (`.gradle`, `.gradle.kts`), Rust, TypeScript, Java and the other common
  languages must be read by their grammar; an unknown file is plain text.
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
- Each file is a `rounded-lg border` panel: the path in `font-mono text-xs` muted, the copy button on hover (as in
  the amend card), then on the right the coloured `+/−` counts in `text-xs` and the status badge in the subtle badge
  colour of its status (added `success`, modified `info`, deleted `destructive`, the others neutral) and the chevron.
  A long path is cut with "…"; the counts and the badge never wrap or shrink.
- The diff is JetBrains Mono at 12px on the `card` background, added lines `success`, deleted `destructive`, a bar in
  the gutter, the changed words a stronger tint ([UI Theme](../../docs/design/ui-theme.md) "Diffs").
- Loading: an inline spinner and "Loading files…" in muted `text-sm`; a file's diff shows "Loading file content…".
- The footer shows a ghost `xs` button `F Files` (pressed while the section is shown) before `E Edit`.

## Testing / Local Run

- `pnpm test:backend`: `crates/git-ops/src/commit_files_test.rs`, `crates/object-cache/src/cache_test.rs`,
  `src-tauri/src/repository_state_test.rs`.
- `pnpm test:unit`: `src/lib/commit-files.test.ts`, `src/stores/commit-files.test.ts`, `src/lib/diff/*.test.ts`.
- `pnpm e2e`: `tests/e2e/commit-files`. The test server runs the commands on the fixture repositories without the
  object cache.

## Open Questions / Risks

- A commit with thousands of files renders a header per file in the preview; nothing caps the list yet.
- The counts are those of the original commit. A virtual copy that git merged differently may differ slightly.
- No e2e test covers a rename, a binary file or a failed read: the templates have none.
- The Activity entry of a push does not count files yet.

## References

- [Git Operations](../../docs/design/git-operations.md) ("Commit files", "Object cache")
- [Diff View](../../docs/design/diff-view.md)
- [Commit Row](commit-row.spec.md)
- [Branch Preview](../branch/branch-preview.spec.md)
- GitHub and GitLab show `+42 −7` per commit and per file, with the added lines green and the deleted ones red.
