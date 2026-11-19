# Proposal

## Why

On 2026-11-19 the supervisors of five runs of the Studio were killed by pid while they hung on a volume. `brd-host runs`
listed them as running for up to an hour more: only a follow, a cancel, a forget and the hourly cleanup ended a run that
lost its supervisor, and its refusal did not say what to do.

## What Changes

- `Request::Runs` ends each run that lost its supervisor before it answers, and the refusal of such a run says that
  whether its command ended is unknown, and to run it again.
