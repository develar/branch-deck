## MODIFIED Requirements

### Requirement: The files: Each end keeps its files at the paths of the table

brd-host MUST keep the files of the client, of the agent and of a checkout at the paths of the table below, each with
the content that the table names. In a lease of the pool, whose `notes.local` links to the notes of the lease, the log
and the patch of a run MUST go to `notes.local/brd-host/` instead, in the pool directory, which the summary names.

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
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply`; in a lease the patch is `notes.local/brd-host/<run>.patch` |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.log`, `<run>.end.json`, `<run>.lock` | The output of the run from its first byte, which its end writes; how the run ended here (`{"exit", "command_exit", "refusal", "phases", "fetched", "applied_files", "forgotten"}`); the lock that one process of the checkout holds while it finishes the run |

#### Scenario: The log of a run in a lease
- **WHEN** a wait finishes a run in a lease whose `notes.local` links to the notes of the lease
- **THEN** the log is `notes.local/brd-host/<run>.log`, by that path in the summary, and the end stays in the git
  directory
  [@test] crates/host-client/src/wait_test.rs (`a_wait_in_a_lease_puts_the_log_into_the_notes_of_the_lease`)

### Requirement: The fetch, the outputs and the changes: --apply applies the changes all or nothing

With `--apply`, the patch of the changes that the command made MUST go to `<git dir>/branch-deck/host/runs/<run>.patch`
(in a lease `notes.local/brd-host/<run>.patch`) and apply to the files of the working tree, not to the index, all or nothing (`git_ops::host_client::apply_patch`). A
patch that does not apply MUST refuse with `apply_failed` (exit 255), name the patch, and leave the working tree as it
was.

#### Scenario: A patch that applies
- **WHEN** a run with `--apply` brings back the changes of the command
- **THEN** the patch goes to `<git dir>/branch-deck/host/runs/<run>.patch` and applies to the files of the working
  tree, not to the index
  [@test] crates/host-client/src/run_test.rs (`apply_brings_the_changes_back_all_or_nothing`)

#### Scenario: A patch that does not apply
- **WHEN** the patch does not apply
- **THEN** `run` refuses with `apply_failed` (exit 255) and names the patch
- **AND** the working tree stays as it was
  [@test] crates/host-client/src/run_test.rs (`apply_brings_the_changes_back_all_or_nothing`)
