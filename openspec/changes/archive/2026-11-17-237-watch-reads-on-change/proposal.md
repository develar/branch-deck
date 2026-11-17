# Proposal

## Why

Issue #237, "Coalesce the event-driven looks of a watch": the watch of the operations registry read `operations.json`
at every look, and a look follows every event of `$GIT_COMMON_DIR/branch-deck`, of any file there (the leases, the
landed commits, the fetch outcomes, the lock files, the stamp, the journal), so every write of another file of the
directory read and applied the registry again. On Linux, inotify tells every open of a file of the directory
(`IN_OPEN`), the read of the look itself among them, so each look asked the next one, for ever.

## What Changes

- The registry is in the `Fingerprints` of a look: a look reads it only when its stat changed since the last look (its
  size, its time and its file id, which the rename of every write changes), at the first look, at the look of a
  session that joined the watch or of a focus, after a read that failed, and once its last read is 2 s (`RECHECK`)
  old, as the records shown depend on whether their processes still run and a stat may miss a change; the wait for
  the next look ends when that read is due at the latest.
- A session is given the operations of other processes only once the change reached its actor, so one that opens
  after a read of the watch gets them from the read of its subscription.
- An open, a read or a close without a write of a file of a watched directory wakes no waiter (`WatchedDir::wake_of`),
  so a waiter's own read asks no look; every watch of `sync_utils::dir_watch` gains it.
- No timer coalesces the looks: a look of an unchanged registry is four stats.

Capabilities: `branch/patronus`.
