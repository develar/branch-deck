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
| Client | `~/.config/brd-host/sets/<name>.json` | A set of hosts (`brd-host set`): `{"name", "members": [{"alias", "idle_only"}]}`, `idle_only` left out when false |
| Client | `~/.config/brd-host/bases/<alias>/<affinity>` | The commit of the base of this affinity on that host after the last base sync |
| Agent | `~/.config/brd-host/agent/identity.crt`, `identity.key` | The certificate and the key of the agent |
| Agent | `~/.config/brd-host/agent/clients` | The allow list: one `<64 hex> <name>` line per client |
| Agent | `~/.cache/brd-host/runs/<run>/` | `spec.json`, `events.log`, `exit.json`, `supervisor.log`, `outputs/`, `changes.patch`, `admitted` (a run of `admit_now` that its supervisor admitted) |
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared`, `heavy/turn.json` and `heavy/turn.lock` (the heavy turn of the host, in a directory of its own), `heavy/cores.json` and `heavy/cores.lock` (the measures of the commands), `heavy/host.json` (the busy cores of the host every 2 s, with the most of the last 30 s), `heavy/exclusive.json` (the exclusive run that holds the turn), `heavy/drain.json` (the exclusive run that drains the host, since when), `heavy/heavy.json` (the heavy turn of the build before, which this build only reads), `runs/<run>/cores.json` (the cores of the command of a run) |
| Agent | `~/.local/bin/brd-host`, on a local host `~/.local/share/brd-host/agent/brd-host` | The binary that the LaunchAgent runs, and `<binary>.previous` after an upgrade |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply`; in a lease the patch is `notes.local/brd-host/<run>.patch` |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.host` | The alias of the host that the run went to, one line, beside the record, which refuses a field that it does not know |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.log`, `<run>.end.json`, `<run>.lock` | The output of the run from its first byte, which its end writes; how the run ended here (`{"exit", "command_exit", "refusal", "phases", "fetched", "applied_files", "forgotten"}`); the lock that one process of the checkout holds while it finishes the run |

#### Scenario: The log of a run in a lease
- **WHEN** a wait finishes a run in a lease whose `notes.local` links to the notes of the lease
- **THEN** the log is `notes.local/brd-host/<run>.log`, by that path in the summary, and the end stays in the git
  directory
  [@test] crates/host-client/src/wait_test.rs (`a_wait_in_a_lease_puts_the_log_into_the_notes_of_the_lease`)

### Requirement: The files: The records and patches of old runs go

A record, a host, a patch, a log, an end and a lock of a run older than 7 days MUST go at the next run, wait or fetch of the
checkout.

#### Scenario: A record older than 7 days
- **GIVEN** a run record, a host, a patch, a log, an end and a lock of the checkout older than 7 days
- **WHEN** the next run, wait or fetch of the checkout starts
- **THEN** they go
  [@test] crates/host-client/src/record_test.rs (`a_run_record_names_what_a_fetch_takes_and_old_records_go`)

### Requirement: run: The command line of run

`brd-host run` MUST take the alias of a host or the name of a set of hosts, the options of its synopsis and, after `--`, the command with its
arguments. `--fetch` MUST be repeatable, `--nice` MUST take 0 to 19, and `--max` a duration as `brd-host wait` takes
it.

#### Scenario: The synopsis of run
- **WHEN** a caller runs `brd-host run`
- **THEN** it takes this command line

```sh
brd-host run <alias|set> [--head] [--apply] [--fetch <dir>]... [--no-outputs] [--heavy] [--exclusive] [--nice <0-19>]
                  [--repo <path>] [--affinity <key>] [--max <duration>] [--report-fd <n>] -- <command> <args>
```

## ADDED Requirements

### Requirement: run: A run to a set goes to the member that ranks first

A run to the name of a set MUST ask every member at once what it would meet there (`Request::Estimate`), 3 s each,
and MUST run on the member that ranks first by `start_in_ms + unknown_ahead × 10 min + expected_wall_ms`, a tie going to
the earlier member, after one line of the plan on the stderr. Its report MUST name the member that ran it.
`--exclusive` with a set MUST refuse with `usage`.

#### Scenario: The rank of the members
- **WHEN** a run goes to a set of hosts
- **THEN** the members rank by when the run would end there: its start, 10 min per run of an unknown length before it,
  and its wall, a wall that a member does not know being the longest that another one knows
- **AND** a tie goes to the earlier member, and a member without an estimate ranks last
  [@test] crates/host-client/src/choose_test.rs (`the_members_of_a_set_rank_by_when_the_run_would_end_there`)

#### Scenario: The line of the plan
- **WHEN** the client chose the member
- **THEN** the stderr says what each member told and the choice: `brd-host: studio: starts in ~12 min, runs ~4 min;
  local: starts now, runs ~6 min, only at once; chose local`
