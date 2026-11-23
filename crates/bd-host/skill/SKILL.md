---
name: brd-host
description: Use to run a command, a build or a test on another machine of the user with `brd-host run <host>` or `bt --host <host>`, or to wait for, cancel or fetch a run.
---

# brd-host

`brd-host` runs a command of this checkout on another machine of the user, the host. The agent on the host is
`brd-host serve`. It checks out the commit in a warm host slot of Branch Deck and runs the command there. The output
streams back, and the exit code is the one of the command. Use it for heavy work: a long test suite, flake trials with
many runs, or a measurement that needs a quiet machine.

`brd-host skill install` writes this skill into `~/.claude/skills/brd-host/`. The full documents of this build are in
`references/`: `spec.md` (the contract: every command, the files, `[host]`, the refusal codes, the report of a run)
and `design.md` (why, the slot model, the processes and the upgrade).

## Who runs what

| Command | Who |
|---|---|
| `brd-host run`, `wait`, `follow`, `runs`, `slots`, `disk`, `trim --plan`, `hello`, `cancel`, `fetch`, `forget`, `bt --host <host> …` | An agent, at any time. Cancel and forget only your own runs. |
| `brd-host upgrade <host>`, `brd-host trim <host>` | An agent, only after the user agrees: the one deploys a new agent, the other deletes worktrees, on a machine that the user shares. |
| `brd-host setup …`, `slots --shrink`, the install of Homebrew, and every `ssh` or `sudo` on the host | The user only. They use ssh or change the host. |

A run needs no ssh: each end pins the certificate of the other end, so a run works from a background session too.
Never install, configure or restart the agent by hand. Tell the user what a refusal names.

The host `local` is this machine, after the user ran `brd-host setup --local`: a run there builds in a warm host slot
of this machine instead of the checkout of the session, and no pack travels.

## Run a command

```sh
brd-host run <host> [--head] [--fetch <dir>]... [--apply] [--no-outputs] [--heavy] [--exclusive] [--nice <0-19>]
               [--repo <path>] [--affinity <key>] [--report-fd <n>] -- <command> <args>
```

- `brd-host` ships the working tree as it is: HEAD plus the tracked and untracked changes that git does not ignore, as
  one commit. Your index, files and refs stay as they are. Ignored files stay local, and the host has its own.
- `--head` runs the committed HEAD only, and refuses a checkout with local changes (`local_changes`).
- `--fetch <dir>` copies the new entries of `<dir>` back into this checkout. The path is relative to the root of the
  slot, such as `out/test-results`. Repeatable. In a lease of Branch Deck, `--fetch notes.local` brings the files into
  the notes of the lease: `notes.local` is the one link a fetch writes through.
- `--apply` brings back the changes that the command made to tracked files, such as `cargo fmt` or new snapshots, and
  applies them to the working tree, not to the index. A patch that does not apply refuses with `apply_failed` and keeps
  the patch in `<git dir>/branch-deck/host/runs/<run>.patch`.
- `--nice` is 10 by default, so the runs of `bt --host` go first. bt passes `--nice 0 --no-outputs`. A niceness below
  the one of the agent runs at the agent's, and the `running in` line names the niceness that the command got.
- `--heavy` makes a heavy run of a command that `heavy` of `[host]` does not name (see "Heavy runs").
- `--exclusive` makes a measurement that needs a quiet host, an A/B or a benchmark (see "Exclusive runs"). Never for
  an ordinary check: it stops every other run of the host while it runs.
- The run goes to the repository of the same name as this one on the host, beside the main repository of the host
  file. `--repo` names another one.
- `--report-fd <n>` writes one JSON report of the run to the descriptor `n` on every way out. stdout then holds only
  the stdout of the command, byte for byte. `brd-host schema` prints the shape of the report.
- The command gets the login environment of the user on the host, 65536 open files, and `BD_HOST_RUN`,
  `BD_HOST_AFFINITY`, `BD_HOST_COMMIT` and `BD_HOST_SLOT`. The tools come from the login `PATH` of the host.
- The command needs no login shell: it has the login environment already. A shell line goes through `zsh -c`. On
  macOS never use `bash -lc` or `sh -lc`: their `/etc/profile` puts `/usr/bin` first, so the command gets the old
  Apple Git and not the git of Homebrew. The run then writes a warning after its `running in` line.

```sh
brd-host run studio --fetch out/test-results -- zsh -c 'make test > out/test-results/log.txt'
```

A Mac host stays awake while a client talks to it and while a run goes on.

### Heavy runs

