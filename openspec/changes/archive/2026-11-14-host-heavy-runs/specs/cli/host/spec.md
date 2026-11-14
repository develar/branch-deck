## ADDED Requirements

### Requirement: run: A heavy run is heavy by --heavy or by heavy of [host] of its commit

A run MUST be heavy with `--heavy`, else when a pattern of `heavy` of `[host]` of the shipped commit matches its
command, and its `Start` MUST carry `heavy`. Its line MUST say why it is heavy. An agent without the feature `heavy`
MUST get the run with a line that names `brd-host upgrade <alias>`.

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
- **WHEN** a heavy run starts on an agent whose hello has no feature `heavy`
- **THEN** the client says `brd-host: the agent on <alias> predates heavy runs, so this one runs beside the others:
  `brd-host upgrade <alias>` sends this build`, and the run goes on as one that is not heavy
  [@test] crates/host-client/src/run_test.rs (`a_run_is_heavy_by_a_pattern_of_its_commit_or_by_heavy_and_says_why`)

### Requirement: The environment of a run and `[host]`: heavy names the commands of the heavy runs

`heavy` of `[host]` MUST be a list of patterns, none empty. A pattern MUST match the words of the command joined by one
space, whole, after a leading `env` and the `NAME=value` words after it; `*` matches any characters, the rest is
literal. The first pattern that matches names the run.

#### Scenario: The patterns of heavy
- **WHEN** `heavy` is `["cargo nextest run*--workspace*", "pnpm e2e*", "pnpm test"]`
- **THEN** `cargo nextest run --no-fail-fast --workspace`, `pnpm e2e-up` and `env DEBUG_E2E=1 pnpm e2e x.spec.ts`
  are heavy runs, and `cargo nextest run -p host-link`, `pnpm test:unit` and `env -i pnpm e2e` are not
  [@test] crates/host-link/src/config_test.rs (`a_heavy_pattern_matches_the_whole_command_line_after_env_and_its_assignments`)

### Requirement: serve: Heavy runs take the heavy turn of the host one at a time

Before its lend, a heavy run MUST take the heavy turn of the host (`lease_pool::host_turn`), one table for every
repository, in the phase `heavy`, and wait for it up to 2 h first come first, its line at once and once a minute; it
MUST give it back after its give-back. A run that is not heavy MUST take no turn.

#### Scenario: Two heavy runs and one that is not heavy
- **WHEN** a heavy run holds the turn, a second heavy run starts, and a run that is not heavy starts
- **THEN** the second waits for the turn without a slot, and its command starts only after the first gave the turn back
- **AND** the run that is not heavy runs beside the first at once
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`heavy_runs_take_the_turn_one_at_a_time_first_come_first`, `a_waiting_run_takes_the_turn_once_the_holder_leaves`)

#### Scenario: The line of the wait
- **WHEN** a heavy run waits for the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for the heavy turn of the host, <time>
  left: run <id> of <repository> holds it <time> (<command>), <n> runs ahead: <id> of <repository> (<command>), …`
- **AND** after the wait it says `took the heavy turn of the host after <n> s`
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_turn_line_names_the_wait_the_time_left_the_holder_and_the_runs_ahead`)
  [@test] crates/host-agent/src/turn_test.rs (`a_heavy_run_says_its_wait_at_once_and_a_stop_ends_it`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_command_of_an_entry_is_one_line_of_at_most_200_characters`)

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
- **THEN** it ends with `heavy_timeout` and the line of its wait, and its ticket leaves the queue
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_heavy_run_that_waits_past_its_time_ends_with_heavy_timeout`)

## MODIFIED Requirements

### Requirement: run: The command line of run

`brd-host run` MUST take the alias of a host, the options of its synopsis and, after `--`, the command with its
arguments. `--fetch` MUST be repeatable, and `--nice` MUST take 0 to 19.

#### Scenario: The synopsis of run
- **WHEN** a caller runs `brd-host run`
- **THEN** it takes this command line

```sh
brd-host run <alias> [--head] [--apply] [--fetch <dir>]... [--no-outputs] [--heavy] [--nice <0-19>] [--repo <path>]
            [--affinity <key>] [--report-fd <n>] -- <command> <args>
```

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
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared`, `heavy.json` and `heavy.lock` (the heavy turn of the host) |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply` |

### Requirement: The report of a run: The phases are the steps of both ends in their order

