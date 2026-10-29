# Runs on Another Mac (`bd-host`)

**Updated:** 2026-10-29  
**Tags:** [bd-host, host, remote-run, setup]

`bd-host` runs a command of your checkout on another Mac of yours, the **host**: a long test suite, flake trials with
many runs, a measurement that needs a quiet machine. It sends the working tree as it is, committed or not, runs the
command there in a warm copy of the repository, streams the output back and exits with the exit code of the command.
Your index, files and branches stay as they are.

This guide sets up a host and walks through the first runs. The exact contract of every command is in the
[bd-host spec](../../spec/cli/host.spec.md), the why in [Host Runs](../design/host-runs.md).

## How it works

```text
 This Mac                                          The host
+------------------------------------+          +------------------------------------------+
| bd-host run studio -- <command>    |          | launchd: com.jetbrains.bd-host           |
|   |                                |          |   |                                      |
|   | 1. the working tree as one     |  TLS 1.3 |   v                                      |
|   |    commit (nothing changes     |  port    | bd-host serve ---- keeper, every minute: |
|   |    in your checkout)           |  7742    |   |                fills the slots,      |
|   | 2. a thin pack of the commits  | -------> |   |                upgrades git)         |
|   |    the host lacks              |          |   v                                      |
|   |                                |          | bd-host supervise, one per run           |
|   | 3. the output, line by line    | <------- |   |                                      |
|   | 4. the exit code               |          |   v                                      |
|   | 5. --fetch dirs, [host]        |          | slot 1 | slot 2 | slot 3 | slot 4        |
|   |    outputs, --apply patch      |          | (warm worktrees of the repository)       |
+------------------------------------+          +------------------------------------------+
```

- **One binary, both ends.** `bd-host` is the client on this Mac and the agent on the host. The app bundle carries it.
- **No ssh for a run.** Each end pins the certificate of the other end. ssh is needed once, for the setup.
- **Commits, not files.** Only the commits that the host does not have travel. A second run of the same tree sends
  nothing.
- **The run lives on the host.** A lost link connects again and the output goes on from where it stopped. The host
  stays awake while a run goes on.

## What you need

On this Mac:

- Branch Deck for macOS, and its agent CLI: Help → Install Agent CLI…, or `bd install`. It links
  `~/.local/bin/bd-host` to the binary in the app bundle. The Windows app has no bd-host.
- An ssh alias of the host that logs in without a prompt, with a key in the ssh agent. Check it with
  `ssh -o BatchMode=yes studio true`. Only the setup uses it.

On the host, a Mac (on Linux the agent runs only by hand, `bd-host serve`, without `upgrade`):

