# Proposal

## Why

On 2026-11-18 the heavy turn of the Studio was the main wait of every session: one `zsh -c` with unit tests, four A/B
runs of `pnpm e2e`, the full E2E, WebKit, perf, lint and nextest held it for 1 h 19 min, and the same session queued
that batch again on every new commit (runs 29519, 48712 of one checkout), so each copy would have run in its turn.
`brd-host slots studio` marked every lent slot `(abandoned)`: the client judged the pids of the host by its own
processes. And agents read only the output, so the lines of a wait, of another build and of a keeper that cannot free
the disk have to say what happened and what to do.

## What Changes

- A heavy run new to the queue supersedes the runs that wait there with its checkout (affinity) and command by what it
  runs (`host_link::heavy::run_key`); each ends with the new refusal `superseded` (exit 75) before its lend, and its
  message names the later run and `brd-host follow <host> <later run>`. The holder is never superseded. The later run
  says whom it replaced and which other runs of its checkout wait. `heavy.json` gets the optional `affinity`,
  `command_key` and `superseded`, within its schema.
- `Reply::Slots` names the slots that the agent found abandoned on its host (`abandoned`, optional); the client marks
  only those, `slots --local` judges its own.
- `RunInfo::refused` (optional): `brd-host runs` says `finished, refused <code>` and the message on the next line.
- A message of the agent names its host as `<host>`, and the client puts its alias there.
- The wait line says it is a wait, not a hang; the hint of another build says that runs work and that an upgrade waits
  for the user; the line of a relief that left the volume low names the largest pools and slots and what a person can
  do.
- The skill, AGENTS.md and `docs/contributing/studio-runs.md`: one heavy run is one check.
