---
name: Commit Files
description: Requirements for the changed files of a commit, the file and line counts on its meta line, the Files section with its directory tree and the diffs in the commit preview, the actions on a file, and Copy Changed Paths.
style: plain-1
targets:
  - crates/git-ops/src/commit_files.rs
  - crates/object-cache/src/commit_files.rs
  - crates/file-diffs/src/range_files.rs
  - crates/app-api/src/reads/commits.rs
  - crates/app-api/src/repository_state.rs
  - src/stores/commit-files.ts
  - src/hooks/useCommitFiles.ts
  - src/lib/commit-files.ts
  - src/components/commit/CommitFilesSection.tsx
  - src/components/commit/CommitMetadata.tsx
  - src/components/commit/CommitList.tsx
  - src/components/commit/CommitPreviewDialog.tsx
  - src/components/branch/BranchPreviewDialog.tsx
  - src/components/commit/InlineCommitFileDiff.tsx
  - src/components/files/FileTree.tsx
  - src/components/files/FileContextMenu.tsx
  - src/lib/file-actions.ts
  - src/lib/file-tree.ts
  - crates/app-api/src/reads/diffs.rs
  - src/lib/diff/view-files.ts
  - src/lib/diff/file-diff-model.ts
  - src/lib/diff/rows.ts
  - src/lib/diff/line-parts.ts
  - src/components/diff/DiffView.tsx
  - src/stores/file-diffs.ts
  - src/stores/blob-texts.ts
  - src/components/shared/DiffViewToggle.tsx
  - src/stores/settings/diff-view-settings.ts
  - src/components/settings/DiffsPage.tsx
  - src/lib/preview-keys.ts
  - crates/git-ops/src/cat_file_batch.rs
  - crates/file-diffs/src/service.rs
  - crates/file-diffs/src/worktree.rs
  - crates/file-diffs/src/model.rs
  - crates/text-diff/src/model.rs
  - crates/object-cache/src/file_diff.rs
  - crates/app-api/src/reads/blobs.rs
  - crates/test-utils/src/repo_template.rs
  - src/components/conflict/CollapsibleFileHeader.tsx
  - src/components/diff/LazyDiffViews.tsx
  - src/lib/diff/file-languages.ts
  - src/lib/diff/bundled-languages.ts
  - vite.config.ts
  - src/lib/diff/islands-themes.ts
  - src/index.css
  - src/hooks/useCommitCopyActions.ts
  - tests/e2e/helpers/aria-snapshot-helpers.ts
---

# Commit Files

Status: Implemented (the typed file diffs: the backend; the views still show the patch)
Date: 2026-10-18

## Purpose

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
- The diffs of uncommitted changes stay in the amend card ([Amend Changes](../amend/spec.md)).

## Concepts

### Files section

Before the one-sided view of an added or a deleted file, Split showed an empty grey half beside it.

## Requirements

### Requirement: Reading the files: The files are the changes against the first parent

The files of a commit MUST be its changes against its first parent, with renames detected. A root commit adds every
file. A merge commit shows the changes against its first parent.

#### Scenario: A root commit and a renamed file
- **WHEN** the files of a root commit and of a commit that renames a file are read
- **THEN** the root commit adds every file
- **AND** the renamed file is detected as a rename
  [@test] crates/git-ops/src/commit_files_test.rs (`lists_the_status_paths_and_line_counts_of_each_file`)

#### Scenario: A merge commit
- **WHEN** the files of a merge commit are read
- **THEN** they are the changes against its first parent
  [@test] crates/git-ops/src/commit_files_test.rs (`lists_a_merge_against_its_first_parent_and_nothing_for_an_empty_commit`)

### Requirement: Reading the files: Each file has its status, paths, line counts and blob ids

Each file MUST have its status (added, modified, deleted, renamed, copied, type changed), its path, the path before a
rename or copy, its added and deleted lines, whether git counts no lines (a binary file), and the blob ids of its two
sides: none for a side the file does not exist on, and the commit of a submodule.

#### Scenario: The status, the paths and the line counts of each file
- **WHEN** the files of a commit are read
- **THEN** each file has its status, its path, the path before a rename, its added and deleted lines
- **AND** a binary file is marked as one git counts no lines for
  [@test] crates/git-ops/src/commit_files_test.rs (`lists_the_status_paths_and_line_counts_of_each_file`)

#### Scenario: The blob ids of both sides
- **WHEN** a file exists on both sides, or on one side only
- **THEN** it has the blob id of each side it exists on, and none for the other
  [@test] crates/git-ops/src/commit_files_test.rs (`each_file_has_the_blob_ids_of_its_sides`)

#### Scenario: A submodule
- **WHEN** a commit changes a submodule
- **THEN** the ids of its sides are the commits of the submodule
  [@test] crates/git-ops/src/commit_files_test.rs (`a_submodule_has_the_ids_of_its_commits`)

#### Scenario: The output of git is read by its entries
- **WHEN** git lists the line counts of the files in another order than their raw entries
- **THEN** each file still gets its own status, paths, line counts and blob ids
  [@test] crates/git-ops/src/commit_files_test.rs (`parses_numstat_entries_in_another_order_than_the_raw_entries`, `rejects_output_it_does_not_understand`)

### Requirement: Reading the files: The paths are the bytes git stores

The paths MUST be the bytes git stores: spaces, tabs and non-ASCII letters stay as they are.

#### Scenario: A path with a space, a tab and a non-ASCII letter
- **WHEN** a commit adds a file under a path with a space, a tab and a non-ASCII letter
- **THEN** its path is listed as git stores it
  [@test] crates/git-ops/src/commit_files_test.rs (`lists_the_status_paths_and_line_counts_of_each_file`)

### Requirement: Reading the files: The files of many commits are read by one process

The files of many commits MUST be read by one `diff-tree --stdin --raw --numstat -z` process.

#### Scenario: A list of many commits
- **WHEN** the files of many commits are read
- **THEN** one `diff-tree --stdin --raw --numstat -z` process reads them all

### Requirement: Reading the files: The files are cached by commit id

The files MUST be cached by commit id in the object cache of the repository, which the app shares with `brd`. A
commit read before MUST cost no git process.

#### Scenario: The object cache keeps the files of a commit
- **WHEN** the files of a commit are put into the object cache
- **THEN** they are found again by the commit id
  [@test] crates/object-cache/src/cache_test.rs (`round_trip`)

#### Scenario: A commit read before
- **GIVEN** the files of a commit were read before
- **WHEN** they are read again
- **THEN** no git process runs
  [@test] crates/git-ops/src/commit_files_test.rs (`reads_through_the_cache_once_per_commit`)

### Requirement: Reading the files: A commit git lists nothing for is not cached

A commit git lists nothing for MUST NOT be cached: it has no changes, or it is not in the repository yet.

#### Scenario: An empty commit
- **WHEN** git lists nothing for a commit
- **THEN** the commit is not cached
  [@test] crates/git-ops/src/commit_files_test.rs (`reads_through_the_cache_once_per_commit`)

### Requirement: Reading the files: The object cache lives in the common git directory

The object cache MUST live in the common git directory of the repository, which the app resolves once per repository.
Outside a repository the cache is disabled, and the files are still read.

#### Scenario: The cache of a repository and of a directory outside one
- **WHEN** the app opens the object cache of a repository twice, and of a directory outside a repository
- **THEN** the cache lives in the common git directory, resolved by the first call only
- **AND** outside a repository the cache is disabled, and the files are still read
  [@test] crates/app-api/src/repository_state_test.rs (`object_cache_lives_in_the_common_dir_of_the_repository`)

### Requirement: Reading the files: A commit id is a full object id

A commit id MUST be a full object id: an abbreviation, a ref or an option is refused.

#### Scenario: What is not an object id
- **WHEN** the files of an abbreviation, a ref or an option are asked for
- **THEN** the read is refused
  [@test] crates/git-ops/src/commit_files_test.rs (`refuses_what_is_not_an_object_id`)

### Requirement: Reading the files: Nothing writes to a ref, the index or the working tree

Nothing MUST write to a ref, the index or the working tree.

#### Scenario: A read of the files
- **WHEN** the files of commits are read
- **THEN** nothing is written to a ref, the index or the working tree

### Requirement: Reading the files: The files of a range are one diff-tree of its two commits

