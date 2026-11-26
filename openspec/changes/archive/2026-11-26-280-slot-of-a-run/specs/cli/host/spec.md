## ADDED Requirements

### Requirement: follow, cancel, fetch, forget, runs: runs names the repository and the slot of each run

`RunInfo` of `brd-host runs --json` MUST name the repository of the run as its spec does (`repo`) and, once a lend gave
it a slot, the slot (`slot`): its number `n`, how the lend chose it (`choice`) and `waited` when the lend came after a
wait in the queue of the slots. A forgotten run MUST keep its slot; an older agent sends neither field.

#### Scenario: The slot of a run
- **WHEN** `brd-host runs <alias> --json` runs after the lend of a run
- **THEN** its `RunInfo` has `"repo": "<repository>"` and `"slot": {"n": <n>, "choice": "<choice>"}`, with
  `"waited": true` after a wait; `choice` is `affinity` for the idle slot that the affinity of the run used last,
  `idle` for the idle slot used longest ago, which another affinity used last or no run did, and a choice of a later
  build reads by its name
- **AND** a run before its lend, a run that got no slot, and a run of an older agent or supervisor have no `slot`,
  and a reply of an older agent no `repo`: the client shows nothing for them
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/host-agent/src/run_test.rs (`a_forgotten_run_keeps_its_end_and_its_log_and_its_files_go`)
  [@test] crates/host-link/src/proto_test.rs (`the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema`)

### Requirement: stats: A run writes its slot and how its lend chose it

A lend MUST say how it chose the slot, `affinity` for the idle slot that the affinity of the run used last, else
`idle`, and whether the ticket of the run waited in the queue of the slots before it; the supervisor MUST write it
with the number of the slot to `slot.json` of the run after the lend, and the line of the run MUST carry it as `slot`.

#### Scenario: Each kind of choice
- **WHEN** a run gets the slot that no run used, a run of the same affinity waits for it and gets it as it comes
  free, the next run of that affinity gets it at once, and a run of another affinity waits for it and gets it
- **THEN** their lends say `idle`, `affinity` after a wait, `affinity`, and `idle` after a wait; a repeat of a lend
  that the run holds already says no choice
- **AND** the line of the run has `"slot": {"n": <n>, "choice": "<choice>", "waited": true}`, `waited` only after a
  wait; a line of a supervisor of a build before it has no `slot`, and a choice of a later build reads as `other`
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_says_whether_it_got_the_slot_of_its_affinity_and_whether_it_waited`)
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

### Requirement: stats: The summary counts the runs that got the slot of their affinity

Per repository, the summary MUST count the runs whose line names their slot, those that got the slot of their
affinity (`affinity`, after a wait or not) and those that waited, and give the p50 and p90 of the command of the
first and of the others; a run without a slot of record counts in none.

#### Scenario: The slots of two repositories
- **WHEN** the agent sums a window whose lines name their slots in two repositories
- **THEN** the summary has `slots`, one entry per repository by its path: `repo`, `runs`, `affinity`, `waited`,
  `affinity_command_ms` and `other_command_ms` (`{"p50", "p90"}`, none without a command), a choice of a later build
  among the others
- **AND** its line, after the heavy queue, is `slots of <repo>: <affinity> of <runs> run(s) got the slot that their
  checkout used last (<percent> %), command p50 <time>; the other <n>, command p50 <time>; <n> after a wait in the
  queue of the slots`, each `, command p50` only with a command
- **AND** a window without a slot of record, and an agent of an older build, give no `slots`
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

## MODIFIED Requirements

### Requirement: stats: The agent keeps a line of each ended run for 90 days

`serve` MUST append the line of an ended run to `history/<YYYY-MM>.jsonl` of the month of its end as its run
directory goes, by its age or past the kept forgotten runs, read before the removal and written after it, once per
run; the hourly cleanup MUST remove a month whose every line is more than 90 days old.

#### Scenario: A run directory goes
- **WHEN** the run directory of an ended run goes, a day after its end or past the kept forgotten runs
- **THEN** its line is in the month of its end: the run, the repository, the affinity, the commit, the command in one
  line of at most 200 characters, the key of its measures, `heavy`, `exclusive`, its start and end, its exit and
  refusal, `lost`, its phases, its cores, its bytes, its queue and its slot
- **AND** a run that goes on stays and has no line
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`, `a_forgotten_run_past_the_kept_ones_leaves_its_line_with_its_whole_output`)

#### Scenario: The months past 90 days
- **WHEN** the hourly cleanup runs
- **THEN** a month of the history, of runs or of bases, whose every line is more than 90 days old goes, and the other
  files stay
  [@test] crates/host-agent/src/history_test.rs (`the_months_of_the_history_past_ninety_days_go`)