The heavy runs of a host share its cores, whatever their repository: two full test suites at once both slow down
until their tests fail on timeouts, which are no flakes. A heavy run is admitted while what its command used in its
last runs fits beside the holders and the busy cores of the host outside them (the runs that are not heavy and
processes of no run: the agent samples the whole host every 2 s); a command without a measure (its first runs, any new
command) takes the whole host and waits for a quiet one (at most a fifth of its cores busy), and so does one that
measured the whole host. The queue is first come first, and a head of the queue that waited 15 min starts on a free
turn whatever the load. The measures of a command do not depend on the paths of its `--fetch` directories, so give
each run an output directory of its own and fetch it. A line `the heavy turn of the host is locked by …; this run
keeps its place in the queue and looks again` is a wait too, never a reason to cancel. A run is heavy when a pattern of `heavy` of `[host]` of the
commit matches its command, or with `--heavy`. `bt --host` passes no flag: a bt run is heavy only once its repository
names `./community/tools/bt.cmd …` in `heavy`, and the IntelliJ monorepo decides that on its own. A heavy run says
`brd-host: a heavy run by …`, then waits for the heavy turn before it takes a slot, with a line at once and once a
minute: `brd-host: run <run> waits 3 min for the heavy turn of the host, 7.5 of its 20 cores, 117 min left: run <run>
of <repo> holds 12.0 for 4 min (<command>, measured), the host is busy with 6.3 of 20 cores outside the heavy runs, 1
run ahead: …; a wait, not a hang: …`. At the end of its command
a run says `brd-host: the command used 7.5 cores on average and 14.0 at its peak, of the 20 of the host`, which the
next runs of that command are expected to use. Never cancel and rerun it locally for it, and never
treat a test that failed on a timeout beside another heavy run as a flake to fix. `brd-host runs <host>` names the
holder and each waiting run with their times. After 2 h the run refuses with `heavy_timeout` (exit 75): never run it
locally then, as under the load of this machine its tests fail on timeouts; read `brd-host runs <host>` and run it
again later. A run that is not heavy runs at once.

### Exclusive runs

`brd-host run <host> --exclusive -- <command>` is for measurements only (an A/B pair, a benchmark, a timing that a
neighbour would spoil), never for an ordinary check or gate. It is a heavy run that waits until no other run is in its
command, of any repository, and every sample of the host of the last 30 s is quiet; its wait line says `all its 20 cores
of a quiet host, as an exclusive run`, `waits for run <run> of <repo> to end: <command>` and `the host was busy with up
to 11.2 of 20 cores in the last 30 s`. While it waits at the head of the queue for the runs to end, the new runs that
are not heavy wait for it for at most 15 min (`… waits for the host to drain (…); this run starts when the measurement
ends, or in at most 13 min`), and no heavy run starts past it. Once that drain has ended without a quiet host, the
heavy runs behind it start past it in their order while they fit (`took the heavy turn of the host after <n> s, past
the exclusive run <run> of <repo> at the head of the queue, …`); it keeps its place and starts once the host is quiet,
or ends with `heavy_timeout` after 2 h. While it holds the turn no heavy run is admitted, and each new run that is not heavy waits
before its slot with
`brd-host: run <run> waits 2 min for a measurement: run <run> of <repo> holds the host for 5 min (<command>); a
measurement holds the host; this run starts when it ends` (the step `exclusive` of its phases, at most 1 h, then
`slot_timeout`). A process of no run can still load the host: read the line `took the heavy turn of the host after
<n> s, the host busy with <x> of its 20 cores` before the numbers. An
agent of an older build refuses it before the start with `bad_config`: `brd-host upgrade <host>` after the user agrees.

One heavy run is one check: `cargo nextest run --workspace`, `pnpm lint:backend` and `pnpm e2e` go as three runs, never
as one `zsh -c '… && … && …'`, and an A/B loop of `pnpm e2e` is one run per side. A bundle holds the turn for an hour
and more, while the fast lanes and the checks of other sessions wait; between runs the turn goes round.

A heavy run that waits is superseded by a later run of the same checkout and the same command (the words it runs,
without `nice` or `time` before them): the later run says `brd-host: replaces run <run>, which waited <time> …` and takes
its place, and the waiting run ends with `superseded` (exit 75), before its lend, so nothing of it ran. Never run a
superseded run again, here or there: `brd-host follow <host> <later run>` follows the one that replaced it. A run that
holds the turn is never superseded. `brd-host: run <run> of this checkout also waits for the heavy turn, …: <command>`
says that your checkout queued another command: cancel the one you no longer need.

