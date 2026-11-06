use crate::lease::{Lease, LeaseState, Slot};
use crate::shrink::shrink_order;
use pretty_assertions::assert_eq;

/// A slot `name` of the state, free unless `lease`, returned as the `returned`-th of its pool at `returned_at`
fn slot(name: &str, returned: Option<u64>, returned_at: Option<u64>) -> Slot {
  Slot {
    name: name.to_string(),
    path: format!("/pool/{name}"),
    dirty: false,
    dirty_check: None,
    returned,
    returned_at,
    lease: None,
    reclaimed: None,
  }
}

fn leased(mut slot: Slot, state: LeaseState) -> Slot {
  slot.lease = Some(Lease::marker("test", "test", state));
  slot
}

fn names(order: &[(usize, &Slot)]) -> Vec<(usize, String)> {
  order.iter().map(|(pool, slot)| (*pool, slot.name.clone())).collect()
}

/// The free slots of every pool go by the time of their return, the never returned ones first and those without a return time
/// before the time by their number; the slot each pool's next get would take comes last, in the same order; a slot
/// with a lease and one `usable` refuses (a process in it) are no candidates
#[test]
fn the_shrink_takes_the_free_slots_returned_longest_ago_across_pools_and_each_pools_next_one_last() {
  let a = [
    slot("a-old", Some(1), Some(100)),
    slot("a-next", Some(3), Some(400)),
    slot("a-mid", Some(2), Some(300)),
    leased(slot("a-leased", Some(4), Some(50)), LeaseState::Ready),
    leased(slot("a-cleaning", None, None), LeaseState::Cleaning),
  ];
  let b = [
    slot("b-never", None, None),
    slot("b-busy", Some(1), Some(10)),
    slot("b-before-the-time", Some(2), None),
    slot("b-next", Some(5), Some(200)),
  ];
  let order = shrink_order(&[&a, &b], |slot| slot.name != "b-busy");
  assert_eq!(
    names(&order),
    [
      (1, "b-never".to_string()),
      (1, "b-before-the-time".to_string()),
      (0, "a-old".to_string()),
      (0, "a-mid".to_string()),
      (1, "b-next".to_string()),
      (0, "a-next".to_string()),
    ]
  );
}

/// A get takes a clean free slot before a dirty one, and the slots never returned last: the next slot of a pool is the
/// clean one returned last, else the first never returned, else the dirty one returned last
#[test]
fn the_next_slot_of_a_pool_is_the_one_its_get_would_take() {
  let mut dirty = slot("dirty", Some(9), Some(900));
  dirty.dirty = true;
  let pool = [dirty.clone(), slot("clean", Some(1), Some(100)), slot("never", None, None)];
  assert_eq!(names(&shrink_order(&[&pool], |_| true)).last().unwrap().1, "clean");
  let pool = [dirty.clone(), slot("never-1", None, None), slot("never-2", None, None)];
  assert_eq!(
    names(&shrink_order(&[&pool], |_| true)),
    [(0, "never-2".to_string()), (0, "dirty".to_string()), (0, "never-1".to_string())]
  );
  assert_eq!(names(&shrink_order(&[&[dirty]], |_| true)), [(0, "dirty".to_string())]);
  assert_eq!(names(&shrink_order(&[&[][..]], |_| true)), Vec::<(usize, String)>::new());
}
