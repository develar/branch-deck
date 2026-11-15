# A write of a companion holds the writers of both configs

## Why

The writer of each config ran one step of a companion's add at a time, so an add and a removal of one pair at once
interleaved across the two writers (`branch/declared-needs`, "A write of a companion is one write of both configs").

## What Changes

- A write of a companion holds the writers of both configs, the primary's and the companion's, for its whole span,
  taken one by one in the order of their files (`ConfigWriters::write_together`), so no write of Settings to either
  runs inside it and two such writes never wait for each other.
- Capability `repository/settings`.
