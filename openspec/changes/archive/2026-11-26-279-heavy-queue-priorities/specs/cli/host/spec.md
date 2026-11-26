## ADDED Requirements

### Requirement: run: --priority names the priority of a heavy run inside its group

`--priority high|normal|low` MUST name the priority of a run inside its group of the heavy queue, which its `Start`
and `Estimate` MUST carry as `priority`; without it the run keeps the priority of the waiting run it supersedes, else
normal. A named priority on an agent without the feature `priority` MUST refuse with `bad_config` before the start.

#### Scenario: A run of a priority
- **WHEN** a run starts with `--priority high`
- **THEN** its `Start` and its `Estimate` have `"priority": "high"`, and a run without the flag has no `priority`
- **AND** a level other than `high`, `normal` and `low` refuses with `usage`
  [@test] crates/bd-host/src/cli_test.rs (`a_run_takes_its_group_from_the_lease_and_names_its_priority`)
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_priority_names_it_in_the_start_and_an_agent_before_priorities_refuses_it`)
  [@test] crates/host-link/src/proto_test.rs (`the_priority_of_a_run_is_an_optional_field_of_a_start_and_an_estimate_and_prioritize_a_request`)

#### Scenario: An agent without the feature priority
- **WHEN** a run of `--priority` starts on an agent whose hello has no feature `priority`
- **THEN** the client refuses with `bad_config` (exit 75) before the start: `the agent on <alias> predates the
  priorities of the heavy queue, so it would queue this run first come first: `brd-host upgrade <alias>` sends this
  build`
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_priority_names_it_in_the_start_and_an_agent_before_priorities_refuses_it`)

### Requirement: follow, cancel, fetch, forget, runs: prioritize changes the priority of a waiting run

`brd-host prioritize <host> <run> high|normal|low` MUST give a heavy run that waits in the queue of the host that
priority inside its group and order the runs of its group again, as a join does, and say where it moved. A holder MUST
change nothing, and a run that the turn does not name MUST refuse with `unknown_run`.

#### Scenario: A waiting run
- **WHEN** `brd-host prioritize studio <run> high` names a run that waits behind runs of its group
- **THEN** it moves ahead of those of a lower priority, never ahead of the head or of a turn of another group, and the
  command prints `run <run> waits with the priority high in its group: place <n> of the queue, was <m>, ahead of
  <runs>; the runs of other groups keep their turns`; its later looks keep the order
  [@test] crates/lease-pool/src/host_turn_test.rs (`prioritize_moves_a_queued_run_inside_its_group_and_leaves_a_holder_as_it_is`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_name_a_priority_and_prioritize_says_where_the_run_moved`)

#### Scenario: A holder, an unknown run and an older agent
- **WHEN** the run holds the turn, the turn does not name it, or the agent has no feature `priority`
- **THEN** a holder prints `run <run> holds the heavy turn already: its priority orders nothing`, an unknown run
  refuses with `unknown_run`, and an older agent refuses with `agent_failed` and the upgrade before the request
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_name_a_priority_and_prioritize_says_where_the_run_moved`)
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_priority_names_it_in_the_start_and_an_agent_before_priorities_refuses_it`)

## MODIFIED Requirements

### Requirement: run: --group names the group of a run in the heavy queue

`--group <group>`, else `BRD_HOST_GROUP` when it is not empty, else the live session that holds the lease of the
checkout, MUST name the group of a run in the heavy queue, else its affinity, and its `Start` and `Estimate` MUST carry
it as `group`. An agent without the feature `group` MUST get no `group`: it queues the run first come first, so the
client does not refuse, and a heavy run says so.

#### Scenario: A run of a group
- **WHEN** a run starts with `--group <group>`, or with `BRD_HOST_GROUP=<group>` set once by an orchestrator for all its
  agents
- **THEN** its `Start` and its `Estimate` have `"group": "<group>"`, the flag over the variable
- **AND** a run without either, or with an empty variable, takes the default group of its checkout (below)
  [@test] crates/bd-host/src/cli_test.rs (`group_names_the_group_of_the_run_by_the_flag_or_the_environment`)
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_group_names_it_in_the_start_and_an_agent_before_groups_queues_it_first_come_first`)
  [@test] crates/host-client/src/choose_test.rs (`the_estimate_of_a_run_of_a_group_names_its_group`)
  [@test] crates/host-link/src/proto_test.rs (`the_group_of_a_run_is_an_optional_field_of_a_start_and_an_estimate`)

#### Scenario: The default group of a run in a lease
- **WHEN** a run without `--group` and `BRD_HOST_GROUP` starts in a slot of `brd lease` whose holder lives (its
  `holder_pid` and `holder_started`, the coding agent that took the lease, whose subagents take their leases so too)
- **THEN** its group is `session-<12 hex>` of that holder, so the runs of all the leases of one session take one turn
  between the turns of other groups
