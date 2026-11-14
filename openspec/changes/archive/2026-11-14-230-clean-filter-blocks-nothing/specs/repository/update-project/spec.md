## ADDED Requirements

### Requirement: Check: A file that differs from its blob only by the clean filter does not block

A file whose bytes are its blob in HEAD MUST NOT block where git keeps such bytes (the safer autocrlf of `text=auto`,
`-text` or no `text` and `eol`), also when its clean filter gives another id: the check then compares the id of its
bytes without the filters (`hash-object --no-filters`). Elsewhere (`text`, an `eol` alone, `ident`, a `filter`) it
MUST block. The early `session.blocked_paths` of [brd update](../../cli/update/spec.md) takes the same check.

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
- **GIVEN** such a file under `text eol=lf`, `text eol=crlf`, `eol=lf` or `ident`
- **WHEN** the check runs
- **THEN** it blocks
  [@test] crates/update-core/src/update_project_test.rs (`preview_blocks_a_crlf_blob_where_git_converts_it`)
  [@test] crates/git-ops/src/crlf_test.rs (`a_blob_with_crlf_is_kept_where_git_add_keeps_it`, `ident_a_filter_and_an_encoding_are_never_kept`)

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
