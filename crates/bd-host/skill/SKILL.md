---
name: bd-host
description: Use to run a command, a build or a test on another machine of the user with `bd-host run <host>` or `bt --host <host>`, or to follow, cancel or fetch a run.
---

# bd-host

`bd-host` runs a command of this checkout on another machine of the user, the host. The agent on the host is
`bd-host serve`. It checks out the commit in a warm host slot of Branch Deck and runs the command there. The output
streams back, and the exit code is the one of the command. Use it for heavy work: a long test suite, flake trials with
many runs, or a measurement that needs a quiet machine.

`bd-host skill install` writes this skill into `~/.claude/skills/bd-host/`. The full documents of this build are in
`references/`: `spec.md` (the contract: every command, the files, `[host]`, the refusal codes, the report of a run)
and `design.md` (why, the slot model, the processes, the upgrade and the migration from yonder).

## Who runs what

| Command | Who |
|---|---|
| `bd-host run`, `follow`, `runs`, `slots`, `hello`, `cancel`, `fetch`, `forget`, `bt --host <host> …` | An agent, at any time. Cancel and forget only your own runs. |
| `bd-host upgrade <host>` | An agent, only after the user agrees: it deploys a new agent to a machine that the user shares. |
| `bd-host setup …`, `slots --shrink`, the install of Homebrew, and every `ssh` or `sudo` on the host | The user only. They use ssh or change the host. |

A run needs no ssh: each end pins the certificate of the other end, so a run works from a background session too.
Never install, configure or restart the agent by hand. Tell the user what a refusal names.

## Run a command

```sh
bd-host run <host> [--head] [--fetch <dir>]... [--apply] [--no-outputs] [--nice <0-19>] [--repo <path>]
               [--affinity <key>] [--report-fd <n>] -- <command> <args>
```

- `bd-host` ships the working tree as it is: HEAD plus the tracked and untracked changes that git does not ignore, as
  one commit. Your index, files and refs stay as they are. Ignored files stay local, and the host has its own.
- `--head` runs the committed HEAD only, and refuses a checkout with local changes (`local_changes`).
- `--fetch <dir>` copies the new entries of `<dir>` back into this checkout. The path is relative to the root of the
  slot, such as `out/test-results`. Repeatable.
- `--apply` brings back the changes that the command made to tracked files, such as `cargo fmt` or new snapshots, and
  applies them to the working tree, not to the index. A patch that does not apply refuses with `apply_failed` and keeps
  the patch in `<git dir>/branch-deck/host/runs/<run>.patch`.
- `--nice` is 10 by default, so the runs of `bt --host` go first.
- The run goes to the repository of the same name as this one on the host, beside the main repository of the host
  file. `--repo` names another one.
- `--report-fd <n>` writes one JSON report of the run to the descriptor `n` on every way out. stdout then holds only
  the stdout of the command. `bd-host schema` prints the shape of the report.
- The command gets the login environment of the user on the host, 65536 open files, and `BD_HOST_RUN`,
  `BD_HOST_AFFINITY`, `BD_HOST_COMMIT` and `BD_HOST_SLOT`. The tools come from the login `PATH` of the host.
- The command needs no login shell: it has the login environment already. A shell line goes through `zsh -c`. On
  macOS never use `bash -lc` or `sh -lc`: their `/etc/profile` puts `/usr/bin` first, so the command gets the old
  Apple Git and not the git of Homebrew. The run then writes a warning after its `running in` line.

```sh
bd-host run studio --fetch out/test-results -- zsh -c 'make test > out/test-results/log.txt'
```

A Mac host stays awake while a client talks to it and while a run goes on.

`bt --host <host> <the usual bt args>` runs a test of the IntelliJ monorepo the same way. It runs the committed HEAD
only and refuses a dirty tree. The new run directories of `out/bt-runs` come back.

### The lines of a run

- `bd-host: run <run> of <commit> on <host>` comes first. Keep the run id: `follow`, `cancel` and `fetch` need it.
- `bd-host: running in <slot> (slot <n>): load …, nice …, … open files, git <version>, …` comes right before the
  command.
- `bd-host: phases: …` comes at the end. Read it before you guess where the time went.

All the lines of `bd-host` and the stderr of the command go to stderr. stdout is only the stdout of the command.

| Step | Where | A long step means |
|---|---|---|
| `pack`, `upload` | here | The pack of the missing commits is large: the host is far behind this history. |
| `git` | host | Branch Deck upgraded the git of the host with Homebrew, and the run waited for it. |
| `slot` | host | Every slot was lent, and the run waited in the queue. |
| `pack_wait` | host | The slot was ready before the pack came. |
| `checkout` | host | A diff with many files, or a forced checkout. The log then says why. |
| `prepare` | host | A `[[host.prepare]]` step of `.branchdeck.toml` ran, because its inputs changed. |
| `command` | host | The command itself. |
| `stage`, `fetch` | host, here | The outputs and the fetched files are large. |
| `total` | here | The whole call. |

