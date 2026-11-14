## ADDED Requirements

### Requirement: Check: A file that differs from its blob only by the clean filter does not block

A file that the update changes and whose bytes are its blob in HEAD MUST NOT block, also when its clean filter gives
another id (a blob committed with CRLF under `text=auto eol=lf`): once the filtered id differs, the check compares the
id of its bytes without the filters (`hash-object --no-filters`). The early `session.blocked_paths` of
[brd update](../../cli/update/spec.md) takes the same check.

#### Scenario: A file committed with CRLF under `eol=lf`
- **GIVEN** a file committed with CRLF, `text=auto eol=lf` given to it after, untouched in the working tree
- **WHEN** the upstream changes it and the check runs
- **THEN** it does not block, and the update applies
  [@test] crates/update-core/src/update_project_test.rs (`preview_does_not_block_a_file_that_differs_from_its_blob_only_by_the_clean_filter`)

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
