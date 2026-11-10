# Tasks

- [x] 1.1 `remove_measured_until` asks its stop every 256 entries; `Trash::empty_until` stops with it and leaves the
  rest to a sweep (`abandon`, `left-…`), which `holds_gone_runs` and `sweep` take.
- [x] 1.2 The shrink stops within a slot at its target; the eviction's threads stop at the minimum (`remove_all_until`).
- [x] 1.3 The fake volume of the lease tests frees what the trash holds only once it is deleted, as a volume does.
- [x] 1.4 Tests: a delete that stops at its ask, a trash left to the next sweep, a delete side by side that stops, a
  shrink that stops inside the delete of a slot of 1,001 files.
- [x] 1.5 The wait for the turn is told once a minute with the holder's time and the free space against the minimum.