- **AND** the report of the run names the member that ran it
  [@test] crates/host-client/src/choose_test.rs (`the_plan_of_a_run_names_what_each_member_told_and_the_choice`)
  [@test] crates/bd-host/tests/cli.rs (`a_run_to_a_set_of_hosts_says_its_plan_and_a_wait_to_the_set_goes_to_its_member`)

#### Scenario: The members are asked at once
- **WHEN** the client asks the members
- **THEN** it asks each of them at once, within 3 s, and reads the feature `estimate` from the hello of the same
  connection
- **AND** an agent without the feature gets no request, and one that does not answer in 3 s has no estimate
  [@test] crates/host-client/src/choose_test.rs (`the_members_are_asked_at_once_and_one_without_estimates_or_an_answer_has_none`)

### Requirement: run: A heavy run goes to a member only with its measure there

The first member of a set MUST be eligible for every run. For a heavy run, another member MUST be eligible only with its
own measure of the command; for a run that is not heavy, every member that answered.

#### Scenario: A new heavy command
- **WHEN** a heavy run goes to a set whose other members have no measure of its command
- **THEN** it runs on the first member, and the line of the plan says that a heavy run does not go to the others
- **AND** an exclusive run to a set refuses with `usage` (2)
  [@test] crates/host-client/src/choose_test.rs (`a_heavy_run_without_a_measure_stays_on_the_first_member_and_an_exclusive_one_refuses`)

### Requirement: run: A busy member of a set sends the run to the next one

A member of `idle_only` MUST get the start that admits at once or refuses (`RunSpec::admit_now`), and its `busy` MUST send
the run to the next eligible member by the rank, with a line that says so. Of two clients that race for one free member
of `idle_only`, only one MUST run there; a run that every member refused MUST end with the refusal of the first member.

#### Scenario: A busy member
- **WHEN** the member that ranks first takes runs only at once and answers the start with `busy`
- **THEN** the run goes to the next member, and the stderr says `brd-host: <member>: <message>; the run goes to <next>`
- **AND** `<run>.host` of the checkout names the member that took the run
  [@test] crates/host-client/src/choose_test.rs (`a_busy_member_that_takes_runs_only_at_once_sends_the_run_to_the_next_one`)

#### Scenario: Two clients and one free member
- **WHEN** two clients start a run to the set at once, and both rank the free member first
- **THEN** its agent admits one, and the other run goes to the next member
  [@test] crates/host-client/src/choose_test.rs (`two_clients_that_race_for_a_free_member_get_one_admission`)

#### Scenario: Every member busy
- **WHEN** every member that the run tries answers `busy`
- **THEN** the run ends with the refusal `busy` of the first member of the set (exit 75): nothing ran anywhere, and the
  caller runs it to the set again later, never as a plain command here
  [@test] crates/host-client/src/choose_test.rs (`a_run_that_every_member_refuses_with_busy_ends_with_the_refusal_of_the_first_member`)

### Requirement: follow, cancel, fetch, forget, runs: The name of a set goes to the host of the run

`wait`, `cancel`, `follow`, `fetch` and `forget` MUST take the name of a set for the alias, and MUST go to the member that
`<run>.host` of the checkout of the working directory names. A host file of the name MUST come first. A set without that
file MUST refuse with `usage` and name its members.

#### Scenario: A wait to the name of a set
- **WHEN** a wait names a set and a run that went to one of its members
- **THEN** it waits for the run on that member
- **AND** a run whose host this checkout does not name refuses with `usage` (2), naming the members
  [@test] crates/host-client/src/choose_test.rs (`a_busy_member_that_takes_runs_only_at_once_sends_the_run_to_the_next_one`)
  [@test] crates/bd-host/tests/cli.rs (`a_run_to_a_set_of_hosts_says_its_plan_and_a_wait_to_the_set_goes_to_its_member`)

### Requirement: set: The command line of set

`brd-host set <name> <alias>... [--idle-only <alias>]... [--json]` MUST write the set of hosts `<name>` with its members
in their order, the members of `--idle-only` taking runs only at once; without aliases it MUST print the set.

#### Scenario: The synopsis of set
- **WHEN** a caller runs `brd-host set`
- **THEN** it takes this command line, and prints `set <name>: <member>, <member> (only at once); <path>` or, with
  `--json`, the set as its file holds it
  [@test] crates/bd-host/tests/cli.rs (`a_run_to_a_set_of_hosts_says_its_plan_and_a_wait_to_the_set_goes_to_its_member`)

```sh
brd-host set <name> [<alias>]... [--idle-only <alias>]... [--json]
```

### Requirement: set: A set names its members and never an alias

A set MUST name at least one member, each a host file, once. Its name MUST NOT be the alias of a host file, which
refuses with `usage`, and `idle_only` MUST stay in the file of the set, never in a host file.

#### Scenario: A set and its file
- **WHEN** `set` writes a set
- **THEN** the set reads back as it was written, and its name is no host
- **AND** the alias of a host file, a member twice or no member refuses with `usage`, and a member without a host file
  with `no_host`
  [@test] crates/host-client/src/config_test.rs (`a_set_of_hosts_names_its_members_and_never_an_alias`)
