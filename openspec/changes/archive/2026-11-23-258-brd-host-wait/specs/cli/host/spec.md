## ADDED Requirements

### Requirement: wait: The command line of wait

`brd-host wait <alias> <run> [--max <duration>]` MUST wait for the end of a run of the host for at most `--max`, 9 min
by default, under the 10 min after which the tool of an agent ends a call. A duration MUST be seconds (`90`) or numbers
with `h`, `m` and `s` in this order (`90s`, `9m`, `1h30m`), from 1 s to 24 h; any other refuses with `usage` (2). The
wait MUST run in a directory of a git checkout: the checkout of the run, whose record says which files come back.
stdout MUST stay empty: every line goes to the stderr.

#### Scenario: The synopsis of wait
- **WHEN** a caller runs `brd-host wait`
- **THEN** it takes this command line, and `--max` takes `90`, `90s`, `9m`, `1h` or `1h30m`, from 1 s to 24 h
  [@test] crates/bd-host/src/cli_test.rs (`max_takes_seconds_minutes_and_hours_and_refuses_the_rest`)

```sh
brd-host wait <alias> <run> [--max <duration>]
```

### Requirement: wait: wait says each state of the run as it changes

A wait MUST ask the agent for its runs (`Request::Runs`) every 5 s, each time on a new connection, and MUST say one line
for each state of the run that it sees, the first one at once, with the local time:
`brd-host: <hh:mm:ss> run <run> <state>`. It MUST say nothing else while it waits: not the output of the run, not the
lines of the heavy turn. The state comes from the steps of the run that have ended so far and the heavy turn of the
host, which an agent of any build sends:

| The run | State |
|---|---|
| in the queue of the heavy turn, behind `n` runs | `waits for the heavy turn of <alias>, <n> runs ahead: …` (`first in its queue`, `1 run ahead`) |
| not heavy, no step ended, while an exclusive run holds the host | `waits for the end of a measurement that holds <alias>` |
| else, before `slot` ended | `waits for a host slot of <alias>` |
| `slot` ended | `checks out its commit and prepares on <alias>` |
| `prepare` ended | `runs its command on <alias>` |
| `command` ended | `ended its command; <alias> stages its files` |
| the run ended | `ended on <alias>` |

#### Scenario: A wait from the heavy turn to the end
- **WHEN** a heavy run goes from the heavy turn through its slot and its command to its end during one wait
- **THEN** the wait says one line for each state it saw, with the time, and nothing more until the end
  [@test] crates/host-client/src/wait_test.rs (`a_wait_says_each_state_of_the_run_then_takes_its_log_fetches_and_forgets_it_by_its_record`)
  [@test] crates/host-client/src/state_test.rs (`the_state_of_a_run_comes_from_its_ended_steps_and_the_heavy_queue`)

### Requirement: wait: wait finishes an ended run as run does

At the end of the run a wait MUST take its output from its first byte into `<run>.log` of the checkout by one
`Follow`, before the forget cuts the log on the host. With the record of the run in the checkout, it MUST then copy the
files that the record asks for, apply the changes when the record asks for them, write the end of the run
(`<run>.end.json`), and forget the run on the host; a run that the agent refused brings no files. A wait MUST then say
the summary: `brd-host: run <run> on <alias> exited with <code> after <time> on the host` (or `ended with <code>` for a
refusal), the phases, the copied files, `brd-host: the output of the run: <log> (<n> lines)`, and its last 30 lines as
they are. It MUST exit as `run` would: with the exit code of the command, or the exit of the refusal by how far the run
came (`superseded` 75). A fetch that fails MUST leave no end file and keep the run on the host, so the next wait copies
the files again. A checkout without the record MUST say so, take the log, and neither copy a file nor forget the run.

#### Scenario: The end of a run with its record
- **WHEN** the run of a wait ends and the checkout has its record
- **THEN** the wait takes the log, copies the files of the record, forgets the run, writes its end, says the summary
  and exits with the exit code of the command
  [@test] crates/host-client/src/wait_test.rs (`a_wait_says_each_state_of_the_run_then_takes_its_log_fetches_and_forgets_it_by_its_record`)
  [@test] crates/bd-host/tests/cli.rs (`wait_waits_for_a_run_past_its_max_and_the_next_one_ends_it`)

#### Scenario: A fetch that fails
- **WHEN** the fetch of the end of a wait fails
- **THEN** the wait exits 255, writes no end and keeps the run on the host, and the next wait copies its files again
  [@test] crates/host-client/src/wait_test.rs (`a_wait_whose_fetch_failed_copies_the_files_again_on_the_next_wait`)

#### Scenario: A run that the agent refused
- **WHEN** the agent ended the run with a refusal, such as `superseded`
- **THEN** the wait exits with the exit of that refusal, copies no file, and forgets the run
  [@test] crates/host-client/src/wait_test.rs (`a_superseded_run_exits_75_with_the_refusal_of_the_agent`)

#### Scenario: A checkout without the record
- **WHEN** a wait runs in a checkout that has no record of the run
- **THEN** it says so, takes the log, and neither copies a file nor forgets the run
  [@test] crates/host-client/src/wait_test.rs (`a_wait_in_another_checkout_says_so_and_fetches_nothing`)

### Requirement: wait: One process of a checkout finishes a run

The process that finishes a run in its checkout, a `wait` or the `run` of the run, MUST hold the lock `<run>.lock`
from before its fetch to its end file, so the changes of a run never apply twice. A wait while another process holds
it MUST finish nothing and leave with `still_running` at its `--max`. A wait after the end file MUST answer from it
without the host, but for the forget of a run that the call before could not forget. A `run` whose end a wait finished
first MUST report that end and copy nothing again.

