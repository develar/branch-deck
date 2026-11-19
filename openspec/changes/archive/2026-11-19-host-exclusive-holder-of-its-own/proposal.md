# Proposal

## Why

On 2026-11-19 the first exclusive run on the Studio (1795097891317-95717) took the heavy turn, and runs that are not
heavy started beside it at once, of idea-1 and of accio. A supervisor of the build before `exclusive`, of a run that
began before the upgrade, rewrote `turn.json` at each sample of its run and dropped the field `exclusive` of the
holder, which the runs that are not heavy looked for.

## What Changes

- The exclusive holder is also in `exclusive.json` beside the table, which no older build touches: written at the
  admission under the lock, removed at the leave; the wait of a run that is not heavy and `brd-host runs` read it.
- A look of an exclusive run puts the field back on its entry, queued or holding.
- `took the heavy turn of the host after <n> s` names the busy cores of the host by the sample that the admission read.
