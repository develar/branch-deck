# Proposal

## Why

`brd lease disk` walked the 16 slots of the idea-1 pool for 19.5 minutes (2026-11-21): every slot in full, though each
had a saved mark of the `FSEvents` history. The replay of the history had 0.5 s, one stream a slot; on a Mac with an
overloaded `fseventsd` it never ended in time, and a history that did not end walked the whole slot. Where it ended, a
change in a part with a hard link, a new hard link or a renamed directory walked the whole slot too, and the parts of a
fixed depth of 3 were tens of gigabytes (a cargo `target/` at depth 4, `bazel-out`), so a build walked most of a slot
again. A walk of a slot is cold: the vnode cache holds less than one slot.

## What Changes

- One replay of the history for every measure of a volume (marks far apart in streams of their own, all at once), by a
  deadline of 10 s, each event given to the measures under whose roots it lies and whose mark it follows by a lookup of
  the ancestors of its path; a lost subtree (`MustScanSubDirs`) is that subtree to walk again; a refusal names its reason.
- A history that comes late keeps the saved bytes and their mark: a late replay is slow, not lost. Such a measure is
  walked in full in the budget the other walks leave, so its next replay is short; one that the history has not checked
  for a day is walked with the others.
- A file of `n` hard links counts `1/n` of its bytes at each link: the bytes of the parts add up whatever else is walked,
  so neither a hard link nor a new one nor a renamed directory walks a slot in full.
- The parts of a measure are the subtrees of at most 20,000 entries and the directories above them with their own
  files: a change inside a subtree walks it again, a change of a file of a directory above reads that one directory,
  a new directory in it is walked alone, a gone one leaves the measure.
- On macOS a return, a lease or a reclaim since the measure walks nothing more: the history names what changed.

Capabilities: `repository/disk-usage`.
