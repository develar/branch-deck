//! The shrink of the pools of this machine below the minimum free space (spec/cli/lease.spec.md, "Get"): the free slots
//! a get may deep-clean, in the order it takes them. A free slot keeps the warm build output of its last lease (cargo
//! keeps a unit while it is among the newest of its group, `lease_clean`), tens of gigabytes in `target/` of each slot of
//! branch-deck, and a get reuses the free slot returned last first (`lease::reserve`): the warm output of the others is
//! rarely used again.

use crate::lease::Slot;
use std::cmp::Reverse;

/// The free slots of `pools` that `usable` admits (its directory is there, no process works in it), in the order a
/// shrink takes them: the ones returned longest ago first across every pool (`Slot::returned_at`, then `returned` for a
/// slot a bd before that time returned; one never returned first of all), then, once no other is left, the slot of each
/// pool that its next get would take (clean before dirty, returned last first, as `lease::reserve` orders them), whose
/// warm output that get reuses. A slot with a lease of any state is no candidate. Each with the index of its pool
pub fn shrink_order<'a>(pools: &[&'a [Slot]], usable: impl Fn(&Slot) -> bool) -> Vec<(usize, &'a Slot)> {
  let mut first: Vec<(usize, &Slot)> = Vec::new();
  let mut last: Vec<(usize, &Slot)> = Vec::new();
  for (pool, slots) in pools.iter().enumerate() {
    let free: Vec<&Slot> = slots.iter().filter(|slot| slot.lease.is_none() && usable(slot)).collect();
    // the first of the minimum, as the stable sort of `reserve` takes it
    let next = free.iter().copied().min_by_key(|slot| (slot.dirty, Reverse(slot.returned)));
    for slot in free {
      if next.is_some_and(|next| std::ptr::eq(next, slot)) {
        last.push((pool, slot));
      } else {
        first.push((pool, slot));
      }
    }
  }
  // stable: the slots of the same time in the order of their pools and of the state
  first.sort_by_key(|(_, slot)| (slot.returned_at, slot.returned));
  last.sort_by_key(|(_, slot)| (slot.returned_at, slot.returned));
  first.extend(last);
  first
}

#[cfg(test)]
#[path = "shrink_test.rs"]
mod shrink_test;
