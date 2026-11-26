# Proposal

## Why

The heavy queue of a host is first come first and cannot tell who submits a run (#279). On one day on a 20-core host,
the 5 to 6 agents of one orchestrating session, each in its own checkout, queued 87 heavy runs of one measure class;
the heavy runs of other sessions and repositories waited p50 25 to 60 min behind them. The affinity is per checkout and
also picks the slot, the base of the packs and the supersede, so it cannot group the checkouts of one session; the
sessions serialized themselves with a hand-made `mkdir` lock instead, which leaks and idles the host.

## What Changes

- `brd-host run --group <group>`, else `BRD_HOST_GROUP` (an orchestrator sets it once for all its agents), names the
  group of a run in the heavy queue; `RunSpec::group` and `EstimateAsk::group`, behind the agent feature `group`. The
  default group is the affinity of the run, so a single checkout queues as before, and so do checkouts with one run
  waiting each; a second run of one checkout waits a round, as the second run of one agent.
- The heavy queue takes turns between groups: the round of a run is the count of the runs of its group before it, and
  a new run joins behind the last entry it may not pass: the head, a run of its group, a short run, a run passed 8
  times by the turns, a run whose round is not above the count of its own group's waiting runs. First come first
  inside a group, and exactly first come first with one waiting run per group. The queue is still reordered at its join
  only, so a supervisor of an older build, which admits the head, reads it right; the groups and the passes of the
  turns live in `groups.json`, which no older build reads or writes.
- The bound: a run is passed by the turns at most 8 times (`TURN_PASS_BUDGET`), whatever the count of the groups and
  whether the head can start; a run that leaves without the turn gives its passes back. The turns keep none of the
  short lane's state.
- Unchanged: the admission by cores, measures and `--measure-as`, the short lane and its budget of 4 passes, `--exclusive`
  and its drain, the 15-min load bound of the head, `superseded` (a run that supersedes one of its group takes its
  place and its passes, one of another group its own turn), `idle_only` sets and `admit_now`.
- `brd-host runs` (text and `--json`), the line of a wait, the history line and `brd-host stats` name the group;
  `stats` sums the runs, the waits, the wait for the heavy turn and the work by group.

## Decisions

- **No cap per group.** The turns already keep a group of many runs from holding the queue; the admission by cores
  decides how many of its runs hold cores at once, as for any run. A cap would idle the host when only one group waits,
  the defect of the hand-made lock. Left out until a measurement shows a group that holds the cores of the host for
  hours while others wait.
- **The default is the affinity**, not the run alone: a run alone would always be of round 0 and pass every named
  group, which would make naming a group a loss; by the affinity a checkout is one group, as one agent is.
- **Rounds read from the queue, and a budget of 8 passes**, not stamped rounds with a memory of the served groups: the
  stamped rounds put a checkout whose run just took the turn behind every new checkout, and left the passes unbounded in
  the count of groups while the head could not start.
- **An older agent ignores the group**: the client sends no `group` to an agent without the feature and says so for a
  heavy run; the run queues first come first there, which is what it got before, so the client does not refuse it.

Capabilities: `cli/host`.
