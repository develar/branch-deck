## ADDED Requirements

### Requirement: follow, cancel, fetch, forget, runs: cancel-unfinished cancels the runs of a checkout without an end

`brd-host cancel-unfinished [--max <duration>] [--json]` MUST cancel each run of the checkout whose record has
`<run>.host` and no `<run>.end.json`, on that host, as `cancel` does: the hosts in parallel, each within `--max` (5 s
by default). A run that the agent took, ended or does not know is cancelled; its record goes under its lock. A
run unknown within 120 s of its record, and every other run, stays. It MUST exit 0 when no such run stays, else 255.

#### Scenario: The runs of a checkout without an end
- **GIVEN** a checkout with a run that ended, runs without an end on one host, runs of a host that no agent serves,
  and a record without `<run>.host`
- **WHEN** `brd-host cancel-unfinished` runs in it
- **THEN** only the runs without an end get a cancel, each on the host of its record, and a run of a host that refused
  the link gets the refusal of the run before it without a try
- **AND** a run whose cancel the agent took, that ended there or that the agent does not know (`unknown_run`) is in
  `cancelled` with the answer of the agent, and its `<run>.json` and `<run>.host` go, under its `<run>.lock` when that
  file exists; the lock file stays
- **AND** a run whose `<run>.lock` another process holds is cancelled, but its records stay, and its answer says so
- **AND** a run that the agent does not know, whose `<run>.host` is younger than 120 s, stays in `not_cancelled` with
  `unknown_run` and its records: `Client::run` writes them before it sends the start, which can still come
- **AND** a host that does not answer within `--max` keeps its runs in `not_cancelled` with `timed_out`, beside the
  refusals of the other runs, and their records stay
  [@test] crates/host-client/src/unfinished_test.rs (`only_the_runs_without_an_end_are_cancelled_and_a_silent_host_keeps_its_runs_past_max`)
  [@test] crates/bd-host/tests/cli.rs (`a_return_of_a_lease_cancels_only_its_runs_without_an_end_and_a_silent_host_holds_it_within_the_bound`)

#### Scenario: A checkout without run records
- **WHEN** the checkout has no run directory
- **THEN** it connects to no host, makes no directory and answers no run
  [@test] crates/host-client/src/unfinished_test.rs (`a_checkout_without_run_records_connects_to_no_host`)

#### Scenario: The answer
- **WHEN** `cancel-unfinished` ends
- **THEN** with `--json` it prints `{"schema", "cancelled": [{"run", "host", "answer"}], "not_cancelled": [{"run",
  "host", "code", "message"}]}`, else one line per run, `cancelled the run <run> on <host>: <answer>` or `cannot cancel
  the run <run> on <host>: <code>: <message>`, or `no run of this checkout waits for its end here`
  [@test] crates/bd-host/src/cli_test.rs (`cancel_unfinished_says_a_line_per_run_and_takes_a_max`)