- **AND** a checkout that is no slot, a slot without a lease or whose holder is gone has no `group`, and its group is
  its affinity; a pool whose state cannot be read says so in one line and counts as none
  [@test] crates/bd-host/src/cli_test.rs (`a_run_takes_its_group_from_the_lease_and_names_its_priority`)
  [@test] crates/lease-pool/src/lease_test.rs (`the_live_holder_of_a_slot_is_the_session_of_its_lease_while_it_lives`)

#### Scenario: An agent without the feature group
- **WHEN** a run of a group starts on an agent whose hello has no feature `group`
- **THEN** its `Start` has no `group`, the run goes on, and a heavy one says `brd-host: the agent on <alias> predates
  the groups of the heavy queue, so this run of the group <group> waits first come first: `brd-host upgrade <alias>`
  sends this build`
- **AND** an older agent that gets `group` in an estimate ignores it and estimates the end of its queue, where it
  queues the run
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_group_names_it_in_the_start_and_an_agent_before_groups_queues_it_first_come_first`)

#### Scenario: A group that no name may be
- **WHEN** `--group` or `BRD_HOST_GROUP` names a group of other than 1 to 128 letters, digits, `.`, `_` and `-`, the
  first a letter or a digit, such as `a b`
- **THEN** the client refuses with `usage` (exit 2) before it asks the host anything, and an agent refuses such a start
  or estimate with `bad_request`
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_group_names_it_in_the_start_and_an_agent_before_groups_queues_it_first_come_first`)
  [@test] crates/host-link/src/heavy_test.rs (`a_group_is_named_by_the_rule_of_a_host_ref_name`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_heavy_queue_takes_turns_between_the_groups_of_its_runs`)

### Requirement: serve: The heavy queue takes turns between the groups of its runs

A new heavy run MUST join behind the last entry it may not pass: the head, a run of its group, a short run, a run
that the turns passed 8 times, and a run with no more runs of its group before it than its own group has waiting; so
first come first inside a group and exactly first come first with one waiting run per group. The turns MUST keep
none of the short lane's state.

#### Scenario: A group of many runs and a later run of another group
- **GIVEN** a group of many heavy runs from checkouts of their own, such as the agents of one orchestrating session
- **WHEN** a heavy run of another group, or of a checkout without a group, joins the queue later
- **THEN** it waits behind the first waiting run of each group before it, not behind all of them, and the runs of each
  group keep their order
- **AND** the head of the queue alone is admitted, as before
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_queue_takes_turns_between_groups_and_each_group_keeps_its_order`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_heavy_queue_takes_turns_between_the_groups_of_its_runs`)

#### Scenario: Runs without a group
- **WHEN** heavy runs of other checkouts join without `--group`, each checkout with at most one run waiting, also a
  checkout whose run just took the turn and that joins again
- **THEN** each is of the group of its checkout, and they wait first come first exactly as before the groups
- **AND** a second waiting run of one checkout waits behind the first waiting runs of the other checkouts
  [@test] crates/lease-pool/src/host_turn_test.rs (`without_groups_the_runs_of_checkouts_with_one_run_waiting_each_are_first_come_first`)

#### Scenario: How often a run is passed
- **WHEN** runs of other groups join while a run waits, any count of groups, while the head can start or not
- **THEN** a run is passed by the turns at most 8 times, and only by a run of a group with fewer runs waiting than are
  before it in its own group; the first waiting run of a group is passed by none
- **AND** the count is in `groups.json` of the heavy turn, and a run that leaves the queue without the turn, a cancel
  or a start of `admit_now` refused `busy`, gives its passes back
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_is_passed_by_the_turns_at_most_eight_times_whatever_the_count_of_groups`)

#### Scenario: The short lane, the head, the supersede and the starts at once
- **WHEN** a short run, an exclusive head, a run that supersedes or a start of `admit_now` meets the turns of the groups
- **THEN** a run of the turns adds no pass of the short lane (`passes.json`), sets no join of its own from another run
  and counts its load bound from its own join; a short run still joins ahead of the runs it may pass, a backlog that
  the turns passed too, and no run of the turns goes ahead of a short run or of the head
- **AND** a run that supersedes a run of its group takes its place and its passes; one of another group takes its own
  turn, and the passes of the run it replaces go back; an exclusive run, its drain and its pass, and a start of
  `admit_now` keep their rules
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_short_lane_the_head_and_the_supersede_keep_their_rules_beside_the_turns`, `a_run_that_supersedes_a_run_of_another_group_takes_its_own_turn`, `a_short_run_goes_ahead_of_the_runs_that_wait_for_more_and_runs_beside_the_holder`, `a_heavy_run_that_fits_starts_past_an_exclusive_run_whose_drain_ended`)
  [@test] crates/host-agent/tests/serve/admit.rs (`of_two_starts_that_admit_at_once_on_a_free_host_one_runs_and_the_other_is_busy`)

#### Scenario: A table that an older build rewrote
- **WHEN** a supervisor of an older build rewrites `turn.json` without `group`, and `passes.json` without the fields it
  does not know, as builds 1.24 to 1.27 do
- **THEN** no turn changes, as the groups and their passes are in `groups.json`, which no older build reads or writes,
  and only the lines lose the group
- **AND** a run that `groups.json` does not name is of the group of its entry, its `--group`, else its affinity, else
  alone: the queue goes on and no run is lost
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_rewrite_by_an_older_build_changes_no_turn_and_without_the_groups_a_run_is_of_its_checkout`)

