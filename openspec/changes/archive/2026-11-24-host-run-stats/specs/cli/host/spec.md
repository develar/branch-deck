## MODIFIED Requirements

### Requirement: The files: Each end keeps its files at the paths of the table

brd-host MUST keep the files of the client, of the agent and of a checkout at the paths of the table below, each with
the content that the table names. In a lease of the pool, whose `notes.local` links to the notes of the lease, the log
and the patch of a run MUST go to `notes.local/brd-host/` instead, in the pool directory, which the summary names.

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
| Agent | `~/.cache/brd-host/runs/<run>/` | `spec.json`, `events.log`, `exit.json`, `supervisor.log`, `outputs/`, `changes.patch`, `queue.json` (the place and the wait of a heavy run in the queue of the heavy turn), `pack.json` and `fetched.json` (the bytes of its packs and of its fetches) |
| Agent | `~/.cache/brd-host/history/` | `<YYYY-MM>.jsonl`, one line per ended run in the month of its end by UTC, and `bases-<YYYY-MM>.jsonl`, one line per pack of a base, each kept 90 days |
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared`, `heavy/turn.json` and `heavy/turn.lock` (the heavy turn of the host, in a directory of its own), `heavy/cores.json` and `heavy/cores.lock` (the measures of the commands), `heavy/host.json` (the busy cores of the host every 2 s, with the most of the last 30 s), `heavy/exclusive.json` (the exclusive run that holds the turn), `heavy/drain.json` (the exclusive run that drains the host, since when), `heavy/heavy.json` (the heavy turn of the build before, which this build only reads), `runs/<run>/cores.json` (the cores of the command of a run) |
| Agent | `~/.local/bin/brd-host`, on a local host `~/.local/share/brd-host/agent/brd-host` | The binary that the LaunchAgent runs, and `<binary>.previous` after an upgrade |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply`; in a lease the patch is `notes.local/brd-host/<run>.patch` |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.log`, `<run>.end.json`, `<run>.lock` | The output of the run from its first byte, which its end writes; how the run ended here (`{"exit", "command_exit", "refusal", "phases", "fetched", "applied_files", "forgotten"}`); the lock that one process of the checkout holds while it finishes the run |

#### Scenario: The log of a run in a lease
- **WHEN** a wait finishes a run in a lease whose `notes.local` links to the notes of the lease
- **THEN** the log is `notes.local/brd-host/<run>.log`, by that path in the summary, and the end stays in the git
  directory
  [@test] crates/host-client/src/wait_test.rs (`a_wait_in_a_lease_puts_the_log_into_the_notes_of_the_lease`)

## ADDED Requirements

### Requirement: stats: The command line of stats

`brd-host stats` MUST take an optional alias of a host, `--days <n>` of 1 to 90 (7 by default) and `--json`; a count
of days outside them is a usage error.

#### Scenario: The synopsis of stats
- **WHEN** a caller runs `brd-host stats`
- **THEN** it takes this command line, and `--days 91` exits 2
  [@test] crates/bd-host/tests/cli.rs (`stats_summarizes_the_runs_that_ended_on_the_host`)

```sh
brd-host stats [<alias>] [--days <n>] [--json]
```

### Requirement: stats: The agent keeps a line of each ended run for 90 days

`serve` MUST append the line of an ended run to `history/<YYYY-MM>.jsonl` of the month of its end as its run
directory goes, by its age or past the kept forgotten runs, read before the removal and written after it, once per
run; the hourly cleanup MUST remove a month whose every line is more than 90 days old.

#### Scenario: A run directory goes
- **WHEN** the run directory of an ended run goes, a day after its end or past the kept forgotten runs
- **THEN** its line is in the month of its end: the run, the repository, the affinity, the commit, the command in one
  line of at most 200 characters, the key of its measures, `heavy`, `exclusive`, its start and end, its exit and
  refusal, `lost`, its phases, its cores, its bytes and its queue
- **AND** a run that goes on stays and has no line
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`, `a_forgotten_run_past_the_kept_ones_leaves_its_line_with_its_whole_output`)

#### Scenario: The months past 90 days
- **WHEN** the hourly cleanup runs
- **THEN** a month of the history, of runs or of bases, whose every line is more than 90 days old goes, and the other
  files stay
  [@test] crates/host-agent/src/history_test.rs (`the_months_of_the_history_past_ninety_days_go`)

### Requirement: stats: The agent counts the bytes of a run and of a base

A pack MUST add the bytes that came to `pack.json` of its run, a fetch the bytes that it sent to `fetched.json`, and
the output of a run MUST count its log with what a forget cut of it; a pack of a base MUST add a line to
`history/bases-<YYYY-MM>.jsonl`.

