# Proposal

## Why

The review of e1a580253: the sessions closed at two looks in a row that found the git directory missing, a count and
not a time. A look follows an event, a focus or an admission at once, so the burst of events of a volume that unmounts
brings two looks within milliseconds, and the windows of a disk away for a moment closed anyway.

## What Changes

- The sessions close only when the looks in a row that find the git directory missing (`NotFound`) span 2 s
  (`RECHECK`) from the first of them, so two looks at least; a deleted repository is still found within about 4 s.
- Looks that find it missing within milliseconds of each other keep the sessions, as one that finds it missing and the
  next that finds it back.
