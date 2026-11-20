## 1. The rule of the update

- [x] 1.1 `trial_check::lost_bindings::replay_lost_bindings` (two ranges from one `log`, pairs by identity, the tip) and
  `lost_names_in_edit`
- [x] 1.2 `UpdatePlan::into_lost_bindings_conflict`, `FirstConflict.lost_bindings`; `update_project::check_lost_bindings`

## 2. The resolve session

- [x] 2.1 The lost-bindings round (`LostBindingRule`, `ConflictKind::LostBinding`, `RoundConflict::lost_bindings`,
  `Resolution::fixup`, state version 4): at the start and before every finish
- [x] 2.2 `--continue`: the `fixup!` commit, `lost_binding`, `--keep-name`; the output, `brd help update`

## 3. Tests

- [x] 4.1 The incident end to end in `resolve_test.rs`, the process budgets of the session
