## ADDED Requirements

### Requirement: stats: The end of a run says whether a cancel interrupted its command

When a cancel or a stop signal of the supervisor came while the command ran, the supervisor MUST write `"cancelled":
true` into `exit.json`, whatever code the command then exited with, and the line of the run MUST carry it; such a run
MUST count as no failure of its command and in no spread of the lends.

#### Scenario: A cancel of a command that traps SIGINT
- **WHEN** a cancel interrupts a command that exits 7 in its trap, or one that ignores SIGINT and gets SIGKILL (137)
- **THEN** `exit.json` has `"cancelled": true` beside the exit code of the command, and the line `"cancelled": true`
- **AND** a run that no cancel interrupted has no `cancelled`; a line of a build before it tells a cancel only by the
  exit 130
  [@test] crates/host-agent/tests/serve/signals.rs (`a_cancel_interrupts_the_command_group`, `a_command_that_ignores_sigint_gets_sigkill_after_the_grace_time`)
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/host-agent/src/history_test.rs (`the_line_of_a_run_whose_command_a_cancel_interrupted_says_so_whatever_its_exit`, `a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

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
| Agent | `~/.cache/brd-host/runs/<run>/` | `spec.json`, `events.log`, `exit.json` (the end, its phases, `lost` and `cancelled`), `supervisor.log`, `outputs/`, `changes.patch`, `queue.json` (the place and the wait of a heavy run in the queue of the heavy turn), `slot.json` (the slot of the run and how its lend chose it), `pack.json` and `fetched.json` (the bytes of its packs and of its fetches), `admitted` (a run of `admit_now` that its supervisor admitted) |
| Agent | `~/.cache/brd-host/history/` | `<YYYY-MM>.jsonl`, one line per ended run in the month of its end by UTC, and `bases-<YYYY-MM>.jsonl`, one line per pack of a base, each kept 90 days |
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

### Requirement: follow, cancel, fetch, forget, runs: runs names the repository and the slot of each run

`RunInfo` of `brd-host runs --json` MUST name the repository of the run with `~/` expanded, as the history line and
`stats` name it (`repo`), and, once a lend gave it a slot, the slot (`slot`): its number `n`, how the lend chose it
(`choice`), `waited`, `cleaned` and, for a miss, why (`missed`). A forgotten run MUST keep its slot; an older agent
sends neither field.

#### Scenario: The slot of a run
- **WHEN** `brd-host runs <alias> --json` runs after the lend of a run
- **THEN** its `RunInfo` has `"repo": "<repository>"` and `"slot": {"n": <n>, "choice": "<choice>"}`, with
  `"waited": true` after a wait in the queue of the slots of any cause (busy slots, runs before it, a fill, an upgrade
  of git), `"cleaned": true` after a clean, and for `idle` `"missed": "<cause>"`; `choice` is `affinity` for the idle
  slot whose last lend had the affinity of the run, `idle` for the idle slot used longest ago, which another affinity
  used last or no run did; a choice or a cause of a later build reads as `other` on an agent of this build
- **AND** a run before its lend, a run that got no slot, and a run of an older agent or supervisor have no `slot`,
  and a reply of an older agent no `repo`: the client shows nothing for them
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/host-agent/src/run_test.rs (`a_forgotten_run_keeps_its_end_and_its_log_and_its_files_go`)
  [@test] crates/host-link/src/proto_test.rs (`the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema`)

### Requirement: stats: A run writes its slot and how its lend chose it

A lend MUST say how it chose the slot, `affinity` for the idle slot whose last lend had the affinity of the run, else
`idle` with why no idle slot of the affinity was there, whether the ticket of the run waited in the queue of the slots
before it, and whether a clean came since the last run of the slot; the supervisor MUST write it to `slot.json` after
the lend, and the line of the run MUST carry it as `slot`.

#### Scenario: Each kind of choice
- **WHEN** a run gets the slot that no run used, a run of the same affinity waits for it and gets it as it comes
  free, the next run of that affinity gets it at once, and a run of another affinity waits for it and gets it
- **THEN** their lends say `idle`, `affinity` after a wait, `affinity`, and `idle` after a wait; a repeat of a lend
  that the run holds already says no choice, which no supervisor asks for
- **AND** the line of the run has `"slot": {"n": <n>, "choice": "<choice>", "waited": true}`, `waited` only after a
  wait; a line of a supervisor of a build before it has no `slot`, and a choice of a later build reads as `other`
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_says_whether_it_got_the_slot_of_its_affinity_and_whether_it_waited`)
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

