# Tasks

- [x] `sync_utils::page_writes` with tests; `SettingsService::set_in_order`; `ConfigWriters` per config file, the
  companions through it; the mutex of `run_config_write` removed, the reason of a lock that stays; Rust tests with
  mutations (no order, `>` for `>=`, the writes of a file at once)
- [x] `seq` in `set_repository_config` and `set_setting`, null for superseded; bindings regenerated, the command count
  unchanged
- [x] `FieldWrites` `now` and failures, the settings store send at once, `settings_not_saved` in the main window; unit
  and e2e tests with mutations (no send at once in either, no tell, a superseded write told as an error)
- [x] Settings spec, Git Rules, git-operations.md, AGENTS.md line
