## MODIFIED Requirements

### Requirement: stats: The end of a run says whether a cancel interrupted its command

When the supervisor sent the command group the SIGINT of a cancel or a stop signal, it MUST write `"cancelled": true`
into `exit.json`, whatever code the command then exited with, and the line MUST carry it; a cancel after the command
exited MUST leave the run complete. Such a run MUST count as no failure, as an outcome of its own, and in no spread.

#### Scenario: A cancel of a command that traps SIGINT
- **WHEN** a cancel interrupts a command that exits 7 in its trap, or one that ignores SIGINT and gets SIGKILL (137)
- **THEN** `exit.json` has `"cancelled": true` beside the exit code of the command, and the line `"cancelled": true`
- **AND** a run that no cancel interrupted has no `cancelled`; a line of a build before it tells a cancel only by the
  exit 130
- **AND** in the outcomes of the summary it is `cancelled at exit <code> <runs>`, apart from the same exit that no
  cancel caused
  [@test] crates/host-agent/tests/serve/signals.rs (`a_cancel_interrupts_the_command_group`, `a_command_that_ignores_sigint_gets_sigkill_after_the_grace_time`)
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/host-agent/src/history_test.rs (`the_line_of_a_run_whose_command_a_cancel_interrupted_says_so_whatever_its_exit`, `a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

#### Scenario: A cancel after the command exited
- **WHEN** a cancel comes after the command exited, while the supervisor ends the processes that it left in its group
- **THEN** the run ends with the exit of the command and no `cancelled`
  [@test] crates/host-agent/tests/serve/signals.rs (`a_cancel_after_the_command_exited_leaves_its_run_complete`)

### Requirement: stats: A run writes its slot and how its lend chose it

A lend MUST say how it chose the slot, `affinity` for the idle slot whose last lend had the affinity of the run, else
`idle` with why no idle slot of the affinity was there, whether the ticket of the run waited in the queue of the slots
before it, and whether a clean came since the last run of the slot; the supervisor MUST write it to `slot.json` after
the lend, and the line of the run MUST carry it as `slot`.

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
- **WHEN** the slot of an affinity is cleaned after the give-back of its run, by hand, by the clean after a run once
  one is due, by the idle clean, or by a relief, and the next run of that affinity gets it
- **THEN** its lend says `affinity` and `cleaned`, and the line has `"cleaned": true`; the run after a give-back with
  no clean says no `cleaned`; a table whose give-back wrote no `returned` (a build before it) counts the clean of the
  second of the last lend or later
- **AND** a last run of the affinity that ended before its checkout (a refusal while it waited for its pack, a cancel)
  leaves the files of the checkout before it, and the next lend says `affinity` without `cleaned`: the table does not
  tell it
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_of_the_slot_of_its_affinity_says_whether_a_clean_came_since_its_last_run`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

#### Scenario: Why a lend missed the slot of its affinity
- **WHEN** a lend of `idle` finds no idle slot whose last lend had the affinity of the run
- **THEN** it says `missed`: `lent` when a slot of the affinity is lent to another run of the same checkout,
  `unavailable` when one is not ready for a run (its give-back, clean, shrink or fill, a quarantine, a lease not
  `ready`), else `no_slot`; a repeat says none
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_that_misses_the_slot_of_its_affinity_says_why`)

### Requirement: stats: The summary counts the runs that got the slot of their affinity

Per repository, the summary MUST count the runs whose line names their slot, the warm hits, the hits on a slot cleaned
since, the runs that waited, and the misses by cause; per command of the repository, the 10 with the most runs and the
rest in one entry, it MUST give its runs and hits and, for the warm hits, the cleaned hits and the misses, the runs whose
command ran to its end with the p50 and p90 of their `command`, `checkout` and `prepare`.

#### Scenario: The slots of two repositories
- **WHEN** the agent sums a window whose lines name their slots in two repositories
- **THEN** the summary has `slots`, one entry per repository by its path as written with `~/` expanded (two spellings
  of one repository, a symlink or a trailing `/`, are two entries): `repo`, `runs`, `affinity`, `cleaned`, `waited`,
  `misses` (`first`, `taken`, `lent`, `unavailable`, `other`) and `commands`, each with `command_key`, `command`,
  `measure_as`, `runs`, `affinity`, `cleaned` and the buckets `warm`, `after_clean` and `missed`
  (`{"runs", "command_ms", "checkout_ms", "prepare_ms"}`, the spreads `{"p50", "p90"}`, none without a run whose
  command ran to its end: a cancel whatever its exit, a refusal or a lost supervisor leaves a run out), and
  `other_commands` (`{"commands", "runs"}`) past the 10
- **AND** a miss of `no_slot` is `first` when no line of the window whose lend came earlier (the end of its phase
  `slot`, not its start: a heavy run waits before its lend) names a slot of its affinity in the repository, so a
  checkout whose last run came before the window counts as first, else `taken`; a cause or a choice of a later build
  is `other`, and a report of an agent before the causes names the runs on another slot without them
- **AND** its lines, after the heavy queue, are `slots of <repo>: <runs> run(s) with a slot: <n> warm on the slot of
  their checkout (<percent> %), <n> on it cleaned since, <n> on another slot (<n> first of their checkout in the
  window, <n> whose slot another checkout took since, <n> whose slot another run of their checkout had, <n> whose slot
  was not ready, <n> of another cause); <n> waited in the queue of the slots`, then per command `<runs> run(s), <hits>
  on the slot of their checkout; warm <n> run(s), command p50 <time> p90 <time>, checkout p50 <time>, prepare p50
  <time>; after a clean …; missed …: <command>` and `<n> other command(s): <n> run(s)`, a part of no run left out
- **AND** a bucket of fewer than 5 runs prints `<n> run(s), too few for a p50`, and the JSON keeps its spreads with
  its runs
- **AND** the buckets compare the runs of one command, not of one size of change: a miss of a small change can take
  less than a hit of a large one
- **AND** a window without a slot of record, and an agent of an older build, give no `slots`
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`, `the_lends_of_a_repository_name_the_commands_with_the_most_runs_and_the_rest_in_one_entry`, `the_first_run_of_a_checkout_is_the_first_lend_not_the_first_start`, `the_slots_of_a_report_without_the_causes_of_its_misses_read_without_them`)