The steps MUST be `pack` and `upload` of the client; `heavy`, `git`, `slot`, `pack_wait`, `checkout`, `prepare`,
`command` and `stage` of the host; `fetch` and `total` of the client. A run that ends early has only the steps up to its
last one. Only a heavy run has `heavy`, and only a run that waited for an upgrade of git has `git`.

#### Scenario: The steps of a run
- **WHEN** a run goes through
- **THEN** its phases are `pack`, `upload`, `slot`, `pack_wait`, `checkout`, `prepare`, `command`, `stage`, `fetch`
  and `total`, with `heavy` first of the host for a heavy run, and `git` only when it waited for an upgrade of git
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)

#### Scenario: A run that ends early
- **WHEN** a run ends before its last step
- **THEN** its phases have only the steps up to its last one
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)

### Requirement: follow, cancel, fetch, forget, runs: runs lists the runs that the agent keeps

`brd-host runs <alias> [--json]` MUST list the runs that the agent keeps, each with its affinity, commit, command,
start, exit and the steps of 0.1 s or more, then the heavy turn of the host: the run that holds it and for how long, and
each run that waits for it and for how long. An agent without the feature `heavy` has no heavy turn to list.

#### Scenario: The runs of a host
- **WHEN** `brd-host runs <alias>` runs while a run goes on
- **THEN** it lists that run with its affinity, commit, command, start and exit, and the steps of 0.1 s or more
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

#### Scenario: The heavy turn of a host
- **WHEN** `brd-host runs <alias>` runs while a heavy run holds the heavy turn and another waits for it
- **THEN** after the runs it says `heavy turn: run <id> of <repository> holds it <time>: <command>`, then
  `  run <id> of <repository> waits <time>: <command>` for each run that waits, and `heavy turn: free` when no run holds
  it
- **AND** `--json` has the turn as `heavy`: the `holder` and the `queue`, without the runs whose process is gone
  [@test] crates/bd-host/tests/cli.rs (`runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long`, `follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_of_the_heavy_turn_name_the_holder_and_each_waiting_run_with_their_time`)

### Requirement: The environment of a run and `[host]`: The agent reads [host] from the commit of the run

The agent MUST read `[host]` of `.branchdeck.toml` from the commit of the run, never from the files of the slot. A
commit whose `.branchdeck.toml` has no `[host]` table has no prepare steps, no environment of its own and no outputs.
The other tables of `.branchdeck.toml` are not the agent's.

```toml
[host]
# the paths that the command makes and that come back; the agent removes each one before the command
outputs = ["tests/results", "playwright-report"]
# the commands of the heavy runs, which the host runs one at a time; `*` matches any characters
heavy = ["cargo nextest run*--workspace*", "pnpm e2e*"]

[host.env]
RUST_BACKTRACE = "1"                   # a text, as it is
PATH = { prepend = ["~/.cargo/bin"] }  # also `append`; a relative entry is below the slot root
E2E_PORT = { port = 3100 }             # 3100 plus the slot number, or the next free TCP port
CI = { unset = true }                  # removed

[[host.prepare]]
name = "pnpm install"
run = ["pnpm", "install", "--frozen-lockfile"]
inputs = ["pnpm-lock.yaml"]            # the fingerprint: `run` and the object id of each input at the commit
creates = ["node_modules"]             # a missing path runs the step again
```

#### Scenario: A commit with a [host] table
- **WHEN** a run starts on a commit whose `.branchdeck.toml` has a `[host]` table
- **THEN** the agent takes the table of that commit, never the one of the files of the slot
  [@test] crates/host-link/src/config_test.rs (`the_host_table_of_branchdeck_toml_parses_and_an_unknown_key_refuses`, `a_commit_gets_the_host_table_of_its_branchdeck_toml_else_the_default`)
  [@test] crates/host-link/src/config_test.rs (`the_host_table_of_this_repository_parses`)

#### Scenario: A commit without a [host] table
- **WHEN** the `.branchdeck.toml` of the commit has no `[host]` table
- **THEN** the run has no prepare steps, no environment of its own and no outputs
  [@test] crates/host-link/src/config_test.rs (`a_commit_gets_the_host_table_of_its_branchdeck_toml_else_the_default`)

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
  [@test] crates/host-link/src/config_test.rs (`an_unknown_key_names_itself_the_keys_of_this_build_and_the_update_of_brd_host`)

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
| `heavy_timeout` | 75 | The heavy run got no heavy turn of the host within 2 hours: other heavy runs held it. |
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
