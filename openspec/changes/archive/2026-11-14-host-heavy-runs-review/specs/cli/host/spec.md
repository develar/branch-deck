## MODIFIED Requirements

### Requirement: run: A heavy run is heavy by --heavy or by heavy of [host] of its commit

A run MUST be heavy with `--heavy`, else when a pattern of `heavy` of `[host]` of the shipped commit matches its
command, and its `Start` MUST carry `heavy`. Its line MUST say why it is heavy. An agent without the feature `heavy`
MUST get no run of a commit whose `[host]` has `heavy`: the client refuses before the start.

#### Scenario: A heavy run
- **WHEN** a run starts with `--heavy`, or the first pattern of `heavy` that matches its command is `<pattern>`
- **THEN** its `Start` has `"heavy": true`, and after the line of the run comes `brd-host: a heavy run by --heavy: on
  <alias> it waits until no other heavy run goes`, or `by `<pattern>` of [host]`
- **AND** a run whose command no pattern matches and that has no `--heavy` is no heavy run, and its `Start` has no
  `heavy`
  [@test] crates/host-client/src/run_test.rs (`a_run_is_heavy_by_a_pattern_of_its_commit_or_by_heavy_and_says_why`)
  [@test] crates/bd-host/src/cli_test.rs (`the_command_line_of_bt_gives_niceness_0_and_no_outputs`)
  [@test] crates/host-link/src/proto_test.rs (`a_heavy_run_and_the_heavy_turn_are_optional_fields_within_the_schema`)

#### Scenario: An agent without the feature heavy
- **WHEN** a `--heavy` run of a commit without `heavy` in `[host]` starts on an agent whose hello has no feature `heavy`
- **THEN** the client says `brd-host: the agent on <alias> predates heavy runs, so this one runs beside the others:
  `brd-host upgrade <alias>` sends this build`, and the run goes on as one that is not heavy
  [@test] crates/host-client/src/run_test.rs (`a_run_is_heavy_by_a_pattern_of_its_commit_or_by_heavy_and_says_why`)

#### Scenario: A commit with heavy and an agent without the feature
- **WHEN** a run, heavy or not, of a commit whose `[host]` has `heavy`, even empty, starts on an agent whose hello has no
  feature `heavy`, which would refuse every run of that commit
- **THEN** the client refuses before `Start` with `bad_config` (exit 75): `the agent on <alias> predates `heavy` of
  [host] of .branchdeck.toml, so it refuses every run of this commit: `brd-host upgrade <alias>` sends this build`
  [@test] crates/host-client/src/run_test.rs (`a_commit_with_heavy_on_an_agent_before_heavy_runs_refuses_before_the_start`)
  [@test] crates/host-link/src/config_test.rs (`the_known_keys_are_the_fields_of_serde_and_heavy_needs_its_feature`)

### Requirement: run: Ctrl+C after the start cancels the run and follows it

Ctrl+C after `Start` MUST send `Cancel` and stop the pack, and the output follows to the end of the run; SIGTERM and
SIGHUP MUST count as Ctrl+C. A `Cancel` whose connection fails MUST connect again for up to 120 s.

#### Scenario: Ctrl+C after Start
- **WHEN** the user presses Ctrl+C after `Start`
- **THEN** the client sends `Cancel` and stops the pack
- **AND** the output follows to the end of the run
  [@test] crates/host-client/src/run_test.rs (`ctrl_c_cancels_the_run_and_follows_it_to_its_end`)

#### Scenario: A failed Cancel
- **WHEN** the connection of a `Cancel` fails
- **THEN** it connects again for up to 120 s
  [@test] crates/host-client/src/run_test.rs (`a_failed_cancel_connects_again`)

#### Scenario: SIGTERM after Start
- **WHEN** a tool or a closed terminal sends the client SIGTERM or SIGHUP after `Start`
- **THEN** the client sends `Cancel` as for Ctrl+C, says `brd-host: Ctrl+C or SIGTERM: cancelling the run …`, and
  follows the output to the end of the run
  [@test] crates/bd-host/tests/cli.rs (`sigterm_to_the_client_cancels_the_run_on_the_host`)

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
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared`, `heavy/heavy.json` and `heavy/heavy.lock` (the heavy turn of the host, in a directory of its own) |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply` |

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
  [@test] crates/lease-pool/src/host_turn_test.rs (`heavy_runs_take_the_turn_one_at_a_time_first_come_first`, `a_waiting_run_takes_the_turn_once_the_holder_leaves`, `a_holder_holds_the_turn_from_the_moment_it_took_it`)

#### Scenario: The line of the wait
- **WHEN** a heavy run waits for the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for the heavy turn of the host, <time>
  left: run <id> of <repository> holds it <time> (<command>), <n> runs ahead: <id> of <repository> (<command>), …`
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

### Requirement: The environment of a run and `[host]`: A [host] table that does not parse ends the run

An unknown key or a value of the wrong shape in `[host]` MUST end the run with `bad_config` before the command; the
client reads `[host]` of every run, so it refuses before the start. The message of an unknown key MUST name it, the
keys of this build, and that a later Branch Deck may know it with the updates of both ends.

#### Scenario: An unknown key or a bad value
- **WHEN** the `[host]` table of the commit has an unknown key or a value of the wrong shape
- **THEN** the run ends with `bad_config` before the command
  [@test] crates/host-link/src/config_test.rs (`the_host_table_of_branchdeck_toml_parses_and_an_unknown_key_refuses`, `a_typo_or_a_bad_value_is_an_error_that_names_the_file`)
  [@test] crates/host-client/src/run_test.rs (`a_config_that_does_not_parse_starts_no_run`)

#### Scenario: A key of a later Branch Deck
- **WHEN** `[host]`, a table of `[host.env]` or a step of `[[host.prepare]]` has a key that this build does not know
- **THEN** the message is `.branchdeck.toml [host]: unknown key `<key>`: a typo, or a key of a later Branch Deck than
  this brd-host, which knows <its keys>; `brd install` updates brd-host on this Mac, and `brd-host upgrade <alias>` the
  agent of a host`
  [@test] crates/host-link/src/config_test.rs (`an_unknown_key_names_itself_the_keys_of_this_build_and_the_update_of_brd_host`, `the_known_keys_are_the_fields_of_serde_and_heavy_needs_its_feature`)