- [Homebrew](https://brew.sh). The agent keeps the git of Homebrew at the version that Branch Deck needs.
- A clone of each repository that you run there, under the same directory name as here, side by side, for example
  `~/projects/branch-deck`. A run goes to the clone of the same name beside the main repository of the setup
  (`--repo`).
- The tools of your commands (cargo, pnpm, a JDK) on the login `PATH` of your user. A run gets the login environment.
- You logged in. The agent is a LaunchAgent of your user: it starts at your login and runs while you are logged in.

## Set up a host

Run this on this Mac. `studio` is the ssh alias, and `--repo` is a repository that the host has:

```sh
bd-host setup studio --repo ~/projects/branch-deck
bd-host hello studio
```

What `setup` does:

```text
  This Mac                                   The host
  bd-host setup studio                       (ssh, this one time)
        |                                          |
        |--- ssh: shasum ~/.local/bin/bd-host ---->|
        |<-- the build there ----------------------|
        |                                          |
        |--- ssh: copy the binary ---------------->|  ~/.local/bin/bd-host
        |    (only when the build differs)         |
        |                                          |
        |--- ssh: bd-host setup --agent ---------->|  ~/.config/bd-host/agent/identity.*  (once)
        |         --client <this Mac>              |  ~/.config/bd-host/agent/clients     (+1 line)
        |                                          |  the LaunchAgent com.jetbrains.bd-host
        |                                          |  launchctl bootstrap gui/<uid>
        |                                          |      `-> bd-host serve, port 7742
        |<-- {"fingerprint", "port"} --------------|
        |                                          |
  ~/.config/bd-host/hosts/studio.json              |
  (the fingerprint, the addresses, --repo)         |
        |                                          |
        |=== hello over TLS, no ssh ==============>|
        |<== the version and the build ============|
```

`bd-host hello studio` prints the version and the build of both ends. `same_build: false` is fine: the run goes on
and says once to run `bd-host upgrade studio`.

Setup again whenever you like: it keeps the identity of the host and adds this Mac to its list. Another Mac of yours
becomes a client the same way, from that Mac.

## Tell the host what a repository needs

A repository asks the host for more in the `[host]` table of `.branchdeck.toml` in its root. The host reads it from
the commit of the run, so it changes with the branch. The table of Branch Deck itself, a little shortened:

```toml
[host]
# What a run makes and brings back into the same paths here
outputs = ["tests/results", "playwright-report", "target/nextest/ci/junit.xml"]

[host.env]
# one port per slot, so two runs never meet
E2E_PORT = { port = 3100 }

[[host.prepare]]
name = "pnpm install"
run = ["pnpm", "install", "--frozen-lockfile"]
inputs = ["package.json", "pnpm-lock.yaml"]
creates = ["node_modules"]
```

A prepare step runs only when its `inputs` changed since its last success in that slot, or a path of `creates` is
missing. `[host]` installs no tool: the host has its own. Every key is in the spec, "The environment of a run and
`[host]`".

## Run a command

```sh
bd-host run studio -- cargo nextest run --workspace
bd-host run studio --fetch out/test-results -- zsh -c 'make test > out/test-results/log.txt'
bd-host run studio --apply -- cargo fmt
bd-host run studio --head -- pnpm e2e
```

- `--fetch <dir>` copies the new files of `<dir>` of the slot back here. Repeatable.
- `--apply` brings back what the command changed in tracked files (`cargo fmt`, new snapshots) and applies it to your
  working tree, not to the index.
- `--head` sends the committed HEAD only, and refuses a checkout with local changes.
- A shell line goes through `zsh -c`. Never use `bash -lc` on macOS: its profile puts the old git of Apple first.

stdout holds only the stdout of the command. bd-host writes its own lines to stderr:

```text
bd-host: run 01M63T… of 5a47e402 on studio          <- keep the run id
bd-host: running in …/branch-deck (slot 2): load 3.1 on 20 cores, nice 10, 65536 open files, …
… the output of the command …
bd-host: phases: pack 0.1s, slot 0.0s, checkout 0.3s, command 3m58s, fetch 0.2s, total 4m00s
```

The `phases` line says where the time went: a long `pack` means the host is far behind, a long `slot` that every slot
was busy, a long `prepare` that a prepare step ran.

| Exit | Meaning | What to do |
|---|---|---|
| the code of the command | The command ran to its end. | Read the output as for a local run. |
| 75 | The host did not run the command: asleep, behind, all slots busy for an hour. | Run it here. |
| 130 | Ctrl+C before the command started, or a second Ctrl+C. | Before the start nothing ran. After a second Ctrl+C the run goes on: `bd-host follow` shows its end. |
| 255 | The link broke after the start, or a fetch failed. | The run goes on: `bd-host follow studio <run>`. |

The first Ctrl+C cancels the run on the host and shows the output to its end. `bd-host follow`, `cancel`, `fetch`
and `forget` take the run id. The host keeps an ended run for 24 h.

## The slots

The host keeps four warm worktrees of each repository (`git config branchdeck.hostSlots` on the host). Each run
borrows one for its own time only, so there is nothing to keep or give back:

```text
 idle: the ignored files stay as the cache (target/, node_modules/) <--------+
   |                                                                         |
   | a run comes and takes 1. the slot this checkout used last               |
   |                       2. else the one idle the longest                  |
   |                       3. else it waits in a queue, at most 1 h          |
   v                                                                         |
 lent to that run only                                                       |
   |                                                                         |
   v                                                                         |
 checkout of the commit, by the diff                                         |
   |                                                                         |
   v                                                                         |
 [[host.prepare]] steps, when their inputs changed                           |
   |                                                                         |
   v                                                                         |
 the command                                                                 |
   |                                                                         |
   v                                                                         |
 give-back: stop the processes of the run, drop its changes -----------------+
```

- `bd-host slots studio` shows each slot, its state, its run and the queue.
- `bd-host runs studio` lists the runs that the host keeps, with the time of each step.
- `bd-host slots --local -C <repo> --shrink <n>` on the host gives the slots past `n` back and keeps `n` as the limit.

## Upgrade the host

```sh
bd-host upgrade studio
```

It sends the `bd-host` of this Mac to the host over the link, without ssh. The host keeps the previous binary as
`~/.local/bin/bd-host.previous`, and when the new one does not start listening, it brings the previous one back and
the command says `rolled_back`. Runs in flight go on. `--force` sends the same build again, or an older one.

`bd-host skill install` writes the agent skill of this build to `~/.claude/skills/bd-host/`, so a coding agent knows
how to use the host. `upgrade` refreshes a copy that you did not edit.

## When a run is refused

A refusal prints `bd-host: <code>: <message>` and exits 75 before the command, or 255 after its start.
`bd-host schema` lists every code.

| Code | What it means | What to do |
|---|---|---|
| `unreachable` | No address of the host answered. | Wake the host or turn it on. When the message names macOS Local Network privacy, give the app that runs bd-host (the terminal, the IDE) the permission in System Settings → Privacy & Security → Local Network. |
| `untrusted` | The host has another identity, or does not know this Mac. | `bd-host setup studio` again. |
| `no_host` | This Mac has no host file for the alias. | `bd-host setup studio`. |
| `behind` | The host has none of the commits near your HEAD. | `git fetch` in the clone of the repository on the host. |
| `schema` | The agent speaks another protocol. | `bd-host upgrade studio`, or setup again. |
| `old_git` | The git of the host is too old and Homebrew cannot upgrade it. | Install or repair Homebrew on the host. |
| `slot_timeout` | Every slot stayed busy for an hour. | `bd-host slots studio`, then try again. |
| `local_changes` | `--head` with uncommitted changes. | Commit, or drop `--head`. |

## The files

| Where | File | What |
|---|---|---|
| This Mac | `~/.config/bd-host/identity.crt`, `identity.key` | The identity of this client |
| This Mac | `~/.config/bd-host/hosts/<alias>.json` | The fingerprint, the addresses and the main repository of a host |
| This Mac | `<git dir>/branch-deck/host/runs/` | The record of each run and the patch of `--apply`, for 7 days |
| Host | `~/.local/bin/bd-host` | The binary that launchd runs |
| Host | `~/.config/bd-host/agent/` | The identity of the agent and `clients`, one line per allowed Mac |
| Host | `~/Library/LaunchAgents/com.jetbrains.bd-host.plist` | The LaunchAgent |
| Host | `~/Library/Logs/bd-host.log` | The log of the agent |
| Host | `~/.cache/bd-host/runs/<run>/` | The events, the exit and the outputs of each run, for 24 h |

## Remove a host

On the host:

```sh
launchctl bootout gui/$(id -u)/com.jetbrains.bd-host
rm ~/Library/LaunchAgents/com.jetbrains.bd-host.plist ~/.local/bin/bd-host
```

To remove only one Mac, delete its line from `~/.config/bd-host/agent/clients`: the agent reads the list at each
connection. On that Mac, delete `~/.config/bd-host/hosts/<alias>.json`.
