# Tasks

- [x] `brd-host cancel-unfinished` (`host_client::unfinished`): the runs without an end, the cancel per host within
  `--max`, the hosts in parallel, the forget of the records of the cancelled runs
- [x] `lease_pool::host_cancel`: the check of the records of a slot and the child `brd-host` at the start of
  `finish_return`, for the return, the takeover and the reclaim
- [x] `cancelled_runs` and `uncancelled_runs` in the answers of `brd lease return`, `pick --return`, `get` and `reclaim`
- [x] The tests: the client on a fake agent, the binary and a lease of the pool on a real agent, brd with a fake
  `brd-host`
- [x] The design doc, the guide and the skills of brd and brd-host
