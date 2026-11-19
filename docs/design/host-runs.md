# Host Runs

**Status:** Draft
**Updated:** 2026-11-15
**Tags:** [host, remote-run, lease, brd-host, yonder]

## Problem

A heavy check (the full `cargo nextest run --workspace`, `pnpm e2e`, a test suite of the IntelliJ monorepo) runs on
another Mac of the user, the host. Until 2026-10-28 yonder did this. yonder lived in `community/tools/yonder` of the
IntelliJ monorepo: a client, the agent `yonderd` and the link that they shared. It called Branch Deck as child
processes: `brd lease host lend|give-back|have|ingest|show|snapshot|changes|git` on the host and `brd host
snapshot|pack|apply` on the client.

That seam was the cost. Each git step of a run was a `brd` process with a JSON answer. The tests of yonderd faked brd in
a shell script (`FAKE_BD`). The host needed a brd of the same features as the agent, and the two shipped in two release
cycles of two repositories. The slot policy and the git rules lived in Branch Deck already (`lease_pool::host_slots`,
the scan tests), so yonder kept only the transport and the processes.

## Solution Overview

The remote runs move into this workspace as one binary, `brd-host`. It is the client and the agent. It calls
`lease-pool`, `git-ops` and `git-executor` in process, so no `brd` child runs and no JSON crosses a process boundary.
The protocol is new (host-link, schema 1, port 7742).

`brd-host` is a binary of its own. Neither `brd` nor the app links it, so the app binary does not grow. On macOS the app
bundle carries it as a file (`BranchDeck.app/Contents/MacOS/brd-host`), and `brd install` links
`~/.local/bin/brd-host`. The release build gets it through its own Tauri config, `tauri.macos-bd-host.conf.json`, as
the Windows build gets `brd.exe`: Tauri merges `tauri.macos.conf.json` into every macOS build, and a plain `cargo build`
of the app would then need the binary first. The contract of the CLI, the refusal codes and the report of a run are in
[brd-host](../../openspec/specs/cli/host/spec.md), the setup of a host for a person in
[Runs on Another Mac](../guide/host-runs.md).

## Design

### Architecture

```mermaid
flowchart LR
  subgraph Client["This Mac"]
    BT["bt --host, a shell"] --> RUN["brd-host run"]
    RUN --> HC["host-client"]
    HC --> GO["git-ops: snapshot, pack, apply"]
  end
  subgraph Host["The host"]
    SERVE["brd-host serve (launchd)"] --> SUP["brd-host supervise, one per run"]
    SERVE --> KEEP["keeper thread"]
    SUP --> HS["lease_pool::host_slots: lend, checkout, give-back"]
    KEEP --> HS
    SERVE --> HO["lease_pool::host_refs, git_ops::host_objects: have, ingest, show"]
  end
  HC -- "host-link: TLS 1.3, pinned certificates" --> SERVE
```

| Crate | Content | Depends on |
|---|---|---|
| `host-link` | The identity files, TLS 1.3 with pinned certificates, the frames, the protocol, the allow list, the relay through `/usr/bin/nc`, the refusal codes, `[host]` of `.branchdeck.toml` | rustls, rcgen, `sync-utils` |
| `host-client` | The run, the commit of the working tree, the thin pack, the follow, the cancel, the fetch, the base sync, the host files, the upgrade | `host-link`, `git-ops`, `git-executor` |
| `host-agent` | `serve`, the supervisor, the run directories, the login environment, the prepare steps, the keeper, the power assertions, the upgrade | `host-link`, `lease-pool`, `git-ops`, `git-executor` |
| `brd-host` | The CLI of both ends | `host-agent`, `host-client`, `host-link` |

