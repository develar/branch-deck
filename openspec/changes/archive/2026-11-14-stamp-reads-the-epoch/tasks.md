# Tasks

- [x] The stamp reads the epoch; the session opens anew by `Actor::new` and `open`; session test with mutations (no
  reopen, no close event, no cut of a read of the lease before)
- [x] The page refreshes on a patch into another epoch and forgets the rows; unit tests and e2e with mutations (no
  check of the next epoch, no forget, no remount, no close of the preview)
- [x] The mirror of `brd mcp` resyncs on a patch into another epoch; mcp-host and test-server tests and an Air e2e with a
  mutation (no resync)
- [x] The writer tells the windows what it forgot; the page drops the synced path of a view without a table; e2e
- [x] The collapsed stacks by the epoch; unit test with a mutation
