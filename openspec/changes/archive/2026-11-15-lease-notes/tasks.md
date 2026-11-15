# Tasks

- [x] `lease_pool::lease_notes`: the directory and record of the notes, the link, the exclude, the fallback directory,
  the set-aside, the expiry and the listing, with their unit tests
- [x] The get opens the notes of an agent's lease, the return (and the takeover, the reclaim) closes them before any
  discard; a clone leaves `notes.local` out; `slot_changes` skips it
- [x] `brd lease notes`, `notes` of the get, the return and the status, `next`; the expiry in the trim and the keeper;
  `notes_bytes` of the disk report; the schema and the agent help
- [x] Tests in bd-lease (`notes_test.rs`), the budget of `brd lease trim`; docs: git-operations ("The notes of a lease"),
  contributing/leases.md, AGENTS.md
