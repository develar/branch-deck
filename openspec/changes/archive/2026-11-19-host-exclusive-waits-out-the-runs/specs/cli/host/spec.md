## MODIFIED Requirements

### Requirement: serve: An exclusive run holds a quiet host alone

An exclusive run MUST be admitted only without a holder, with no run that is not heavy in its command, and when every
sample of the host of the last 30 s is quiet. While it holds, no heavy run MUST be admitted, and each new run that is
not heavy, of any repository, MUST wait for its end before its lend, in the step `exclusive`, up to 1 h. The holder MUST
also be in `exclusive.json`, which no older build rewrites.

#### Scenario: The admission of an exclusive run
- **WHEN** an exclusive run comes to the head of the queue
- **THEN** it waits until no heavy run holds cores and no run that is not heavy is in its command, of any repository
  (a run that is not heavy is in `light` of the turn from the start of its command to its end, at 0 cores in a lull
  too), and then until every sample of the host of the last 30 s is quiet (`host.json` keeps their most); a window of
  less than 30 s, or a lull of one sample, does not admit it, and the wait goes on up to its 2 h
- **AND** then no heavy run is admitted beside it, one of a measure that would fit neither
- **AND** the line of its wait says `all its <cores> cores of a quiet host, as an exclusive run`, the busy cores of the
  host outside the heavy runs, `waits for run <id> of <repository> to end: <command>` (`waits for <n> runs to end: run
  <id> of <repository> (<command>), …`) and `the host was busy with up to <cores> of <cores of the host> cores in the
  last 30 s`, so the run sees what keeps it out
  [@test] crates/lease-pool/src/host_turn_test.rs (`an_exclusive_run_waits_for_a_quiet_host_and_holds_it_alone`, `an_exclusive_run_waits_for_the_runs_in_their_command_and_a_quiet_window`, `the_window_of_the_host_keeps_the_samples_of_the_last_30_s`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`, `the_wait_of_an_exclusive_run_names_the_runs_in_their_command_and_the_window`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`an_exclusive_run_waits_for_a_run_in_its_command_to_end`)

#### Scenario: A run that is not heavy and an exclusive run
- **WHEN** a run that is not heavy starts while an exclusive run holds the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for a measurement: run <id> of
  <repository> holds the host for <time> (<command>); a measurement holds the host; this run starts when it ends`
- **AND** it takes its slot once the exclusive run gave the turn back, with the step `exclusive` first of the host;
  after 1 h it refuses with `slot_timeout`; a run of another repository waits the same
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_run_that_is_not_heavy_waits_for_an_exclusive_run_and_starts_after_it`, `a_run_of_another_repository_waits_for_an_exclusive_run`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_that_is_not_heavy_waits_for_the_end_of_an_exclusive_run`)

#### Scenario: A table that an older build rewrote
- **WHEN** a supervisor of a build before `exclusive` rewrites `turn.json` at a sample of its run and drops the field
  `exclusive` of the holder
- **THEN** the runs that are not heavy still wait, as they read the holder from `exclusive.json`, and `brd-host runs`
  still names the measurement; the next look of the exclusive run puts the field back, and its end removes the file
  [@test] crates/lease-pool/src/host_turn_test.rs (`an_exclusive_holder_outlives_a_rewrite_of_the_table_by_an_older_build`)

#### Scenario: brd-host runs and an exclusive run
- **WHEN** `brd-host runs` shows the heavy turn while an exclusive run holds it or waits
- **THEN** a holder is `run <id> of <repository> holds the whole host for a measurement for <time> (exclusive):
  <command>`, and a waiting one `waits <time> for a quiet host (exclusive)`
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_that_is_not_heavy_waits_for_the_end_of_an_exclusive_run`)

### Requirement: The files: Each end keeps its files at the paths of the table

brd-host MUST keep the files of the client, of the agent and of a checkout at the paths of the table below, each with
the content that the table names.

#### Scenario: The files of each end
- **WHEN** brd-host writes a file of its own
- **THEN** the file is at its path of the table, with its content

| End | File | Content |
|---|---|---|
| Client | `~/.config/brd-host/identity.crt`, `identity.key` | The certificate and the key of the client, made at the first call, mode 0600 |
| Client | `~/.config/brd-host/hosts/<alias>.json` | `{"alias", "fingerprint", "port", "hostname", "addresses", "repo"}` |
| Client | `~/.config/brd-host/bases/<alias>/<affinity>` | The commit of the base of this affinity on that host after the last base sync |
| Agent | `~/.config/brd-host/agent/identity.crt`, `identity.key` | The certificate and the key of the agent |
| Agent | `~/.config/brd-host/agent/clients` | The allow list: one `<64 hex> <name>` line per client |
| Agent | `~/.cache/brd-host/runs/<run>/` | `spec.json`, `events.log`, `exit.json`, `supervisor.log`, `outputs/`, `changes.patch` |
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared`, `heavy/turn.json` and `heavy/turn.lock` (the heavy turn of the host, in a directory of its own), `heavy/cores.json` and `heavy/cores.lock` (the measures of the commands), `heavy/host.json` (the busy cores of the host every 2 s, with the most of the last 30 s), `heavy/exclusive.json` (the exclusive run that holds the turn), `heavy/heavy.json` (the heavy turn of the build before, which this build only reads), `runs/<run>/cores.json` (the cores of the command of a run) |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply` |
