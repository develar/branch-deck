use pretty_assertions::assert_eq;

use super::*;
use crate::prepare::Prepared;

/// While yonderd has its cache, both agents keep the fingerprints of a slot in one file: a step that yonderd ran over
/// the work of bd-host is not current for bd-host. After the retire of yonderd the fingerprints are the ones of bd-host
#[test]
fn the_fingerprints_of_a_slot_are_shared_with_yonderd_while_it_has_its_cache() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let home = Home::new(dir.path().to_path_buf());
  assert_eq!(home.slots_dir(), dir.path().join(".cache/bd-host/slots"));

  let yonderd = dir.path().join(".cache/yonderd/slots");
  std::fs::create_dir_all(&yonderd).expect("the slots of yonderd");
  assert_eq!(home.slots_dir(), yonderd);
  // yonderd ran `pnpm install` for another lockfile in the slot, at the same generation
  std::fs::write(yonderd.join("lease-3.prepared"), "generation=7\nprepared.00aa=f-yonderd\n").expect("the file of yonderd");
  let prepared = Prepared::load(&home.slots_dir(), "lease-3", 7);
  assert_eq!(prepared.steps.get("00aa").map(String::as_str), Some("f-yonderd"));

  std::fs::remove_dir_all(dir.path().join(".cache/yonderd")).expect("the retire");
  assert_eq!(home.slots_dir(), dir.path().join(".cache/bd-host/slots"));
}