#### Scenario: A wait after the end
- **WHEN** a wait runs after an earlier call wrote the end of the run
- **THEN** it says the summary from the end file and exits with its exit, and asks the host only to forget a run that
  is not forgotten
  [@test] crates/host-client/src/wait_test.rs (`a_second_wait_after_the_end_answers_from_the_end_file_and_only_forgets`)

#### Scenario: Another process finishes the run
- **WHEN** a wait runs while another process of the checkout holds the lock of the run
- **THEN** it asks the host nothing and exits 124 with `still_running` at its `--max`
  [@test] crates/host-client/src/wait_test.rs (`a_wait_while_another_process_finishes_the_run_exits_124_and_asks_nothing`)

#### Scenario: A run that a wait finished first
- **WHEN** a wait of the checkout finished the run while its `brd-host run` followed it
- **THEN** the run reports the end of the wait and sends no fetch and no forget, and a run that finished itself writes
  its end
  [@test] crates/host-client/src/run_test.rs (`a_run_that_a_wait_finished_first_reports_its_end_and_fetches_nothing`)

### Requirement: wait: Past --max the run goes on

Past its `--max` a wait MUST leave the run going, send no cancel, and refuse with `still_running` (124), naming the
state of the run and `brd-host wait <alias> <run>`, when the host answered in this call. A wait whose host answered no
poll in the whole `--max` MUST refuse with `timed_out` (255): the run can go on there, and a host that is down does not
keep an agent waiting call after call. `--max` bounds the wait for the end, not the fetch after it.

#### Scenario: A run that goes on past --max
- **WHEN** the run goes on past the `--max` of a wait
- **THEN** the wait exits 124 with `still_running`, the run goes on, and the next wait says its state at once and ends it
  [@test] crates/host-client/src/wait_test.rs (`a_wait_past_its_max_leaves_the_run_going_and_exits_124_and_the_next_one_ends_it`)
  [@test] crates/bd-host/tests/cli.rs (`wait_waits_for_a_run_past_its_max_and_the_next_one_ends_it`)

#### Scenario: A host that never answers
- **WHEN** no poll of a wait gets an answer before its `--max`
- **THEN** the wait exits 255 with `timed_out`
  [@test] crates/host-client/src/wait_test.rs (`a_wait_whose_host_never_answers_exits_255_with_timed_out`)

### Requirement: wait: Ctrl+C leaves a wait

The first Ctrl+C, SIGTERM or SIGHUP MUST leave a wait with 130 and `interrupted: Ctrl+C, SIGTERM or SIGHUP left the
wait of the run <run>, which goes on on <alias>: `brd-host wait <alias> <run>` waits again`, and send no cancel.

#### Scenario: Ctrl+C during a wait
- **WHEN** the user presses Ctrl+C during a wait, or a tool sends it SIGTERM or SIGHUP
- **THEN** the wait leaves with 130, sends no cancel, and the run goes on
  [@test] crates/host-client/src/wait_test.rs (`ctrl_c_leaves_a_wait_and_the_run_goes_on`)

## MODIFIED Requirements

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
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared`, `heavy/turn.json` and `heavy/turn.lock` (the heavy turn of the host, in a directory of its own), `heavy/cores.json` and `heavy/cores.lock` (the measures of the commands), `heavy/host.json` (the busy cores of the host every 2 s, with the most of the last 30 s), `heavy/exclusive.json` (the exclusive run that holds the turn), `heavy/drain.json` (the exclusive run that drains the host, since when), `heavy/heavy.json` (the heavy turn of the build before, which this build only reads), `runs/<run>/cores.json` (the cores of the command of a run) |
| Agent | `~/.local/bin/brd-host`, on a local host `~/.local/share/brd-host/agent/brd-host` | The binary that the LaunchAgent runs, and `<binary>.previous` after an upgrade |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply` |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.log`, `<run>.end.json`, `<run>.lock` | The output of the run from its first byte, which its end writes; how the run ended here (`{"exit", "command_exit", "refusal", "phases", "fetched", "applied_files", "forgotten"}`); the lock that one process of the checkout holds while it finishes the run |

### Requirement: The files: The records and patches of old runs go

A record, a patch, a log, an end and a lock of a run older than 7 days MUST go at the next run, wait or fetch of the
checkout.

#### Scenario: A record older than 7 days
- **GIVEN** a run record, a patch, a log, an end and a lock of the checkout older than 7 days
- **WHEN** the next run, wait or fetch of the checkout starts
- **THEN** they go
  [@test] crates/host-client/src/record_test.rs (`a_run_record_names_what_a_fetch_takes_and_old_records_go`)

### Requirement: Refusal codes: The exit of a refusal says whether the host ran the command

Exit 75 MUST mean that the host did not run the command, so the caller can run it locally. A refusal of `run` before
`Start`, and a refusal of the agent whose phases have no `command`, MUST exit 75. Exit 124 MUST mean that the run goes
on past the `--max` of a wait, and nothing was cancelled (`still_running`). Any other refusal MUST exit 255, except
`usage` (2) and `interrupted` (130).

#### Scenario: A refusal before the command
- **WHEN** `run` refuses before `Start`, or the agent refuses a run whose phases have no `command`
- **THEN** it exits 75, and the caller can run the command locally
  [@test] crates/host-link/src/refusal_test.rs (`the_exit_of_a_code_follows_how_far_the_run_came`)

#### Scenario: Any other refusal
- **WHEN** any other refusal comes
- **THEN** it exits 255, except `usage` (2), `still_running` (124) and `interrupted` (130)
  [@test] crates/host-link/src/refusal_test.rs (`the_exit_of_a_code_follows_how_far_the_run_came`)

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