#### Scenario: A pack, a fetch and a base
- **WHEN** a pack of a run lands, a fetch sends the files of a run, or a pack of a base lands
- **THEN** `pack.json` and `fetched.json` of the run hold the bytes and the count of its packs and fetches, and the
  history a line of the base with its bytes
  [@test] crates/host-agent/tests/serve.rs (`a_run_waits_for_its_pack_checks_out_by_the_diff_and_gives_its_slot_back`)
  [@test] crates/host-agent/tests/serve/pack.rs (`a_pack_after_the_end_of_its_run_lands_and_marks_no_run`)
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`, `a_forgotten_run_past_the_kept_ones_leaves_its_line_with_its_whole_output`)

### Requirement: stats: A heavy run writes its place and its wait in the queue

The supervisor of a heavy run MUST write `queue.json` after its wait for the heavy turn, whatever its end: its place
as it joined, the holders then, the cores of the host, its expected cores, its need, and its wait split into the wait
behind holders or runs before it and the wait at the head of a free turn for the load of the host.

#### Scenario: Two heavy runs
- **GIVEN** a heavy run that holds the turn
- **WHEN** a second heavy run waits for it, and both end
- **THEN** `queue.json` of the first names the place 0, no holder and no wait, and that of the second the place 0, one
  holder and its wait behind it
- **AND** a run that is not heavy has no `queue.json`
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)
  [@test] crates/host-agent/src/history_test.rs (`the_watch_of_a_heavy_run_writes_its_place_and_its_wait_in_the_queue`)

#### Scenario: What a look of the wait finds
- **WHEN** a look finds the run behind a holder or a run before it, or at the head of the queue with no holder
- **THEN** the span until the next look counts behind the others, or for the load of the host, and the need follows
  the measure as the heavy turn counts it
  [@test] crates/lease-pool/src/host_history_test.rs (`a_run_in_the_queue_waits_behind_others_or_for_the_load_of_the_host`, `the_clock_of_the_queue_splits_the_wait_by_the_spot_of_each_look`, `the_need_of_a_heavy_run_follows_its_measure`)

### Requirement: stats: stats asks the agent for the summary of its runs

`brd-host stats <alias>` MUST ask the agent for the summary of the runs that ended in the last `--days` days
(`Request::Stats`, feature `stats`): the agent sums the lines of the history in that window and the ended runs that
it still keeps, heartbeats while it reads, and answers the summary as JSON (`lease_pool::host_history::StatsReport`).

#### Scenario: The summary of a host
- **WHEN** `brd-host stats <alias>` runs against an agent of this build
- **THEN** the agent answers the summary of its history and of the ended runs that it keeps, no run that goes on, and
  skips a line that does not parse
  [@test] crates/host-link/src/proto_test.rs (`a_request_and_a_reply_have_a_tagged_json_shape`, `an_agent_on_macos_has_the_upgrade_and_every_agent_the_disk_the_trim_and_the_heavy_turn`)
  [@test] crates/host-client/src/run_test.rs (`stats_asks_an_agent_of_the_feature_for_the_summary_of_its_runs`)
  [@test] crates/host-agent/src/history_test.rs (`the_summary_reads_the_lines_of_its_window_and_the_ended_runs_that_the_agent_keeps`)

### Requirement: stats: The summary splits the waits from the work and groups the commands

The summary MUST give the waits (`heavy`, `exclusive`, `git`, `slot`, `pack_wait`) against the work by phase, the heavy
queue, the outcomes by exit and refusal, the runs that ended before their command, the cores, per command the runs,
the failures and the p50 and p90 of its wait and its command for the 30 that waited longest and the rest in one
entry, and the bytes by day.

#### Scenario: A day of runs
- **WHEN** the agent sums the runs of a window
- **THEN** the summary has each of these, a timeout or a cancel counts as no failure of a command, and only the 30
  commands that waited longest come one by one
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_splits_the_waits_from_the_work_and_groups_the_commands`, `the_commands_past_the_thirty_that_waited_longest_are_one_entry`, `a_day_is_its_utc_date`)

### Requirement: stats: An agent without the feature gets no request

A client MUST check the feature `stats` in the hello first: an agent without it gets no request, and its host names
`agent_failed` with `brd-host upgrade <alias>`.

#### Scenario: An agent of an older build
- **WHEN** the hello of an agent has no feature `stats`
- **THEN** the client sends it no request, and the host names `agent_failed` with `brd-host upgrade <alias>`
  [@test] crates/host-client/src/run_test.rs (`stats_asks_an_agent_of_the_feature_for_the_summary_of_its_runs`)

### Requirement: stats: stats prints the summaries of the hosts

Without an alias `stats` MUST ask every host of the client, one after the other, and a host that answers nothing MUST
be named while the others go on; `--json` MUST print `{"schema", "hosts": [{"alias", "report"} or {"alias",
"refusal"}]}`, and without it each host MUST be the lines of its summary.

#### Scenario: stats with and without --json
- **WHEN** `brd-host stats --json` and `brd-host stats` run
- **THEN** the first prints the summary of each host as JSON, and the second its lines: the runs, the waits and the
  work, the queue, the outcomes, the cores, the commands by their wait and the bytes
  [@test] crates/bd-host/tests/cli.rs (`stats_summarizes_the_runs_that_ended_on_the_host`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_lines_of_a_summary`)
