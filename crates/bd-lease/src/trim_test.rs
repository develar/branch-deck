//! `brd lease trim` and the trim of a get below the minimum free space (spec/cli/lease.spec.md, "Trim"): the free slots
//! past `branchdeck.leaseMaxSlots`, the ones returned longest ago first, go with their Bazel output bases, and so do the
//! output bases of gone workspaces of a pool. The output bases are fakes: an output user root whose directories name
//! their workspace in `DO_NOT_BUILD_HERE`, found through the `out/bazel-out` link of the source checkout

use crate::lease_test::{Fixture, path_of, slot_path, states};
#[cfg(unix)]
use crate::lease_test::{Killed, busy_in};
use bd_cli::fixture_test::{assert_error, canonical, git};
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::path::{Path, PathBuf};
use test_utils::links::directory_link;

/// The bytes of the file in each fake output base: a delete that frees them counts them
const BASE_BYTES: usize = 64 * 1024;

/// Three slots of the pool, returned in order (the first longest ago), and an output user root with an output base for
/// each, for `community/` of the first, for the source checkout, for a gone slot of the pool and for a workspace
/// outside every pool
struct TrimFixture {
  fixture: Fixture,
  /// The slots, the first returned first
  slots: [String; 3],
  root: PathBuf,
}

impl TrimFixture {
  fn new() -> Self {
    let fixture = Fixture::new();
    let slots: Vec<String> = (1..=3).map(|n| path_of(&fixture.get(&["--holder", &format!("lane-{n}")]))).collect();
    for slot in &slots {
      fixture.give_back(slot);
    }
    let slots: [String; 3] = slots.try_into().unwrap();
    let root = canonical(fixture.home.path()).join("bazel-root");
    let pool = Path::new(&slots[0]).parent().unwrap().parent().unwrap().to_path_buf();
    let source = canonical(fixture.repo.path());
    for (name, workspace) in [
      ("slot-1", PathBuf::from(&slots[0])),
      ("slot-1-community", Path::new(&slots[0]).join("community")),
      ("slot-2", PathBuf::from(&slots[1])),
      ("slot-3", PathBuf::from(&slots[2])),
      ("main", source.clone()),
      ("gone", pool.join("99").join("repo")),
      ("outside", PathBuf::from("/nonexistent/outside")),
    ] {
      let base = root.join(name);
      std::fs::create_dir_all(base.join("execroot/_main/bazel-out")).unwrap();
      std::fs::write(base.join("DO_NOT_BUILD_HERE"), workspace.to_string_lossy().as_bytes()).unwrap();
      std::fs::write(base.join("execroot/_main/bazel-out/action"), vec![7u8; BASE_BYTES]).unwrap();
    }
    // the source checkout is a workspace whose link leads into the root, as `--symlink_prefix=out/bazel-` writes it
    std::fs::write(source.join("MODULE.bazel"), "").unwrap();
    std::fs::create_dir_all(source.join("out")).unwrap();
    directory_link(&root.join("main/execroot/_main/bazel-out"), &source.join("out/bazel-out"));
    Self { fixture, slots, root }
  }

  fn base(&self, name: &str) -> PathBuf {
    self.root.join(name)
  }

  /// `brd lease trim <args>`, which must succeed
  fn trim(&self, args: &[&str]) -> Value {
    let (exit_code, output) = self.fixture.run(&[&["lease", "trim"][..], args].concat());
    assert_eq!(exit_code, 0, "{output}");
    output
  }

  /// The paths of the slots `output` names in `pools[0].<key>`
  fn named(output: &Value, key: &str) -> Vec<String> {
    output["pools"][0][key]
      .as_array()
      .map_or_else(Vec::new, |slots| slots.iter().map(|slot| slot["path"].as_str().unwrap().to_string()).collect())
  }
}

/// The paths of the worktrees git knows in `fixture`'s repository
fn worktrees(fixture: &Fixture) -> Vec<String> {
  git(&fixture.repo, &["worktree", "list", "--porcelain"])
    .lines()
    .filter_map(|line| line.strip_prefix("worktree "))
    .map(slot_path)
    .collect()
}