The direction rules are in [Crates](rust-domain-authority.md#crates-и-направление-зависимостей), and
`crates/bd-cli/src/architecture_test.rs` checks them.

### The slot model in short

A remote slot holds no state of its own. The commit comes from the client, and `--fetch`, the outputs and `--apply`
bring back the rest. So a slot is lent to one run for the time of that run only. The host keeps a fixed set of warm
slots per repository (`branchdeck.hostSlots`, 4 by default) as plain `ready` leases of the holders `brd-host/<n>`.

- A run gets the idle slot that its affinity (`checkout-<12 hex>` of the local checkout path) used last, else the idle
  slot used longest ago, else it waits in a first-come queue. A waiting run gives back a slot that a gone run left
  itself, without waiting for the keeper.
- The give-back at the end of each run stops the processes that the run left, keeps the Bazel servers of the slot,
  and discards the changes. The ignored files stay as the warm cache until the slot has waited idle
  `branchdeck.hostSlotIdleClean` (6 hours by default): then the keeper applies the repository's clean rules. A process
  older than the lend quarantines the slot.
- The keeper fills, cleans and shrinks the slots. Nothing on the client keeps or returns a slot.

The lend table, the give-back, the clean and the shrink are in "Host slots" of
[Git Operations](git-operations.md#host-slots) and of [brd lease](../../openspec/specs/cli/lease/spec.md).

### Heavy runs

A full `cargo nextest run --workspace`, a full `pnpm e2e` or a long `bt` uses every core of the host. Two of them at
once run 3 to 4 times slower, and the tests of both fail on their timeouts: on 2026-11-14 the E2E suite ran 3.6 times
slower beside a `cargo nextest run --workspace` of another agent, and 4 tests failed at 30 s. Both of those were large.
So the heavy runs of a host share its cores by what their commands used before. They wait in one queue, whatever their
repository, and the other runs go as before. Until 2026-11-18 they ran one at a time: a queue waited 45 to 50 min,
mostly behind runs that did not fill the host. `pnpm lint:backend` measured 5 to 17 of the 20 cores of the Studio
while it ran, and `bt --lane fast` alone held a load of 77 to 122, as Bazel counts twice the cores of the host.

- **What is heavy** is the repository's to say: `heavy` of `[host]` lists the patterns of its heavy commands
  (`cargo nextest run*--workspace*`, `pnpm e2e*`), `*` any characters, matched against the words of the command after
  a leading `env` and its assignments. `brd-host run --heavy` marks a command that no pattern names. No flag marks a
  run light: an agent that forgets a flag still waits, and a single E2E spec waits too, since it is the run that a
  heavy neighbour fails. `bt --host` passes no flag, so a bt run is heavy only once its repository names it in
  `heavy`; the IntelliJ monorepo decides that on its own, since every bt there is a Bazel build of every core.
- **The client decides**, from `[host]` of the commit it ships, which it reads for the outputs anyway, and sends
  `RunSpec::heavy`. The agent reads `[host]` only after the lend and the pack, so a decision there would move the pack
  before the lend, and a run would wait for a pack without a slot while a Homebrew upgrade of git holds its
  host-object calls.
- **An older agent** refuses every run of a commit with `heavy`, a light one too, as an unknown key of `[host]`. The
  client knows both the table and the features of the agent, so it refuses before the start with `bad_config` and
  names `brd-host upgrade`; a `--heavy` run of a commit without the key runs on such an agent beside the others.
- **The heavy turn** (`lease_pool::host_turn`) is `~/.cache/brd-host/heavy/turn.json`: the holders, the tickets of
  the waiting runs, the runs that are not heavy with their live cores, each with the pid of its supervisor, the cores
  of the host and the schema of the table, which a supervisor of another build refuses rather than misread. A heavy
  run takes its cores before its lend, in the phase `heavy`, so it holds no slot while it waits and the runs that are
  not heavy keep every slot.
- **The admission**: the head of the queue is admitted while the expected cores of the holders, the live cores of the
  runs that are not heavy and its own expected cores fit the cores of the host (`branchdeck.hostCores` of the global
  git config of the host, else what the system counts). Only the head: a small run never overtakes one that waits for
  more, so none waits forever, and a host that frees cores gives them to the oldest waiting run. A run without a
  measure, or whose command measured the whole host, takes the whole host: it waits for every holder and for a quiet
  host, and no heavy run joins it. The samples of the runs miss what no run counts (on 2026-11-19 the Studio ran at a
  load of 61 to 84, `top` saying 0 % idle, while the turn counted 8.9 of its 20 cores held), so `serve` samples the busy
  cores of the whole host every 2 s (`sync_utils::host_cpu`, `host.json` beside the turn), and an admission counts the
  larger of the live cores of the runs that are not heavy and the busy cores of the host less the live use of the
  holders. A quiet host is busy with at most a fifth of its cores outside the heavy runs. A new command, a first `bt --lane fast` among them, so runs alone until its runs say otherwise, which is
  the behaviour of the turn before. A run that is not heavy waits for nothing; its live cores only keep a heavy run
  from joining a host they already load.
- **An exclusive run** (`brd-host run --exclusive`): a measurement such as the A/B pair of a fast lane needs a quiet
  host, and a run of the whole host gives none, as the runs that are not heavy go beside it. An exclusive run is heavy,
  is admitted only without a holder on a quiet host, and while it holds no heavy run is admitted and each new run that
  is not heavy waits for its end before its lend, in the step `exclusive`. It is an explicit flag, never a pattern of
  `[host]`: a check that took the host this way would stop every agent of the host for its whole run. The holder is also in
  `exclusive.json` beside the table: a supervisor of the build before, of a run that began before an upgrade, rewrites
  `turn.json` at each sample of its run and drops the fields it does not know (on 2026-11-19 the first exclusive run
  lost its mark that way, and runs of idea-1 and accio started beside it).
- **The measure** (`host_agent::cores`, `lease_pool::host_cores`): the supervisor of every run samples the CPU of its
  command every 10 s: the processes of its command group, the Bazel servers of its slot (a server leaves the group, and
  its CPU is the work of the run) and every descendant of either (a test of nextest in a group of its own, an action of
  a Bazel server), each with the CPU of its reaped children (`process_scan::cpu_times`). So a process that starts and
  ends between two samples, a rustc or a test, counts in the process that waited for it, in the live cores too. A
  process that a sample saw alive counts once: its parent counts it again when it reaps it, and its time at that sample
  is a credit that the next growth pays off. At the end the measure takes the larger of the sampled CPU and the CPU of
  the reaped children of the supervisor (`RUSAGE_CHILDREN`) over the wall time. Neither alone was enough: on 2026-11-19 a
  cold `cargo build -p git-ops` used 221 s of CPU in 108 s (2.0 cores) and the run measured 0.8, as the samples missed
  each rustc between two of them and `RUSAGE_CHILDREN` lost the build, since `zsh -c` execs its last command and macOS
  drops the reaped children of a process at its exec. Each run of 10 s or more adds its mean to the last 5 of its
  repository and command (the key of the supersede), in `cores.json` beside the turn, and a command is expected to use
  their 80th percentile. So the stats follow the changes, never a calibration of once: a command that got heavier is
  expected heavier at its next run, one that got lighter (a faster fast lane) frees its cores within a few runs, and a
  cancelled run counts by its own measure. A holder holds the larger of its expectation and its live use, so a run that
  grows past its expectation holds more at once, for the admissions that follow, not only at its end.
- **The upgrade**: `heavy.json`, the table of one holder of the build before, stays for its supervisors, which go on
  after an upgrade; its live holder holds the whole host for the runs of this build. A supervisor of the build before
  that waits in `heavy.json` takes its turn without seeing the holders of `turn.json`, so for the hours after an
  upgrade two heavy runs can run at once, as they do when one of them is not heavy. Its wait is the wait for a slot: looks of 2 s, so a cancel ends it, one watch of the
  directory of the table for the whole wait, which holds nothing else, a look every 5 s, and a line at once and once
  a minute with the holder, the runs ahead and the time left. It gives the turn back after its give-back, so the
  processes that it left are gone first. A killed supervisor holds nothing: the next run takes the turn of a gone
  holder at its next look. A wait past 2 h ends with `heavy_timeout`, and the run then goes nowhere else: a local run
  is the run under load that the turn keeps from failing. The line of the wait says that it is a wait, not a hang,
  since an agent reads only the output and cancelled runs that only waited.
- **A later run supersedes a waiting one of its checkout and command.** On 2026-11-18 one session queued the same
  hour-long A/B batch of `pnpm e2e` on every new commit, and each queued run would have run in its turn. A heavy run new
  to the queue takes the place of the runs that wait there with its affinity (the checkout of its client) and its
  command by what it runs (`host_link::heavy::run_key`: the words without `nice`, `time` and the like before them, the
  environment kept, as it changes what a run checks), and they move to `superseded` with its id. The next look of each
  answers it, and its supervisor ends it with `superseded` (exit 75) before its lend: a run of the queue holds no slot
  and started no command, so nothing of it ran. The holder is never superseded, as its command may run. The later run
  says whom it replaces, and names the other runs of its checkout that wait with another command, so an agent sees a
  second batch that it queued. An entry of an older build has no affinity and no key, and supersedes nothing.
- **A client that a signal stops** cancels its run: SIGTERM and SIGHUP count as Ctrl+C, so a tool that kills an
  agent's call at its timeout leaves no run waiting for the turn and running for nobody. A SIGKILL cannot be caught;
  the run goes on until `brd-host cancel`.
- **`brd-host runs`** names the holder and each waiting run with their times, the first thing an agent reads before
  it starts a heavy run.
- **`brd-host slots`** marks a slot `(abandoned)` only as the agent judged it on the host and named it beside the table
  (`RepositorySlots::abandoned`): the pid of a lend is a process of the host. Until 2026-11-18 the client judged the
  pids of the host by its own processes and marked every lent slot of a live run abandoned; an agent of an older build
  names none, and its client marks none.

### The finished runs

A client forgets its run once it saw the end and every file came, so before 2026-11-16 a run left the host at its
end: `brd-host runs` showed only what ran now, and `brd-host follow` of a run a minute old answered `unknown_run`. A
forget now keeps the run without its files: the stage, the outputs and the changes go, the log keeps its last 1 MiB
from the start of an event (`KEPT_LOG`, after a line of the agent that says how many bytes went), and the spec, the end
and the phases stay. So `brd-host runs` lists the finished runs with their exit, start and duration
(`RunInfo::ended_ms`, `forgotten`), and a follow answers the tail of the output and the exit.
- **On disk, not in memory**: a run is its directory, and an upgrade or a restart of `serve` (several a day while
  the agent changes) must not lose the runs that a person looks for afterwards; the runs that go on live there already.
- **The bound**: the last 100 forgotten runs by their end (`KEPT_FORGOTTEN`), each for at most the day that an ended
  run has always had (`KEEP`), so at most 100 MiB of logs. A forget removes the runs past the count at once, and the
  hourly cleanup removes them by age. A run that ended and that nobody forgot keeps its files for its day, as before:
  its client may still fetch them, and its line says `its files kept`.
- **A forget cut short**: `forgotten` comes first, as the intent, so a run never looks unforgotten with half its files
  gone; the next forget or the hourly cleanup goes on from where it stopped, and the cut is recorded in `forgotten`
  before the cut log (`events.log.cut`) is renamed over `events.log`, so a retry cuts once. A forget holds the flock
  of `forget.lock` for its whole time, so a request and the cleanup never cut one log at once (the cleanup skips a run
  whose forget goes on), and a record of another inode than the log's is repaired.
- **The offsets of a cut log** stay those of the log before the cut: a follow reads the log that it opened, so one
  that began before the rename reads the old file to its end, and a follow of the cut log (told by the inode that
  `forgotten` records) maps the offset of a client into it and each end out of it. A client that follows again goes
  on where it was; from inside the part that went, at the note, so the reader learns that bytes are missing.
- **The end of a lost run** is the time the agent saw the loss, not the end of the run, so its `RunInfo` has no
  `ended_ms` and its line no `took`.

### The process model

- **`serve`** is the agent. launchd starts it (`com.jetbrains.brd-host`, `KeepAlive`, `AbandonProcessGroup`). It
  listens on 7742, answers one request per connection, and keeps no run in memory: a run is its directory.
- **The supervisor** is `brd-host supervise <run dir>`, one process per run, in a session of its own. `serve` starts it
  for each `Start`. It lends the slot with its own pid, so the lend lasts as long as the process. It waits for the
  pack, checks out the commit, runs the prepare steps and the command, stages the outputs, writes `exit.json` and gives
  the slot back on every way out. A restart of `serve` does not stop it. A cancel sends SIGINT to the process group of
  the command, SIGINT again a second later to each process of the group that started as it came, and SIGKILL after
  120 s (decision 9). Its stderr is `supervisor.log` of the run, its stdout the log of `serve`. A client forgets a run
  once it ended and never reads `supervisor.log`, so the lines after `exit.json` (the give-back, a quarantine, the
  heavy turn, their failures) go to the log of the agent too, with the id of the run.
  launchd never rotates that log: a run adds its start line and a few lines of its give-back, and a flood from a host
  of the network one line a minute (`tally`).
- **The keeper** is a thread of `serve`. It wakes once a minute and when a run names a repository. It brings the git
  of the host to its floor (`ensure_host_git`), takes up what a gone process left (`reclaim`, `reclaim_refs`), keeps
  the minimum free space (`keep_free`), and fills the missing slots (`fill`). A failed fill waits 1, 5, then 15
  minutes. Below `branchdeck.leaseMinFree` the relief takes the steps of a get (the trash, the trim of every pool of
  the host, the shrink, the eviction) without one, once per volume and tick (`ReliefRound`), since the repositories
  of a host share one. Those steps take free slots only, so below the minimum still each repository relieves its idle
  host slots, used longest ago first: their clean rules with keep 0, then their Bazel output bases
  (`relieve_host_slots`). A relief that leaves a volume below it waits 15 minutes, and its line names the pools and
  the slots that hold the most by `disk.json`, without a walk, and what a person can do: return leases, `brd-host trim`,
  or move a pool with `branchdeck.leasePool`. On 2026-11-18 the Studio said "nothing to delete" for hours while the
  leased slots held 400 GiB, and nobody could tell from the log what held the space.
- **Sleep.** `serve` holds the power assertions of `caffeinate -i -s -w <pid>` while a client has a connection after
  its hello, and each supervisor holds them for its whole life. So a command runs to its end without its client.

### The output of a run

The supervisor reads the stdout and the stderr of the command and writes them to `events.log` of the run as pieces
(`host_link::proto::Piece`). A piece is a line without its line end, or a part of a line: a line longer than 64 KiB is
several pieces, cut between two characters. `partial` tells that no line end comes after a piece, so a cut and a last
line without a line end add no line end. A piece that is not UTF-8 has its bytes in `base64` beside its lossy `text`.
The reader keeps every byte, a carriage return included. A follow sends each piece as `Reply::Output`, and the client
writes its bytes and, unless it is partial, a line end. So the stdout of `brd-host run` and of `brd-host follow` is the
stdout of the command, byte for byte. A line comes when it ends, and a line longer than 64 KiB comes in parts.

The stderr of a run holds every byte of the stderr of the command in its order, and the lines of brd-host. A line of
brd-host starts a line of its own: the client ends a stderr line of the command that stopped inside a line before it
writes its own line. The log of the agent ends the open line of a stream before a line of the agent, and before a piece
of another pipe. Each pipe is a source of the log (`EventLog::source`); a prepare step writes both of its pipes to the
stderr.

### Versions and the upgrade

The hello carries the protocol schema, the version of Branch Deck (`package.json`, as `brd version` reads it) and the
SHA-256 of the binary as its build id. A different schema refuses with `schema`, exit 75. A different build of the
same schema runs, and the client adds one line that names `brd-host upgrade <alias>`. The client hashes its own binary
only for a run and for `hello`, the commands that compare the builds; the hash reads the whole executable.

A change that an older end cannot read raises the schema. A new field that an older end ignores stays within it, such
as `partial` and `base64` of a piece of output: an older client writes `text` and a line end after each piece, and an
older agent sends neither field, so each of its pieces is a whole line. Only a client and an agent of this build or
later keep every byte of the output.

`brd-host upgrade <alias>` keeps the behavior of `yonder upgrade`:

1. The client sends its own executable over the link, with its size and its SHA-256. The same build goes only with
   `--force`.
2. The agent checks the size, the SHA-256 and `<staged> version` (the schema and the feature `upgrade`). A lower
   version refuses with `downgrade` unless `--force`.
3. The agent keeps the old binary as `~/.local/bin/brd-host.previous`, writes the new one as `~/.local/bin/brd-host` by
   a rename, answers, drains its requests and exits. launchd starts the new `serve`.
4. A watchdog brings back the previous binary when the new one does not listen in time (`rolled_back`).

The binary is ad-hoc signed, so each build is another code identity for macOS. Full Disk Access, which macOS asks of a
process for a volume other than the one of the system and the home, is kept for an identity: after an upgrade the agent
and its runs have none, and an `open` on such a volume waits on a consent that a `LaunchAgent` cannot show (2026-11-18
and 19: every checkout of the pools on `/Volumes/ssd` of the Studio hung for 13 to 50 minutes after each upgrade). So
the pools of a host stay on the home volume, and `upgrade` and `setup` warn of each slot below `/Volumes/`.

The agent links `lease-pool`, so the host needs no `brd` binary of the same version. The strict gates of revision 5
(an installed app only, `origin/master` only) are left out: the user installs local builds.

### The migration from yonderd

The migration ended on 2026-10-28. During it, `brd-host serve` on 7742 and yonderd on 7741 served the Studio side by
side and lent from one `host-slots.json`. `brd-host setup --import-yonder` copied the identities, the host files and
the allow list of yonder, so no new pairing was needed. `brd-host setup --retire-yonderd` then removed yonderd, its
cache and its refs `refs/yonder/*`. After that, the import, the retire, the `.yonder.toml` fallback of `[host]`, and
the `brd lease host …` and `brd host …` commands of brd were removed.

## Key Decisions

1. **Decision**: One binary for both ends, outside `brd` and the app.
   **Rationale**: The release yonder client is 3.4 MB and yonderd 3.7 MB. Linked into the app they would grow its
   binary for every user, and only a few users run a host.
2. **Decision**: `brd-host` does not depend on `bd-core`.
   **Rationale**: `bd-core` reaches the HTTP clients and the engine of the operations. The agent needs none of them.
   The refusal codes of a host run are a contract of their own, with the exits 75, 130 and 255.
3. **Decision**: A new protocol, not schema 3 of yonder.
   **Rationale**: A new port let both agents run side by side during the migration. `Pack` names its target (`{"run": …}` or
   `{"base": …}`) instead of a ref of `refs/yonder/`, and `RunSpec` has no `reference`.
4. **Decision**: The rustls provider of the app (`aws-lc-rs`), not `ring`.
   **Rationale**: The workspace links `aws-lc-rs` through reqwest already, so no second provider comes in.
5. **Decision**: A host has no app and no `brd`. The agent keeps its disk: the keeper frees space below the minimum,
   and `brd-host trim` trims the pools on request (2026-11-05).
   **Rationale**: Before, a host trimmed itself only on a `brd lease get` there, and only with `branchdeck.leaseMinFree`
   set. A host that only runs never got one, and the one way left to free its disk was `brd lease trim` there, which
   needs `brd`, the binary of the app. An agent that wanted to free the disk of the Studio upgraded `brd-host`, found
   the `brd` of an old app there, and copied the app binary over. `brd-host upgrade` replaces only the agent, so every
   function a host needs lives in the agent.
6. **Decision**: A piece of output says whether a line end comes after it (`partial`), and a piece that is not UTF-8
   carries its bytes in base64; the schema stays 1 (2026-11-13).
   **Rationale**: Each event was a line, so the cut of a line longer than 64 KiB added a line end, and bt of the IntelliJ
   monorepo could not parse the JSON on the stdout of a run. A raised schema would refuse each run of an older client
   with `schema`, and the agent of an older build refuses a binary of another schema in `brd-host upgrade`, so each host
   would need `brd-host setup` again. The optional fields keep both older ends as they were.

7. **Decision**: The heavy runs of a host take one turn, and the repository names them in `heavy` of `[host]`; a flag
   only adds a run (2026-11-14).
   **Rationale**: Overlapping heavy runs made agents fix flakes that did not exist. A flag of each run alone would be
   forgotten. A pattern list is an explicit value of the repository, not a detection from its state, and the default
   of a command it does not name is the behaviour before.
8. **Decision**: An unknown key of `[host]` names the keys of the build and the updates of both ends (2026-11-14).
   **Rationale**: Both ends refuse an unknown key, so a typo never drops a step. A new key then fails each run of an
   older end; its message says so, and a repository adds the key only after both ends know it.
9. **Decision**: A second after the SIGINT of a cancel, each process of the command group that started as it came (a
   second before it or later), but the command itself, gets SIGINT once more (2026-11-15).
   **Rationale**: A shell that gets the SIGINT between two commands (`sh -c 'trap … INT; a; b'`) runs the next one
   before its trap, and a process that it forks as the signal comes resets the handlers it had of the shell before its
   `exec`: neither sees the SIGINT, and the shell waits for it. A cancel then ended only when that process ended by
   itself, or at the SIGKILL of 120 s: 9 of 300 runs of the cancel test took 62 s under load, the `sleep 60` that the
   shell started as the cancel came. A second SIGINT to the whole group would force a tool that cleans up on the first
   one (Playwright stops at once on a second), and SIGTERM ends a shell before its trap; only a process that started as
   the SIGINT came can have missed it. A process that a trap started in that second and that still runs gets the
   SIGINT too. The starts compare in the unit of the platform (`process_scan::start_clock`: clock ticks since the boot
   on Linux), not on the wall clock, where Linux adds the ticks to `btime`, the boot in whole seconds, and would tell a
   process forked just before the SIGINT up to a second early.

## Alternatives Considered

- **Borrow only** (revision 3): brd gets `brd lease borrow`, and yonder stays as it is.
  - Why not: two release cycles, coupled by the JSON of a child, and the git of the agent stays outside the rules of
    Branch Deck.
- **`brd host` as a family of `brd`** (revision 5): one binary for everything.
  - Why not: the app binary grows, and the upgrade of a host must then replace the whole `brd`.

## Risks

- A run that is not heavy still runs beside a heavy one: a `cargo nextest run -p <crate>` compiles beside a full suite.
- The command of a killed supervisor lives on until the keeper or a waiting lend gives its slot back, up to a minute,
  beside the next heavy run.
- The holder of the turn waits for a slot of its repository while all of them are lent, up to an hour, and the heavy
  runs of the other repositories wait behind it although their slots are free. Only runs that are not heavy hold those
  slots then (one heavy run at a time), four of them at once by default, and they are short; a run that took a slot
  first and the turn after would hold a slot idle for every heavy run in the queue instead.

- A slot serves several checkouts. Ignored outputs of another branch stay, and a tool that trusts timestamps can build
  stale output. The same risk exists today within one checkout.
- A daemon that leaves the slot directory escapes the process scan of the give-back. The process group kill of the
  run catches what stays in the group.
- The port was large (about 22,000 lines with the tests). yonderd served until the migration ended, so a slow port broke
  nothing.
