## ADDED Requirements

### Requirement: serve: A later heavy run of the same checkout and command supersedes the waiting one

A heavy run new to the queue MUST supersede each waiting run of its repository with the same affinity and the same
command by what it runs (`host_link::heavy::run_key`: without `nice`, `timeout`, `time`, `nohup`, `exec` or `command`
before it, with its environment): it takes the place of the first, and each ends with `superseded` before its lend.
The holder of the turn MUST never be superseded. The later run MUST say whom it replaced and which other runs of its
checkout wait.

#### Scenario: The same checkout and command
- **WHEN** a heavy run waits for the turn, and a later heavy run of the same repository, affinity and command starts
- **THEN** the later run takes the place of the waiting one in the queue, and its log says `replaces run <id>, which
  waited <time> for the heavy turn with the same checkout and command: it ends with `superseded` and runs nothing, and
  this run takes its place in the queue`
- **AND** the waiting run ends at its next look with `superseded` (exit 75), in the phase `heavy` only, with the message
  `run <id> was superseded by run <later id> of the same checkout and command before it started: nothing ran; do not
  run it again, here or on the client; follow <later id>: `brd-host follow <host> <later id>``, which its client and
  `brd-host follow` say with the alias, and `brd-host runs` lists it as `finished, refused superseded`
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_later_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)
  [@test] crates/host-agent/src/turn_test.rs (`a_later_run_says_whom_it_replaces_and_who_else_of_its_checkout_waits`)
  [@test] crates/host-link/src/heavy_test.rs (`the_key_of_a_run_drops_the_wrappers_and_keeps_the_environment`)

#### Scenario: Another command or another checkout
- **WHEN** a later heavy run starts with another command of the same checkout, or the same command of another checkout
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

## MODIFIED Requirements

### Requirement: The link: A refusal of the agent carries a code of the table

A refusal of the agent MUST be `{"reply": "refused", "code", "message"}` with a code of the table "Refusal codes". A
client that does not know the code MUST take it as `agent_error`, and the message names the code, so a code of a newer
build is no protocol error. A message MUST name the host of the agent as `<host>`, and the client MUST put its alias
there.

#### Scenario: A refusal of a known code
- **WHEN** the agent refuses a request
- **THEN** its reply is `{"reply": "refused", "code", "message"}` with a code of the table
  [@test] crates/host-link/src/refusal_test.rs (`a_refusal_prints_its_code_and_an_unknown_code_of_an_agent_is_an_agent_error`)

#### Scenario: A code of a newer build
- **WHEN** the reply names a code that the client does not know
- **THEN** the client takes it as `agent_error`, and the message names the code
  [@test] crates/host-link/src/refusal_test.rs (`a_refusal_prints_its_code_and_an_unknown_code_of_an_agent_is_an_agent_error`)

#### Scenario: A message that names the host
- **WHEN** a message of the agent names its own host as `<host>`, as in `brd-host follow <host> <run>`, which the agent
  cannot know the alias of
- **THEN** the client says it with its alias of the host: `brd-host follow studio <run>`
  [@test] crates/host-link/src/refusal_test.rs (`a_message_of_the_agent_gets_the_alias_of_the_client_for_its_host`)

### Requirement: follow, cancel, fetch, forget, runs: runs lists the runs that the agent keeps

`brd-host runs <alias> [--json]` MUST list the runs that the agent keeps, the finished and forgotten ones too, each
with its state, its start, its affinity, commit, command and the steps of 0.1 s or more, and for a finished run its
exit and how long it took, or the refusal that ended it, then the heavy turn of the host. An agent without the feature
`heavy` has no heavy turn to list.

#### Scenario: The runs of a host
- **WHEN** `brd-host runs <alias>` runs while a run goes on
- **THEN** it lists that run as `<run>  running, started <YYYY-MM-DD HH:MM:SS>` with its affinity, commit, command and
  the steps of 0.1 s or more
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-client/src/phases_test.rs (`a_run_line_says_its_state_its_start_and_how_long_a_finished_run_took`)