#[test_log::test]
fn a_trim_deletes_the_free_slots_past_the_limit_returned_longest_ago_first_with_their_output_bases() {
  let trim = TrimFixture::new();
  let fixture = &trim.fixture;
  git(&fixture.repo, &["config", "branchdeck.leaseMaxSlots", "1"]);
  let output = trim.trim(&[]);
  let [first, second, third] = &trim.slots;
  assert_eq!(TrimFixture::named(&output, "removed"), [first.clone(), second.clone()], "{output}");
  assert_eq!((&output["pools"][0]["max_slots"], &output["pools"][0]["slots"]), (&Value::from(1), &Value::from(3)));
  // each slot names its output bases, and the delete of its directory and of them counted
  let removed = &output["pools"][0]["removed"][0];
  let bases: Vec<&str> = removed["output_bases"].as_array().unwrap().iter().map(|base| base["path"].as_str().unwrap()).collect();
  assert_eq!(bases, [trim.base("slot-1").to_str().unwrap(), trim.base("slot-1-community").to_str().unwrap()], "{output}");
  assert!(removed["freed_bytes"].as_u64().unwrap() >= 2 * BASE_BYTES as u64, "{output}");
  // the number directories go, git forgets the worktrees, and the state keeps the slot returned last
  for slot in [first, second] {
    assert!(!Path::new(slot).parent().unwrap().exists(), "{slot}");
  }
  assert!(Path::new(third).is_dir());
  assert_eq!(worktrees(fixture), [canonical(fixture.repo.path()).to_string_lossy().into_owned(), third.clone()]);
  assert_eq!(states(&fixture.status()), [(third.clone(), "free".to_string())]);
  // the output bases of the deleted slots and of the gone workspace of the pool go; the others stay
  let orphans: Vec<&str> = output["orphans"].as_array().unwrap().iter().map(|base| base["path"].as_str().unwrap()).collect();
  assert_eq!(orphans, [trim.base("gone").to_str().unwrap()], "{output}");
  for gone in ["slot-1", "slot-1-community", "slot-2", "gone"] {
    assert!(!trim.base(gone).exists(), "{gone}");
  }
  for kept in ["slot-3", "main", "outside"] {
    assert!(trim.base(kept).join("DO_NOT_BUILD_HERE").is_file(), "{kept}");
  }
  // the trash of the run is empty and gone
  let trash = fixture.repo.path().join(".git/branch-deck/trash");
  assert_eq!(std::fs::read_dir(&trash).map_or(0, Iterator::count), 0);
  // the kept slot is leased as before
  assert_eq!(path_of(&fixture.get(&["--holder", "after"])), *third);
}

#[test_log::test]
fn a_trim_plan_names_what_would_go_and_deletes_nothing() {
  let trim = TrimFixture::new();
  let fixture = &trim.fixture;
  let output = trim.trim(&["--plan", "--max", "2"]);
  assert_eq!(TrimFixture::named(&output, "removed"), [trim.slots[0].clone()], "{output}");
  assert_eq!((&output["plan"], &output["freed_bytes"]), (&Value::from(true), &Value::from(0)));
  assert!(output.get("volume_freed_bytes").is_none(), "{output}");
  assert_eq!(states(&fixture.status()).len(), 3);
  for slot in &trim.slots {
    assert!(Path::new(slot).is_dir(), "{slot}");
  }
  for kept in ["slot-1", "slot-1-community", "gone"] {
    assert!(trim.base(kept).is_dir(), "{kept}");
  }
}

#[test_log::test]
fn a_pool_within_its_limit_is_not_trimmed_but_the_output_bases_of_gone_workspaces_go() {
  let trim = TrimFixture::new();
  let output = trim.trim(&[]);
  assert_eq!((TrimFixture::named(&output, "removed"), &output["pools"][0]["max_slots"]), (Vec::new(), &Value::from(10)));
  assert_eq!(output["orphans"].as_array().unwrap().len(), 1, "{output}");
  assert!(!trim.base("gone").exists());
  assert!(trim.base("slot-1").is_dir());
}

#[cfg(unix)]
#[test_log::test]
fn a_free_slot_with_a_process_in_it_stays_and_the_next_one_goes() {
  let trim = TrimFixture::new();
  let [first, second, third] = &trim.slots;
  let _busy = Killed(busy_in(first));
  let output = trim.trim(&["--max", "1"]);
  assert_eq!(TrimFixture::named(&output, "busy"), std::slice::from_ref(first), "{output}");
  assert_eq!(TrimFixture::named(&output, "removed"), [second.clone(), third.clone()], "{output}");
  assert!(Path::new(first).is_dir());
  assert!(trim.base("slot-1").is_dir());
  assert_eq!(states(&trim.fixture.status()), [(first.clone(), "free".to_string())]);
}

