# Tasks

- [x] `lease::is_unfinished`; `reservable`, `mark_template`, `shrink::next_free` pass over such a slot; the adopt names its slot at once
- [x] `trim::remove_unfinished_slots` under the clones lock of a get, `GetStep::RemoveUnfinished`, `removed_unfinished`; `forget_slot` drops a locked registration
- [x] `SlotState::Unfinished` in `brd lease status` and the disk report of the app
- [x] The cut of `DirtyCheck` and `IdleCheck` paths (`cut_paths`, `paths_omitted`), read cut from an older state
- [x] The tests and docs/design/git-operations.md