The files of a range `base..head` (the review of a pull request,
[Review Requests](../../repository/review-requests/spec.md#review-window), or of a branch) MUST be one
`diff-tree --raw --numstat -z` of the two commits (never `--stdin`, so a range never lands in the disk cache entry of
its head).

#### Scenario: The files of a range
- **WHEN** the files of a range `base..head` are read
- **THEN** they are the changes of all its commits, from one `diff-tree --raw --numstat -z` of its two commits
- **AND** the range never lands in the disk cache entry of its head
  [@test] crates/git-ops/src/commit_files_test.rs (`range_files_are_the_changes_of_all_its_commits`)

### Requirement: Reading the files: The files of a range are kept in memory only

The files of a range MUST be kept in memory only, at most 16 MiB by their paths, whatever the number of ranges.

#### Scenario: A range read again
- **GIVEN** the files of a range were read
- **WHEN** they are read again
- **THEN** they come from memory, without git
  [@test] crates/file-diffs/src/range_files_test.rs (`the_files_of_a_range_stay_in_memory`)

#### Scenario: Many ranges
- **WHEN** the files of many ranges are read
- **THEN** the memory keeps at most 16 MiB by their paths, whatever the number of ranges

### Requirement: Reading the files: The reads of a review run one process for its range

The reads of a review, its prefetch and the diffs of the review (`get_file_diffs` of the range) MUST run one process,
also when they ask at once.

#### Scenario: The list and the diffs of a range at once
- **WHEN** the list of the files of a range and its diffs are asked for at once
- **THEN** one `diff-tree` runs
  [@test] crates/file-diffs/src/range_files_test.rs (`the_list_and_the_diffs_of_a_range_run_one_diff_tree`)

### Requirement: Reading the files: A range is keyed by its id until its review closes

The id of a range MUST be `<base>..<head>`, both ids validated; the app keys it by that id and drops it when the
review closes.

#### Scenario: A range of what is not an object id
- **WHEN** the base or the head of a range is not an object id
- **THEN** the read is refused
  [@test] crates/git-ops/src/commit_files_test.rs (`a_range_refuses_what_is_not_an_object_id`)

#### Scenario: The app keys a range by its id
- **WHEN** a review loads its range
- **THEN** the app keys its files by `base..head` until its review drops them
  [@test] src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)

### Requirement: Reading the diffs: The views read all diffs with one get_file_diffs

The views MUST read the diffs of every file of a commit or a range with one `get_file_diffs` under the whitespace
policy: typed blocks of the diff engine, no patch text.

#### Scenario: The diffs of a subject under a policy
- **WHEN** a view shows the diffs of a commit or a range under a whitespace policy
- **THEN** one `get_file_diffs` under that policy reads the typed blocks of every file, no patch text
  [@test] src/stores/file-diffs.test.ts (`reads a subject once per policy, the files in view first`)

### Requirement: Reading the diffs: The diffs stream, the files in view first

The diffs stream ([File diffs](#file-diffs)): a file MUST show once its content arrives and "Loading…" before, the
files in view MUST be asked for first (`priorityPaths`, the file a review opens at), and the late scopes of a text
MUST name its folds without losing the lines it expanded.

#### Scenario: The files in view are asked for first
- **WHEN** a view reads the diffs of a subject
- **THEN** it asks for the files in view first (`priorityPaths`, the file a review opens at)
  [@test] src/stores/file-diffs.test.ts (`reads a subject once per policy, the files in view first`)

#### Scenario: A file shows as its content arrives
- **WHEN** the content of a file arrives, and later the scopes of its text
- **THEN** the file shows once its content arrives, and "Loading…" before
- **AND** the late scopes name its folds without losing the lines it expanded
  [@test] src/stores/file-diffs.test.ts (`shows each file as its content arrives, and the late scopes of a text keep the lines it expanded`)

### Requirement: Reading the diffs: The app keeps the diffs of the last 8 subjects per policy

The app MUST keep the diffs of the last 8 subjects per policy, and drop the ranges of a closed review.

#### Scenario: More subjects, and a closed review
- **WHEN** more subjects are read, and a review closes
- **THEN** the app keeps the diffs of the last 8 subjects per policy only
- **AND** it drops the ranges of the closed review under every policy
  [@test] src/stores/file-diffs.test.ts (`keeps the last subjects only, and forgets the subjects of a closed review under every policy`)

### Requirement: Reading the diffs: A subject is read once while it is being read

The app MUST read a subject once while it is being read, and try a failed read again.

#### Scenario: A subject asked for twice
- **WHEN** a subject is asked for again while it is being read
- **THEN** it is read once
  [@test] src/stores/file-diffs.test.ts (`reads a subject once per policy, the files in view first`)

#### Scenario: A failed read
- **WHEN** the read of a subject failed
- **THEN** the next request tries it again

### Requirement: Reading the diffs: The loaded uncommitted changes stay until their new read ends

The app MUST show the loaded uncommitted changes until their new read ends.

#### Scenario: A refresh of the uncommitted changes
- **GIVEN** the uncommitted changes are loaded
- **WHEN** a refresh reads them again
- **THEN** the loaded ones show until the new read ends
  [@test] src/stores/file-diffs.test.ts (`reads the uncommitted changes again on a refresh, and keeps the loaded ones until the new read ends`)

### Requirement: Reading the diffs: The texts are read by blob id

The texts of a diff MUST be read by git blob id (`get_blob_texts`), shared by every commit, range and window, the new
side of an uncommitted change by its path too: the ids of one task in one call (64 at most), each id once.

#### Scenario: The ids of one task
- **WHEN** a task needs the texts of its ids
- **THEN** they are read in one call, each id once
  [@test] src/stores/blob-texts.test.ts (`reads the ids of one task in one call, and each id once`)

#### Scenario: Many ids
- **WHEN** a task needs the texts of more than 64 ids
- **THEN** they are read in calls of 64 at most
  [@test] src/stores/blob-texts.test.ts (`reads many ids in batches`)

### Requirement: Reading the diffs: The texts on screen are read at once

The texts on screen MUST be read at once, also the ones a background read still queues, and a shown text never waits
for a background read.

#### Scenario: A shown text while a background call hangs
- **GIVEN** a background read (Find) queues texts and its call hangs
- **WHEN** the screen shows texts, some of them queued by it
- **THEN** they are read at once, without waiting for it
  [@test] src/stores/blob-texts.test.ts (`reads the shown texts at once, also the ones a Find still queues, while a background call hangs`)

### Requirement: Reading the diffs: The background reads of texts run one call at a time

The background reads (Find, the files after the window) MUST run one call in flight at a time: Find over a review of a
thousand files starts no 16 `cat-file` processes at once.

#### Scenario: A large Find
- **WHEN** Find reads the texts of a review of a thousand files
- **THEN** its batches are read one after another, at most one call in flight
- **AND** no 16 `cat-file` processes start at once
  [@test] src/stores/blob-texts.test.ts (`reads the batches of a large Find one after another, at most one call in flight`)

### Requirement: Reading the diffs: A failed read of a text is tried again

A failed read of a text MUST be tried again.

#### Scenario: A failed read
- **WHEN** the read of a text failed
- **THEN** it is kept as an error and read again on the next request
  [@test] src/stores/blob-texts.test.ts (`keeps a failed read as an error and reads it again on the next request`)

### Requirement: Reading the diffs: The least recently used texts are dropped over 64 Mi units

The least recently used texts MUST be dropped over 64 Mi UTF-16 units, never the ones just read.

#### Scenario: More texts than the length allows
- **WHEN** the texts read are longer than 64 Mi UTF-16 units
- **THEN** the least recently requested texts are dropped, never the ones just read
  [@test] src/stores/blob-texts.test.ts (`drops the least recently requested texts over the length, never the ones just read`)

### Requirement: Reading the diffs: A file's diff is built complete from its blocks and texts

A file's diff MUST be built from its blocks and both texts, complete: the unchanged lines between hunks expand without
a request. The same diff MUST give the same model, so the lines it expanded stay expanded.

#### Scenario: The unchanged lines between hunks
- **WHEN** the user expands the unchanged lines between two hunks
- **THEN** they show without a request
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)

#### Scenario: The same diff again
- **WHEN** the same diff is given again
- **THEN** it gives the same model, and the lines it expanded stay expanded
  [@test] src/lib/diff/rows.test.ts (`finds each row from its segment, and keeps the fold levels of a diff object`)

### Requirement: Reading the diffs: The counts of a file are the engine's

The `+N −M` of a file and a directory in the tree, in the header of a diff and in the uncommitted card MUST be the
counts of the engine under the whitespace policy, and show nothing until its diffs are read, never git's count first.

#### Scenario: Before and after the diff is read
- **WHEN** the tree shows a file before its diff is read, and after
- **THEN** it shows no counts before, and the counts of the engine after
  [@test] src/lib/diff/view-files.test.ts (`takes the line counts of the engine, and none before its diff is read`)

### Requirement: Reading the diffs: The meta line of a commit row keeps git's numstat

The meta line of a commit row SHALL keep git's numstat, which reads no blob, so the two counts can differ.

#### Scenario: A commit whose counts differ under the policy
- **WHEN** the engine counts the lines of a commit otherwise than git's numstat
- **THEN** its row shows git's numstat, and its files the counts of the engine

### Requirement: File diffs: The views show the typed diff of each file

The views SHALL show the typed diff of each file instead of the patch ([Reading the diffs](#reading-the-diffs)).

#### Scenario: A file of a commit in a view
- **WHEN** a view shows a file of a commit
- **THEN** it shows the typed diff of the file, not the patch

### Requirement: File diffs: get_file_diffs gives the diff of each changed file

`get_file_diffs` MUST give the diff of each changed file of a commit, a range or the working tree (`DiffSource`) under
a whitespace policy, in the order of git: its path, the path before a rename or copy, its status, the blob ids of both
sides, and its content.

#### Scenario: The files of a commit
- **WHEN** `get_file_diffs` reads a commit
- **THEN** each changed file has its path, the path before a rename, its status, the blob ids of both sides and its content
  [@test] crates/file-diffs/src/service_test.rs (`a_commit_has_a_typed_diff_of_each_kind_of_file`)
  [@test] crates/test-server/src/commit_files_test.rs (`commit_files_routes_read_the_fixture_repository`)

#### Scenario: The files of a range
- **WHEN** `get_file_diffs` reads a range
- **THEN** it gives the diffs of all its commits
  [@test] crates/file-diffs/src/service_test.rs (`a_range_has_the_diffs_of_all_its_commits`)

### Requirement: File diffs: The content of a file diff is typed

The content MUST be a `TextDiff` of the engine ([Text Diff Engine](../../../../docs/design/text-diff.md), "Model"),
`Binary`, `TooLarge` with the sizes, `Submodule` with the commit of each side ("Subproject commit a → b"), or `Missing`
for a blob git cannot read. No patch text carries it.

#### Scenario: A text, a binary file and a file too large
- **WHEN** a commit changes a text, a binary file and a file over the limits
- **THEN** the text is a `TextDiff`, the binary file `Binary`, and the large file `TooLarge` with the sizes
- **AND** the model is JSON with a kind, and no patch text
  [@test] crates/file-diffs/src/service_test.rs (`a_commit_has_a_typed_diff_of_each_kind_of_file`, `the_model_is_json_with_a_kind`)
  [@test] crates/git-ops/src/cat_file_batch_test.rs (`the_content_of_an_object_over_the_limit_is_not_buffered`)

#### Scenario: A submodule
- **WHEN** a commit changes a submodule
- **THEN** its content is `Submodule` with the commit of each side ("Subproject commit a → b")
  [@test] crates/file-diffs/src/service_test.rs (`a_submodule_names_its_commits`)

#### Scenario: A blob git cannot read
- **WHEN** git cannot read a blob of the file
- **THEN** its content is `Missing`
  [@test] crates/file-diffs/src/service_test.rs (`a_blob_git_cannot_read_is_missing_and_not_cached`)

### Requirement: File diffs: The diffs stream, the files in view first

The diffs MUST stream: the files first, then the content of each file as it is known, the files in view
(`priorityPaths`) first and the others in the order of the tree, then the scopes of a text that the caches did not
have.

#### Scenario: The order of the stream
- **WHEN** `get_file_diffs` streams the diffs of a commit with files in view
- **THEN** the files come first, then the content of each file as it is known, the files in view (`priorityPaths`) first
- **AND** the scopes of a text the caches did not have come after its blocks
  [@test] crates/file-diffs/src/service_test.rs (`the_stream_lists_the_files_then_the_files_in_view_first_and_their_scopes_after_their_blocks`)
  [@test] crates/test-server/src/commit_files_test.rs (`commit_files_routes_read_the_fixture_repository`)

#### Scenario: The files after the ones in view
- **WHEN** the files in view are done
- **THEN** the others follow in the order of the tree
  [@test] crates/file-diffs/src/service_test.rs (`the_work_takes_the_files_in_view_then_the_tree_order_with_directories_first_and_numbers_as_numbers`)

### Requirement: File diffs: The result does not depend on the load of the machine

The result MUST NOT depend on the load of the machine: only a pathological file (100 ms for its inner ranges, 1 s for
its lines) is compared by line.

#### Scenario: A file out of time
- **WHEN** a file takes more than 100 ms for its inner ranges or more than 1 s for its lines
- **THEN** it is compared by line
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_out_of_time_is_by_line_and_not_stored`)

### Requirement: File diffs: The diff of a commit costs one list and one cat-file

The diff of a commit MUST cost one file list and one `cat-file --batch`, and nothing when the caches have it.

#### Scenario: A diff read twice
- **WHEN** the diff of a commit is read, then read again
- **THEN** the first read costs one file list and one `cat-file --batch`, the second nothing
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_costs_one_list_and_one_cat_file_then_nothing`)

### Requirement: File diffs: The caches key a diff by its blob pair, policy and languages

The caches MUST key a diff by its blob pair, the policy and the languages of the scopes, so two commits with the same
blob pair share one diff. The store MUST NOT keep a diff that the time cut short.

#### Scenario: Two commits with the same blob pair
- **WHEN** two commits change a file from the same blob to the same blob
- **THEN** they share one diff
  [@test] crates/file-diffs/src/service_test.rs (`two_commits_with_the_same_blob_pair_share_its_diff`)

#### Scenario: Another policy
- **WHEN** the same blob pair is read under another policy
- **THEN** that policy keys its own diff
  [@test] crates/file-diffs/src/service_test.rs (`the_policy_keys_its_own_diff`)
  [@test] crates/object-cache/src/cache_test.rs (`file_diffs_are_read_by_their_blob_pair_policy_and_version`)

#### Scenario: A diff the time cut short
- **WHEN** the time cut a diff short
- **THEN** the store does not keep it
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_out_of_time_is_by_line_and_not_stored`)

### Requirement: File diffs: The working tree is compared with HEAD

The working tree MUST be compared with `HEAD`: staged, unstaged and untracked files, from one `status`. The new side is
the file on the disk, with the blob id that `git hash-object --no-filters` gives its bytes.

#### Scenario: Staged, unstaged and untracked files
- **WHEN** `get_file_diffs` reads the working tree
- **THEN** its staged, unstaged and untracked files are compared with `HEAD`, from one `status`
- **AND** the new side of each is the file on the disk, by the id `git hash-object --no-filters` gives its bytes
  [@test] crates/file-diffs/src/service_test.rs (`the_working_tree_is_compared_with_head_by_the_ids_git_gives_its_files`)

### Requirement: File diffs: get_blob_texts gives the texts by blob id

`get_blob_texts` MUST give the texts by blob id, from the blobs that the diffs read when it can. A file of the working
tree is read by its path, and only when its bytes still have the id; a path outside the repository is refused.

#### Scenario: The texts of a diff
- **GIVEN** the diffs read the blobs of a commit
- **WHEN** `get_blob_texts` asks for their texts
- **THEN** they come from memory
  [@test] crates/file-diffs/src/service_test.rs (`the_texts_of_a_diff_come_from_memory`)

#### Scenario: A text of the working tree
- **WHEN** `get_blob_texts` asks for a file of the working tree
- **THEN** it is read by its path, and only when its bytes still have the id
- **AND** a path outside the repository is refused
  [@test] crates/file-diffs/src/service_test.rs (`a_working_tree_text_is_read_by_its_path_and_checked_against_its_id`)

### Requirement: File diffs: A hunk header names the declaration around it

The hunk headers MUST name the declaration around each hunk, `Outer › inner`, from the scopes of the diff.

#### Scenario: A member and a Rust impl member
- **WHEN** a hunk changes a member, or a member of a Rust impl
- **THEN** its header names it with its containers, `Outer › inner`, a Rust impl member with its type
  [@test] crates/file-diffs/src/scopes_test.rs (`a_member_is_labelled_with_its_containers`, `a_rust_impl_member_is_labelled_with_its_type`)

### Requirement: File diffs: The views build the hunks from the blocks and the texts

The views MUST build the context, the hunks and the expansion from the blocks and the texts: the tints of IntelliJ
(below), an `Ignored` block as unchanged and skipped by N and P, the inline pairs of a block in one row in the Inline
view, and a notice when the files differ in their line ends only.

#### Scenario: A whitespace-only block and an ignored block
- **WHEN** a diff has a whitespace-only block and a block the policy ignores
- **THEN** the first is drawn in the whitespace tint, the second as unchanged lines inside the context
  [@test] src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`)

#### Scenario: The inline pairs of a block
- **WHEN** the Inline view shows a block of similar pairs
- **THEN** each pair is one row
  [@test] src/lib/diff/rows.test.ts (`makes each similar pair of a block one row only in the Inline view, also at the end of a file`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Inline shows a lightly edited line as one row`)

#### Scenario: The context, the hunks and the expansion
- **WHEN** a file is expanded and its unchanged lines are expanded
- **THEN** the view builds the context, the hunks and the expansion from the blocks and the texts
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)

#### Scenario: Files that differ in their line ends only
- **WHEN** the two sides of a file differ in their line ends only
- **THEN** the view shows a notice

### Requirement: Meta line: A list loads the files of its commits with one call

A list MUST load the files of its commits with one call when it is shown or gets other commits, and only for the
commits not known yet. A second request for a commit MUST wait for the one in flight.

#### Scenario: A list is shown or gets other commits
- **WHEN** a list is shown, or gets other commits
- **THEN** it loads the files of its commits with one call, only for the commits not known yet
  [@test] src/stores/commit-files.test.ts (`loads the commits of a list with one call, and only the ones not known yet`)

#### Scenario: A second request during a load
- **GIVEN** the files of a commit are being loaded
- **WHEN** another request asks for that commit
- **THEN** it waits for the load in flight instead of asking again
  [@test] src/stores/commit-files.test.ts (`waits for a load in flight instead of asking again`)

### Requirement: Meta line: A list is aria-busy while its files are read

A list MUST be `aria-busy` while the files of its commits are read, so that a snapshot waits for the counts.

#### Scenario: The files of a list are being read
- **WHEN** the files of the commits of a list are being read
- **THEN** the list is `aria-busy`, so that a snapshot waits for the counts

### Requirement: Meta line: A failed load is logged, not shown

A failed load MUST be logged, not shown; the rows go without counts, and the next load tries again.

#### Scenario: The load of the files fails
- **WHEN** the load of the files of a list fails
- **THEN** it is logged, not shown, and the rows go without counts
- **AND** the next load tries again
  [@test] src/stores/commit-files.test.ts (`tries again after a failed load`)

### Requirement: Meta line: The meta line ends with the file and line counts

Once known, the meta line of a commit row MUST end with the file count and the line counts: `1 file · +1 −0`.

#### Scenario: The files of a commit are known
- **WHEN** the files of a commit of a list are known
- **THEN** the meta line of its row ends with the file count and the line counts, `1 file · +1 −0`
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should show the file count and the line counts in the meta line of each commit row`)

### Requirement: Meta line: A commit without lines or files shows less

A commit without changed lines (only renames, mode changes or binary files) MUST show the file count only. A commit
without files MUST show neither.

#### Scenario: A commit without changed lines
- **WHEN** a commit has only renames, mode changes or binary files
- **THEN** its meta line shows the file count only

#### Scenario: A commit without files
- **WHEN** a commit has no files
- **THEN** its meta line shows neither count

### Requirement: Meta line: The commits of the branch preview show the same meta line

The commits of the branch preview MUST show the same meta line.

#### Scenario: A commit in the branch preview
- **WHEN** the branch preview shows its commits
- **THEN** each shows the meta line with its files and line counts
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should show the meta line with the files in the commits of the branch preview`)

### Requirement: Meta line: The commit preview shows the counts

The meta line of the commit preview MUST show the counts too.

#### Scenario: The meta line of the commit preview
- **WHEN** the commit preview shows a commit whose files are known
- **THEN** its meta line shows the counts

### Requirement: Files section: The commit preview shows a Files section after the message

The commit preview MUST show a "Files" section after the message, unless the message is being edited.

#### Scenario: The preview of a commit
- **WHEN** the commit preview shows a commit
- **THEN** a "Files" section follows its message
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)

#### Scenario: The message is being edited
- **WHEN** the message of the commit is being edited
- **THEN** the preview shows no "Files" section

### Requirement: Files section: The section starts closed and reads nothing until it opens

The section MUST start closed: its header ("Files 4", the count once a list read the files of the commit) with a
chevron, no tree, and nothing of the files is read or rendered for it until it opens.

#### Scenario: A preview opens
- **WHEN** the commit preview opens
- **THEN** the section shows its header ("Files 4", the count once a list read the files of the commit) with a chevron, and no tree
- **AND** nothing of the files is read or rendered for it until it opens
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`, `should open and close the files with their header, F and the footer button`)

### Requirement: Files section: A commit without files says so

A commit without files MUST say "No changed files" in the section.

#### Scenario: A commit without files
- **WHEN** the section of a commit without files is open
- **THEN** it says "No changed files"

### Requirement: Files section: The files are a tree grouped by directory

The files MUST be a tree grouped by directory. A directory that holds nothing but one directory MUST be one row with
the joined name (`plugins/air/backend/session`).

#### Scenario: A chain of directories with one subdirectory
- **WHEN** a directory holds nothing but one directory, down a chain
- **THEN** the chain is one row with the joined name (`plugins/air/backend/session`)
  [@test] src/lib/file-tree.test.ts (`merges a chain of directories with one subdirectory into one row`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)

#### Scenario: A directory with files and one subdirectory
- **WHEN** a directory holds files beside its one subdirectory
- **THEN** it stays a row of its own, apart from the subdirectory
  [@test] src/lib/file-tree.test.ts (`keeps the files of a directory with one subdirectory apart from it`)

#### Scenario: A file under a path with spaces and non-ASCII letters
- **WHEN** a path has spaces and letters beyond ASCII
- **THEN** the tree takes it as it is
  [@test] src/lib/file-tree.test.ts (`takes a path with spaces and letters beyond ASCII as it is`)

### Requirement: Files section: Directories come first, names in a human order

Directories MUST come before files, names in a human order (`file2` before `file10`, case-insensitive).

#### Scenario: Directories, files and numbered names
- **WHEN** a directory holds directories and files with numbered names
- **THEN** the directories come first, then the files, `file2` before `file10`, case-insensitive
  [@test] src/lib/file-tree.test.ts (`puts directories first and sorts names as a person reads them`, `lists the files in the order of the tree`)

### Requirement: Files section: A directory row shows the sums of its subtree

A directory row MUST show the sums of the line counts of its subtree.

#### Scenario: A directory with files in its subdirectories
- **WHEN** a directory holds files, some in its subdirectories
- **THEN** its row shows the sums of the files and the line counts of its subtree
  [@test] src/lib/file-tree.test.ts (`sums the files and the line counts of each directory over its subtree`)

### Requirement: Files section: Directories start open, below the top level of a large commit collapsed

Every directory MUST start open; in a commit with more than 100 files the directories below the top level start
collapsed. A collapsed directory hides its rows.

#### Scenario: A large commit
- **WHEN** the section opens for a commit with more than 100 files
- **THEN** the directories below the top level start collapsed, and in a smaller commit every directory starts open
  [@test] src/lib/file-tree.test.ts (`collapses the directories below the top level of a large commit only`)

#### Scenario: A collapsed directory
- **WHEN** a directory is collapsed
- **THEN** its rows are hidden, and they show again once the directories above them open
  [@test] src/lib/file-tree.test.ts (`hides the rows below a collapsed directory`, `opens the directories above a row to reveal it, and only those`)

### Requirement: Files section: A file row shows its name, origin, counts and status letter

A file row MUST show its name in the colour of its status, where a renamed or copied file came from (relative to the
directory both paths share), its line counts and the status letter (M, A, D, R, C, T); nothing else takes width from
the name.

#### Scenario: A file in its directory
- **WHEN** the tree shows a file
- **THEN** its row shows its name in its directory, in the colour of its status, its line counts and its status letter
  [@test] src/lib/file-tree.test.ts (`names a file by its name in its directory`)

#### Scenario: A renamed file
- **WHEN** the tree shows a renamed or copied file
- **THEN** its row names where it came from, relative to the directory both paths share
  [@test] src/lib/file-tree.test.ts (`names where a renamed file came from, relative to the directory both paths share`)

### Requirement: Files section: The hover of a file row shows its path

The hover of a file row MUST show the path (`old → new` for a rename) and, in a review, the code owner.

#### Scenario: The hover of a renamed file in a review
- **WHEN** the pointer rests on the row of a renamed file in a review
- **THEN** the hover shows `old → new` and the code owner

### Requirement: Files section: A file row is named by its file name

The accessible name of a file row MUST be the file name, its description the status and the counts.

#### Scenario: A file row read by assistive technology
- **WHEN** assistive technology reads a file row
- **THEN** its name is the file name, its description the status and the counts

### Requirement: Files section: A deleted file is struck through, its status letter never

A deleted file MUST be struck through, its status letter never, in the tree and in the header of its diff.

#### Scenario: A deleted file
- **WHEN** a commit deletes a file
- **THEN** its name is struck through in the tree and in the header of its diff, its status letter never
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should strike a deleted file through, never its status letter, in the tree and above its diff`)

### Requirement: Files section: A file expands to its diff under its row

A file MUST expand to its diff under its row.

#### Scenario: A file is expanded
- **WHEN** the user expands a file row
- **THEN** its diff shows under its row
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)

### Requirement: Files section: The tree is one tab stop moved by the arrows

The tree MUST be one tab stop (roving focus): ↑/↓ move between the rows and never leave the tree, Home and End go to
the first and the last row, → opens a row (a directory, a file's diff) or goes to the first child, ← closes it or goes
to the parent, Space toggles it, Enter toggles a directory and opens a file in the review window at that file
([Commit Review](../commit-review/spec.md)).

#### Scenario: The keys in the tree
- **WHEN** the tree has the focus and the user presses ↑, ↓, Home, End, →, ←, Space and Enter
- **THEN** the focus moves, the rows open and close, and Enter on a file opens the review window at that file
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should move through the tree with the keyboard and switch commits with Alt`)