A heavy run can wait long, so start it in the background (`run_in_background` of a coding agent), keep its run id from
its first line, and wait for it with `brd-host wait <host> <run>` (see "Wait for a run"), rather than in a call that a
tool kills at its timeout. SIGTERM and SIGHUP cancel the run on the host as Ctrl+C does; SIGKILL leaves it going:
`brd-host cancel <host> <run>` then.

`bt --host <host> <the usual bt args>` runs a test of the IntelliJ monorepo the same way. It ships the working tree,
HEAD plus the local changes as one commit, as `brd-host run` does. The new run directories of `out/bt-runs` come back.

### The lines of a run

- `brd-host: run <run> of <commit> on <host>` comes first. Keep the run id: `wait`, `follow`, `cancel` and `fetch` need it.
- `brd-host: running in <slot> (slot <n>): load …, nice …, … open files, git <version>, …` comes right before the
  command.
- `brd-host: phases: …` comes at the end. Read it before you guess where the time went.

All the lines of `brd-host` and the stderr of the command go to stderr. stdout is only the stdout of the command, byte for
byte: a long line, a carriage return and a last line without a line end arrive as the command wrote them. An agent of
an older build does not keep every byte: it cuts a line longer than 64 KiB, drops a carriage return and ends a last
line. `brd-host upgrade <host>` replaces it after the user agrees.

| Step | Where | A long step means |
|---|---|---|
| `pack`, `upload` | here | The pack of the missing commits is large: the host is far behind this history. |
| `heavy` | host | Other heavy runs of the host went first, or the host was busy outside them. |
| `exclusive` | host | A run that is not heavy waited for a measurement (`--exclusive`) that held the host. |
| `git` | host | Branch Deck upgraded the git of the host with Homebrew, and the run waited for it. |
| `slot` | host | Every slot was lent, and the run waited in the queue. |
| `pack_wait` | host | The slot was ready before the pack came. |
| `checkout` | host | A diff with many files, or a forced checkout: the log then says why. Minutes for a small diff: the volume of the slot hangs, and every git on it waits. |
| `prepare` | host | A `[[host.prepare]]` step of `.branchdeck.toml` ran, because its inputs changed. |
| `command` | host | The command itself. |
| `measure` | host | The cores of the command were measured outside it: a look at the Bazel servers of the slot before it, the last sample after it. Minutes: the volume of the slot or of a Bazel output base hangs. |
| `stage`, `fetch` | host, here | The outputs and the fetched files are large. |
| `total` | here | The whole call. |

`brd-host runs <host>` lists the runs that the host keeps, each with its steps of 0.1 s or more, then the heavy turn:
who holds it and who waits; a run that a refusal ended says `finished, refused <code>` and its message on the next line.
`brd-host disk [<host>]` says what the lease pools of a host hold on disk (every host without one). `brd-host slots <host>`
shows the lend table of the host: each slot, its state, its run and the queue. `(abandoned)` marks a slot whose run is
gone by the processes of the host, as its agent judged them; an agent of an older build marks none.

## The disk of a host

A host has the agent only: no Branch Deck app and no `brd` binary. Never install, copy or link either there, and never run
`brd lease` on a host through `brd-host run`. Below `branchdeck.leaseMinFree` of the host the agent frees space by
itself, once a minute: the trash, the free slots past `branchdeck.leaseMaxSlots`, then the build output and the free
slots below the limit, as a `brd lease get` does. `brd-host trim <host> [--max <n>] [--plan]` deletes the free slots past
the limit now, as `brd lease trim` does there; `brd-host disk <host>` shows what a trim deletes next. Ctrl+C, SIGTERM or SIGHUP
ends `trim` and `upgrade` at once (130); the host goes on with the request it has. An agent without
the feature `trim` names `brd-host upgrade <host>`: ask the user for it.

## Exit codes

| Exit | Meaning | What to do |
|---|---|---|
| The code of the command | The command ran to its end. | Read the output as for a local run. |
| 75 | The host did not run the command: a refusal before the start, such as `unreachable`, `behind` or `slot_timeout`. | Run the command locally. |
| 75 with `heavy_timeout` | Other heavy runs held the heavy turn for 2 hours. | Never locally: `brd-host runs <host>`, then run it again later. |
| 75 with `superseded` | A later run of your checkout and command took its place before it started: nothing ran. | Never run it again: `brd-host follow <host> <later run>`, which the message names. |
| 124 with `still_running` | `brd-host wait` passed its `--max`; the run goes on, and nothing was cancelled. | `brd-host wait <host> <run>` again. A command that itself exits 124 has no `still_running` line. |
| 130 | Ctrl+C before the command, or a second Ctrl+C. | Before the command, nothing ran. After a second Ctrl+C, the run goes on: `brd-host follow <host> <run>` shows its end. |
| 255 with `unknown_run` | The run started and is gone on the host. Whether its command ran is unknown. | Never rerun it locally on your own. Tell the user. |
| 255, any other | The link broke after the start, the fetch or the changes failed, or a request other than a run failed. | Read the message. A lost run goes on: `brd-host follow`. |
| 2 | A usage error. | Fix the command line. |

