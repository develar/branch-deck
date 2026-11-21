## 1. The resolve and the note

- [x] 1.1 `ResolvedFile::generated_by` from the one `check-attr` of `resolve_by_words`; an update refuses a file its
  attributes mark before any `cat-file`
- [x] 1.2 `WordResolutionSummary.generated`, `GeneratedFile` in `domain-types`; the tail of the resolve line, read by
  `mapped_generated`, an unreadable tail an older resolve; `RESOLVE_VERSION` 5, `CONFLICT_CHAIN_VERSION` 4,
  `STACK_BUILD_VERSION` 4; `brd move-hunk` keeps the tail

## 2. The phase of the sync

- [x] 2.1 `git_ops::generated::held_word_merges` and `commits_changing`; `sync_core::regenerate`, called once after the
  builds; `SyncReport.regenerate`, `BranchTable.regenerate` and `stack_regenerate`, the warm-start frame
  (`FRAME_SCHEMA` 21)
- [x] 2.2 Tests: the resolve, the note, the held merges, the phase and its process budgets

## 3. `brd`

- [x] 3.1 `regenerate`, `regenerate_fix` and `resolved[].generated` of `brd sync`; `generated` of `resolved_by_words`
  in `brd status`
