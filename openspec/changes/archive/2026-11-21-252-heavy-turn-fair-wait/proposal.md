# Proposal

## Why

develar/branch-deck#252: a UI lane run of idea-1 got no heavy turn of the Studio for more than 5 hours.

- A run without a measure waits on a free turn for a quiet host, busy with at most 4 of its 20 cores. The runs that
  are not heavy wait for no heavy run, and on the Studio they kept 20 of 20 cores busy, so the run waited its whole 2 h
  and ended with `heavy_timeout`. The runs behind it waited too, as only the head of the queue is admitted.
- The measure of the lane did not count for its next run: the key of the measures was the key of the supersede, all
  the words of the command, and the two runs differed by their output directory (`out/air/light1`, `out/air/light5`).
  A run also read the measures once, at its first look.
- A look that waited 60 s for `turn.lock` ended the run with `agent_error`, after a wait of 72 min, and its end took
  its ticket out of the queue. Each change of the table woke every run that waited, and each of them took the lock.

## What Changes

- On a free turn the head of the queue is admitted whatever the load outside the heavy runs once it has waited 15 min
  in the queue (`LOAD_BOUND`). An exclusive run still waits for its quiet host.
- The key of the measures (`host_link::heavy::measure_key`) puts each path of a directory that the run fetches as one
  word. A run that waits reads the measures again at each look.
- A lock of the table that another process holds for 2 s costs a look, not the run: the run keeps its place, says so
  at once and once a minute, and looks again. A run behind the head of the queue looks without the lock.

Capabilities: `cli/host`.
