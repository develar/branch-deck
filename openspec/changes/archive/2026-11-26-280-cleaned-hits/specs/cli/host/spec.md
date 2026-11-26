## MODIFIED Requirements

### Requirement: follow, cancel, fetch, forget, runs: runs names the repository and the slot of each run

`RunInfo` of `brd-host runs --json` MUST name the repository of the run with `~/` expanded, as the history line and
`stats` name it (`repo`), and, once a lend gave it a slot, the slot (`slot`): its number `n`, how the lend chose it
(`choice`), `waited` when the lend came after a wait and `cleaned` when a clean came since the last run of the slot. A
forgotten run MUST keep its slot; an older agent sends neither field.

#### Scenario: The slot of a run
- **WHEN** `brd-host runs <alias> --json` runs after the lend of a run
- **THEN** its `RunInfo` has `"repo": "<repository>"` and `"slot": {"n": <n>, "choice": "<choice>"}`, with
  `"waited": true` after a wait in the queue of the slots of any cause (busy slots, runs before it, a fill, an upgrade
  of git) and `"cleaned": true` after a clean; `choice` is `affinity` for the idle slot whose last lend had the
  affinity of the run, `idle` for the idle slot used longest ago, which another affinity used last or no run did, and a
  choice of a later build reads by its name
- **AND** a run before its lend, a run that got no slot, and a run of an older agent or supervisor have no `slot`,
  and a reply of an older agent no `repo`: the client shows nothing for them
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/host-agent/src/run_test.rs (`a_forgotten_run_keeps_its_end_and_its_log_and_its_files_go`)
  [@test] crates/host-link/src/proto_test.rs (`the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema`)

### Requirement: stats: A run writes its slot and how its lend chose it

A lend MUST say how it chose the slot, `affinity` for the idle slot whose last lend had the affinity of the run, else
`idle`, whether the ticket of the run waited in the queue of the slots before it, and whether a clean came since the
last run of the slot, which keeps its affinity; the supervisor MUST write it to `slot.json` after the lend, and the
line of the run MUST carry it as `slot`.

#### Scenario: Each kind of choice
- **WHEN** a run gets the slot that no run used, a run of the same affinity waits for it and gets it as it comes
  free, the next run of that affinity gets it at once, and a run of another affinity waits for it and gets it
- **THEN** their lends say `idle`, `affinity` after a wait, `affinity`, and `idle` after a wait; a repeat of a lend
  that the run holds already says no choice, which no supervisor asks for
- **AND** the line of the run has `"slot": {"n": <n>, "choice": "<choice>", "waited": true}`, `waited` only after a
  wait; a line of a supervisor of a build before it has no `slot`, and a choice of a later build reads as `other`
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_says_whether_it_got_the_slot_of_its_affinity_and_whether_it_waited`)
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

#### Scenario: Two idle slots
- **WHEN** two slots are idle, one of them used last by the affinity of the run and the other idle longer
- **THEN** the lend takes the slot of the affinity and says `affinity`; a run of an affinity that neither had takes
  the slot idle longest and says `idle`, though another affinity used the other one later
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_names_the_slot_of_its_affinity_over_one_idle_longer_and_else_the_one_idle_longest`)

#### Scenario: A clean since the last run of the slot
- **WHEN** the slot of an affinity is cleaned after the give-back of its run (the clean after a run, the idle clean, a
  relief), and the next run of that affinity gets it
- **THEN** its lend says `affinity` and `cleaned`, and the line has `"cleaned": true`; the run after a give-back with
  no clean says no `cleaned`
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_of_the_slot_of_its_affinity_says_whether_a_clean_came_since_its_last_run`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

### Requirement: stats: The summary counts the runs that got the slot of their affinity

Per repository, the summary MUST count the runs whose line names their slot, those that got the slot of their
affinity (`affinity`, after a wait or not), of them those on a slot cleaned since, and those that waited, and give the
p50 and p90 of the command of the warm hits, of the cleaned hits and of the others, each of the commands that ran to
their end; a run without a slot of record counts in none.

#### Scenario: The slots of two repositories
- **WHEN** the agent sums a window whose lines name their slots in two repositories
- **THEN** the summary has `slots`, one entry per repository by its path: `repo`, `runs`, `affinity`, `cleaned`,
  `waited`, `warm_command_ms`, `cleaned_command_ms` and `other_command_ms` (`{"p50", "p90"}`, none without a command
  that ran to its end: a cancel, a refusal or a lost supervisor leaves a run out), a choice of a later build among the
  others
- **AND** its line, after the heavy queue, is `slots of <repo>: <affinity> of <runs> run(s) got the slot that their
  checkout used last (<percent> %); <n> warm, command p50 <time>; <n> cleaned since, command p50 <time>; the other
  <n>, command p50 <time>; <n> waited in the queue of the slots`, a part of no run left out and each `, command p50`
  only with a command
- **AND** a window without a slot of record, and an agent of an older build, give no `slots`
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)