### Requirement: Files section: Alt-arrows switch the commit from anywhere in the preview

⌥↑/⌥↓ MUST switch the commit from anywhere in the preview; the letters (F, E, D) keep working in the tree.

#### Scenario: Alt-arrows in the tree
- **WHEN** the tree has the focus and the user presses ⌥↓, or a letter (F, E, D)
- **THEN** ⌥↓ switches the commit, and the letter does what it does elsewhere in the preview
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should move through the tree with the keyboard and switch commits with Alt`)

### Requirement: Files section: A file without a text diff says why

A binary file MUST show "Binary file", and a file without changed lines "No changed lines", instead of a diff; a file
over the limits of the engine shows "Too large to show: 12.3 MB".

#### Scenario: A binary file, a file without changed lines, a file too large
- **WHEN** a binary file, a file without changed lines or a file over the limits of the engine is expanded
- **THEN** it shows "Binary file", "No changed lines" or "Too large to show: 12.3 MB" instead of a diff
  [@test] src/lib/diff/view-files.test.ts (`says why a file shows no lines: binary, too large, a submodule, a blob git cannot read, a text that cannot be read, no changed lines`)

### Requirement: Files section: The diff under a row is virtualized

The diff under a row MUST be virtualized in the scroller of the preview, so a large file shows at once, without a
click.

#### Scenario: A large file is expanded
- **WHEN** the user expands a large file
- **THEN** its diff shows at once, without a click, virtualized in the scroller of the preview

### Requirement: Files section: The diff marks the changed words and highlights the code as IntelliJ

The diff MUST mark the changed words of a changed line, and highlight the code as the IntelliJ editor does, in the
light and the dark scheme: a declaration is coloured, a call, a package, an import and a class used as a type are
plain text.

#### Scenario: The roles of the tokens
- **WHEN** a diff of Kotlin, Rust or TypeScript is highlighted
- **THEN** each token gets the role of the Islands themes, in the light and the dark scheme
  [@test] src/lib/diff/highlight/engine.test.ts (`gives each token the role of the Islands themes, in runs that change only with the style`, `gives Rust and TypeScript tokens the roles of the Islands themes too`)

#### Scenario: A declaration, a call, a package, an import and a type
- **WHEN** a diff of Java holds a declaration, a call, a package, an import and a class used as a type
- **THEN** the declaration is coloured, and the call, the package, the import and the class used as a type are plain text
  [@test] src/lib/diff/highlight/engine.test.ts (`leaves a Java call, a package, an import and a class used as a type plain, and colours a Rust call, as IntelliJ and RustRover do`)

### Requirement: Files section: The common languages are read by their grammar

Kotlin, Gradle (`.gradle`, `.gradle.kts`), Rust, TypeScript, Java and the other common languages MUST be read by their
grammar.

#### Scenario: A file of a common language
- **WHEN** a diff shows a file of Kotlin, Gradle (`.gradle`, `.gradle.kts`), Rust, TypeScript, Java or another common language
- **THEN** it is highlighted by its grammar
  [@test] src/lib/diff/file-languages.test.ts (`maps our custom extensions to grammars Shiki has`, `highlights %s as %s`)
  [@test] src/lib/diff/bundled-languages.test.ts (`highlights %s with a bundled grammar`, `maps the extension table and our custom extensions to bundled grammars only`)

### Requirement: Files section: A language the app does not bundle is plain text

A file of a language the app does not bundle (`BUNDLED_LANGUAGE_IDS`: 84 of Shiki's grammars, no Emacs Lisp, COBOL or
Wolfram) and an unknown file MUST be plain text, never a failed diff.

#### Scenario: The bundled grammars
- **WHEN** the app is built
- **THEN** only the grammars of `BUNDLED_LANGUAGE_IDS` are kept, and an id Shiki does not have fails the build
  [@test] src/lib/diff/bundled-languages.test.ts (`keeps only the bundled entries of Shiki's language list, so the other grammars leave the build`, `fails the build for an id Shiki does not have`)

