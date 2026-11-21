# Proposal

## Why

On 2026-11-21 an exclusive run of idea-1 waited at the head of the heavy queue of the Studio. Runs that are not heavy
(accio benchmarks on 8 cores for hours, cargo builds) kept 20 of its 20 cores busy. An exclusive run waits for no
holder, no run in its command and a quiet window, and the load bound of 15 min does not apply to it. Only the head of
the queue is admitted, so the 8 heavy runs behind it waited their 2 h and ended with `heavy_timeout`, though no heavy
run held cores.

## What Changes

- Once the drain of an exclusive head has ended without a quiet host, the first heavy run behind it that is not
  exclusive starts past it while the head cannot start and the admission of the head of the queue admits that run.
- No run passes an exclusive head before its drain ends: behind a heavy holder its drain has not begun, and while it
  drains the drain is its window of the host. Its drain never starts again.
- The holder names the exclusive run that it passed (`passed`), the lines of the wait name the drain of an exclusive
  head, and `brd-host runs` reads it from `head_drain` of the reply.

Capabilities: `cli/host`.