#### Scenario: The finished runs of a host
- **WHEN** `brd-host runs <alias>` runs after a run ended
- **THEN** it lists the run as `<run>  finished, exit <code>, started <YYYY-MM-DD HH:MM:SS>, took <duration>`, the
  start in the time zone of the client, with `, its files kept` while nobody forgot it, and without `took` from an
  agent that sends no end
- **AND** `--json` has the run as `RunInfo` with `ended_ms`, and `forgotten` once its client forgot it; a run that lost
  its supervisor has no `ended_ms` and no `took`, as the agent saw the loss later
  [@test] crates/host-agent/src/run_test.rs (`a_lost_run_has_no_time_of_its_end`)
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-client/src/phases_test.rs (`a_run_line_says_its_state_its_start_and_how_long_a_finished_run_took`)
  [@test] crates/host-link/src/proto_test.rs (`the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema`)

#### Scenario: A run that a refusal ended
- **WHEN** `brd-host runs <alias>` runs after a refusal of the agent ended a run, such as `superseded`
- **THEN** it lists the run as `<run>  finished, refused <code>, started …`, and on the next line `    <code>: <message>`
  with the alias of the host for `<host>`
- **AND** `--json` has the refusal as `refused` of `RunInfo`: `{"code", "message"}`, the code by its name; an older
  agent sends none
  [@test] crates/host-client/src/phases_test.rs (`a_refused_run_says_its_code_and_its_message_with_the_alias`)
  [@test] crates/host-link/src/proto_test.rs (`the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)

#### Scenario: The heavy turn of a host
- **WHEN** `brd-host runs <alias>` runs while a heavy run holds the heavy turn and another waits for it
- **THEN** after the runs it says `heavy turn: run <id> of <repository> holds it <time>: <command>`, then
  `  run <id> of <repository> waits <time>: <command>` for each run that waits, and `heavy turn: free` when no run holds
  it
- **AND** `--json` has the turn as `heavy`: the `holder` and the `queue`, without the runs whose process is gone
  [@test] crates/bd-host/tests/cli.rs (`runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long`, `follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_of_the_heavy_turn_name_the_holder_and_each_waiting_run_with_their_time`)

### Requirement: hello: A hello of another build runs with a hint

A hello of the same schema and another build MUST let the run go on, with one line that names
`brd-host upgrade <alias>`, says that runs work across builds, and that the upgrade waits for the user, so an agent
that reads only the output neither stops nor upgrades on its own.

#### Scenario: An agent of another build
- **WHEN** the agent answers the hello with the same schema and another build
- **THEN** the run goes on, with the line `brd-host: <alias> runs another build of brd-host (<version> <build>, this
  client <build>); runs work across builds, so this is no error: `brd-host upgrade <alias>` sends this one, only after
  the user agrees`
  [@test] crates/host-client/src/host_test.rs (`another_schema_refuses_and_another_build_runs_with_a_hint`)

### Requirement: slots: slots asks the agent for its lend tables

`brd-host slots <alias>` MUST ask the agent for the lend table of each repository that it served since its first
`Start` (`Request::Slots`): each slot, its state, its run, its affinity, its last use, and the queue. It MUST write
nothing. It MUST mark `(abandoned)` only the slots that the agent named so: the pid of a lend is a process of the host,
which the client cannot look up.

#### Scenario: The slots of a host
- **WHEN** `brd-host slots <alias>` runs
- **THEN** it shows, for each repository that the agent served since its first `Start`, each slot with its state, its
  run, its affinity and its last use, and the queue
- **AND** nothing is written
  [@test] crates/host-link/src/proto_test.rs (`the_slots_of_the_agent_carry_the_lend_table_as_the_pool_writes_it`)

#### Scenario: A lent slot of a live run on the host
- **WHEN** a run of the host holds a slot, and its supervisor is no process of the client
- **THEN** `brd-host slots <alias>` shows the slot lent without `(abandoned)`, and marks a slot `(abandoned)` only when
  the agent named it; an agent of an older build names none, and no slot gets the mark
  [@test] crates/host-agent/src/table_test.rs (`the_lines_mark_the_slots_that_the_machine_of_the_table_found_abandoned`)
  [@test] crates/host-agent/tests/serve.rs (`two_runs_of_one_affinity_run_in_parallel_and_a_third_waits_for_a_slot`)

### Requirement: slots: The reply carries each table as the pool writes it

The reply MUST carry each table as the JSON of its `host-slots.json`, so the protocol keeps no second shape of the
table, and beside it `abandoned`: the numbers of the slots whose run or step is gone by the processes of the host
(`lease_pool::host_slots::is_abandoned`), as the agent judged them there. The field MUST be optional, so the reply of
an older agent parses and marks no slot.

#### Scenario: The reply of Request::Slots
- **WHEN** the agent answers `Request::Slots`
- **THEN** each table is the JSON of its `host-slots.json`, with `abandoned` when the agent judged a slot so
- **AND** a reply without `abandoned` parses with none
  [@test] crates/host-link/src/proto_test.rs (`the_slots_of_the_agent_carry_the_lend_table_as_the_pool_writes_it`, `the_abandoned_slots_come_from_the_agent_and_an_older_agent_names_none`)

### Requirement: slots: --local reads the table without the agent

`--local` MUST read the table of the repository of `-C` on this machine, without the agent, and judge the abandoned
slots by the processes of this machine, whose table it is.

#### Scenario: The table of this machine
- **WHEN** `brd-host slots --local -C <repo>` runs
- **THEN** it shows the lend table of that repository on this machine, without the agent, with `(abandoned)` for a slot
  whose run is gone here
  [@test] crates/bd-host/tests/cli.rs (`slots_shows_the_lend_table_and_shrinks_it_without_the_agent`)
  [@test] crates/host-agent/src/table_test.rs (`the_lines_of_a_table_name_the_limit_the_queue_and_each_slot`, `the_lines_mark_the_slots_that_the_machine_of_the_table_found_abandoned`)

### Requirement: serve: Heavy runs take the heavy turn of the host one at a time

Before its lend, a heavy run MUST take the heavy turn of the host (`lease_pool::host_turn`), one table of its schema
for every repository, in the phase `heavy`, and wait for it up to 2 h first come first, with one watch and its line at
once and once a minute; it MUST give it back after its give-back. A run that is not heavy MUST take no turn.

#### Scenario: Two heavy runs and one that is not heavy
- **WHEN** a heavy run holds the turn, a second heavy run starts, and a run that is not heavy starts
- **THEN** the second waits for the turn without a slot, and its command starts only after the first gave the turn back
- **AND** the run that is not heavy runs beside the first at once
- **AND** the first gives the turn back after the give-back of its slot
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`heavy_runs_take_the_turn_one_at_a_time_first_come_first`, `a_waiting_run_takes_the_turn_once_the_holder_leaves`, `a_wait_without_the_event_takes_the_turn_at_its_next_look`, `a_wait_whose_time_is_up_answers_its_place_and_keeps_its_ticket`, `a_holder_holds_the_turn_from_the_moment_it_took_it`)

#### Scenario: The line of the wait
- **WHEN** a heavy run waits for the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for the heavy turn of the host, <time>
  left: run <id> of <repository> holds it <time> (<command>), <n> runs ahead: <id> of <repository> (<command>), …; a
  wait, not a hang: do not cancel it and do not run it on the client; `brd-host runs <host>` shows the heavy turn`
- **AND** after the wait it says `took the heavy turn of the host after <n> s`
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_turn_line_names_the_wait_the_time_left_the_holder_and_the_runs_ahead`)
  [@test] crates/host-agent/src/turn_test.rs (`a_heavy_run_says_its_wait_at_once_and_a_stop_ends_it`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_command_of_an_entry_is_one_line_of_at_most_200_characters`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_line_of_a_heavy_wait_comes_again_with_the_time_waited_and_the_time_left`)

#### Scenario: A cancel in the heavy queue
- **WHEN** a heavy run that waits for the turn gets a cancel
- **THEN** its wait ends within 2 s with the exit code 130 before its lend, and its ticket leaves the queue
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_cancel_in_the_heavy_queue_ends_the_run_before_its_lend_and_its_ticket_leaves`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_that_leaves_the_queue_waits_no_more`)

#### Scenario: A killed heavy run
- **WHEN** the supervisor of the heavy run that holds the turn is gone, or the process of a ticket ahead
- **THEN** the next heavy run takes the turn at its next look, within 5 s, without a keeper
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_killed_heavy_run_frees_the_turn_for_the_next_one`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_gone_holder_frees_the_turn_and_a_gone_ticket_leaves_the_queue`)

#### Scenario: No turn within 2 h
- **WHEN** a heavy run gets no turn within 2 h
- **THEN** it ends with `heavy_timeout`, the line of its wait and the advice to run it on the host again later, not on
  the client, and its ticket leaves the queue
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_heavy_run_that_waits_past_its_time_ends_with_heavy_timeout`)

