## MODIFIED Requirements

### Requirement: Check: A file that differs from its blob only by the clean filter does not block

A file whose bytes are its blob in HEAD MUST NOT block where git keeps such bytes (the safer autocrlf of `text=auto`,
`-text` or no `text`, `crlf` and `eol`), also when its clean filter gives another id: the check then compares the id of
its bytes without the filters (`hash-object --no-filters`). Elsewhere (`text`, the old `crlf`, an `eol` alone,
`ident`, a `filter`) it MUST block. The early `session.blocked_paths` of [brd update](../../cli/update/spec.md) takes
the same check.

#### Scenario: A file committed with CRLF under `eol=lf`
- **GIVEN** a file committed with CRLF, `text=auto eol=lf` given to it after, untouched in the working tree, also when
  a probe of the preview answers for it
- **WHEN** the upstream changes it and the check runs
- **THEN** it does not block, and the update applies
  [@test] crates/update-core/src/update_project_test.rs (`preview_does_not_block_a_file_that_differs_from_its_blob_only_by_the_clean_filter`)

#### Scenario: Under `core.autocrlf` without attributes
- **GIVEN** a file committed with CRLF, no attributes, `core.autocrlf` true
- **WHEN** the upstream changes it and the check runs
- **THEN** it does not block
  [@test] crates/update-core/src/update_project_test.rs (`preview_does_not_block_a_crlf_blob_under_core_autocrlf`)

#### Scenario: Where git converts the bytes
- **GIVEN** such a file under `text eol=lf`, `text eol=crlf`, `text=input`, `eol=lf`, the old `crlf` or `crlf=input`,
  or `ident`
- **WHEN** the check runs
- **THEN** it blocks; under `-crlf` and `crlf=auto` it does not
  [@test] crates/update-core/src/update_project_test.rs (`preview_blocks_a_crlf_blob_where_git_converts_it`)
  [@test] crates/git-ops/src/crlf_test.rs (`a_blob_with_crlf_is_kept_where_git_add_keeps_it`, `ident_a_filter_and_an_encoding_are_told_apart`)

#### Scenario: A probe under other attributes
- **GIVEN** a probe of the preview that hashed a file under its attribute sources: the `.gitattributes` of each of
  its directories, `info/attributes`, the global file (`core.attributesFile`, as git places it, else its default
  place) and the system file of the git binary
- **WHEN** one of them changes, the file untouched, also a rewrite within the racy margin of the hash that keeps the
  stamp of the source
- **THEN** the probe does not answer for the file, and the check sees what git sees; under the same sources it answers
  with no `hash-object`, also for a file its filters keep and in the next sync
  [@test] crates/update-core/src/update_project_test.rs (`a_change_of_the_attributes_is_seen_without_a_touch_of_the_file`, `the_probe_of_a_file_its_filters_keep_answers_no_change`, `the_attributes_of_the_directory_of_a_file_are_a_source_of_its_probe`, `attributes_changed_within_the_racy_margin_of_the_hash_are_read_again`, `a_change_of_the_global_attributes_file_is_seen_without_a_touch_of_the_file`, `a_change_of_the_system_attributes_file_hashes_the_file_again`)
  [@test] crates/sync-core/src/process_budget_test.rs (`the_update_check_of_an_unchanged_sync_hashes_no_file_again`)
  [@test] crates/git-ops/src/update_branch_test.rs (`the_global_attributes_file_is_where_git_reads_it`)
  [@test] crates/sync-core/src/config_snapshot_test.rs (`the_attributes_file_is_the_last_core_attributes_file`)

#### Scenario: A resolve session with such a file
- **GIVEN** such a file, which the upstream changes
- **WHEN** an update conflicts and its resolve session opens
- **THEN** its output has no `blocked_paths` and no `ask_user`
  [@test] crates/bd-update/src/resolve_test.rs (`a_file_that_differs_from_its_blob_only_by_the_clean_filter_blocks_nothing`)

#### Scenario: A real edit of such a file
- **GIVEN** such a file with a local edit
- **WHEN** the check runs
- **THEN** it blocks
  [@test] crates/update-core/src/update_project_test.rs (`preview_does_not_block_a_file_that_differs_from_its_blob_only_by_the_clean_filter`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_file_that_differs_from_its_blob_only_by_the_clean_filter_blocks_nothing`)

## ADDED Requirements

### Requirement: Check: A file under a tracked `.gitattributes` the working tree lacks blocks

A file under a `.gitattributes` that the index has and the working tree lacks MUST block: git reads its attributes
from the index blob, `hash-object` none, so the preview can't tell what git sees. The check reads such files in the
same `cat-file --batch-check` as the index entries of the changed paths.

#### Scenario: A tracked `.gitattributes` the working tree lacks
- **GIVEN** a `.gitattributes` in the index that the working tree lacks (deleted, or a link, which git does not follow
  there), above files the upstream changes
- **WHEN** the check runs
- **THEN** each of them blocks, also one git keeps; with the file back, only what git sees as changed
  [@test] crates/update-core/src/update_project_test.rs (`a_tracked_gitattributes_the_worktree_lacks_blocks_the_files_under_it`)
