# Proposal

## Why

An agent cannot return its own lease while one of its own background commands still runs in the slot (#238). Claude
Code runs each tool call as a shell in a session of its own below `claude`; a Monitor (`tail -F log | grep x`) or a
command run in the background is then no ancestor of `brd` and in no process group of one, so `brd lease return`
refuses as `slot_busy` and names the command a process of another session, which the skill says never to kill.

## What Changes

- A process in the slot that runs below the holder process of the lease (`holder_pid` and `holder_started`), seen by
  a `brd` that the holder runs, in a tree whose root has no controlling terminal, is a command of the holder:
  `slot_busy` names it `kind: holder`, and the message names `--stop-own` with its condition. A shell of a terminal
  tab has a controlling terminal and stays `other`, as does every process of another session.
- `brd lease return --stop-own` ends each tree of the commands of the holder in the slot once nothing else refuses
  the return, then checks the slot again without them; it never ends a process of another session, and names what it
  ended in `stopped_processes`.
- The process scan reads whether a process has a controlling terminal (`ProcessInfo::terminal`), and the kill of a
  tree takes a running process that another process started (`process_tree::kill_running_tree`).

Not in it: a resumed session runs as another process than the recorded holder, so its commands stay `other`.

Capabilities: `cli/lease`.