#### Scenario: A table of another schema
- **WHEN** `heavy.json` has a schema other than the one of this build, or none, as an agent of another build wrote it
- **THEN** each take, leave and read of the turn refuses, and the run ends with `agent_error`, never with a free turn
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_table_of_another_schema_refuses`)

### Requirement: serve: A relief that left the volume low makes the next one wait

A relief that leaves a volume below the minimum after every step MUST say so, name what holds the space and what a
person can do, and the next one waits 15 minutes; without a minimum it deletes nothing. So a host that only runs keeps
its disk without the app and without `brd`.

#### Scenario: A volume below the minimum after every step
- **WHEN** a relief leaves a volume below the minimum after every step
- **THEN** it says so, and the next relief waits 15 minutes
- **AND** the line goes on with `the pools that hold the most: <repository> <size> in <n> slot(s) (largest: <slot>
  <size>, …); …`, the three largest pools and the three largest slots of each by `disk.json`, without a walk, then
  `The relief deleted all it may: the rest is held by leases, the host slots of runs among them. A person frees more by
  returning the leases that `brd lease status` names on the host, by `brd-host trim <host> --max <n>`, which deletes the
  free slots past n, or by moving a pool to a larger volume: `git -C <repository> config branchdeck.leasePool <dir>` on
  the host, whose new slots go there`
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_keeps_the_minimum_free_space_once_per_volume_and_waits_after_a_relief_that_left_it_low`)
  [@test] crates/host-agent/src/keeper_test.rs (`the_line_of_a_volume_still_low_names_the_largest_pools_and_slots_and_what_a_person_can_do`)

