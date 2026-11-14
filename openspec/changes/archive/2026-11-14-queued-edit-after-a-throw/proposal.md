# Proposal

## Why

The review of 6987f5d1 found that the `finally` of `FieldWrites.run` dropped the edit queued behind a write that threw:
it was never written, and on a close it was lost.

## What Changes

- A write that throws ends no run: the edit queued behind it is written, then the save rejects with the first error.
- Capability `repository/settings`: the scenario of a write that threw says so.
