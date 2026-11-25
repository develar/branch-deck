# Proposal

## Why

On 2026-11-25 the exclusive rules caused 47.7 % of the wait of the Studio: 28.4 % while a measurement held the host,
19.3 % for an exclusive head that waited for the holders or drained. The 8 runs of `bt --lane fast --no-cache --json`
of the day ended with exit 3. A rule cannot tell a needed measurement from a routine one; the person who starts them
can, once the summary shows what they cost.

## What Changes

- `queue.json` of an exclusive run names the part of its wait in which it drained the host, and when the drain began.
- `brd-host stats` gives per exclusive command the time its runs held and drained the host, the other runs that waited
  in those spans and for how long, and its outcomes.

Capabilities: `cli/host`.
