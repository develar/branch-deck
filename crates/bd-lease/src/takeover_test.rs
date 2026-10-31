//! The takeover of an idle lease by a get, `bd lease reclaim` and the refusal of a resumed session
//! (spec/cli/lease.spec.md, "Takeover"): an idle lease is faked by a holder process that is gone and a get and a HEAD
//! older than the quiet time
use crate::lease_test::{Fixture, assert_trash_deleted_once_free, commit_content, commit_in, path_of, proofs, slot_git, slot_path, states};
#[cfg(unix)]
use bd_cli::fixture_test::PREFIX;
use bd_cli::fixture_test::{assert_covered, assert_error, make_lease_idle, run_in};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

/// The lease of the slot at `path` idle (`make_lease_idle`)
fn make_idle(fixture: &Fixture, path: &str) {
  make_lease_idle(&fixture.state_file(), &slot_path(path));
}

/// Change the lease of the slot at `path` in the state file
fn update_lease(fixture: &Fixture, path: &str, change: impl FnOnce(&mut Value)) {
  let mut state: Value = serde_json::from_slice(&std::fs::read(fixture.state_file()).unwrap()).unwrap();
  let path = slot_path(path);
  let slot = state["slots"].as_array_mut().unwrap().iter_mut().find(|slot| slot["path"] == json!(path)).unwrap();
  change(&mut slot["lease"]);
  std::fs::write(fixture.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();
}

/// The slot of `bd lease status` at `path`
fn slot_status(fixture: &Fixture, path: &str) -> Value {
  let path = slot_path(path);
  fixture.status()["slots"]
    .as_array()
    .unwrap()
    .iter()
    .find(|slot| slot["path"] == json!(path))
    .cloned()
    .unwrap()
}

/// A lease of `holder` on the branch `lane` whose commit landed on master by a pick without a follow, onto a commit of
/// a peer: the slot's HEAD is the commit itself, master has its copy. Returns the get and the commit
fn landed_lease(fixture: &Fixture, holder: &str) -> (Value, String) {
  landed_lease_on(fixture, holder, "lane")
}

/// `landed_lease` on the branch `branch`
fn landed_lease_on(fixture: &Fixture, holder: &str, branch: &str) -> (Value, String) {
  let got = fixture.get(&["--holder", holder, "-b", branch]);
  let path = path_of(&got);
  let commit = commit_in(&path, &format!("({branch}) Add c"), &format!("{branch}-c.txt"));
  let peer = if branch == "lane" { "p.txt".to_string() } else { format!("p-{branch}.txt") };
  fixture.repo.create_commit("(peer) Add p", &peer, "p\n");
  let (exit_code, picked) = run_in(&fixture.repo, &["pick", "--from", &path]);
  assert_eq!(exit_code, 0, "{picked}");
  (got, commit)
}

/// A get with no free slot takes over the idle lease whose work landed instead of adding a slot: it names the lease, its
/// holder, its branch, which stays, and each landed commit with its proof; the slot remembers the lease it took
#[test_log::test]
fn a_get_takes_over_an_idle_lease_whose_commits_landed() {
  let fixture = Fixture::new();
  let (old, commit) = landed_lease(&fixture, "gone-agent");
  let path = path_of(&old);
  make_idle(&fixture, &path);

  let got = fixture.get(&["--holder", "next-agent"]);
  assert_eq!((path_of(&got), &got["reused"]), (path.clone(), &json!(true)), "{got}");
  assert_eq!(
    (&got["took_over"]["holder"], &got["took_over"]["lease_id"], &got["took_over"]["branch"]),
    (&json!("gone-agent"), &old["lease_id"], &json!("lane")),
    "{got}"
  );
  assert_eq!(proofs(&got["took_over"]["landed"]), vec![(commit, "picked".to_string())], "{got}");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), fixture.repo.head(), "the slot is at the source HEAD");
  assert_eq!(slot_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]), "HEAD", "detached");
  assert!(fixture.repo.git(&["branch", "--list", "lane"]).contains("lane"), "the branch of the old lease stays");
  let slot = slot_status(&fixture, &path);
  assert_eq!(
    (&slot["reclaimed"]["lease_id"], &slot["reclaimed"]["by_holder"], &slot["lease"]["holder"]),
    (&old["lease_id"], &json!("next-agent"), &json!("next-agent")),
    "{slot}"
  );
  assert_eq!(states(&fixture.status()).len(), 1, "no slot was added");
  assert_matches_schema(&fixture, "lease_get", &got);
  assert_matches_schema(&fixture, "lease_status", &fixture.status());
}