`brd-host schema` prints every refusal code with its exit code. Act on the code, never on the message.

## Wait for a run

`brd-host wait <host> <run> [--max <duration>]` waits for the end of a run in one call of at most `--max` (9 min by
default, `90s`, `9m`, `1h30m`), under the timeout of your tool. Run it in the checkout of the run.

- While it waits it says only one line per change of the state of the run, with the time: `brd-host: 14:02:11 run <run>
  waits for the heavy turn of studio, 1 run ahead: …`, `… checks out its commit and prepares on studio`, `… runs its
  command on studio`. Never filter its output, and never poll `brd-host runs` or a marker file beside it.
- At the end it copies the files of the run, applies its changes and forgets it, as `run` does, puts the whole output
  into `<git dir>/branch-deck/host/runs/<run>.log`, and says a summary: the exit, the phases, the copied files, the
  path of the log with its line count, and its last 30 lines. Read the log with your tools for more. It exits with the
  exit code of the command.
- Past `--max` it exits 124 with `still_running` and the state, and the run goes on: run the same `wait` again. Never
  wrap it in `timeout`. A host that did not answer the whole time is `timed_out` (255): the run can go on there.
- A `wait` after the end answers again at once from what the first one wrote. While the `run` of the run still goes on
  in the background, that process finishes it, and a `wait` says `still_running` until it did.
- Ctrl+C, SIGTERM or SIGHUP leaves a wait (130), and the run goes on.

## A lost link, follow and cancel

The run lives on the host, not on the connection. A lost link connects again, with waits from 1 s up to 30 s, and the
output follows from the last line it showed. After 10 min without a reply the client exits with 255 and names the
command that follows the run.

- `brd-host follow <host> <run>` follows a run from its first line. Ctrl+C, SIGTERM or SIGHUP leaves at once (130), and the
  run goes on, so stopping a follow in the background is safe.
- `brd-host cancel <host> <run>` asks the host to interrupt a run. `follow` shows its end.
- Ctrl+C in `brd-host run` cancels the run on the host and shows the output to its end. A second Ctrl+C leaves at once.
- `brd-host fetch <host> <run> [--fetch <dir>]` copies the files of a run whose copy failed (`fetch_failed`). Run it in
  the checkout of the run: the record of the run there says which files come. Without the record, only `--fetch`
  directories come.
- `brd-host forget <host> <run>` removes the files of an ended run on the host; `run` does it at the end of each run.
  The host keeps the end and the last 1 MiB of the log of the last 100 forgotten runs, each a day at most, and an ended
  run that nobody forgot with its files for a day.
- `brd-host runs <host>` lists the runs that go on and the finished ones: `finished, exit N, started <time>, took
  <duration>`. `brd-host follow <host> <run>` of a finished run shows what the host keeps of its log and exits with its
  exit code.

## The host slots

The host keeps a fixed set of warm slots per repository (`branchdeck.hostSlots` of its git config, 4 by default). Each
run borrows one for that run only. A run gets the slot that its affinity used last, else the one used longest ago,
else it waits in a queue with one line a minute. After an hour it refuses with `slot_timeout`. At the end of each run
the slot comes back: the processes of the run stop, and its changes go. The ignored files stay as the warm cache, until
the slot has been idle 6 hours (`branchdeck.hostSlotIdleClean`): then the keeper applies the clean rules. So
nothing keeps or returns a slot by hand. The agent does the lend and the git of the host in process, so the host needs
no `brd` binary.

## Move commits, not trees

Code reaches the host only as a thin pack of the commits that the host lacks. Never rsync or scp a working tree, and
never push to the host by hand. Bring files back with `--fetch`, the outputs of `[host]`, or `--apply`.

A host that has none of the commits near HEAD refuses with `behind`, because the pack would hold the whole history.
Only the user moves the host, such as with `bt --host <host> --prepare` in the IntelliJ monorepo.

## `[host]` of `.branchdeck.toml`

