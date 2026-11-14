# Proposal

## Why

The review of the mark of a sync found that only the plain return had a test of it, and that a takeover whose return
of the idle lease failed before the mark went gave the slot back, free, with the mark of the lease before.

## What Changes

- The return of a slot (`finish_return`: a return, a takeover, a reclaim) takes the mark out first, so one that fails
  after it leaves none.
- Capability `cli/lease`: a reclaim and a takeover that fails take out the mark too, each with its test.
