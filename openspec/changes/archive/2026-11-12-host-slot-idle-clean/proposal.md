# Proposal

## Why

A host slot keeps its ignored files as the warm cache from run to run, and its clean rules ran only when they were due
at a give-back: by size with `branchdeck.hostSlotMaxSize`, after 50 runs or 7 days without it, or below
`branchdeck.leaseMinFree`. A slot that no run takes again is never given back again, so it kept its caches for days. On
the Studio on 2026-11-12 the four host slots of accio held 20 to 50 GB each of `.bench` (the indexes of A/B arms and
suites, which accio's `.branchdeck.toml` hands to a `delete` rule) with 350 GB free, below no minimum.

## What Changes

- **An idle host slot is cleaned once it waited after its last run**: the keeper applies the clean rules of a host slot
  that waited idle `branchdeck.hostSlotIdleClean` (6 hours without it, `0` never) since its last run gave it back,
  with no clean after that run. The runs of one session in a row still find the caches warm.
- **The keeper's round** runs that clean on the relief's thread, before the relief of each repository.
- A give-back records when it gave the slot back (`returned` of the lend table).
