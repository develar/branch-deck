# Proposal

## Why

An agent waits for a run of `brd-host run <host>` in calls that its tool ends after 10 min, and a SIGTERM or SIGHUP of
`brd-host run` cancels the run on the host. So agents started their runs in the background and waited by hand: on
2026-11-23, in 107 sessions, 93 `brd-host follow` calls wrapped in `timeout 59x`, 33 follows that broke, 59 polls of a
`run.exit` marker, 82 `brd-host runs | grep`, and 92 `grep -v` of the lines of the heavy turn: about 250 turns a day
beyond the builds (issue #258).

## What Changes

- `brd-host wait <alias> <run> [--max <duration>]` (9 min by default) waits for the end of a run in one bounded call:
  it asks the agent for its runs every 5 s and says one line per change of the state of the run, with its time.
- At the end it finishes the run as `run` does: the output into `<run>.log` of the checkout, the files of the record,
  the changes, `<run>.end.json`, the forget; then a summary with the last 30 lines of the output, and the exit of the
  run.
- Past `--max` the run goes on and the wait exits 124 with the new code `still_running`; a host that never answered is
  `timed_out` (255).
- One process of a checkout finishes a run (`<run>.lock`): a `run` and a `wait` of one run never apply its changes
  twice, and a later wait answers from the end file.
- No change of the agent: the state comes from the steps that `Reply::Runs` already carries.

Capabilities: `cli/host`.
