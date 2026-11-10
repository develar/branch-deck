# Tasks

- [x] 1.1 `lock_pressure` takes the `canceled` of its wait; `pressure_turn` ends the wait once the volume has the
  minimum (`PressureTurn::Enough`), and the shrink or the eviction is then skipped and the get measures again.
- [x] 1.2 A test of a get that goes on while the other still holds the lock; the test of a get that waited made
  deterministic (the volume has the minimum only once the holder deleted both slots).
- [x] 1.3 The design doc names the span of the lock and why the wait ends at the minimum.
