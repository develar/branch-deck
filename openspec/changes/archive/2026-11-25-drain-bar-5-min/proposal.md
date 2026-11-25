# Proposal

## Why

On 2026-11-25 the commands of the runs that are not heavy took 0.9 min (p50) and 5.0 min (p95) over 272 runs on the
Studio. Six exclusive runs were the head of the queue after 07:54: four drains found a quiet host within 2 min, and
the two that ran their full 15 min ended without one. In the drain at 10:27 no heavy run held, about 16 cores were
idle and 11 heavy runs waited. A drain that is not quiet after 5 min was not quiet after 15 either.

## What Changes

- The drain of an exclusive run at the head of the queue bars the new runs that are not heavy for at most 5 min from
  its start, not 15 min (`DRAIN_BAR`). After it, the heavy runs behind it that fit start past it, as before.

Capabilities: `cli/host`.