/// Below a minimum no volume has, the trim deletes the slots past the limit first, the orphans with them, and the get
/// refuses naming them; the slot within the limit stays with its output base, as no shrink and no eviction runs for a
/// minimum past the size of the volume. Where no free space is measured (Windows) nothing is checked: the get leases
/// and nothing is deleted
#[test_log::test]
fn a_get_below_the_minimum_free_space_trims_the_pools_first_and_names_what_it_deleted() {
  let trim = TrimFixture::new();
  let fixture = &trim.fixture;
  git(&fixture.repo, &["config", "branchdeck.leaseMaxSlots", "1"]);
  git(&fixture.repo, &["config", "branchdeck.leaseMinFree", "1P"]);
  let (exit_code, output) = fixture.run(&["lease", "get", "--holder", "low"]);
  if cfg!(not(unix)) {
    assert_eq!(exit_code, 0, "{output}");
    assert!(output.get("trimmed").is_none(), "{output}");
    assert_eq!(states(&fixture.status()).len(), 3);
    for kept in ["slot-1", "gone", "slot-3"] {
      assert!(trim.base(kept).is_dir(), "{kept}");
    }
    return;
  }
  assert_error(&output, exit_code, (4, "low_disk_space"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("the get deleted 2 free slot(s) past `branchdeck.leaseMaxSlots`"), "{message}");
  assert!(!message.contains("below `branchdeck.leaseMaxSlots`,"), "{message}");
  assert_eq!(states(&fixture.status()), [(trim.slots[2].clone(), "free".to_string())]);
  for gone in ["slot-1", "gone"] {
    assert!(!trim.base(gone).exists(), "{gone}");
  }
  assert!(trim.base("slot-3").is_dir());
  assert!(trim.base("main").join("DO_NOT_BUILD_HERE").is_file());
}

#[test_log::test]
fn the_disk_report_names_the_bytes_of_each_slot_and_keeps_them_until_the_slot_is_leased() {
  let trim = TrimFixture::new();
  let fixture = &trim.fixture;
  let [first, second, third] = &trim.slots;
  // a build directory of the clean rules of the slot (`target`, the rules of the fixture's `.branchdeck.toml`)
  std::fs::write(
    Path::new(first).join(".branchdeck.toml"),
    "[lease]\nclean = [{ cargo = \"target\", keep = 8, age = \"1h\" }]\n",
  )
  .unwrap();
  std::fs::create_dir_all(Path::new(first).join("target/debug")).unwrap();
  std::fs::write(Path::new(first).join("target/debug/bin"), vec![1u8; BASE_BYTES]).unwrap();
  // a file of the worktree of its own, which shares no block with the clone the slot came from
  std::fs::write(Path::new(first).join("notes.bin"), vec![2u8; 2 * BASE_BYTES]).unwrap();
  let report = |args: &[&str]| {
    let (exit_code, output) = fixture.run(&[&["lease", "disk"][..], args].concat());
    assert_eq!(exit_code, 0, "{output}");
    output
  };
  let slot = |output: &Value, path: &str| -> Value {
    output["pools"][0]["slots"]
      .as_array()
      .unwrap()
      .iter()
      .find(|slot| slot["path"] == path)
      .unwrap_or_else(|| panic!("no {path}: {output}"))
      .clone()
  };
  let output = report(&[]);
  let walked = slot(&output, first);
  assert_eq!((&walked["state"], &walked["cached"]), (&Value::from("free"), &Value::from(false)), "{output}");
  assert_eq!(walked["build"][0]["path"], "target", "{output}");
  assert!(walked["build"][0]["bytes"].as_u64().unwrap() >= BASE_BYTES as u64, "{output}");
  let bases: Vec<&str> = walked["output_bases"].as_array().unwrap().iter().map(|base| base["path"].as_str().unwrap()).collect();
  assert_eq!(bases, [trim.base("slot-1").to_str().unwrap(), trim.base("slot-1-community").to_str().unwrap()]);
  // the worktree counts its own file, not the build directory, which counts apart
  let worktree = walked["worktree_bytes"].as_u64().unwrap();
  assert!((2 * BASE_BYTES as u64..3 * BASE_BYTES as u64).contains(&worktree), "{output}");
  assert!(walked["bytes"].as_u64().unwrap() >= 5 * BASE_BYTES as u64, "{output}");
  // the output base of the source checkout and the orphan are named apart, the one outside every pool not at all
  assert_eq!(output["pools"][0]["checkout_bases"][0]["path"], trim.base("main").to_str().unwrap(), "{output}");
  assert_eq!(output["orphans"][0]["path"], trim.base("gone").to_str().unwrap(), "{output}");
  assert_eq!((&output["pools"][0]["max_slots"], &output["pools"][0]["past_limit"]), (&Value::from(10), &Value::from(0)));
  // a repeat answers from `disk.json`; a limit of 2 marks the slot a trim deletes next, the one returned first
  git(&fixture.repo, &["config", "branchdeck.leaseMaxSlots", "2"]);
  let again = report(&[]);
  assert_eq!((&again["pools"][0]["max_slots"], &again["pools"][0]["past_limit"]), (&Value::from(2), &Value::from(1)));
  assert_eq!((&slot(&again, first)["trims_next"], slot(&again, second).get("trims_next")), (&Value::from(true), None));
  // a get of the slot returned last makes its bytes stale
  assert_eq!(slot(&again, first)["cached"], true, "{again}");
  assert_eq!(slot(&again, first)["bytes"], walked["bytes"]);
  assert_eq!(path_of(&fixture.get(&["--holder", "later"])), *third);
  git(&fixture.repo, &["config", "--unset", "branchdeck.leaseMaxSlots"]);
  let leased = report(&[]);
  assert_eq!(
    (&slot(&leased, third)["state"], &slot(&leased, third)["cached"]),
    (&Value::from("ready"), &Value::from(false))
  );
  assert_eq!(slot(&leased, second)["cached"], true);
  assert_eq!(slot(&report(&["--fresh"]), first)["cached"], false);
}
