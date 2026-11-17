# Proposal

## Why

The teardown of a full E2E run named one run-out clock of a deleted test repository that served 2988 waits
(stacked-branches.spec.ts:1703), where a wait with a deadline serves at most 40. Its backtrace is the per-session loop of
the scheduler of the facts (`SessionFacts::run`). The dispatcher dropped the task of a closed session by dropping its
inbox, but the task ended only once no read of it held the inbox any more. Meanwhile its timers still fired: a timer due
before the reads in flight answered asked Patronus or GitHub for a closed session. In the app that is one more request
of a session closed while a window watched it (the registry watch closes the sessions of a repository that went away).
On the E2E server's clock of a deleted repository, which moves to each timer at once, the timers fired one after another
while one read after another held the inbox.

## What Changes

- The dispatcher marks the task of a session closed (`SessionTask::closed`) when the session closes; the task ends at its
  next turn, a timer that wakes it asks nothing, and the answers of its reads in flight go to no task.
- The reads of a task take their tickets only of the epoch of its first one (`TaskPort`): a read whose `begin` the
  session answered after it opened anew under the same key (`Actor::reopened` keeps its mailbox) is refused, never
  applied to the new lease.
- `Actor::reopened` always tells the scheduler the session closed, also when the stamp of the new lease told it during a
  run: a window that came back meanwhile made a task of the lease before, which ends there.
- The specs: a session that closes while a read of it is in flight asks nothing more, its timers stop at the close; a
  read of a session opened anew under its key takes no ticket; the session closes again as it opens anew.
