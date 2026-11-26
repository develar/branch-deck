# Proposal

## Why

A review of the turns of the groups of the heavy queue (#279) found that a short run waiting at the end of the queue,
as one whose backlog had used up its short passes, put every later run of another group behind the whole backlog
again; that the estimate placed a run that supersedes a run of another group one run off the join, by a second copy of
the rule; that a run whose process ended gave none of its passes back, and that a failed write of the table could count
a pass for a run not in the queue; and that `groups.json` had no schema, so a file that did not parse silently reset
every budget.

## What Changes

- A new run may not pass the head, a run of its own group, or a run that 8 runs of the turns got ahead of; behind the
  last of those it joins before the first run whose round is above the count of its own group's waiting runs, and
  passes every run from there, a short run too. A short run at the end no longer suspends the turns.
- The join is one function of the table and its files in memory (`host_turn::join_queue`); the estimate applies it to
  a copy, so both place a run alike by construction, which a test over generated queues checks.
- `groups.json` names the runs each run went ahead of and whether it took the turn, and the passes of a run are derived
  from it and the queue: a run that leaves without the turn, by any way, counts none, whatever write failed.
- `groups.json` has a schema: a file of another schema refuses as the table does, one that does not parse says so in
  the log and gives every waiting run its whole budget again.
- The text states the bound as it holds: at most 8 runs of the turns ahead of a run, waiting or having taken the turn,
  12 with the short lane's 4, and a passed run can reach `heavy_timeout` behind long runs.

Capabilities: `cli/host`.
