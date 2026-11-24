# Tasks

- [x] `legacy_notes_root` in `git_ops::lease_slot`; `links_to_lease_notes` reads the record of the notes
- [x] `slot_notes_root`, `new_notes_root`, `config_pool_dir`, `NotesRoots` in `lease_pool::lease_notes`; every reader
  looks in every place, the old one last
- [x] `machine_pools` skips `notes/`; the tests of the root, the old place, the trim and the fetch
- [x] The design doc, the guide of leases, the help of `brd lease`
