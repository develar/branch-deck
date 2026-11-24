## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: The link: The runs of one run are a field that an older agent ignores

`Request::Runs` MUST take an optional `run`, which an agent of the feature `estimate` MUST answer with that run alone. An
older agent ignores the field and answers every run, so a client MUST find its run in the list either way.

#### Scenario: The runs of one run
- **WHEN** a wait asks for the state of its run
- **THEN** it sends `{"request": "runs", "run": "<run>"}`, which an agent of the build before reads as the request of
  every run
- **AND** the request of every run keeps its shape, `{"request": "runs"}`
  [@test] crates/host-link/src/proto_test.rs (`the_runs_of_one_run_are_an_optional_field_that_an_older_agent_ignores`)

### Requirement: serve: An estimate says when a run would start there

`Request::Estimate` (feature `estimate`) MUST answer `has_commit`, `warm`, `cores`, `expected_cores`, `expected_wall_ms`,
`start_in_ms`, `unknown_ahead` and `os`, read without a lock. A heavy run MUST take its place in the queue as a new run
would, and the holders and the runs before it MUST end after the walls of their commands, admitted by the rule of the
turn, the load outside the heavy runs the most of the last 30 s.

#### Scenario: The shapes of an estimate
- **WHEN** a client asks an agent for an estimate
- **THEN** the request and the reply are `{"request": "estimate", "repo", "affinity", "commit", "command", "fetch",
  "heavy"}` and `{"reply": "estimate", …}`, the fields of a measure left out without one
  [@test] crates/host-link/src/proto_test.rs (`an_estimate_and_its_reply_have_a_tagged_json_shape`)

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

### Requirement: serve: A start that admits at once or refuses with busy

The supervisor of a run of `RunSpec::admit_now` MUST take the heavy turn in one look under the lock of the table, and
a run that is not heavy MUST find no measurement; else the run MUST end with `busy` before its lend, its ticket out of the
queue at once. The start MUST answer once the supervisor decided, up to 30 s, so `busy` is its reply.

#### Scenario: Two starts that admit at once on a free host
- **WHEN** two starts of `admit_now` of runs of the whole host come at once
- **THEN** one gets `started` and runs, and the other `busy` (exit 75), with the phase `heavy` only and nothing run
- **AND** once the first gave the turn back, the next start of `admit_now` runs
  [@test] crates/host-agent/tests/serve/admit.rs (`of_two_starts_that_admit_at_once_on_a_free_host_one_runs_and_the_other_is_busy`)
  [@test] crates/host-agent/src/admit_test.rs (`a_start_that_admits_at_once_answers_the_decision_of_its_supervisor`)

### Requirement: serve: branchdeck.hostJobs gives each command its jobs

With `branchdeck.hostJobs` in the git config of the host, the agent MUST set `CARGO_BUILD_JOBS`, `NEXTEST_TEST_THREADS`
and `BRD_HOST_JOBS` of each prepare step and command to it, before `[host.env]`, which MUST still decide.

#### Scenario: The jobs of a host
- **WHEN** the host has `branchdeck.hostJobs` 6 and `[host.env]` sets `NEXTEST_TEST_THREADS` to 2
- **THEN** the command gets `CARGO_BUILD_JOBS=6`, `NEXTEST_TEST_THREADS=2` and `BRD_HOST_JOBS=6`
- **AND** a host without the key sets none of them
  [@test] crates/host-agent/src/run_env_test.rs (`the_jobs_of_the_host_go_to_the_command_under_the_config_of_the_commit`, `the_variables_of_the_run_and_of_the_config_go_on_top_of_the_base`)
