# brd lease disk tells each step on stderr, and the progress counts the slots that remain after the FSEvents check

## Why

`brd lease disk` printed nothing until the JSON at the end, so a walk of minutes looked like a hang. The progress of
the core also counted wrong after an FSEvents check: `Planning` sent the new count of slots to walk, but each `Slot`
event kept the count of `Cached`, which also held the slots that the check found unchanged. The Disk window of the app
showed "1 of 10" for a walk of one slot.

## What Changes

- The core sets its count to the slots that remain after the FSEvents check, so `Slot` counts to what `Planning` set.
- `pool_disk::step::DiskStep` words each step: the start with the count to check or measure, the FSEvents check, each
  walked slot with its bytes and its count, and the end.
- `brd lease disk` prints `brd lease disk: <step> (<seconds> s)` on stderr for each step; stdout keeps one JSON object.
- Capabilities `cli/lease` ("Disk") and `repository/disk-usage` (a test link).
