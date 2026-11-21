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

### Requirement: setup: The command line of setup

`brd-host setup` MUST take either the alias of a host, with `--repo <path>`, `--port`, `--name <name>` and
`--binary <path>`, or `--local` with an optional alias (`local` by default) and the same options, or `--agent` with
`--client <fingerprint>`, `--name <name>` and `--port`. `--port` MUST be 7742 by default.

#### Scenario: The synopsis of setup
- **WHEN** a caller runs `brd-host setup`
- **THEN** it takes one of these command lines
  [@test] crates/bd-host/src/cli_test.rs (`setup_takes_an_alias_or_local_or_agent`)

```sh
brd-host setup <alias> [--repo <path>] [--port 7742] [--name <name>] [--binary <path>]
brd-host setup --local [<alias>] [--repo <path>] [--port 7742] [--name <name>] [--binary <path>]
brd-host setup --agent --client <fingerprint> --name <name> [--port 7742]
```

### Requirement: setup: The agent setup writes the identity, the allow list and the LaunchAgent

`brd-host setup --agent` MUST make the identity of the agent when it has none, add the client to the allow list, write
the LaunchAgent `com.jetbrains.brd-host` (`KeepAlive`, `AbandonProcessGroup`, `~/.local/bin/brd-host serve --port <port>`,
then `--bind <address>` for a bind address), and bootstrap or restart it. It MUST print one JSON line
`{"fingerprint", "port", "launchd": "bootstrapped" | "restarted" | "none"}`.

#### Scenario: The setup of the agent
- **WHEN** `brd-host setup --agent --client <fingerprint> --name <name>` runs on the host
- **THEN** the agent has its identity, the allow list names the client, and the LaunchAgent `com.jetbrains.brd-host`
  is written and bootstrapped or restarted
- **AND** it prints one JSON line of the fingerprint, the port and what launchd did
  [@test] crates/host-agent/src/setup_test.rs (`the_agent_setup_writes_the_identity_the_allow_list_and_the_launch_agent`)

#### Scenario: The setup of an agent with a bind address
- **WHEN** the agent setup has a bind address, as the one of `setup --local` has the loopback address
- **THEN** the LaunchAgent runs `serve --port <port> --bind <address>`
- **AND** an upgrade reads the bind back, so the new binary finds the LaunchAgent of its setup the same
  [@test] crates/host-agent/src/setup_test.rs (`the_plist_of_a_bind_runs_serve_on_that_address_only`)

### Requirement: serve: serve prints where it listens

`brd-host serve [--port 7742] [--bind <address>]` MUST listen on every address, or on `--bind` only, and print one
JSON line `{"listening": "<address>", "fingerprint": "<64 hex>"}`, then log on stderr.

#### Scenario: The start of serve
- **WHEN** `brd-host serve` starts
- **THEN** it prints one JSON line `{"listening": "<address>", "fingerprint": "<64 hex>"}`, then logs on stderr
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)

## ADDED Requirements

### Requirement: setup: setup --local makes this machine its own host

`brd-host setup --local [<alias>]` MUST make this machine its own host without ssh: put `--binary`, else this
executable, at `~/.local/share/brd-host/agent/brd-host` when another build is there, run the agent setup in process
with that binary and the loopback address as its bind, and write the host file of `127.0.0.1` with the fingerprint of
the agent. `~/.local/bin/brd-host` MUST stay as it is.

#### Scenario: A local setup
- **WHEN** the user runs `brd-host setup --local`
- **THEN** the allow list of the agent of this machine names this client, the LaunchAgent binds `127.0.0.1`, and the
  host file `local.json` names `127.0.0.1`, the port and the fingerprint of the agent
- **AND** no ssh runs, and the hello answers as after `setup <alias>`
  [@test] crates/host-client/src/setup_test.rs (`a_local_setup_pins_the_agent_of_this_machine_on_the_loopback_without_ssh_and_a_request_goes_there`, `a_local_setup_refuses_a_bad_alias_before_the_agent_setup_and_an_agent_setup_without_its_line`)
  [@test] crates/bd-host/tests/cli.rs (`setup_local_makes_this_machine_its_own_host_on_the_loopback_and_a_run_goes_there`)

#### Scenario: The binary of a local host
- **WHEN** `~/.local/share/brd-host/agent/brd-host` is missing or another build
- **THEN** the setup puts a copy of the binary there by a rename, and the LaunchAgent runs that copy
- **AND** a file of the same build stays
- **AND** `~/.local/bin/brd-host` stays as it is: it is the client of this machine, which `brd install` links to the
  bundle of the app, so the client follows the app and launchd never runs a file of the bundle
- **AND** `brd-host upgrade <alias>` replaces the copy, as it replaces the binary of the LaunchAgent of any host
  [@test] crates/host-agent/src/setup_test.rs (`a_local_setup_runs_a_copy_of_its_own_outside_the_path_binds_the_loopback_address_and_leaves_the_client_alone`)
  [@test] crates/bd-host/tests/cli.rs (`setup_local_makes_this_machine_its_own_host_on_the_loopback_and_a_run_goes_there`)

#### Scenario: A run on a local host
- **WHEN** a run goes to the alias of a local host
- **THEN** it connects to `127.0.0.1`, which macOS Local Network privacy never refuses, so no relay through `nc` starts
- **AND** no pack travels when the repository of the host is the repository of the checkout
  [@test] crates/bd-host/tests/cli.rs (`setup_local_makes_this_machine_its_own_host_on_the_loopback_and_a_run_goes_there`)

### Requirement: setup: The agent of a local host keeps this machine as a host

The agent of a local host MUST be the agent of every host: its keeper relieves every pool of this machine below
`branchdeck.leaseMinFree`, runs `brew upgrade git` below the floor of the git of a host, and `serve` holds the power
assertions while a run goes on. No other machine reaches it.

#### Scenario: The keeper of a local host
- **WHEN** the agent of a local host runs
- **THEN** its keeper fills the host slots of the repositories of its runs, relieves every pool of this machine below
  `branchdeck.leaseMinFree`, and upgrades the git of Homebrew below the floor, as on any host
- **AND** it listens on the loopback address only, so a connection from another machine finds no agent