/// `output` has every field the schema of `name` (`bd schema --all`) requires, and no other
fn assert_matches_schema(fixture: &Fixture, name: &str, output: &Value) {
  let (_, schema) = fixture.run(&["schema", "--all"]);
  let schema = &schema["outputs"][name];
  assert_covered(schema, schema, output, name);
}

/// The commit of an idle lease landed on master with a conflict resolution: the twin of `bd update` proves it, with its
/// fixup, and the get takes the lease over
#[test_log::test]
fn a_get_takes_over_an_idle_lease_whose_commit_landed_with_a_conflict_resolution() {
  let fixture = Fixture::new();
  fixture.repo.create_commit("Add f", "f.txt", "a\nz\n");
  let path = path_of(&fixture.get(&["--holder", "gone-agent"]));
  let commit = commit_content(&path, "(lane) Add b to f", "f.txt", "a\nb\nz\n");
  let fixup = commit_content(&path, "fixup! (lane) Add b to f", "f.txt", "a\nb\nb2\nz\n");
  fixture.repo.create_commit("(peer) Add m to f", "f.txt", "a\nm\nz\n");
  std::fs::write(fixture.repo.path().join("f.txt"), "a\nb\nb2\nm\nz2\n").unwrap();
  fixture.repo.git(&["commit", "-q", "-a", "-C", &commit]);
  make_idle(&fixture, &path);

  let got = fixture.get(&[]);
  assert_eq!(path_of(&got), path, "{got}");
  assert_eq!(
    proofs(&got["took_over"]["landed"]),
    vec![(fixup, "twin".to_string()), (commit, "twin".to_string())],
    "{got}"
  );
}

/// An idle lease with work its home branch lacks is never taken over: an unlanded commit, local changes, a rebase in
/// progress. The get adds a slot and names each with its work, and the status shows it as `idle_with_work`
#[test_log::test]
fn an_idle_lease_with_unlanded_work_is_kept_and_named() {
  let fixture = Fixture::new();
  let with_commit = path_of(&fixture.get(&["--holder", "with-commit"]));
  let commit = commit_in(&with_commit, "(lane) Add c", "lane-c.txt");
  let dirty = path_of(&fixture.get(&["--holder", "dirty"]));
  std::fs::write(Path::new(&dirty).join("left.txt"), "left\n").unwrap();
  let rebasing = path_of(&fixture.get(&["--holder", "rebasing"]));
  let git_dir = slot_git(&rebasing, &["rev-parse", "--absolute-git-dir"]);
  std::fs::create_dir_all(Path::new(&git_dir).join("rebase-merge")).unwrap();
  for path in [&with_commit, &dirty, &rebasing] {
    make_idle(&fixture, path);
  }

  let got = fixture.get(&[]);
  assert_eq!(got["reused"], json!(false), "{got}");
  assert_matches_schema(&fixture, "lease_get", &got);
  let skipped: Vec<(String, Value)> = got["skipped"]
    .as_array()
    .unwrap()
    .iter()
    .map(|skipped| (skipped["reason"].as_str().unwrap().to_string(), skipped["idle"]["holder"].clone()))
    .collect();
  assert_eq!(skipped.len(), 2, "two full checks per get: {got}");
  assert!(skipped.iter().all(|(reason, _)| reason == "idle_with_work"), "{got}");
  // the third is checked by the next get
  fixture.get(&[]);
  let idle = |path: &str| slot_status(&fixture, path)["lease"]["idle"].clone();
  assert_eq!(idle(&with_commit)["state"], json!("idle_with_work"));
  assert_eq!(idle(&with_commit)["commits"], json!([{ "commit": commit, "subject": "(lane) Add c" }]));
  assert_eq!(idle(&dirty)["paths"], json!(["left.txt"]));
  assert_eq!(idle(&rebasing)["operation"], json!("rebase"));
  for path in [&with_commit, &dirty, &rebasing] {
    assert_eq!(slot_status(&fixture, path)["state"], json!("ready"), "{path} stays leased");
  }
}