#### Scenario: A file of a grammar left out, and an unknown file
- **WHEN** a diff shows a file of a grammar the build leaves out, or a file without a known name
- **THEN** it shows as plain text, never as a failed diff
  [@test] src/lib/diff/bundled-languages.test.ts (`shows a file of a grammar the build leaves out as plain text`)
  [@test] src/lib/diff/file-languages.test.ts (`shows a file without a known name as plain text`)

### Requirement: Files section: A grammar reads what it embeds by the grammar of that language

A grammar MUST read what it embeds by the grammar of that language: the front matter of a Markdown file as YAML, never
as a paragraph whose `*` is an emphasis, a fenced block by the language it names, a block of Vue or Svelte by its own.

#### Scenario: The front matter and a fenced block of Markdown
- **WHEN** a Markdown file has a front matter and a fenced block
- **THEN** the front matter is read as YAML, never as a paragraph whose `*` is an emphasis, and the block by the language it names
  [@test] src/lib/diff/highlight/engine.test.ts (`reads the front matter of a Markdown file as YAML and a fenced block by its language, never as emphasis`)

#### Scenario: The grammars a grammar embeds
- **WHEN** a grammar embeds others, as a block of Vue or Svelte does
- **THEN** the embedded grammars load before it, and a file of one of them later leaves its grammar as it is
  [@test] src/lib/diff/highlight/engine.test.ts (`loads the grammars a grammar embeds before it, so a file of one of them later leaves its grammar as it is`)

### Requirement: Files section: Kotlin, Java, Rust, TypeScript and JavaScript are coloured from their tree

Kotlin, Java, Rust, TypeScript and JavaScript MUST be coloured as the IntelliJ editor (and RustRover) colours them
without resolving a name, from their syntax tree.

#### Scenario: The fixtures of each language
- **WHEN** the fixture files of each language are highlighted
- **THEN** each matches its golden file, its lines and UTF-16 offsets counted as the view counts them
  [@test] crates/code-symbols/src/highlight_test.rs (`every_fixture_matches_its_golden_file`, `splits_lines_and_counts_utf16_offsets_as_the_view`)

#### Scenario: The lines a view asks for
- **WHEN** a view asks the backend for the colours of a run of pages of a file
- **THEN** the backend answers the runs of the lines asked for, and the view keeps its answer as exact pages
  [@test] crates/file-diffs/src/syntax_test.rs (`answers_the_runs_of_the_lines_asked_for_with_the_line_starts_from_zero`)
  [@test] src/lib/diff/highlight/backend-syntax.test.ts (`asks the backend for a run of pages and the next one, and keeps its answer as exact pages`)

