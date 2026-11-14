# Proposal

## Why

Agents ran heavy checks on the Mac Studio through `brd-host run studio -- …` at the same time: a full `pnpm e2e`,
`cargo nextest run --workspace`, `pnpm lint:backend`, long `bt` runs of the IntelliJ monorepo. When two of them met,
the tests ran 3 to 4 times slower and failed on timeouts, and the agents fixed flakes that did not exist: in one case
the E2E tests ran 3.6 times slower beside a `cargo nextest run --workspace` of another agent, and 4 of them failed at
30 s.

## What Changes

- `heavy` of `[host]` of `.branchdeck.toml`: the patterns of the heavy commands of a repository (`*` matches any
  characters, the rest is literal, after a leading `env` and its assignments), and `brd-host run --heavy` for a command
  that no pattern names. No flag makes a run light: the repository decides, so an agent that forgets a flag still
  waits, and so does `bt --host`.
- The client reads `[host]` of the shipped commit for every run, sends a heavy run with `RunSpec::heavy` and says why
  it is heavy; an agent without the feature `heavy` gets a line that names `brd-host upgrade`.
- The agent runs the heavy runs of its host one at a time, whatever their repository: the heavy turn of the host
  (`lease_pool::host_turn`, `~/.cache/brd-host/heavy.json`), the queue of the host slots with one place. A heavy run
  takes it before its lend (the phase `heavy`), says its wait at once and once a minute with the time left, and gives
  it back after its give-back. A cancel takes it out of the queue, a gone holder frees the turn, and a wait past 2 h
  ends with the new code `heavy_timeout`. A run that is not heavy runs as before.
- `brd-host runs` names who holds the heavy turn and for how long, and each run that waits and for how long.
- An unknown key of `[host]` says that it can be a key of a later Branch Deck and how to update brd-host, so the next
  key of `[host]` is no dark refusal of an older brd-host.
- Capability `cli/host`.
