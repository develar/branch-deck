# An operation that holds more holds it for every process, and never waits for one that waits for it

## Why

Issue #237, plan step "the hold of what is missing": `RunningGuard::hold`, which the check and the move of Update
Project use and the cover of a plan decided once admitted will, had two defects.

- Only the session of this process learned what it held more: the record of the operation in the operations registry
  of the repository kept the resources of its admission, so a `brd` that needed one of them was admitted beside it.
- `requeue` admitted the operation again over the others but those queued directly behind it: x holds r1, w1 waits
  behind x for r1 and r3, w2 behind w1 for r3 and r2; x needs r2 and queues behind w2, which waits for w1, which waits
  for x, and no end readmits anyone. And as x went to the end of the line while w1 stayed before it, the end of any
  other operation admitted w1 over a prefix without x: w1 ran beside x, both holding r1.

## What Changes

- `RunningGuard::hold` extends the record of the operation first (`RecordHandle::extend`, under the lock of the
  registry): `busy` while an operation of another process holds one of them, the record as it was. It answers whether
  the operation waited in the queue.
- `requeue` admits the operation over the ones but those that wait for it, directly or behind one that does
  (`waiting_for`); those follow it at the end of the line in their order, then the waiting ones are admitted again as
  at an end (`readmit`, factored out of `end`).
- Only the operations that wait in the queue (a wake-up in their channel) follow: one queued behind it inside
  `wait_behind` runs already. A debug build checks after each admission again that every operation in the queue waits
  behind one before it.
- `RunningGuard::hold` says that a hold that waits gives the place of the operation up: one that waited behind another
  for what it held may run first, so it comes before the first effect, and its caller decides again.
- Capability `branch/patronus`: two requirements of the operations.
