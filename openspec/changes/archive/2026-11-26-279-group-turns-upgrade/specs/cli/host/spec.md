## MODIFIED Requirements

### Requirement: serve: The heavy queue takes turns between the groups of its runs

A new heavy run MUST NOT pass the head, a run of its own group, or a run that 8 runs of the turns are ahead of; behind
the last of those it MUST join before the first run with more runs of its group ahead than its own group has
waiting, else last. So the order is first come first inside a group, and exactly first come first with one waiting run
per group. The turns MUST keep none of the short lane's state.

#### Scenario: A group of many runs and a later run of another group
- **GIVEN** a group of many waiting heavy runs from checkouts of their own, such as the agents of one orchestrating
  session, none of them with 8 runs of the turns ahead of it
- **WHEN** a heavy run of another group, or of a checkout without a group, joins the queue later
- **THEN** it waits behind the first waiting run of that group, not behind all of them, also when a short run waits at
  the end of the queue, and the runs of each group keep their order
- **AND** it passes every run from its place to the end, a short run there too, and the head of the queue alone is
  admitted, as before
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_queue_takes_turns_between_groups_and_each_group_keeps_its_order`, `a_short_run_at_the_end_of_the_queue_keeps_the_turns_of_the_groups`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_heavy_queue_takes_turns_between_the_groups_of_its_runs`)

#### Scenario: Runs without a group
- **WHEN** heavy runs of other checkouts join without `--group`, each checkout with at most one run waiting, also a
  checkout whose run took the turn from the head of the queue and that joins again
- **THEN** each is of the group of its checkout, and they wait first come first exactly as before the groups
- **AND** a second waiting run of one checkout waits behind the first waiting runs of the other checkouts
  [@test] crates/lease-pool/src/host_turn_test.rs (`without_groups_the_runs_of_checkouts_with_one_run_waiting_each_are_first_come_first`)

#### Scenario: How often a run is passed
- **WHEN** runs of other groups join while a run waits, any count of groups, while the head can start or not
- **THEN** at most 8 runs of the turns get ahead of it, counting those that wait ahead of it and those that took the
  turn ahead of it; a joining run passes it only when a run whose round is above the count of the joining group's
  waiting runs stands at or ahead of it behind the joining run's barriers, so a run of round 0 behind a run of a later
  round is passed too, within its budget, and a run with no run of a later round ahead of it is passed by none
- **AND** a run of the turns that leaves the queue without the turn (a cancel, a start of `admit_now` refused `busy`,
  the end of its process, a write of the table that failed) counts none: the count is derived from `groups.json`,
  which names the runs that each run went ahead of and whether it took the turn
- **AND** with the 4 passes of the short lane a run waits at most 12 runs longer than first come first; behind long
  runs that can pass its 2 h of a heavy wait, and it ends with `heavy_timeout` then
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_is_passed_by_the_turns_at_most_eight_times_whatever_the_count_of_groups`, `a_first_waiting_run_behind_a_run_of_a_later_round_is_passed_within_its_budget`, `a_run_of_the_turns_that_is_gone_or_whose_table_write_failed_counts_no_pass`)
  [@test] crates/lease-pool/src/host_estimate_test.rs (`the_estimate_places_a_run_where_the_join_puts_it_over_generated_queues`)

#### Scenario: The short lane, the head, the supersede and the starts at once
- **WHEN** a short run, an exclusive head, a run that supersedes or a start of `admit_now` meets the turns of the groups
- **THEN** a run of the turns adds no pass of the short lane (`passes.json`), sets no join of its own from another run
  and counts its load bound from its own join; a short run still joins ahead of the runs it may pass, a backlog that
  the turns passed too, and no run of the turns goes ahead of the head (index 0 of the queue)
- **AND** a run that supersedes a run of its group takes its place, the runs it went ahead of, and the passes that runs
  of the turns made of it, so a run that an agent starts again is not passed 8 more times; one of another group takes
  its own turn, and the passes of the run it replaces end with it; an exclusive run, its drain and its pass, and a start
  of `admit_now` keep their rules
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_short_lane_the_head_and_the_supersede_keep_their_rules_beside_the_turns`, `a_run_that_supersedes_a_run_of_another_group_takes_its_own_turn`, `a_run_that_supersedes_a_run_of_its_group_keeps_the_passes_of_its_place`, `a_short_run_goes_ahead_of_the_runs_that_wait_for_more_and_runs_beside_the_holder`, `a_heavy_run_that_fits_starts_past_an_exclusive_run_whose_drain_ended`)
  [@test] crates/host-agent/tests/serve/admit.rs (`of_two_starts_that_admit_at_once_on_a_free_host_one_runs_and_the_other_is_busy`)

#### Scenario: A table that an older build rewrote
- **WHEN** a supervisor of an older build rewrites `turn.json` without `group`, and `passes.json` without the fields it
  does not know, as builds 1.24 to 1.27 do
- **THEN** no turn changes, as the groups and their passes are in `groups.json`, which no older build reads or writes,
  and only the lines lose the group
- **AND** a run that `groups.json` does not name is of the group of its entry, its `--group`, else its affinity, else
  alone: the queue goes on and no run is lost
- **AND** no `groups.json` of any build makes the turn refuse: the file of the first build of the groups (no `schema`,
  a number of `passes` per run), which a host that ran it has and its supervisors write again after an upgrade, keeps
  its groups and the runs each went ahead of, and the passes of the runs that took the turn before count no more; a
  file of a later schema, after a downgrade, and one that does not parse name no group and no pass, and every waiting
  run gets its whole budget of 8 again; the next join writes the file of this schema; a field of a mark that the build
  does not know is ignored
- **AND** of those two the log of the agent (`~/Library/Logs/brd-host.log`) and the log of the run that read it say
  `<path> has the schema <n> of a later build of brd-host, and this one reads 1: …` or `<path> does not parse (…): …`
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_rewrite_by_an_older_build_changes_no_turn_and_without_the_groups_a_run_is_of_its_checkout`, `the_groups_of_the_queue_name_their_schema_and_a_file_of_a_later_build_or_that_does_not_parse_starts_anew`, `the_groups_of_the_first_build_of_the_groups_keep_their_turns_after_an_upgrade`, `a_mark_of_the_groups_with_a_field_this_build_does_not_know_is_read_and_kept`)
