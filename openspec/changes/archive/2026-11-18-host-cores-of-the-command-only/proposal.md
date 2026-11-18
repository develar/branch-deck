# Proposal

## Why

The first measures on the Studio after the turn by cores had bt runs at a mean of 25, 59 and 153 cores and peaks up to
414 on a host of 20: a Bazel server of the slot that the first look of a sampler missed, found at a later sample,
counted the CPU of its whole life as the work of the run. Such a measure makes a command take the whole host for its
next 5 runs.

## What Changes

- A process new to a sample counts its whole CPU only when it started after the command; one that started before
  counts from that sample on. A sample counts at most the cores of the machine over its time.
