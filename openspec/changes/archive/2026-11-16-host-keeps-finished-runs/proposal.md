# Proposal

## Why

A client forgets its run once it saw the end and every file came, and the forget removed the run from the host: a
minute after a run ended, `brd-host runs <host>` no longer showed it and `brd-host follow` answered `unknown_run`, so
nobody could look up how a run of another session ended or what it printed last.

## What Changes

- A forget removes the files of a run (the stage, the outputs, the changes) and keeps its spec, its end, its phases and
  the last 1 MiB of its log; a fetch of a forgotten run refuses, a second forget says it was forgotten already.
- The agent keeps the last 100 forgotten runs by their end, each at most a day, on disk, so an upgrade keeps them.
- `RunInfo` gets the optional `ended_ms` and `forgotten`; `brd-host runs` says `running` or `finished, exit N`, the
  start in the time zone of the client, and how long a finished run took.
- `brd-host follow` of a forgotten run answers the tail of its output and its exit.