#### Scenario: Kotlin in a review
- **WHEN** a review shows a Kotlin file
- **THEN** a property is coloured as a field, and a declaration, an annotation and a keyword as IntelliJ colours them
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should colour Kotlin as IntelliJ does: a property as a field, a declaration, an annotation, a keyword`)
  [@test] crates/code-symbols/src/kotlin/highlight_test.rs (`a_file_annotation_is_an_annotation_with_its_target_and_its_arguments_are_code`, `a_raw_string_has_no_escapes`, `a_parameter_of_a_function_type_binds_nothing`)

#### Scenario: The keywords and the strings of Java
- **WHEN** a Java file has a keyword the parser reads in context, a character, a string and a text block
- **THEN** they are coloured as IntelliJ colours them, from the syntax tree
  [@test] crates/code-symbols/src/java/highlight_test.rs (`the_keywords_the_parser_reads_in_context_are_keywords_and_names_elsewhere`, `a_character_a_string_and_a_text_block_have_their_escapes`)

### Requirement: Files section: Fields, static members and constants have their colours

A property, a field and an unqualified use of one as a field MUST be coloured as a field; a static member, a constant
and an enum entry italic; a static method in the colour of a declaration and italic.

#### Scenario: The fields of Java
- **WHEN** a Java file declares and uses fields, static members, constants and enum entries
- **THEN** a field and an unqualified use of one are fields, the static ones italic, a static method a declaration and italic
  [@test] crates/code-symbols/src/java/highlight_test.rs (`a_field_of_an_anonymous_class_and_of_an_enum_constant_is_a_field`, `a_pattern_variable_and_a_vararg_hide_the_field_of_their_name`, `a_qualified_this_is_a_keyword_and_the_fields_through_it_are_the_instance_s`, `a_field_is_static_by_its_modifier_and_in_an_interface_only`, `an_element_of_an_annotation_type_is_a_method_declaration_and_an_argument_name_no_field`, `the_name_of_a_method_reference_is_no_field`, `a_pattern_variable_hides_the_field_only_where_java_sees_it`, `the_arguments_of_an_anonymous_class_see_no_member_of_it`)

#### Scenario: The properties of Kotlin
- **WHEN** a Kotlin file declares and uses properties
- **THEN** a property and an unqualified use of one are fields
  [@test] crates/code-symbols/src/kotlin/highlight_test.rs (`a_name_after_a_receiver_is_no_field_of_the_scopes`, `an_operand_of_an_infix_call_is_read_by_the_scopes`, `a_script_has_its_top_level_properties_as_members`)

### Requirement: Files section: A Rust call has the colour of the function it calls

A Rust call MUST be coloured in the colour of the function it calls; a Java or Kotlin call is plain.

#### Scenario: A Rust call and a Java call
- **WHEN** a Rust file and a Java file call a function
- **THEN** the Rust call has the colour of the function it calls, and the Java call is plain
  [@test] src/lib/diff/highlight/engine.test.ts (`leaves a Java call, a package, an import and a class used as a type plain, and colours a Rust call, as IntelliJ and RustRover do`)

### Requirement: Files section: Type parameters, labels, arguments and doc comments have their colours

A type parameter, a label (a Rust one as a lifetime, a Java one plain), a named argument, a doc comment with its tags,
the code of a string template as code, a macro, a lifetime and Rust's `self` and `?` MUST be coloured in their own
colours.

#### Scenario: Type parameters and labels
- **WHEN** a file has a type parameter and a label
- **THEN** they are coloured as a type parameter and a label
  [@test] crates/code-symbols/src/kotlin/highlight_test.rs (`the_label_of_this_is_a_label`, `the_subject_of_a_where_clause_is_a_type_parameter`)
  [@test] crates/code-symbols/src/java/highlight_test.rs (`an_annotated_type_parameter_is_a_type_parameter`)
  [@test] crates/code-symbols/src/js/highlight_test.rs (`a_mapped_key_and_infer_are_type_parameters`)

#### Scenario: Doc comments
- **WHEN** a file has a doc comment with its tags
- **THEN** it is coloured as a doc comment with its tags
  [@test] crates/code-symbols/src/java/highlight_test.rs (`a_markdown_doc_comment_is_a_doc_comment_from_the_slashes_of_each_line`, `a_comment_inside_a_text_block_is_its_text`)

#### Scenario: A named argument, a string template, a macro, a lifetime, `self` and `?`
- **WHEN** a file has a named argument, a string template, a macro, a lifetime, Rust's `self` or `?`
- **THEN** each is coloured in its own colour, the code of the string template as code

### Requirement: Files section: Mutable bindings are underlined, Kotlin's implicit names bold

A Kotlin `var` and a Rust `mut` binding MUST be underlined; Kotlin's `it`, backing `field` and lambda braces bold.

#### Scenario: A mutable binding and an implicit name
- **WHEN** a file has a Kotlin `var`, a Rust `mut` binding, Kotlin's `it`, a backing `field` or a lambda
- **THEN** the `var` and the `mut` binding are underlined, and `it`, `field` and the lambda braces bold

### Requirement: Files section: A JavaScript name is coloured by its declaration in the file

A JavaScript name MUST be coloured by its declaration in the file (a local variable, a parameter, a local function, an
exported, imported or undeclared variable or function as global, a method also at its call), and a regular
expression and a JSX component apart from an HTML tag and the brackets of a tag in their own colours.

#### Scenario: The names of a JavaScript file
- **WHEN** a JavaScript or TypeScript file declares and uses names
- **THEN** each name is coloured by its declaration, an exported, imported or undeclared one as global, a method also at its call
  [@test] crates/code-symbols/src/highlight_test.rs (`colours_a_javascript_name_by_its_declaration_as_intellij_does`)
  [@test] crates/code-symbols/src/js/highlight_test.rs (`an_exported_ambient_declaration_is_global`, `names_by_what_they_name`, `a_call_with_type_arguments_is_a_call`, `a_string_has_the_escapes_of_javascript`)

#### Scenario: A JSX component and an HTML tag
- **WHEN** a TSX file has a component, an HTML tag and a fragment
- **THEN** the component is coloured apart from the HTML tag, and the brackets of a tag have their colour
  [@test] crates/code-symbols/src/js/highlight_test.rs (`a_member_tag_is_a_component_and_a_fragment_has_brackets`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should colour TSX as IntelliJ does: a name by its declaration, a component, a method, the brackets of a tag`)

#### Scenario: A regular expression
- **WHEN** a JavaScript file has a regular expression
- **THEN** it is coloured as a regular expression

### Requirement: Files section: TypeScript and JavaScript are read in the dialect of the extension

TypeScript and JavaScript MUST be read in the dialect of the extension, with a template literal type as a string, a
decorator from its `@` to its arguments, which are code, and the name of an index signature or of a tuple member
plain.

#### Scenario: The dialect of the extension
- **WHEN** a TypeScript or JavaScript file is highlighted
- **THEN** its grammar is the dialect of its extension
  [@test] crates/code-symbols/src/highlight_test.rs (`chooses_the_grammar_of_the_dialect_of_the_extension`)
  [@test] crates/code-symbols/src/js/highlight_test.rs (`the_grammar_reads_its_dialect`)

#### Scenario: A template literal type, a decorator and a mapped type
- **WHEN** a TypeScript file has a template literal type, a decorator with arguments, an index signature or a tuple member
- **THEN** the template literal type is a string, the decorator is coloured from its `@` to its arguments, which are code
- **AND** the name of the index signature or of the tuple member is plain
  [@test] crates/code-symbols/src/js/highlight_test.rs (`a_template_literal_type_is_a_string`, `a_decorator_is_coloured_to_its_arguments`)

### Requirement: Files section: The colours are those of the Islands schemes

The colours MUST be those of the schemes of Islands Light ("Light") and Islands Dark, resolved as IntelliJ resolves a
key, with what a plugin gives one language only (Rust's and TypeScript's type parameters, the underline of a mutable
name), and a font style where the schemes differ.

#### Scenario: The roles of the view
- **WHEN** the backend gives a token a role
- **THEN** the role is one the view resolves in the schemes of Islands Light ("Light") and Islands Dark
  [@test] crates/code-symbols/src/highlight_test.rs (`roles_match_the_view`)

#### Scenario: A font style where the schemes differ
- **WHEN** a comment, a JavaScript parameter and label, or a global variable is shown in each scheme
- **THEN** a comment is italic in the light one only, a JavaScript parameter and label underlined in the light one only
- **AND** a global variable is italic in the dark one only

### Requirement: Files section: Kotlin's keywords are the tokens of its compiler's parser

Kotlin's keywords MUST be the tokens of its compiler's parser (`as?`, `!in`, `!is`, a soft keyword only where it is
one), and a string or a number that a parse error holds keeps its colour.

#### Scenario: Keywords and soft keywords
- **WHEN** a Kotlin file has `as?`, `!in`, `!is`, and a soft keyword used as a name
- **THEN** the tokens of the parser are keywords, and the soft keyword used as a name is a name
  [@test] crates/code-symbols/src/kotlin/highlight_test.rs (`the_keywords_are_the_tokens_of_the_parser`)
  [@test] crates/code-symbols/src/highlight_test.rs (`colours_a_kotlin_name_that_is_a_soft_keyword_as_a_name`)

#### Scenario: A parse error
- **WHEN** a parse error holds a string or a number
- **THEN** it keeps its colour
  [@test] crates/code-symbols/src/kotlin/highlight_test.rs (`a_string_that_a_parse_error_holds_is_still_a_string`, `a_construct_past_the_nesting_limit_is_highlighted_as_its_error`)

### Requirement: Files section: Rust's tokens are those of rust-analyzer's lexer

Rust's tokens MUST be those of rust-analyzer's lexer: `Self` and a keyword among the tokens of a macro are keywords, a
metavariable (`$x`, `$crate`) is plain and its fragment (`$e:expr`) a keyword, an attribute is one colour but its
literals, a raw string has no escapes and a string's line continuation is one.

#### Scenario: `Self` and the tokens of a macro
- **WHEN** a Rust file has `Self`, and a macro with keywords, metavariables (`$x`, `$crate`) and a fragment (`$e:expr`)
- **THEN** `Self` and the keywords are keywords, the metavariables plain, the fragment a keyword
  [@test] crates/code-symbols/src/rust/highlight_test.rs (`self_type_is_a_keyword`, `the_tokens_of_a_macro_are_lexed`)

#### Scenario: An attribute and the strings
- **WHEN** a Rust file has an attribute with literals, a raw string and a string with a line continuation
- **THEN** the attribute is one colour but its literals, the raw string has no escapes, the line continuation is one
  [@test] crates/code-symbols/src/rust/highlight_test.rs (`an_attribute_is_one_colour_but_its_literals`, `the_escapes_of_a_string`)

### Requirement: Files section: A Rust name is coloured by the binding its use sees

