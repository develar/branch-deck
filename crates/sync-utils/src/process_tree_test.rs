use crate::process_scan::Descendant;
use crate::process_tree::kill_bottom_up;
use std::cell::RefCell;
use std::collections::HashMap;

/// 1 (the root) → 2 → 3 → 5, and 1 → 4, each after its parent as `process_scan::descendants` lists them
const TREE: [(u32, u32); 4] = [(2, 1), (3, 2), (5, 3), (4, 1)];

fn below() -> Vec<Descendant> {
  TREE.iter().map(|&(pid, parent)| Descendant { pid, started: 0, parent }).collect()
}

/// The kills of `kill_bottom_up` over `TREE`, each process exiting `exit_after` waits after its kill (never when
/// `None`), and how many waits the run took; a run waits at most `patience` times
fn kill_tree(exit_after: impl Fn(u32) -> Option<u32>, patience: u32) -> (Vec<(u32, u32)>, u32) {
  let tick = RefCell::new(0u32);
  let killed: RefCell<HashMap<u32, u32>> = RefCell::new(HashMap::new());
  let order = RefCell::new(Vec::new());
  let waited = kill_bottom_up(
    1,
    &below(),
    |pid, _| {
      let killed_at = killed.borrow().get(&pid).copied();
      killed_at.zip(exit_after(pid)).is_some_and(|(at, after)| at + after <= *tick.borrow())
    },
    |process| {
      killed.borrow_mut().insert(process.pid, *tick.borrow());
      order.borrow_mut().push((process.pid, *tick.borrow()));
    },
    || {
      *tick.borrow_mut() += 1;
      *tick.borrow() <= patience
    },
  );
  assert_eq!(waited, exit_after(5).is_some(), "whether every wait ended in its exit");
  (order.into_inner(), tick.into_inner())
}

/// A stopped process that outlives its parent goes to launchd or init alive: on macOS the exits of such processes made
/// the kernel send SIGHUP to the process group of the caller. Each process is killed only once its children exited, the
/// deepest first, and the kill returns once the children of the root exited, for the caller to kill the root
#[test]
fn a_tree_is_killed_from_the_bottom_up_each_process_once_its_children_exited() {
  let (order, waits) = kill_tree(|_| Some(3), 1000);
  let pids: Vec<u32> = order.iter().map(|(pid, _)| *pid).collect();
  assert_eq!(pids, [4, 5, 3, 2]);
  let at: HashMap<u32, u32> = order.iter().copied().collect();
  for (pid, parent) in TREE {
    if let Some(parent_at) = at.get(&parent) {
      assert!(at[&pid] + 3 <= *parent_at, "{parent} was killed before its child {pid} exited: {order:?}");
    }
  }
  // the children of the root, 2 and 4, exited before the return
  assert!(waits >= at[&2] + 3, "{waits} {order:?}");
}

/// A killed process that does not exit (a thread stuck in a call) holds the kill up only until the wait gives up: then
/// the rest of the tree is killed without waiting
#[test]
fn a_process_that_does_not_exit_holds_the_kill_only_until_the_wait_gives_up() {
  let (order, waits) = kill_tree(|pid| (pid != 5).then_some(1), 10);
  let pids: Vec<u32> = order.iter().map(|(pid, _)| *pid).collect();
  assert_eq!(pids, [4, 5, 3, 2]);
  assert_eq!(waits, 11, "{order:?}");
}
