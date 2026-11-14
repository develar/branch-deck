# Tasks

- [x] The reopen after a run in flight, at once during a check; the session tests of the probe and of a check, with
  mutations (no deferral, the reopen while the run runs, the follow-up dropped, a deferral past a check, the waiters
  of a check dropped)
- [x] The four stores and the side stacks forget the rows; a unit test of each, with mutations; the `row-store` rule
  with its test and the ALLOWLIST of the stores of the view
- [x] The epoch of the view in the witness of a rewrite and its refusal; the operations test, with a mutation
- [x] The mark written by the session under the epoch of the run; a session test with a git host that writes it, with a
  mutation (the mark written across the change)
- [x] One list of the files of a lease, forgotten at its end; a lease test and a unit test, with a mutation (the landed
  state left out of the list)
- [x] `row-store` keyed on the identities of a row; its test, and the mutation of a store keyed by `name`