The last binding of a Rust name MUST be the one its uses see (a `let`'s from after it), a `mut` shorthand of a struct
pattern is a mutable binding, and a variant after a type in a pattern (`Either::One(_t)`) is a constant.

#### Scenario: A shadowed binding
- **WHEN** a Rust name is bound again
- **THEN** its uses see the last binding, a `let`'s from after it
  [@test] crates/code-symbols/src/rust/highlight_test.rs (`a_binding_shadows_the_ones_before_it`)

#### Scenario: The names of patterns
- **WHEN** a pattern has a `mut` shorthand of a struct, and a variant after a type (`Either::One(_t)`)
- **THEN** the shorthand is a mutable binding, and the variant a constant
  [@test] crates/code-symbols/src/rust/highlight_test.rs (`the_names_of_patterns_and_declarations`)

### Requirement: Files section: A file the syntax tree cannot colour is coloured by Shiki, both sides

The two sides of a file MUST always be coloured alike: a file the syntax tree cannot colour (over 8 MiB, a Rust or a
script file nested deeper than its parse reads) is coloured by its Shiki grammar, both sides. A file nested deeper
than rust-analyzer reads is coloured by Shiki.

#### Scenario: A file nested too deep
- **WHEN** a Rust or a script file is nested deeper than its parse reads
- **THEN** it is coloured by its Shiki grammar, on a small stack
  [@test] crates/code-symbols/src/rust/highlight_test.rs (`deep_nesting_is_highlighted_on_a_small_stack`)
  [@test] crates/code-symbols/src/js/highlight_test.rs (`deep_nesting_is_highlighted_on_a_small_stack`)

#### Scenario: A file over 8 MiB or without a grammar of the tree
- **WHEN** a file is over 8 MiB, or of a language the syntax tree does not colour
- **THEN** the backend colours none of it, and both sides are coloured by the Shiki grammar
  [@test] crates/code-symbols/src/highlight_test.rs (`has_no_highlighting_over_the_limit`)
  [@test] crates/file-diffs/src/syntax_test.rs (`falls_back_to_the_worker_for_a_file_without_a_grammar`)
  [@test] src/lib/diff/highlight/backend-syntax.test.ts (`colours both sides of a file the backend cannot colour in the worker`)

### Requirement: Files section: The text of a blob git lacks goes to the backend once

The text of a blob git lacks (the result of a merge) MUST go to the backend once: the pages after an answer go without
it, and an answer that the backend no longer has it brings it back once.

#### Scenario: A blob the backend lacks
- **WHEN** a view asks for the colours of a blob git lacks, or of a text the view made
- **THEN** it sends the text with the first call, and the backend highlights the text the view sends
  [@test] src/lib/diff/highlight/backend-syntax.test.ts (`sends the text of a blob the backend lacks, and of one the view made, with the first call`)
  [@test] crates/file-diffs/src/syntax_test.rs (`says_missing_for_a_blob_it_cannot_read_and_highlights_the_text_a_view_sends`)

#### Scenario: The pages after an answer, and a Missing answer
- **WHEN** later pages of the same blob are asked for, and the backend answers that it no longer has it
- **THEN** the later pages go without the text, and the Missing answer brings it back once
  [@test] src/lib/diff/highlight/backend-syntax.test.ts (`sends the text of a blob once: later pages go without it, and a Missing answer brings it back once`)

### Requirement: Files section: C# is coloured as Rider colours it, from Roslyn's tree

C# MUST be coloured as Rider colours it without resolving a name, from Roslyn's syntax tree, in the same roles: the
keywords of the parser (a contextual one where the parser makes it a keyword, `var` of a declaration and `nameof` of a
call).

#### Scenario: The fixtures of C#
- **WHEN** the C# fixture is highlighted
- **THEN** it matches its golden file
  [@test] crates/code-symbols/src/highlight_test.rs (`every_fixture_matches_its_golden_file`)

#### Scenario: Contextual keywords
- **WHEN** a C# file has a contextual keyword the parser reads as a keyword, `var` of a declaration, `nameof` of a call, and the same words as names elsewhere
- **THEN** the words the parser reads as keywords are keywords, and names elsewhere
  [@test] crates/code-symbols/src/csharp/highlight_test.rs (`the_contextual_keywords_the_parser_reads_as_keywords_are_keywords_and_names_elsewhere`, `a_verbatim_identifier_binds_its_value_text`)

#### Scenario: C# in a review
- **WHEN** a review shows a C# file
- **THEN** a field, a declaration, an attribute, a keyword and an escape are coloured as Rider colours them
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should colour C# as Rider does: a field, a declaration, an attribute, a keyword, an escape`)

### Requirement: Files section: The members of C# are coloured by their declarations

A field, a property, an event and a record's positional parameter MUST be coloured as a field, static and `const` ones
and an enum member italic, a static method in the colour of a declaration and italic, a member after a value
(`item.Count`, `this.count`, `x?.Count`) as a field and one after a type name plain, and a name where C# reads a type
never as the member of its name (`Color Color`).

#### Scenario: Fields, properties, events and records
- **WHEN** a C# file declares fields, properties, events, static and `const` members, enum members and a record
- **THEN** they are fields, the static and `const` ones and the enum members italic, a static method a declaration and italic
- **AND** the positional parameters of the record are fields
  [@test] crates/code-symbols/src/csharp/highlight_test.rs (`members_are_fields_by_their_declarations_and_static_ones_are_italic`, `the_parameters_of_a_record_are_its_properties`)

#### Scenario: A member after a value or a type name
- **WHEN** a member follows a value (`item.Count`, `this.count`, `x?.Count`) or a type name
- **THEN** after a value it is a field, after a type name plain
  [@test] crates/code-symbols/src/csharp/highlight_test.rs (`a_member_of_a_value_is_a_field_and_one_of_a_type_name_is_unknown`)

#### Scenario: A name in a type position
- **WHEN** C# reads a type where a member has the same name (`Color Color`)
- **THEN** the name in the type position is a type, never the member of its name
  [@test] crates/code-symbols/src/csharp/highlight_test.rs (`a_name_in_a_type_position_is_a_type_not_the_member_of_its_name`)

### Requirement: Files section: The other roles of C# have their colours

A type parameter, a label, a named argument and an attribute with its arguments as code MUST be coloured in their
roles, and so a documentation comment with its tags and their values, the keywords of a directive, and a branch of
`#if` the file's configuration does not take as a comment.

#### Scenario: A type parameter, a label and an attribute
- **WHEN** a C# file has a type parameter, a label with its `goto`, a named argument and an attribute with arguments
- **THEN** the type parameter is one where declared and used, the label and the `goto` to it are labels
- **AND** the attribute is one colour and its arguments are code
  [@test] crates/code-symbols/src/csharp/highlight_test.rs (`a_type_parameter_is_one_where_declared_and_used`, `a_label_and_the_goto_to_it_are_labels`, `an_attribute_is_one_and_its_arguments_are_code`)

#### Scenario: A documentation comment and a directive
- **WHEN** a C# file has a documentation comment, a directive and a branch of `#if` its configuration does not take
- **THEN** the comment has its tags and their values, the directive its keywords, and the branch not taken is a comment
  [@test] crates/code-symbols/src/csharp/highlight_test.rs (`a_documentation_comment_has_its_tags_and_their_values`, `a_directive_has_its_keywords_and_a_branch_not_taken_is_a_comment`)

### Requirement: Files section: The strings of C# have the escapes of their kind

The escapes of a C# string MUST be those of its kind (`\x41`, `""` of a verbatim string, `{{` of an interpolated one,
none of a raw one), with the code of its holes.

#### Scenario: Strings of each kind
- **WHEN** a C# file has a regular, a verbatim, an interpolated and a raw string
- **THEN** each has the escapes of its kind (`\x41`, `""`, `{{`, none), and the holes are code
  [@test] crates/code-symbols/src/csharp/highlight_test.rs (`strings_have_the_escapes_of_their_kind`)

### Requirement: Files section: Starlark is coloured as the Bazel plugin of IntelliJ, from its PSI

Starlark (Bazel's BUILD, BUILD.bazel, MODULE.bazel, WORKSPACE and REPO.bazel files by their names, `.bzl`, `.star` and
`*.BUILD` by their extension) MUST be coloured as the Bazel plugin of IntelliJ colours it, from its PSI.

#### Scenario: The files of Bazel
- **WHEN** a diff shows a BUILD, BUILD.bazel, MODULE.bazel, WORKSPACE or REPO.bazel file, or one of `.bzl`, `.star` or `*.BUILD`
- **THEN** it is read as Starlark
  [@test] crates/code-symbols/src/starlark/highlight_test.rs (`bazel_files_are_starlark_by_their_names`)

### Requirement: Files section: The roles of Starlark are the plugin's