#### Scenario: Two idle slots
- **WHEN** two slots are idle, one of them used last by the affinity of the run and the other idle longer
- **THEN** the lend takes the slot of the affinity and says `affinity`; a run of an affinity that neither had takes
  the slot idle longest and says `idle`, though another affinity used the other one later
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_names_the_slot_of_its_affinity_over_one_idle_longer_and_else_the_one_idle_longest`)

#### Scenario: A clean since the last run of the slot
- **WHEN** the slot of an affinity is cleaned after the give-back of its run, by hand, by the clean after a run once
  one is due, or by the idle clean, and the next run of that affinity gets it
- **THEN** its lend says `affinity` and `cleaned`, and the line has `"cleaned": true`; the run after a give-back with
  no clean says no `cleaned`; a table whose give-back wrote no `returned` (a build before it) counts the clean of the
  second of the last lend or later
- **AND** a last run of the affinity that ended before its checkout (a refusal while it waited for its pack, a cancel)
  leaves the files of the checkout before it, and the next lend says `affinity` without `cleaned`: the table does not
  tell it
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_of_the_slot_of_its_affinity_says_whether_a_clean_came_since_its_last_run`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`)

#### Scenario: Why a lend missed the slot of its affinity
- **WHEN** a lend of `idle` finds no idle slot whose last lend had the affinity of the run
- **THEN** it says `missed`: `lent` when a slot of the affinity is lent to another run of the same checkout,
  `unavailable` when one is not ready for a run (its give-back, clean, shrink or fill, a quarantine, a lease not
  `ready`), else `no_slot`; a repeat says none
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_that_misses_the_slot_of_its_affinity_says_why`)

### Requirement: stats: The summary counts the runs that got the slot of their affinity

Per repository, the summary MUST count the runs whose line names their slot, the warm hits, the hits on a slot cleaned
since, the runs that waited, and the misses by cause; per command of the repository, the 10 with the most runs and the
rest in one entry, it MUST give its runs and hits and, for the warm hits, the cleaned hits and the misses, the runs whose
command ran to its end with the p50 and p90 of their `command`, `checkout` and `prepare`.

#### Scenario: The slots of two repositories
- **WHEN** the agent sums a window whose lines name their slots in two repositories
- **THEN** the summary has `slots`, one entry per repository by its path as written with `~/` expanded (two spellings
  of one repository, a symlink or a trailing `/`, are two entries): `repo`, `runs`, `affinity`, `cleaned`, `waited`,
  `misses` (`first`, `taken`, `lent`, `unavailable`, `other`) and `commands`, each with `command_key`, `command`,
  `measure_as`, `runs`, `affinity`, `cleaned` and the buckets `warm`, `after_clean` and `missed`
  (`{"runs", "command_ms", "checkout_ms", "prepare_ms"}`, the spreads `{"p50", "p90"}`, none without a run whose
  command ran to its end: a cancel whatever its exit, a refusal or a lost supervisor leaves a run out), and
  `other_commands` (`{"commands", "runs"}`) past the 10
- **AND** a miss of `no_slot` is `first` when no earlier line of the window names a slot of its affinity in the
  repository, so a checkout whose last run came before the window counts as first, else `taken`; a cause or a choice
  of a later build is `other`
- **AND** its lines, after the heavy queue, are `slots of <repo>: <runs> run(s) with a slot: <n> warm on the slot of
  their checkout (<percent> %), <n> on it cleaned since, <n> on another slot (<n> first of their checkout in the
  window, <n> whose slot another checkout took since, <n> whose slot another run of their checkout had, <n> whose slot
  was not ready, <n> of another cause); <n> waited in the queue of the slots`, then per command `<runs> run(s), <hits>
  on the slot of their checkout; warm <n> run(s), command p50 <time> p90 <time>, checkout p50 <time>, prepare p50
  <time>; after a clean …; missed …: <command>` and `<n> other command(s): <n> run(s)`, a part of no run left out
- **AND** a bucket of fewer than 5 runs prints `<n> run(s), too few for a p50`, and the JSON keeps its spreads with
  its runs
- **AND** the buckets compare the runs of one command, not of one size of change: a miss of a small change can take
  less than a hit of a large one
- **AND** a window without a slot of record, and an agent of an older build, give no `slots`
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity`, `the_lends_of_a_repository_name_the_commands_with_the_most_runs_and_the_rest_in_one_entry`)
