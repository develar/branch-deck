# Tasks

- [x] 1.1 `DiffOptions` and `diff_options`, shared by the service and `blocks_of`; `DiffKey::grammars` in the store.
- [x] 1.2 `MODEL_VERSION` 8 with the version of the tokens (`model_version`); `MERGE_MODEL_VERSION` 7.
- [x] 1.3 Tests: the same pair read by two grammars is two diffs, in memory, in the store and for a scan; a diff
  stored with the tokens of other parsers is not read (`store_version`).