Starlark MUST colour its keywords (not a word it reserves and leaves plain, as `class`), a string, a number and a
comment, the name of a `def` as a declaration, a named argument's name and its `=`, and in a BUILD or MODULE.bazel file
the callee of a top-level call as a declaration (a target's rule, `bazel_dep`), never a call of a `.bzl` file.

#### Scenario: The keywords
- **WHEN** a Starlark file has its keywords and a word it reserves, as `class`
- **THEN** the keywords are coloured, and the reserved word is plain
  [@test] crates/code-symbols/src/starlark/highlight_test.rs (`the_keywords_are_the_plugin_s`)

#### Scenario: A `def` and a named argument
- **WHEN** a Starlark file has a `def` and a call with a named argument
- **THEN** the name of the `def` is a declaration, and the named argument's name and its `=` are coloured as the plugin annotates them
  [@test] crates/code-symbols/src/starlark/highlight_test.rs (`a_def_s_name_and_a_named_argument_are_painted_as_the_plugin_annotates_them`)

#### Scenario: A top-level call
- **WHEN** a BUILD or MODULE.bazel file has a top-level call (a target's rule, `bazel_dep`), and a `.bzl` file one too
- **THEN** the callee in the BUILD or MODULE.bazel file is a declaration, never the one of the `.bzl` file
  [@test] crates/code-symbols/src/starlark/highlight_test.rs (`a_build_file_s_top_level_call_is_a_declaration`)

### Requirement: Files section: A diff shows the blocks of the engine with 4 lines of context

A diff MUST show the blocks of the diff engine with 4 lines of context (IntelliJ's), changes whose context touches
without a fold between them, and the changed ranges of the engine instead of a word diff, also on a changed line
without a line beside it.

#### Scenario: The context of a change
- **WHEN** a diff shows a change in a file
- **THEN** it keeps 4 lines of context next to the change and folds the others
- **AND** changes whose context touches show without a fold between them, and the context is cut at both ends of the file
  [@test] src/lib/diff/rows.test.ts (`keeps 4 lines of context next to a change, as IntelliJ, and folds the others`, `skips a level that shows no more lines, and folds no fewer than 2 lines`, `cuts the context at both ends of the file`)

#### Scenario: The changed ranges of a line
- **WHEN** a changed line has changed ranges, also without a line beside it
- **THEN** the line is cut at each bound of a changed range of the engine, not by a word diff
  [@test] src/lib/diff/line-parts.test.ts (`cuts a line at each run of the tokens and each bound of a changed range`)
  [@test] crates/text-diff/src/model_test.rs (`an_inserted_word_is_typed_and_marks_its_empty_side`)

### Requirement: Files section: The tints of a block are IntelliJ's

The tints MUST be IntelliJ's: a block with inner ranges draws its lines pale and its ranges in the full colour; a
block without them (line-level only, or ranges dropped) draws its lines in the full colour; a whitespace-only block
draws its lines pale with nothing strong.

#### Scenario: Blocks with and without inner ranges
- **WHEN** a diff has a block with inner ranges and a block without them
- **THEN** the first draws its lines pale and its ranges in the full colour, the second its lines in the full colour
  [@test] src/lib/diff/rows.test.ts (`draws the lines of a block with inner ranges pale and the lines of a block without them in the full tint, as IntelliJ`)

#### Scenario: A whitespace-only block
- **WHEN** a diff has a whitespace-only block
- **THEN** its lines are pale with nothing strong
  [@test] src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`)

### Requirement: Files section: The colours are IntelliJ's diff types

The colours MUST be IntelliJ's diff types (inserted green, deleted grey, modified blue): Split colours a block by its
type on both sides and each changed range by its own type, with a bar 2 px wide where a range has an empty side;
Unified and Inline colour a deleted line grey and an inserted line green.

#### Scenario: The parts of a changed line
- **WHEN** a changed line has inserted, deleted and modified ranges, one with an empty side
- **THEN** each part is typed, and a marker part stands where the empty side of a change is, drawn as a bar 2 px wide in Split
  [@test] src/lib/diff/line-parts.test.ts (`types each changed part, and puts a marker part where the empty side of a change stands`)

#### Scenario: Split and the other views
- **WHEN** a block is shown in Split, Unified and Inline
- **THEN** Split gives it the old and the new column, coloured by its type on both sides, the shorter side filled
- **AND** Unified and Inline give it one column, a deleted line grey and an inserted line green
  [@test] src/lib/diff/row-cells.test.ts (`gives the old and the new column in Split, one column with the ranges of the line otherwise`)
  [@test] src/lib/diff/rows.test.ts (`fills the shorter side of a block in the split view`, `shows a new file and a deleted file as one block of one side`)

### Requirement: Files section: The colour of a line fills its line number

The colour of a line MUST fill its line number; no bar stands beside the code.

#### Scenario: A changed line
- **WHEN** a diff shows a changed line
- **THEN** its colour fills its line number, and no bar stands beside the code

### Requirement: Files section: The line indices are the engine's

The line indices MUST be the engine's: the empty line after a final line end is a line, and it shows only when a block
touches it.

#### Scenario: The empty line after a final line end
- **WHEN** a text ends with a line end
- **THEN** the empty line after it is a line, shown only when a block touches it
  [@test] src/lib/diff/rows.test.ts (`leaves out the empty line after a final line end, unless a block touches it`)

#### Scenario: The ends of the lines
- **WHEN** a text has LF, CRLF and lone CR line ends, or ends without one
- **THEN** its lines are split as IntelliJ splits them
  [@test] src/lib/diff/rows.test.ts (`splits lines as IntelliJ: LF, CRLF and a lone CR end a line, and a text may end without one`)

### Requirement: Files section: A change of the line ends is named under the header

A file whose line ends changed MUST say so under its header ("Line endings: LF → CRLF"), and "Only line endings
changed: …" when that is its only change.

#### Scenario: The line ends changed
- **WHEN** the line ends of a file changed, with other changes or alone
- **THEN** its header says "Line endings: LF → CRLF", or "Only line endings changed: …" when that is its only change
  [@test] src/lib/diff/view-files.test.ts (`names a change of the line ends, and says so when it is the only change`)

### Requirement: Files section: A fold names the innermost declaration around its lines

A fold of unchanged lines MUST name the innermost declaration around its lines (the scopes of the engine, such as
`fun main() {`, the new side first) after their count.

#### Scenario: A fold inside a function
- **WHEN** a fold hides unchanged lines inside a declaration
- **THEN** it names the innermost scope around its lines, such as `fun main() {`, the new side first, after their count
  [@test] src/lib/diff/rows.test.ts (`labels a fold with the innermost scope around its lines, the new side first`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)

### Requirement: Files section: The folds nest as IntelliJ's

The folds MUST nest as IntelliJ's: a click on a fold shows 8 lines next to each change, the next one 16, then all.
"Expand All" of a file header in a review, and ⌥-click (Alt) on a fold, show all the lines of the file.

#### Scenario: Clicks on a fold
- **WHEN** the user clicks a fold, then again, then again
- **THEN** it shows 8 lines next to each change, then 16, then all
  [@test] src/lib/diff/rows.test.ts (`nests the folds at 4, 8 and 16 lines of context, then shows all lines`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should group the files by directory and expand the unchanged lines of a diff`)

#### Scenario: Expand All and Alt-click
- **WHEN** the user chooses "Expand All" of a file header in a review, or ⌥-clicks (Alt) a fold
- **THEN** all the lines of the file show
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

### Requirement: Files section: Highlighting does not block the window

Highlighting MUST NOT block the window: the backend colours the languages of the syntax tree off the main thread,
each blob parsed once while the app runs, and a worker colours the others; it starts with the first diff shown and
lives as long as the window, and a row shows plain text until its tokens arrive.

#### Scenario: A blob of a language of the syntax tree
- **WHEN** a diff shows a file of a language of the syntax tree, then again
- **THEN** the backend parses its blob once, from the blobs the diff read, off the main thread
  [@test] crates/file-diffs/src/syntax_test.rs (`parses_a_blob_once_from_the_blobs_a_diff_read_and_reads_a_cold_one_with_one_cat_file`)

#### Scenario: The other languages
- **WHEN** a diff shows a file of another language, or the app runs without a backend
- **THEN** a worker colours it, and the row shows plain text until its tokens arrive
  [@test] src/lib/diff/highlight/backend-syntax.test.ts (`leaves every language to the worker without a backend`)

### Requirement: Files section: Without workers the main thread highlights in idle slices

When the workers cannot start, the diff MUST be highlighted on the main thread in idle slices.

#### Scenario: The workers cannot start
- **WHEN** the workers cannot start
- **THEN** the main thread highlights the diff in idle slices, a page far into a file first as provisional, then exact
  [@test] src/lib/diff/highlight/engine.test.ts (`answers a page far into a file first as provisional, then exact, and confirms equal runs without arrays`)
  [@test] src/lib/diff/highlight/client.test.ts (`keeps the pages the engine on the main thread answers, a provisional one confirmed exact`)

### Requirement: Files section: The section offers the split, the unified and the inline view

The section MUST offer the split, the unified and the inline view. The choice is one setting of every diff view (the
section, the reviews, the uncommitted changes, the conflicts and the missing commits), also in Settings → Diffs.

#### Scenario: The views of the section
- **WHEN** the section is open
- **THEN** it offers the split, the unified and the inline view
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)

#### Scenario: One setting for every diff view
- **WHEN** the user chooses a view in one diff view, or in Settings → Diffs
- **THEN** the section, the reviews, the uncommitted changes, the conflicts and the missing commits show that view

### Requirement: Files section: Split is the default, a stored choice wins

Split MUST be the default, as in IntelliJ; a stored choice keeps winning over it.

#### Scenario: A stored choice
- **GIVEN** the user chose another view before
- **WHEN** a diff view opens
- **THEN** it shows the stored choice, not the Split default
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should keep a stored diff view mode over the Split default`)

### Requirement: Files section: Inline shows a similar pair of a block as one row

Inline MUST show each pair of a block that the diff engine finds similar as one row: the new line with the deleted
parts of the old line before the inserted parts, each part in the syntax colours of its own side, the deleted parts
left out of a copy, the line number of the new line.

#### Scenario: A lightly edited line
- **WHEN** Inline shows a block whose old and new line are similar
- **THEN** they are one row, the new line with the deleted parts of the old line before the inserted parts
- **AND** each part has the syntax colours of its own side, the deleted parts are left out of a copy, the line number is the new line's
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Inline shows a lightly edited line as one row`)
  [@test] src/lib/diff/line-parts.test.ts (`puts the deleted part of the old line before the inserted part, each with the runs of its side`)

### Requirement: Files section: A line of a pair without a range has an empty one

A line of a pair without a range of its own (a pure insertion or deletion) MUST have an empty one where the other line
changes.

#### Scenario: A pure insertion
- **WHEN** a pair is a pure insertion, so its old line has no range
- **THEN** the old line gets an empty range where the new line changes, so the row is the new line
  [@test] src/lib/diff/line-parts.test.ts (`aligns a pure insertion: the old line without a range gets an empty one, so the row is the new line`)

### Requirement: Files section: The other blocks show in Inline as in Unified

The other blocks MUST show in Inline as in Unified, and so does a pair at the end of a file without a newline at its
end.

#### Scenario: A pair at the end of a file without a newline
- **WHEN** Inline shows a block that is no similar pair, or a pair at the end of a file without a newline at its end
- **THEN** it shows as in Unified
  [@test] src/lib/diff/rows.test.ts (`makes each similar pair of a block one row only in the Inline view, also at the end of a file`)

### Requirement: Files section: An added or a deleted file shows its one side alone

An added and a deleted file (a side without a blob, `oneSideOf`) MUST show that side alone in every view, Split too:
one column the width of the view with its line numbers, every line inserted or deleted, as IntelliJ's diff shows such
a file in its one-sided viewer (`SimpleOnesideDiffViewer`, [Diff View](../../../../docs/design/diff-view.md#the-row-model)).

#### Scenario: An added file in Split
- **WHEN** Split shows an added file
- **THEN** it shows its one side, one column the width of the view with its line numbers, every line inserted
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Split shows an added file as its one side, with its line numbers, as IntelliJ's one-sided viewer`)

#### Scenario: A file with one side in every layout
- **WHEN** a file with one side is shown in Split, Unified or Inline
- **THEN** that side shows alone, with its line numbers
  [@test] src/lib/diff/row-cells.test.ts (`shows a file with one side as that side alone in every layout, with its line numbers, as IntelliJ's one-sided viewer`)

### Requirement: Files section: A file with two sides keeps the view the toggle chose

A file with two sides MUST keep the view the toggle chose; a long line keeps its horizontal scroll.

#### Scenario: A modified file in Split
- **WHEN** Split shows a file with two sides and a long line
- **THEN** it shows both sides, and the long line keeps its horizontal scroll

### Requirement: Files section: Split scrolls its two sides together

Split MUST scroll its two sides together: a horizontal scroll of one side moves the other to the same place, as
IntelliJ's diff does.

#### Scenario: A horizontal scroll of one side
- **WHEN** the user scrolls one side of a Split diff horizontally
- **THEN** the other side moves to the same place
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Split scrolls both sides together`)

### Requirement: Files section: Another commit shows its own files, collapsed

Another commit (↑, ↓) MUST show its own files, collapsed.

#### Scenario: The next commit
- **WHEN** the user switches the preview to another commit with ↑ or ↓
- **THEN** the section shows the files of that commit, collapsed
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)

### Requirement: Files section: The header, F and the footer button open or close the section

The header of the section, F and the `F Files` button of the footer MUST open or close it. The choice MUST hold for
the next preview, until the app restarts.

#### Scenario: The three ways to open and close
- **WHEN** the user clicks the header, presses F, or clicks the `F Files` button of the footer
- **THEN** the section opens or closes
- **AND** the next preview opens it as it was left, until the app restarts
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should open and close the files with their header, F and the footer button`)

### Requirement: Files section: A preview of a commit no list loaded loads its files alone

A preview of a commit no list loaded MUST load its files alone (`get_commit_files`).

#### Scenario: A commit no list loaded
- **WHEN** the preview shows a commit no list loaded the files of
- **THEN** it loads its files alone with `get_commit_files`
  [@test] src/stores/commit-files.test.ts (`loads the one commit a preview shows with the single command`)

### Requirement: Files section: A failed read shows in the section

A failed read MUST show "Cannot read the files of the commit" in the section.

#### Scenario: The read of the files fails
- **WHEN** the read of the files of the commit fails
- **THEN** the section shows "Cannot read the files of the commit"

### Requirement: Soft Wraps: Ligatures draw the ligatures of JetBrains Mono

"Ligatures" in Settings → Diffs MUST draw the ligatures of JetBrains Mono, on by default; each character keeps its
cell, and a changed character inside a ligature stays visible.

#### Scenario: Ligatures on, and a changed range inside one
- **WHEN** "Ligatures" is on and a changed range cuts a ligature
- **THEN** each character keeps its cell width, and the changed character stays visible
  [@test] tests/e2e/commit-files/diff-font.spec.ts (`keeps the cell width of each character with ligatures on, also where a changed range cuts a ligature`)

#### Scenario: The default
- **WHEN** the user never changed "Ligatures"
- **THEN** it is on

### Requirement: Soft Wraps: Inline strikes its deleted parts through

Inline MUST strike its deleted parts through, still grey and left out of a copy; Split and Unified strike nothing.

#### Scenario: A changed line in each view
- **WHEN** a changed line is shown in Inline, Split and Unified
- **THEN** Inline strikes its deleted parts through, still grey and left out of a copy
- **AND** Split and Unified strike nothing
  [@test] tests/e2e/commit-files/diff-font.spec.ts (`strikes the deleted parts of an Inline row through, and no change of Split or Unified`)

### Requirement: Soft Wraps: Soft wraps break a long line at the edge of the diff

"Soft wraps" in Settings → Diffs MUST break a long line at the edge of the diff. It is off by default, as IntelliJ's
`USE_SOFT_WRAPS`: a long line keeps its shape and scrolls sideways. One setting for every diff view.

#### Scenario: Soft wraps on and off
- **WHEN** a diff has a long line, with "Soft wraps" on in Settings → Diffs, then off
- **THEN** with it on the line breaks at the edge of the diff
- **AND** with it off, as by default, the line keeps its shape and scrolls sideways
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should break a long line with Soft Wraps on in Settings, and scroll it with them off`)

#### Scenario: One setting for every diff view
- **WHEN** the user turns "Soft wraps" on
- **THEN** every diff view breaks its long lines

### Requirement: Whitespace: The Whitespace menu chooses what every diff ignores

The Whitespace menu beside the layout switch (also in Settings → Diffs, where it names the choice) MUST choose what the
diff of every view ignores: Do Not Ignore (the default, as IntelliJ), Trim Whitespace, Ignore Whitespace, Ignore
Whitespace and Blank Lines.

#### Scenario: The choices of the menu
- **WHEN** the user opens the Whitespace menu beside the layout switch, or its row in Settings → Diffs
- **THEN** it offers Do Not Ignore, the default, Trim Whitespace, Ignore Whitespace, Ignore Whitespace and Blank Lines
- **AND** Settings → Diffs names the choice

### Requirement: Whitespace: The choice marks the button and goes with every read

The button MUST be marked while it ignores anything. The choice MUST go to the backend with every read of the diffs of
the files.

#### Scenario: A choice that ignores something
- **WHEN** the user chooses a policy other than Do Not Ignore
- **THEN** the button is marked, and the next read of the diffs of the files sends the choice to the backend
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Ignore Whitespace hides an indentation-only change`)

### Requirement: Whitespace: Each policy ignores what IntelliJ's does

Trim Whitespace MUST hide a change of indentation. Under Ignore Whitespace a blank line added holds no character the
engine compares: it is a whitespace-only change, pale, counted, and N and P stop at it, as in IntelliJ. Ignore
Whitespace and Blank Lines ignores it. The counts of the file follow the choice.

#### Scenario: A change of indentation
- **WHEN** a file changes only its indentation, under Trim Whitespace or Ignore Whitespace
- **THEN** the change is hidden, and the counts of the file follow the choice
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Ignore Whitespace hides an indentation-only change`)

#### Scenario: A blank line added
- **WHEN** a blank line is added, under Ignore Whitespace, then under Ignore Whitespace and Blank Lines
- **THEN** under Ignore Whitespace it is a whitespace-only change, pale, counted, and N and P stop at it
- **AND** under Ignore Whitespace and Blank Lines it is ignored

### Requirement: Whitespace: Under Do Not Ignore a change of indentation has inner ranges

Under Do Not Ignore a change of indentation MUST be a change with inner ranges, as in IntelliJ: its lines are pale,
and the indentation is in the full colour.

#### Scenario: A change of indentation under Do Not Ignore
- **WHEN** a file changes only its indentation, under Do Not Ignore
- **THEN** its lines are pale, and the indentation is in the full colour

### Requirement: Whitespace: A block only the choice ignores shows as unchanged lines

A block that only the choice ignores MUST show as unchanged lines: the unified and the inline view show its new lines
as context and leave its old lines out (the old line numbers jump, as in IntelliJ), the split view shows both sides
plain, the shorter one filled. N and P skip it.

#### Scenario: An ignored block in the views
- **WHEN** a diff has a block only the choice ignores
- **THEN** the unified and the inline view show its new lines as context and leave its old lines out, the old line numbers jump
- **AND** the split view shows both sides plain, the shorter one filled
  [@test] src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`, `folds the lines an ignored block holds, with their own count on each side`)

#### Scenario: A file whose only blocks are ignored
- **WHEN** the choice ignores every block of a file
- **THEN** the file shows no changed rows
  [@test] src/lib/diff/rows.test.ts (`shows no rows for a file whose only blocks the policy ignores`)

#### Scenario: N and P
- **WHEN** the user presses N or P in a diff with an ignored block
- **THEN** they skip it
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`N and P skip a block the whitespace policy ignores`)
  [@test] src/lib/diff/rows.test.ts (`starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)

### Requirement: Whitespace: A whitespace-only block is drawn pale and counts

A block that differs only in whitespace MUST be drawn pale; it counts, and N and P stop at it.

#### Scenario: A whitespace-only block
- **WHEN** a diff has a block that differs only in whitespace
- **THEN** it is drawn pale, it counts, and N and P stop at it
  [@test] src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`, `starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)

### Requirement: The actions on a file: A file and a directory have one set of actions

A file and a directory SHALL have one set of actions (`fileMenuItems`), the same wherever they show: the rows of the
tree, the headers of the diffs of a review, the headers of the conflict and missing-commits accordions, the files of a
pull request.

#### Scenario: The same actions everywhere
- **WHEN** a file shows in a row of the tree, the header of a diff of a review, the header of a conflict or missing-commits accordion, or the files of a pull request
- **THEN** it offers the same actions (`fileMenuItems`)

### Requirement: The actions on a file: A file copies its path, a directory its changed paths too

A file SHALL offer "Copy Path"; a directory "Copy Path" and "Copy Changed Paths" (the files under it, one per line).

#### Scenario: The actions of a file
- **WHEN** the user opens the menu of a file
- **THEN** it offers "Copy Path", which copies its path
  [@test] src/lib/file-actions.test.ts (`copies the path of a file, with the key only where the key works`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should copy the path of a file or the changed paths of a directory from the menu of its row, and with ⌘C`)