/// The commits of an idle lease on its own branch: that branch holds them, its home branch does not, and only the home
/// branch counts
#[test_log::test]
fn an_idle_lease_on_its_own_branch_is_taken_over_only_when_home_has_its_commits() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["--holder", "gone-agent", "-b", "lane"]));
  let commit = commit_in(&path, "(lane) Add c", "lane-c.txt");
  make_idle(&fixture, &path);

  let got = fixture.get(&[]);
  assert_ne!(path_of(&got), path, "{got}");
  assert_eq!(slot_status(&fixture, &path)["lease"]["idle"]["commits"][0]["commit"], json!(commit));
}

/// A get that found work in an idle lease records what the verdict rests on: the next get passes over the lease without
/// a new check while the slot and the home tip stay (the same `checked_at`), and takes it over once the commit landed
#[test_log::test]
fn a_kept_idle_lease_is_checked_again_only_when_its_slot_or_home_moved() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["--holder", "gone-agent"]));
  let commit = commit_in(&path, "(lane) Add c", "lane-c.txt");
  make_idle(&fixture, &path);
  let first = fixture.get(&[]);
  let checked_at = slot_status(&fixture, &path)["lease"]["idle"]["checked_at"].clone();
  update_lease(&fixture, &path, |lease| lease["idle_check"]["at"] = json!(1));
  fixture.give_back(&path_of(&first));

  // the free slot first, then the kept one passed over by its record
  let second = fixture.get(&[]);
  assert_eq!(path_of(&second), path_of(&first), "{second}");
  fixture.give_back(&path_of(&second));
  assert_ne!(checked_at, json!(1));
  let third = fixture.get(&[]);
  let fourth = fixture.get(&[]);
  assert_eq!(slot_status(&fixture, &path)["lease"]["idle"]["checked_at"], json!(1), "{third} {fourth}");

  fixture.repo.git(&["cherry-pick", "-x", &commit]);
  let fifth = fixture.get(&[]);
  assert_eq!(path_of(&fifth), path, "{fifth}");
  assert_eq!(proofs(&fifth["took_over"]["landed"]), vec![(commit, "picked".to_string())], "{fifth}");
}

/// A get killed while it checks an idle lease leaves its `preparing` lease with the idle one in `taking_over`: the next
/// get and `bd lease clean` give the idle lease back, never a free slot
#[test_log::test]
fn a_get_killed_while_it_checks_an_idle_lease_gives_the_lease_back() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["--holder", "gone-agent"]));
  commit_in(&path, "(lane) Add c", "lane-c.txt");
  make_idle(&fixture, &path);
  let killed = |fixture: &Fixture| {
    update_lease(fixture, &path, |lease| {
      let old = lease.clone();
      *lease = json!({
        "id": "killed-get", "holder": "killed", "leased_at": 1, "state": "preparing", "pid": u32::MAX - 1,
        "source_head": old["source_head"], "taking_over": old,
      });
    });
  };

  killed(&fixture);
  let (exit_code, cleaned) = fixture.run(&["lease", "clean", &path]);
  assert_eq!(exit_code, 0, "{cleaned}");
  assert_eq!(cleaned["targets"][0]["skipped"], json!("leased by gone-agent"), "{cleaned}");
  assert_eq!(slot_status(&fixture, &path)["lease"]["holder"], json!("gone-agent"));

  killed(&fixture);
  assert_eq!(slot_status(&fixture, &path)["lease"]["holder"], json!("gone-agent"), "the status shows it back too");
  let got = fixture.get(&[]);
  assert_ne!(path_of(&got), path, "{got}");
  let slot = slot_status(&fixture, &path);
  assert_eq!((&slot["state"], &slot["lease"]["holder"]), (&json!("ready"), &json!("gone-agent")), "{slot}");
}

