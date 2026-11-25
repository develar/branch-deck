## ADDED Requirements

### Requirement: serve: A host without a slot for the repository refuses its runs at once

A host whose slot limit for the repository is 0, the smaller of `branchdeck.hostSlots` and the limit of the last
`brd-host slots --local --shrink`, MUST refuse the start of a run with `no_slots` before the run exists, and a run that
finds it 0 before its heavy turn or while it waits for a slot MUST end with `no_slots`, so it never holds cores of the
heavy turn for a slot that never comes.

#### Scenario: A slot limit of 0
- **WHEN** a run starts on a host whose `branchdeck.hostSlots` of the repository is 0, or whose last shrink kept 0
- **THEN** the agent refuses the start with `no_slots` (exit 75): `the host keeps no slot for the runs of <repo>:
  <why>; a run there would wait for a slot forever, so nothing ran`, `<why>` each limit at 0: ``branchdeck.hostSlots`
  is 0 (`git -C <repo> config branchdeck.hostSlots 4` on the host raises it)`` and ``the last shrink kept 0 slots
  (`brd-host slots --local -C <repo> --shrink 4` on the host raises it)``, joined by `, and `
- **AND** no run directory is made, and the run takes no heavy turn
- **AND** a supervisor that finds the limit 0 before its heavy turn, or while it waits for a slot of a host that has
  none, ends the run with the same refusal, not with a wait until `slot_timeout`
  [@test] crates/host-agent/tests/serve/lend.rs (`a_host_without_a_slot_for_the_repository_refuses_the_run_at_once_with_no_slots`)
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_slot_limit_of_0_names_each_limit_and_how_to_raise_it`)

## MODIFIED Requirements

### Requirement: serve: An estimate says when a run would start there

`Request::Estimate` (feature `estimate`) MUST answer `has_commit`, `warm`, `cores`, `expected_cores`, `expected_wall_ms`,
`start_in_ms`, `unknown_ahead`, `os` and `slots`, read without a lock. A heavy run MUST take its place in the queue as a new run
would, and the holders and the runs before it MUST end after the walls of their commands, admitted by the rule of the
turn, the load outside the heavy runs the most of the last 30 s.

#### Scenario: The shapes of an estimate
- **WHEN** a client asks an agent for an estimate
- **THEN** the request and the reply are `{"request": "estimate", "repo", "affinity", "commit", "command", "fetch",
  "heavy", "measure_as"}` and `{"reply": "estimate", …}`, `measure_as` left out without a class (feature
  `measure_as`) and the fields of a measure without one; the measures of a class count for a run of it
- **AND** `slots` is the slot limit of the repository, the smaller of `branchdeck.hostSlots` and the limit of the last
  shrink, left out by an agent before it and when the limit cannot be read
  [@test] crates/host-agent/tests/serve/lend.rs (`a_host_without_a_slot_for_the_repository_refuses_the_run_at_once_with_no_slots`)
  [@test] crates/host-link/src/proto_test.rs (`an_estimate_and_its_reply_have_a_tagged_json_shape`, `the_class_of_the_measures_is_an_optional_field_of_a_start_and_an_estimate`)

#### Scenario: The queue played forward
- **WHEN** a heavy run would wait for holders and for runs of the queue
- **THEN** its start is when the runs before it end by the walls of their commands, the ones that fit starting together
- **AND** a run of an unknown wall, and an exclusive run that a busy host keeps out, count in `unknown_ahead`, and a run
  of the whole host on a busy free turn waits for the bound of 15 min
  [@test] crates/lease-pool/src/host_estimate_test.rs (`an_estimate_plays_the_queue_forward_by_the_walls_of_the_commands`, `an_exclusive_run_ahead_on_a_busy_host_counts_apart`)

#### Scenario: A short run in the estimate
- **WHEN** the run is short by the walls of its passed runs and its measure
- **THEN** the estimate places it ahead of the long runs that it may pass, by the place function of the queue
- **AND** the wall of a command is the one of its passed runs, else of its measures, the failed and cancelled ones too
  [@test] crates/lease-pool/src/host_estimate_test.rs (`a_short_run_is_placed_ahead_of_the_long_runs_as_the_queue_places_it`)
  [@test] crates/lease-pool/src/host_cores_test.rs (`the_wall_of_a_command_is_of_its_last_passed_runs_over_three_at_least`)

### Requirement: run: A heavy run goes to a member only with its measure there

The first member of a set MUST be eligible for every run, but a member whose estimate tells a slot limit of 0, the
first one too. For a heavy run, another member MUST be eligible only with its
own measure of the command, or of its class with `--measure-as`; for a run that is not heavy, every member that
answered. A member whose agent lacks the feature `measure_as` MUST have no estimate of a run of a class.

#### Scenario: A new heavy command
- **WHEN** a heavy run goes to a set whose other members have no measure of its command
- **THEN** it runs on the first member, and the line of the plan says that a heavy run does not go to the others
- **AND** an exclusive run to a set refuses with `usage` (2)
  [@test] crates/host-client/src/choose_test.rs (`a_heavy_run_without_a_measure_stays_on_the_first_member_and_an_exclusive_one_refuses`)

#### Scenario: A run of a class to a set
- **WHEN** a run of `--measure-as <class>` goes to a set
- **THEN** each member is asked by the class, and its measure of the class decides
- **AND** a member whose agent lacks the feature `measure_as` gets no request and has no estimate: `no estimate: its
  agent predates --measure-as, so it knows no measure of the class (`brd-host upgrade <alias>`)`
  [@test] crates/host-client/src/choose_test.rs (`a_run_of_a_class_asks_by_its_class_and_a_member_before_classes_has_no_estimate`)

#### Scenario: A member without a slot for the repository
- **WHEN** a member of a set tells `slots` 0 in its estimate, as a laptop of `branchdeck.hostSlots 0`
- **THEN** no run goes there, the first member of the set neither, and the line of the plan says `<alias>: it keeps no
  slot for the runs of this repository (`branchdeck.hostSlots` or the limit of `brd-host slots --local --shrink` is 0
  there), so no run goes there`; a set whose every member is so refuses with `no_host` before anything ran
- **AND** a member of an agent before `slots` ranks as before
  [@test] crates/host-client/src/choose_test.rs (`a_member_without_a_slot_for_the_repository_takes_no_run_the_first_neither`)

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
| `no_host` | 75 | No host file and no set of hosts for the name. The message names `brd-host setup <alias>` and `brd-host set <name> <alias>…`. |
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
| `no_slots` | 75 | The host keeps no slot for the runs of the repository: `branchdeck.hostSlots` of its git config is 0, or the last `brd-host slots --local --shrink` kept 0, so a run would wait for a slot forever. The message names each and how to raise it. |
| `heavy_timeout` | 75 | The heavy run got no heavy turn of the host within 2 hours: other heavy runs held it. Run it there again later, or through a set of hosts whose other members can take it; never as a plain command where the client runs. |
| `superseded` | 75 | A later heavy run of the same checkout and command came while this one waited for the heavy turn: nothing ran. Never run it again, here or there: the message names the later run and its `brd-host follow`. |
| `busy` | 75 | A member of a set that takes runs only at once (`idle_only`) had no room for the run now: nothing ran, and the client goes to the next member. |
| `still_running` | 124 | The run goes on past `--max` of `wait` or `run --max`, or another brd-host of the checkout finishes it; nothing was cancelled. The next `brd-host wait <alias> <run>` waits again. |
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