#### Scenario: The actions of a directory
- **WHEN** the user opens the menu of a directory
- **THEN** it offers "Copy Path" and "Copy Changed Paths", the files under it, one per line
  [@test] src/lib/file-actions.test.ts (`copies the path of a directory, or the changed paths under it`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should copy the path of a file or the changed paths of a directory from the menu of its row, and with ⌘C`)

### Requirement: The actions on a file: The map of the files of a review has no actions

The map of the files beside the diffs of a review SHALL have no actions ([Commit Review](../commit-review/spec.md)): it
is a scrollbar, and the tree and the header of each file it stands for have them.

#### Scenario: The map of a review
- **WHEN** the user right-clicks the map of the files beside the diffs of a review
- **THEN** it offers no actions; the tree and the header of each file it stands for do

### Requirement: The actions on a file: The right click on a row or a header opens its menu

The right click on a row or a header MUST open the actions as its menu, one menu per row or header: never on a diff,
whose text keeps its own selection and native menu. A right click on a row focuses it.

#### Scenario: A right click on a row
- **WHEN** the user right-clicks a row of the tree
- **THEN** the row takes the focus and its menu opens
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should copy the path of a file or the changed paths of a directory from the menu of its row, and with ⌘C`)

#### Scenario: A right click on a diff
- **WHEN** the user right-clicks the text of a diff
- **THEN** no menu of the file opens; the text keeps its own selection and native menu

### Requirement: The actions on a file: ⌘C on a focused row copies its path

⌘C (Ctrl+C off macOS, the key of the layout) on a focused row (of the tree, of the files of a pull request) MUST copy
its path, unless text is selected; the menu of a row shows the key, the menu of a header, where the key does nothing,
does not.

#### Scenario: ⌘C on a focused row
- **WHEN** a row has the focus, no text is selected, and the user presses ⌘C (Ctrl+C off macOS)
- **THEN** its path is copied
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should copy the path of a file or the changed paths of a directory from the menu of its row, and with ⌘C`)

#### Scenario: The key in the menus
- **WHEN** the menu of a row and the menu of a header open
- **THEN** the menu of the row shows the key, the menu of the header does not
  [@test] src/lib/file-actions.test.ts (`copies the path of a file, with the key only where the key works`)

### Requirement: The actions on a file: The headers keep their copy button on hover

The headers SHALL keep their copy button on hover: they have the room. A tree row has none.

#### Scenario: The hover of a header and of a row
- **WHEN** the pointer rests on the header of a diff, and on a row of the tree
- **THEN** the header shows its copy button, the row none

### Requirement: Copy Changed Paths: A commit row copies the paths its commit changed

The context menu of a commit row MUST offer "Copy Changed Paths": the paths the commit changed, one per line, a renamed
file by its new path.

#### Scenario: Copy Changed Paths of a commit
- **WHEN** the user chooses "Copy Changed Paths" in the context menu of a commit row
- **THEN** the paths the commit changed are copied, one per line, a renamed file by its new path
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should copy the changed paths of the commits from the context menu`)
  [@test] src/lib/commit-files.test.ts (`lists each path once, in the order of the commits, a renamed file by its new path`)

### Requirement: Copy Changed Paths: A selection copies each path once

For a selection it MUST be "Copy Changed Paths of 3 Commits": each path once, in the order of the commits.

#### Scenario: Copy Changed Paths of a selection
- **WHEN** the user selects 3 commits and chooses "Copy Changed Paths of 3 Commits"
- **THEN** each path they changed is copied once, in the order of the commits
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should copy the changed paths of the commits from the context menu`)
  [@test] src/lib/commit-files.test.ts (`lists each path once, in the order of the commits, a renamed file by its new path`)

### Requirement: Copy Changed Paths: Loaded paths are copied in the click

The paths of the commits the list loaded MUST be copied in the click, without waiting for git.

#### Scenario: Commits the list loaded
- **GIVEN** the list loaded the files of the commits
- **WHEN** the user chooses "Copy Changed Paths"
- **THEN** the paths are copied in the click, without waiting for git

### Requirement: Copy Changed Paths: A failed read copies nothing

When the files cannot be read, the copy MUST show an error toast and copy nothing.

#### Scenario: The files cannot be read
- **WHEN** the user chooses "Copy Changed Paths" and the files cannot be read
- **THEN** an error toast shows, and nothing is copied

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
- The tree rows are `h-6 rounded-md text-sm`, `hover:bg-muted`, indented by 0.75rem per level, the focus outline
  inside the focused row (`bd-focus-inset`). A row starts with a `ChevronRightIcon` (`size-3.5` muted, turned down when open). A 1px
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
  the gutter, the changed words a stronger tint ([UI Theme](../../../../docs/design/ui-theme.md) "Diffs"). The rows of a
  whitespace-only block are paler, their gutter bar faded. The scope of a hunk follows the count of its separator in
  `foreground`, cut with "…" when it is long.
- Loading: an inline spinner and "Loading files…" in muted `text-sm`; a file's diff shows "Loading diff…". A failed
  read of the patch is a destructive callout "Cannot read the changes of the commit" in the panel.
- The footer shows the ghost `xs` buttons `D Review Changes` and `F Files` (pressed while the section is open) before
  `E Edit`.

## Testing / Local Run

- `pnpm test:backend`: `crates/git-ops/src/commit_files_test.rs`, `crates/object-cache/src/cache_test.rs`,
  `crates/app-api/src/repository_state_test.rs`, `crates/file-diffs/src/range_files_test.rs`.
- `pnpm test:unit`: `src/lib/commit-files.test.ts`, `src/lib/file-tree.test.ts`, `src/stores/commit-files.test.ts`,
  `src/stores/file-diffs.test.ts`, `src/stores/blob-texts.test.ts`, `src/lib/diff/*.test.ts`.
- `pnpm e2e`: `tests/e2e/commit-files`, on the `issue_links` and `commit_tree` templates (nested directories, a Kotlin
  file with two hunks). The test server runs the commands of the app on the fixture repositories without the object
  cache and without the warm; the memory cache of the diffs is the test repository's own (`AppContext::file_diffs`),
  and the events of `get_file_diffs` stream on its channel as the app sends them.

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

- [Git Operations](../../../../docs/design/git-operations.md) ("Commit files", "Object cache")
- [Diff View](../../../../docs/design/diff-view.md)
- [Commit Review](../commit-review/spec.md)
- [Commit Row](../commit-row/spec.md)
- [Branch Preview](../../branch/branch-preview/spec.md)
- GitHub and GitLab show `+42 −7` per commit and per file, with the added lines green and the deleted ones red.
