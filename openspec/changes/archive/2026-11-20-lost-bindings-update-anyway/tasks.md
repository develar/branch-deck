## 1. The check of a sync

- [x] 1.1 The paths of the local commits from the `log` of the pairing (`read_landed_with_paths`), the rule only for a
  file both change (`UpdatePlan::check_lost_bindings`), its findings in the preview cache
- [x] 1.2 `SyncOptions::lost_bindings`, `preview_update` of `brd update --dry-run`, the process budgets

## 2. Update Anyway

- [x] 2.1 `Intent::UpdateProject::keep` to `UpdateProjectParams::keep`, `UpdateProjectResult::kept_lost_bindings`
- [x] 2.2 The button, the toast and Activity, the E2E test and the HTML snapshot

## 3. brd

- [x] 3.1 `conflict.lost_bindings` of `--dry-run`; `replayed` without the fixups
