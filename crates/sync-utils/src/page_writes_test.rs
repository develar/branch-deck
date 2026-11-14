use crate::page_writes::{PageWrite, PageWrites};

fn write(window: &str, page: &str, seq: u32) -> PageWrite {
  PageWrite {
    window: window.to_owned(),
    page: page.to_owned(),
    seq,
  }
}

/// Two writes of one key of a page that arrive in reverse order: the newer applies, the older is superseded; another key
/// and another window are their own
#[test]
fn an_older_write_of_a_key_of_a_page_is_superseded() {
  let mut writes = PageWrites::default();
  assert!(writes.applies(&write("settings", "a", 2), "branchPrefix"));
  assert!(!writes.applies(&write("settings", "a", 1), "branchPrefix"), "the older one");
  assert!(!writes.applies(&write("settings", "a", 2), "branchPrefix"), "the same one again");
  assert!(writes.applies(&write("settings", "a", 1), "githubRepository"), "another key");
  assert!(writes.applies(&write("main", "a", 1), "branchPrefix"), "another window");
  assert!(writes.applies(&write("settings", "a", 3), "branchPrefix"));
}

/// A window opened again under the same label is another page, which counts from 1: its first write applies after the
/// last ones of the page before, whatever the host knew of the load, and a late write of the page before applies in the
/// order it comes
#[test]
fn a_page_loaded_again_counts_anew_and_the_writes_of_two_pages_apply_as_they_come() {
  let mut writes = PageWrites::default();
  assert!(writes.applies(&write("settings", "first", 7), "ai"));
  assert!(writes.applies(&write("settings", "second", 1), "ai"), "the page loaded again");
  assert!(writes.applies(&write("settings", "first", 8), "ai"), "a late write of the page before");
  assert!(!writes.applies(&write("settings", "second", 1), "ai"));
}