/// A resumed session of the old holder in a slot taken over is refused (`lease_reclaimed`), with the next step: its
/// writes and `bd workspace`, its reads not. The session that holds the slot now passes, and a coordinator with the
/// current lease id; once the new lease is returned, a write in the free slot is refused too
#[cfg(unix)]
#[test_log::test]
fn a_command_of_another_session_in_a_taken_over_slot_is_refused_as_lease_reclaimed() {
  let fixture = Fixture::new();
  let (old, _) = landed_lease(&fixture, "gone-agent");
  let path = path_of(&old);
  make_idle(&fixture, &path);
  let got = fixture.get(&["--holder", "next-agent"]);
  let slot = Path::new(&path);
  let (exit_code, output) = fixture.run_in(slot, &["--prefix", PREFIX, "lease", "pick", "--dry-run"]);
  assert_eq!(exit_code, 0, "the holder's own session: {output}");

  let other = crate::lease_test::Killed(crate::lease_test::busy_in(fixture.repo.path().to_str().unwrap()));
  let started = sync_utils::process_scan::process_started(other.0.id());
  update_lease(&fixture, &path, |lease| {
    lease["holder_pid"] = json!(other.0.id());
    lease["holder_started"] = json!(started.unwrap_or(1));
  });
  let (exit_code, output) = fixture.run_in(slot, &["lease", "return"]);
  assert_error(&output, exit_code, (4, "lease_reclaimed"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("gone-agent") && message.contains("next-agent") && message.contains("lane"), "{output}");
  assert_eq!(
    output["error"]["next_steps"][0]["command"],
    json!(format!("bd lease get --holder gone-agent -C {}", slot_path(fixture.repo.path().to_str().unwrap()))),
    "{output}"
  );
  assert_matches_schema(&fixture, "error", &output);
  let (exit_code, output) = fixture.run_in(slot, &["workspace"]);
  assert_error(&output, exit_code, (4, "lease_reclaimed"));
  let (exit_code, output) = fixture.run_in(slot, &["--prefix", PREFIX, "status"]);
  assert_eq!(exit_code, 0, "a read passes: {output}");

  let lease_id = got["lease_id"].as_str().unwrap();
  let (exit_code, output) = fixture.run(&["lease", "return", "--lease-id", lease_id]);
  assert_eq!(exit_code, 0, "a coordinator names the current lease: {output}");
  let (exit_code, output) = fixture.run_in(slot, &["--prefix", PREFIX, "lease", "pick"]);
  assert_error(&output, exit_code, (4, "lease_reclaimed"));
}

/// `bd lease reclaim --idle` returns the idle leases whose work landed and keeps the rest; `--dry-run` reports the same
/// and writes nothing
#[test_log::test]
fn reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest() {
  let fixture = Fixture::new();
  let (landed, commit) = landed_lease(&fixture, "landed-agent");
  let landed = path_of(&landed);
  let kept = path_of(&fixture.get(&["--holder", "kept-agent"]));
  let unlanded = commit_in(&kept, "(other) Add d", "other-d.txt");
  let young = path_of(&fixture.get(&["--holder", "young-agent"]));
  make_idle(&fixture, &landed);
  make_idle(&fixture, &kept);
  let state = std::fs::read(fixture.state_file()).unwrap();

  let (exit_code, dry_run) = fixture.run(&["lease", "reclaim", "--idle", "--dry-run"]);
  assert_eq!(exit_code, 0, "{dry_run}");
  assert_eq!(std::fs::read(fixture.state_file()).unwrap(), state, "a dry run writes nothing");
  let (exit_code, reclaimed) = fixture.run(&["lease", "reclaim", "--idle"]);
  assert_eq!(exit_code, 0, "{reclaimed}");
  for output in [&dry_run, &reclaimed] {
    assert_eq!(output["reclaimed"][0]["path"], json!(slot_path(&landed)), "{output}");
    assert_eq!(proofs(&output["reclaimed"][0]["landed"]), vec![(commit.clone(), "picked".to_string())], "{output}");
    assert_eq!(
      (&output["kept"][0]["holder"], &output["kept"][0]["idle"]["commits"][0]["commit"]),
      (&json!("kept-agent"), &json!(unlanded)),
      "{output}"
    );
    assert_eq!(output["reclaimed"].as_array().unwrap().len() + output["kept"].as_array().unwrap().len(), 2, "{output}");
  }
  assert_matches_schema(&fixture, "lease_reclaim", &dry_run);
  assert_matches_schema(&fixture, "lease_reclaim", &reclaimed);
  assert_matches_schema(&fixture, "lease_status", &fixture.status());
  let slot = slot_status(&fixture, &landed);
  assert_eq!((&slot["state"], &slot["reclaimed"]["holder"]), (&json!("free"), &json!("landed-agent")), "{slot}");
  assert_eq!(slot_status(&fixture, &kept)["lease"]["idle"]["state"], json!("idle_with_work"));
  assert_eq!(slot_status(&fixture, &young)["state"], json!("ready"));
}

/// `bd lease reclaim` frees every slot it reclaims before it deletes their trash: what the clean rules of the first slot
/// moved into the trash is deleted only after the second slot is free, and nothing of it is left
#[test]
fn reclaim_deletes_the_trash_once_every_reclaimed_slot_is_free() {
  let log = test_utils::captured_log::CapturedLog::install(false);
  let fixture = Fixture::new();
  fixture.repo.create_commit_with_files(
    "Add the clean rules",
    &[(".gitignore", "out/\n"), (".branchdeck.toml", "[lease]\nclean = [\n  { delete = \"out/cache\" },\n]\n")],
  );
  let slots: Vec<String> = ["first", "second"]
    .into_iter()
    .map(|holder| {
      let path = path_of(&landed_lease_on(&fixture, holder, holder).0);
      std::fs::create_dir_all(Path::new(&path).join("out")).unwrap();
      std::fs::write(Path::new(&path).join("out/cache"), vec![b'x'; 1 << 16]).unwrap();
      path
    })
    .collect();
  for path in &slots {
    make_idle(&fixture, path);
  }

  let (exit_code, reclaimed) = fixture.run(&["lease", "reclaim", "--idle"]);
  assert_eq!(exit_code, 0, "{reclaimed}");
  assert_eq!(reclaimed["reclaimed"].as_array().unwrap().len(), 2, "{reclaimed}");
  for (slot, output) in slots.iter().zip(reclaimed["reclaimed"].as_array().unwrap()) {
    assert!(!Path::new(slot).join("out/cache").exists(), "{slot}");
    assert_eq!(output["trash_left"], Value::Null, "nothing is left in the trash: {output}");
    assert!(output["cleaned"][0]["freed_bytes"].as_u64().unwrap() > 0, "{output}");
  }
  assert_trash_deleted_once_free(&fixture, &log.text(), ("an idle lease is reclaimed", 2), "deleting the trash of the reclaimed slots");
}

/// Below the minimum free space a get deletes the trash of the runs that are gone before it refuses, which a takeover
/// leaves for later, and names what the trash of a running process keeps
#[test_log::test]
fn a_get_below_the_minimum_free_space_deletes_the_trash_of_gone_runs_first() {
  let fixture = Fixture::new();
  let trash = fixture.repo.path().join(".git/branch-deck/trash");
  let mut child = std::process::Command::new("true").spawn().unwrap();
  let (pid, started) = (child.id(), sync_utils::process_scan::process_started(child.id()).unwrap_or(1));
  child.wait().unwrap();
  let gone = trash.join(format!("{pid}-{started}-gone"));
  let me = std::process::id();
  let running = trash.join(format!("{me}-{}-running", sync_utils::process_scan::process_started(me).unwrap_or(1)));
  for dir in [&gone, &running] {
    std::fs::create_dir_all(dir.join("0")).unwrap();
    std::fs::write(dir.join("0").join("out.bin"), vec![1; 1 << 20]).unwrap();
  }

  let (exit_code, output) = fixture.run(&["lease", "get", "--min-free", "1P"]);
  assert_error(&output, exit_code, (4, "low_disk_space"));
  assert!(!gone.exists(), "the trash of a gone run is deleted");
  assert!(running.exists(), "the trash of a running process stays");
  assert!(output["error"]["message"].as_str().unwrap().contains("in the trash of a run that still runs"), "{output}");
}