#### Scenario: A host without a minimum
- **WHEN** the host has no `branchdeck.leaseMinFree`
- **THEN** the relief deletes nothing

### Requirement: Refusal codes: Each code has one name and one exit

Each code MUST be one variant of the refusal type of `host-link`, with its name and exit in one table that
`brd-host schema` prints, so a code cannot have two exits.

#### Scenario: The table of the codes
- **WHEN** brd-host refuses with a code
- **THEN** the code has the exit and the words of its row of the table
  [@test] crates/host-link/src/refusal_test.rs (`every_refusal_code_has_one_name_and_one_exit`)

| Code | Exit | When |
|---|---|---|
| `usage` | 2 | An alias that is not `[0-9A-Za-z._-]+`, a run without a command, a working directory in no git checkout, a descriptor of `--report-fd` that is not open, a fetch without a record and without `--fetch`. A command line that clap refuses exits 2 too. |
| `interrupted` | 130 | Ctrl+C before the start, or a second Ctrl+C during the follow. |
| `config` | 75 | No home directory, or an identity, a record or a file of brd-host that cannot be read or written. |
| `no_host` | 75 | No host file for the alias. The message names `brd-host setup <alias>`. |
| `bad_host_file` | 75 | A host file that is not JSON of its shape, or that names another alias. |
| `unreachable` | 75 | No address answers. The message names each address and the lookup, and Local Network privacy when the relay cannot pass it. |
| `untrusted` | 75 | The agent shows another certificate than the pin, or it refuses the certificate of the client. |
| `schema` | 75 | The agent speaks another schema of host-link. |
| `timed_out` | 75, 255 after the start | No reply within the deadline of the request; for a follow, 10 min over all reconnects. |
| `link` | 75, 255 after the start | The connection closed, or a reply came that the protocol does not allow there. |
| `local_changes` | 75 | `--head` with tracked or untracked changes. |
| `behind` | 75 | The host has none of the candidate commits. |
| `git_failed` | 75 | A git call of the client failed: the snapshot, the candidates or the pack. The message has the words of git. |
| `bad_request` | 75, 255 after the start | A request that the agent does not accept, such as a fetch of a run that goes on. |
| `bad_run_id` | 75 | A run id that cannot name a directory. |
| `run_exists` | 75 | A start with the id of a kept run. |
| `unknown_run` | 255 | No kept run has the id. After a start: the run is gone, and whether its command ran is unknown. |
| `repo_missing` | 75 | The repository path is no git repository on the host. |
| `pack_failed` | 75 | The agent cannot land the pack: no pack, or git refused it. |
| `pack_missing` | 75 | The pack of the commit did not come within 30 min, or sent no chunk for 2 min. |
| `slot_timeout` | 75 | The run got no host slot within an hour. |
| `heavy_timeout` | 75 | The heavy run got no heavy turn of the host within 2 hours: other heavy runs held it. Run it there again later, not where the client runs. |
| `superseded` | 75 | A later heavy run of the same checkout and command came while this one waited for the heavy turn: nothing ran. Never run it again, here or there: the message names the later run and its `brd-host follow`. |
| `lease_failed` | 75 | The lend failed. The message has the error of the pool. |
| `old_git` | 75 | The git of the host is below the floor of Branch Deck, and Homebrew cannot upgrade it. The message is the hint of Branch Deck. |
| `checkout_failed` | 75 | The commit cannot be checked out in the slot. |
| `bad_config` | 75 | `[host]` of `.branchdeck.toml` of the commit does not parse, has an unknown key or a value of the wrong shape. |
| `prepare_failed` | 75 | A prepare step exited with another code than 0. |
| `spawn_failed` | 75 | The command cannot start. |
| `agent_error` | 75, 255 after the command started | The agent cannot read or write its own files, or a supervisor got lost. |
| `fetch_failed` | 255 | The fetch did not copy every file. The run stays on the host for `brd-host fetch` and `brd-host forget`. |
| `apply_failed` | 255 | The changes do not apply to the working tree, or they did not come because the fetch failed. |
| `bad_binary` | 255 | The binary of an upgrade has another size or SHA-256, or `version --json` does not answer as an agent of this schema with `upgrade`. |
| `downgrade` | 255 | The binary of an upgrade has a lower version, and the request has no `force`. |
| `plist_changed` | 255 | The new binary would write another LaunchAgent than the one on the host: `brd-host setup` installs it. |
| `upgrade_failed` | 255 | The agent cannot stage, keep or swap its binary, or start the watchdog, or another upgrade runs. |
| `rolled_back` | 255 | The new agent did not listen in time, and the previous binary answers. |
| `agent_failed` | 255 | The binary for a host cannot be read or is of another platform, the host is not macOS for `upgrade`, or `setup --agent` on the host failed. |
| `ssh_failed` | 255 | An ssh call of `setup <alias>` failed or ran past its deadline. |
| `launchd_failed` | 255 | A `launchctl` call of `setup` failed. |
| `skill_edited` | 255 | `skill install` found a copy with local edits, without `--force`. |

#### Scenario: The exit after the start
- **WHEN** a code whose row has two exits refuses
- **THEN** its exit is the one of how far the run came
  [@test] crates/host-link/src/refusal_test.rs (`the_exit_of_a_code_follows_how_far_the_run_came`)