`bd-host runs <host>` lists the runs that the host keeps, each with its steps of 0.1 s or more. `bd-host slots <host>`
shows the lend table of the host: each slot, its state, its run and the queue.

## Exit codes

| Exit | Meaning | What to do |
|---|---|---|
| The code of the command | The command ran to its end. | Read the output as for a local run. |
| 75 | The host did not run the command: a refusal before the start, such as `unreachable`, `behind` or `slot_timeout`. | Run the command locally. |
| 130 | Ctrl+C before the command, or a second Ctrl+C. | Before the command, nothing ran. After a second Ctrl+C, the run goes on: `bd-host follow <host> <run>` shows its end. |
| 255 with `unknown_run` | The run started and is gone on the host. Whether its command ran is unknown. | Never rerun it locally on your own. Tell the user. |
| 255, any other | The link broke after the start, the fetch or the changes failed, or a request other than a run failed. | Read the message. A lost run goes on: `bd-host follow`. |
| 2 | A usage error. | Fix the command line. |

`bd-host schema` prints every refusal code with its exit code. Act on the code, never on the message.

## A lost link, follow and cancel

The run lives on the host, not on the connection. A lost link connects again, with waits from 1 s up to 30 s, and the
output follows from the last line it showed. After 10 min without a reply the client exits with 255 and names the
command that follows the run.

- `bd-host follow <host> <run>` follows a run from its first line. Ctrl+C leaves, and the run goes on.
- `bd-host cancel <host> <run>` asks the host to interrupt a run. `follow` shows its end.
- Ctrl+C in `bd-host run` cancels the run on the host and shows the output to its end. A second Ctrl+C leaves at once.
- `bd-host fetch <host> <run> [--fetch <dir>]` copies the files of a run whose copy failed (`fetch_failed`). Run it in
  the checkout of the run: the record of the run there says which files come. Without the record, only `--fetch`
  directories come.
- `bd-host forget <host> <run>` removes a run and its files on the host. The host keeps an ended run for 24 h.

## The host slots

The host keeps a fixed set of warm slots per repository (`branchdeck.hostSlots` of its git config, 4 by default). Each
run borrows one for that run only. A run gets the slot that its affinity used last, else the one used longest ago,
else it waits in a queue with one line a minute. After an hour it refuses with `slot_timeout`. At the end of each run
the slot comes back: the processes of the run stop, and its changes go. The ignored files stay as the warm cache. So
nothing keeps or returns a slot by hand. The agent does the lend and the git of the host in process, so the host needs
no `bd` binary.

## Move commits, not trees

Code reaches the host only as a thin pack of the commits that the host lacks. Never rsync or scp a working tree, and
never push to the host by hand. Bring files back with `--fetch`, the outputs of `[host]`, or `--apply`.

A host that has none of the commits near HEAD refuses with `behind`, because the pack would hold the whole history.
Only the user moves the host, such as with `bt --host <host> --prepare` in the IntelliJ monorepo.

## `[host]` of `.branchdeck.toml`

A repository asks the host for more with the table `[host]` of `.branchdeck.toml` in its root. A commit without that
table gets its `.yonder.toml`. The host reads the file from the commit of the run. An unknown key or a value of the
wrong shape ends the run with `bad_config` before the command. A step that fails ends it with `prepare_failed`.

```toml
[host]
# Paths that the command makes and that come back to this checkout. The host removes each one before the command.
outputs = ["tests/results", "playwright-report"]

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

A refusal says `bd-host: <code>: <message>` on stderr. Give the message to the user when the cause is on the host.

| Code | Meaning |
|---|---|
| `unreachable` | No address answered. The host can sleep or be off. |
| `behind` | The host lacks the history near HEAD. The user moves the host. |
| `untrusted` | The agent identity changed, or the agent does not allow this client. The user runs `bd-host setup <host>`. |
| `no_host` | This machine has no host file for the alias: the host was never set up from here. |
| `schema` | The agent speaks another protocol. Ask the user for `bd-host upgrade <host>`. |
| `lease_failed`, `slot_timeout` | The host lent no slot. Exit 75: run locally, or try again later. |
| `local_changes` | `--head` or `bt --host` with uncommitted or untracked changes. Commit first. |
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

`bd-host upgrade <host>` sends this `bd-host` binary to the agent of a macOS host over the link, without ssh. Run it
only after the user agrees. `--binary <path>` sends another binary. `--force` sends the same build again, or a lower
version. A watchdog on the host brings back the previous binary when the new one does not listen (`rolled_back`). Runs
in flight go on.

`bd-host upgrade` also replaces this skill in `~/.claude/skills/bd-host/` when the installed copy is unedited and comes
from another build. An edited copy stays: `bd-host skill install --force` replaces it.

## The files

- Here: `~/.config/bd-host/identity.crt` and `identity.key`, and one host file per alias in `hosts/<alias>.json`.
- On the host: `~/.cache/bd-host/runs/<run>/` (`events.log`, `exit.json`, `supervisor.log`) and, on macOS,
  `~/Library/Logs/bd-host.log`. Read them only through the user or a run, never over ssh on your own.
