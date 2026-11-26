# Proposal

## Why

A third review of the turns of the groups (#279) found that the `groups.json` a host has from the first build of the
groups (no `schema`, a number of `passes` per run) made every join, admission and estimate refuse after an upgrade,
with nothing to rewrite it; that a run superseding a run of its group lost the passes made of its place, so a gate
started again on each commit moved back by the budget each time; that the spec claimed a first waiting run is passed
by none; and that the note of an unreadable file went to a `tracing` call that no binary installs.

## What Changes

- No `groups.json` of any build makes the turn refuse: the file of the first build keeps its groups and the runs each
  went ahead of; a file of a later schema and one that does not parse start the groups anew, and the log of the agent
  and the line of the run say so.
- A run that supersedes a run of its group takes the passes made of its place.
- The spec states which runs a joining run passes: a run of round 0 behind a run of a later round too, within its
  budget.
- Tests: the exact shape of the first build, a later schema, repeated supersedes, a round-0 run behind a later round, a
  gone run and a failed write, and the estimate test pinned to the supersede of a run of another group.

Capabilities: `cli/host`.