A repository asks the host for more with the table `[host]` of `.branchdeck.toml` in its root. A commit without that
table has no prepare steps, no environment of its own and no outputs. The host reads the file from the commit of the
run. An unknown key or a value of the wrong shape ends the run with `bad_config` before the command; the message of
an unknown key says when it can be a key of a later Branch Deck, and then the user updates brd-host on both ends. A
step that fails ends it with `prepare_failed`.

```toml
[host]
# Paths that the command makes and that come back to this checkout. The host removes each one before the command.
outputs = ["tests/results", "playwright-report"]
# The commands of the heavy runs, which share the cores of the host by their measure: `*` matches any characters.
heavy = ["cargo nextest run*--workspace*", "pnpm e2e*"]

[host.env]
RUST_BACKTRACE = "1"                   # a text, as it is
PATH = { prepend = ["~/.cargo/bin"] }  # also `append`; a relative entry is below the slot root
E2E_PORT = { port = 3100 }             # 3100 plus the slot number, or the next free TCP port
CI = { unset = true }                  # removed

# Runs before the command when its inputs changed since its last success in the slot, or a path of `creates` is missing.
[[host.prepare]]
name = "pnpm install"
run = ["pnpm", "install", "--frozen-lockfile"]
inputs = ["pnpm-lock.yaml"]
creates = ["node_modules"]
```

`[host]` installs no tool on the host. The host provides the tools on its login `PATH`.

## Refusals

A refusal says `brd-host: <code>: <message>` on stderr. Give the message to the user when the cause is on the host.

| Code | Meaning |
|---|---|
| `unreachable` | No address answered. The host can sleep or be off. |
| `behind` | The host lacks the history near HEAD. The user moves the host. |
| `untrusted` | The agent identity changed, or the agent does not allow this client. The user runs `brd-host setup <host>`. |
| `no_host` | This machine has no host file for the alias: the host was never set up from here. |
| `schema` | The agent speaks another protocol. Ask the user for `brd-host upgrade <host>`. |
| `lease_failed`, `slot_timeout` | The host lent no slot. Exit 75: run locally, or try again later. |
| `heavy_timeout` | Other heavy runs held the heavy turn for 2 hours. Exit 75, but never run it locally: `brd-host runs <host>`, then run it again later. |
| `superseded` | A later heavy run of the same checkout and command replaced it while it waited: nothing ran. Exit 75, but never run it again: follow the later run that the message names. |
| `local_changes` | `--head` with uncommitted or untracked changes. Commit first, or run without `--head`. |
| `git_failed` | A local git call failed: the snapshot, the candidates or the pack. Exit 75: run locally. |
| `old_git` | The git of the host is below the floor of Branch Deck, and Homebrew cannot upgrade it. Exit 75. Give the message to the user. |

## The Local Network permission on macOS

macOS decides a connection to the local network by the responsible app of the process. An app without the permission
gets `No route to host` for each child. The client handles this itself: after a refused connect it connects through
`/usr/bin/nc`, which it starts responsible for itself. TLS stays end to end.

- A plain `nc` or `ping` from such an app fails too, so it does not tell the permission from the network.
- When the relay cannot pass the permission, `unreachable` says `macOS Local Network privacy can refuse …`. Without
  that sentence, `No route to host` is the network: the host sleeps or is off. Tell the user. Do not work around it.

## The upgrade

`brd-host upgrade <host>` sends this `brd-host` binary to the agent of a macOS host over the link, without ssh. Run it
only after the user agrees. `--binary <path>` sends another binary. `--force` sends the same build again, or a lower
version. A watchdog on the host brings back the previous binary when the new one does not listen (`rolled_back`). Runs
in flight go on. `brd-host: <host> runs another build of brd-host (…); runs work across builds, so this is no error: …`
is a hint, not a failure: go on, and ask the user before an upgrade.

`brd-host: warning: slots on <host> are on another volume than the home of the agent: …` after an upgrade or a setup
means: macOS keeps Full Disk Access for the code identity of a binary, and the ad-hoc signed agent is a new identity
after each upgrade, so its runs in those slots hang in their checkout. Tell the user: the pools go to the home volume, or
the agent gets Full Disk Access again in System Settings on the host.

`brd-host upgrade` also replaces this skill in `~/.claude/skills/brd-host/` when the installed copy is unedited and comes
from another build. An edited copy stays: `brd-host skill install --force` replaces it.

## The files

- Here: `~/.config/brd-host/identity.crt` and `identity.key`, and one host file per alias in `hosts/<alias>.json`.
- On the host: `~/.cache/brd-host/runs/<run>/` (`events.log`, `exit.json`, `supervisor.log`) and, on macOS,
  `~/Library/Logs/brd-host.log`. Read them only through the user or a run, never over ssh on your own.