#### Scenario: A priority inside a group
- **WHEN** a run of `--priority high` joins after runs of its group that wait with `normal`, or `brd-host prioritize`
  changes the priority of a waiting run
- **THEN** after its turn places it, the runs of its group and its class (short, exclusive or neither) are ordered by
  priority over their own places, first come first within one priority, and the join says `by its priority, goes ahead
  of <n> run(s) of its group: <runs>`
- **AND** the head keeps its place, the runs of other groups keep their turns, a long run never takes the place of a
  short one, no pass of the turns or of the short lane counts, and the run keeps its own join
- **AND** the priority is in `groups.json`; `brd-host runs` says `, priority <level> in its group` of a waiting run whose
  priority is not normal, and an estimate places a run by the same order
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_high_run_goes_ahead_of_the_normal_runs_of_its_group_never_of_another_group_nor_the_head`, `the_priority_never_moves_a_long_run_into_the_place_of_a_short_one`)
  [@test] crates/lease-pool/src/host_estimate_test.rs (`an_estimate_places_a_run_by_its_priority_and_its_commit_as_the_queue_places_it`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_name_a_priority_and_prioritize_says_where_the_run_moved`)

### Requirement: serve: A later heavy run of the same checkout and command supersedes the waiting one

A heavy run new to the queue MUST supersede each waiting run of its repository and affinity with the same command by
what it runs (`host_link::heavy::run_key`), or of another commit with the same key of its measures: it takes the
place of the first, and each ends with `superseded` before its lend. A holder of the turn MUST never be superseded.
The later run MUST say whom it replaced and which other runs of its checkout wait.

#### Scenario: The same checkout and command
- **WHEN** a heavy run waits for the turn, and a later heavy run of the same repository, affinity and command starts
- **THEN** the later run takes the place of the waiting one in the queue, and its log says `replaces run <id>, which
  waited <time> for the heavy turn with the same checkout and command, or of another commit with the same measures: it
  ends with `superseded` and runs nothing, and
  this run takes its place in the queue`
- **AND** the waiting run ends at its next look with `superseded` (exit 75), in the phase `heavy` only, with the message
  `run <id> was superseded by run <later id> of the same checkout and command, or of another commit with the same
  measures, before it started: nothing ran; do not
  run it again, here or on the client; follow <later id>: `brd-host follow <host> <later id>``, which its client and
  `brd-host follow` say with the alias, and `brd-host runs` lists it as `finished, refused superseded`
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_later_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)
  [@test] crates/host-agent/src/turn_test.rs (`a_later_run_says_whom_it_replaces_and_who_else_of_its_checkout_waits`)
  [@test] crates/host-link/src/heavy_test.rs (`the_key_of_a_run_drops_the_wrappers_and_keeps_the_environment`)

#### Scenario: The gate of an amended head
- **WHEN** a heavy run waits, and a later heavy run of the same repository and affinity starts on another commit with
  the same key of its measures (the class of `--measure-as`, else of its command), whatever its command
- **THEN** it supersedes the waiting run as a run of the same command does, and keeps its priority unless it names one
- **AND** a later run of the same commit with another command of the class, such as an A/B of two `--base`, supersedes
  nothing, and a waiting run that names no commit, of an older client, is superseded by the same command only
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_of_another_commit_supersedes_a_waiting_run_of_its_measures_and_keeps_its_priority`)
  [@test] crates/lease-pool/src/host_estimate_test.rs (`an_estimate_places_a_run_by_its_priority_and_its_commit_as_the_queue_places_it`)

#### Scenario: Another command or another checkout
- **WHEN** a later heavy run starts with another command of the same checkout and commit, or the same command of
  another checkout
- **THEN** both wait, each in its place
- **AND** a later run of the same checkout with another command says `run <id> of this checkout also waits for the heavy
  turn, <time>: <command>. Both run, one after the other; if this run replaces it, cancel it: `brd-host cancel <host>
  <id>``
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_later_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`, `another_command_another_checkout_and_the_holder_are_never_superseded`)
  [@test] crates/host-agent/src/turn_test.rs (`a_later_run_says_whom_it_replaces_and_who_else_of_its_checkout_waits`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)

#### Scenario: A run that started
- **WHEN** a later heavy run of the same checkout and command starts while the run that holds the turn runs
- **THEN** the holder runs on, and the later run waits for it; an entry without an affinity and a key, of an older
  build, is never superseded
  [@test] crates/lease-pool/src/host_turn_test.rs (`another_command_another_checkout_and_the_holder_are_never_superseded`)
