## MODIFIED Requirements

### Requirement: Host runs: A commit of the files of a worktree is what add all stages

A commit of the files of a worktree (`worktree_commit`) MUST be what `git add --all` stages, on HEAD, without a write of
the index, the files or the refs of the worktree: one `status --porcelain=v1 -z --no-renames` of its index, read only,
in the untracked mode of the repository (its `core.fsmonitor` and `core.untrackedCache` apply).

#### Scenario: The paths the status names
- **WHEN** the status names paths of the worktree
- **THEN** a file is written by `hash-object -w --stdin-paths` (its clean filter and end of line apply) with the mode
  of its executable bit, a link by its target, a directory with `.git` as the gitlink of its HEAD, a missing file as a
  deletion, an untracked directory by its files (`ls-files --others --exclude-standard`)
- **AND** one `fast-import` writes the commit from the tree of HEAD, and one ref transaction deletes its temporary ref
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`the_commit_of_a_worktree_is_what_add_all_stages_from_its_files_or_a_copy_of_its_index`)

#### Scenario: An edited file whose blob holds CRLF
- **WHEN** a tracked file that holds a CRLF, whose blob in the index is text with CRLF, is edited under `text=auto`,
  where `git add` keeps its bytes as they are (the safer autocrlf)
- **THEN** it is written as it is (`hash-object -w --no-filters`), with its CRLF, after one `check-attr` and one
  `cat-file --batch` of its index blob; a file under `text` or the old `crlf`, and a new file, take the conversion,
  and a file under `-text` its bytes, neither reading the index blob; a binary file with a lone CR costs no `check-attr`;
  a CRLF that the chunks of the read split is a CRLF
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`a_crlf_blob_edited_under_text_auto_keeps_its_crlf_as_add_does`, `the_old_crlf_attribute_and_text_off_take_the_filters_as_add_does`, `a_crlf_split_by_the_chunks_of_the_read_is_kept_as_add_keeps_it`)

#### Scenario: `ident` beside a kept CRLF blob
- **WHEN** such a file has `ident`, a `filter` or a `working-tree-encoding` too, whose bytes only `git add` gives
- **THEN** the commit takes the copy of the index, and is what `git add` stages
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`ident_beside_a_kept_crlf_blob_takes_the_index_copy`)

#### Scenario: A wide change
- **WHEN** the change of the worktree is wide
- **THEN** the objects of the commit are loose (no pack file per run)
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`a_wide_change_leaves_loose_objects_and_no_pack`)
