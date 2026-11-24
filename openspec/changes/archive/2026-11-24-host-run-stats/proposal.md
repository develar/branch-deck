# Proposal

## Why

The heavy queue is the bottleneck of the Studio: on 2026-11-24, in 22.6 h, its 144 runs waited 3189 min for the heavy
turn against 346 min of commands. `brd-host runs` shows the last day one run at a time, and nothing counts the bytes
of a run. The next rules of the queue (a lane for short runs, a choice of the host at the start, a link of 6 MB/s)
need the waits, the outcomes and the bytes of several days, by command, and a wait behind other runs told from a wait
for a quiet host.

## What Changes

- The agent keeps a line of each ended run for 90 days, `~/.cache/brd-host/history/<YYYY-MM>.jsonl`, written by
  `serve` as the run directory goes, and a line of each pack of a base.
- A pack counts its bytes into `pack.json` of its run, a fetch into `fetched.json`.
- The supervisor of a heavy run writes `queue.json`: its place as it joined, what it needed, and its wait behind the
  others and for the load of the host.
- `Request::Stats` (feature `stats`) answers the summary of a window, summed on the host; `brd-host stats [<alias>]
  [--days <n>] [--json]` prints it for a host or every host.
- The schemas of the protocol and of `turn.json` stay: the request is behind a feature, the new state in new files.

Capabilities: `cli/host`.
