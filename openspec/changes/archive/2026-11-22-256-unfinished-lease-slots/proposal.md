# Proposal

## Why

Issue 256: in the idea-1 pool nine gets queued for 26 minutes behind one that cloned a slot, while two slots were free.
Both were adds that never finished (a get failed or was killed after it registered the slot): their trees lacked
hundreds of thousands of tracked files, a get marked them dirty with every path, and the eviction kept them as never
returned. A get cloned the next slot from one of them. `leases.json` held their 3.3 million paths, 380 MB, which every
change of the pool parsed and wrote under its lock, so a return gave up on that lock after 10 s.

## What Changes

- A slot whose add never finished is never reserved as free, never a template, and the next get that adds a slot
  removes it as the trim removes a free slot; `brd lease status` shows it as `unfinished`.
- An adopt names its slot in its first write.
- The state keeps the first 100 local changes of a dirty slot and of an idle lease, with the count of the rest.

## Impact

`crates/lease-pool` (`lease.rs`, `trim.rs`, `shrink.rs`), `crates/git-ops/src/lease_slot.rs` (`forget_slot` drops a
locked registration), `crates/bd-lease` (output), `crates/app-api/src/app/disk.rs`.
