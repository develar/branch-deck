//! The quiet local changes of an idle lease, its only work, preserved as patches in its notes by a get or `brd lease
//! reclaim`, which then take the slot (openspec/specs/cli/lease/spec.md, "Get: The quiet local changes of an idle lease
//! are preserved in its notes"). Most tests make every change quiet at once (`branchdeck.leaseIdleChangesAfter` `0s`); a
//! wrapped git edits a file or fails between the steps of a preserve
use crate::lease_test::{Fixture, commit_in, path_of, slot_git, slot_path};
use crate::takeover_test::{assert_kept_beside, assert_matches_schema, reclaim_dry_run, slot_status};
use bd_cli::fixture_test::{make_lease_idle, run_json};
use bd_core::context::Environment;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use test_utils::fake_executable::{fake_executable, shell_path};

/// A pool whose idle leases have quiet local changes at once
fn quiet_fixture() -> Fixture {
  let fixture = Fixture::new();
  fixture.repo.git(&["config", "branchdeck.leaseIdleChangesAfter", "0s"]);
  fixture
}

fn make_idle(fixture: &Fixture, path: &str) {
  make_lease_idle(&fixture.state_file(), &slot_path(path));
}

/// `git status --porcelain` of the slot, its notes left out by `info/exclude`, trimmed as `slot_git` trims it
fn porcelain(path: &str) -> String {
  slot_git(path, &["status", "--porcelain", "--untracked-files=all"])
}

/// git in `path` with `stdin`
fn git_with_input(path: &str, args: &[&str], stdin: &str) {
  GitCommandExecutor::new().cmd(args, path).stdin(stdin.as_bytes()).output().unwrap();
}

/// The trees of the slot at `path` as `git write-tree` of a copy of its index gives them, before and after `add --all`
/// on that copy: the tree of its index and the tree of its files
fn trees_of_slot(path: &str) -> (String, String) {
  let index = slot_git(path, &["rev-parse", "--path-format=absolute", "--git-path", "index"]);
  let copy = tempfile::tempdir().unwrap();
  let copy = copy.path().join("index");
  std::fs::copy(&index, &copy).unwrap();
  let git = |args: &[&str]| GitCommandExecutor::new().cmd(args, path).env(&[("GIT_INDEX_FILE", copy.to_str().unwrap())]).text().unwrap();
  let staged = git(&["write-tree"]).trim().to_string();
  git(&["add", "--all"]);
  (staged, git(&["write-tree"]).trim().to_string())
}

/// The tree that `patch` gives on the tree of `base`, applied in a new index (`apply --cached`), run in `path`
fn tree_of_patch(path: &str, base: &str, patch: &Path) -> String {
  let index = tempfile::tempdir().unwrap();
  let index = index.path().join("index");
  let git = |args: &[&str]| {
    GitCommandExecutor::new()
      .cmd(args, path)
      .env(&[("GIT_INDEX_FILE", index.to_str().unwrap())])
      .text()
      .unwrap()
  };
  git(&["read-tree", base]);
  git(&["apply", "--cached", patch.to_str().unwrap()]);
  git(&["write-tree"]).trim().to_string()
}

/// `brd lease reclaim --idle`, which must succeed and return no lease, and the holder and `not_preserved` of each kept
/// lease
fn reclaim_keeps(fixture: &Fixture) -> Vec<(String, String)> {
  let (exit_code, reclaimed) = fixture.run(&["lease", "reclaim", "--idle"]);
  assert_eq!(exit_code, 0, "{reclaimed}");
  assert_eq!(reclaimed["reclaimed"], json!([]), "{reclaimed}");
  assert_matches_schema(fixture, "lease_reclaim", &reclaimed);
  reclaimed["kept"]
    .as_array()
    .unwrap()
    .iter()
    .map(|kept| {
      (
        kept["holder"].as_str().unwrap().to_string(),
        kept["idle"]["not_preserved"].as_str().unwrap_or("").to_string(),
      )
    })
    .collect()
}

/// A lease of `gone-agent` on the branch `lane` with local changes of every kind (staged, then edited; unstaged;
/// untracked, a path with a space and other than ASCII; a binary file; a deletion; on Unix a link and a mode): the get
/// and the number of the changes
fn varied_lease(fixture: &Fixture) -> (Value, usize) {
  fixture.repo.create_commit_with_files("Add c and run", &[("c.txt", "c\n"), ("run.sh", "echo run\n")]);
  let old = fixture.get(&["--holder", "gone-agent", "-b", "lane"]);
  let path = path_of(&old);
  let slot = Path::new(&path);
  std::fs::write(slot.join("b.txt"), "b\nstaged\n").unwrap();
  slot_git(&path, &["add", "b.txt"]);
  std::fs::write(slot.join("b.txt"), "b\nstaged\nthen edited\n").unwrap();
  std::fs::write(slot.join("a.txt"), "a\nunstaged\n").unwrap();
  std::fs::create_dir_all(slot.join("new")).unwrap();
  std::fs::write(slot.join("new").join("ünï code.txt"), "untracked\n").unwrap();
  std::fs::write(slot.join("blob.bin"), (0..=255u8).collect::<Vec<u8>>()).unwrap();
  std::fs::remove_file(slot.join("c.txt")).unwrap();
  let mut changed = 5;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::os::unix::fs::symlink("a.txt", slot.join("link")).unwrap();
    std::fs::set_permissions(slot.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    changed += 2;
  }
  (old, changed)
}

/// Every file under `root` but `.git` and the notes of a lease, by path: its bytes (a link: its target) and whether it is
/// a link or executable
fn files_of(root: &Path) -> std::collections::BTreeMap<String, (Vec<u8>, bool, bool)> {
  let mut files = std::collections::BTreeMap::new();
  let mut pending = vec![root.to_path_buf()];
  while let Some(dir) = pending.pop() {
    for entry in std::fs::read_dir(&dir).unwrap() {
      let entry = entry.unwrap();
      let name = entry.file_name();
      if dir == root && (name == ".git" || name == "notes.local") {
        continue;
      }
      let metadata = entry.path().symlink_metadata().unwrap();
      let relative = entry.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
      if metadata.is_dir() {
        pending.push(entry.path());
      } else if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(entry.path()).unwrap();
        files.insert(relative, (target.to_string_lossy().into_owned().into_bytes(), true, false));
      } else {
        files.insert(relative, (std::fs::read(entry.path()).unwrap(), false, executable_bit(&metadata)));
      }
    }
  }
  files
}

/// Whether the file of `metadata` has an executable bit, as git keeps its mode; none where the system has no such bit
fn executable_bit(metadata: &std::fs::Metadata) -> bool {
  #[cfg(unix)]
  return std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 != 0;
  #[cfg(not(unix))]
  {
    let _ = metadata;
    false
  }
}

/// A fresh clone at the base commit of the preserve, `git apply` of `changes.patch` there: every file is the file of the
/// slot before the get, byte for byte, with its mode and its links, the untracked files too
#[test_log::test]
fn the_patch_applied_to_a_fresh_checkout_of_its_base_gives_every_file_of_the_slot() {
  let fixture = quiet_fixture();
  let (old, _) = varied_lease(&fixture);
  let path = path_of(&old);
  let before = files_of(Path::new(&path));
  make_idle(&fixture, &path);

  let got = fixture.get(&["--holder", "next-agent"]);
  let preserved = &got["took_over"]["preserved"];
  let clone = tempfile::tempdir().unwrap();
  let checkout = clone.path().join("checkout");
  let checkout_text = checkout.to_str().unwrap();
  let git = GitCommandExecutor::new();
  git
    .execute_command(
      &["clone", "--quiet", "--no-checkout", fixture.repo.path().to_str().unwrap(), checkout_text],
      clone.path().to_str().unwrap(),
    )
    .unwrap();
  git
    .execute_command(&["checkout", "--quiet", "--detach", preserved["base"].as_str().unwrap()], checkout_text)
    .unwrap();
  let patch = Path::new(preserved["dir"].as_str().unwrap()).join("changes.patch");
  git.execute_command(&["apply", patch.to_str().unwrap()], checkout_text).unwrap();
  assert_eq!(files_of(&checkout), before);
}

/// A get takes over an idle lease whose only work is quiet local changes (staged, unstaged, untracked, a deletion, a
/// binary file, a path with a space and other than ASCII, and on Unix a link and a mode): it preserves them in the
/// notes of the lease, the record of the notes names the patches, and the slot is clean at the HEAD of the source.
/// `changes.patch` on its base gives the whole tree of the files of the slot before, `staged.patch` the tree of its
/// index
#[test_log::test]
fn a_get_preserves_the_quiet_local_changes_of_an_idle_lease_and_takes_its_slot() {
  let fixture = quiet_fixture();
  let (old, changed) = varied_lease(&fixture);
  let path = path_of(&old);
  let (index_tree, files_tree) = trees_of_slot(&path);
  make_idle(&fixture, &path);

  let got = fixture.get(&["--holder", "next-agent"]);
  assert_eq!(path_of(&got), path, "{got}");
  assert_matches_schema(&fixture, "lease_get", &got);
  let preserved = &got["took_over"]["preserved"];
  assert_eq!(
    (&got["took_over"]["holder"], &preserved["paths"], &preserved["base"], &preserved["staged"]),
    (&json!("gone-agent"), &json!(changed), &old["source_head"], &json!(true)),
    "{got}"
  );
  assert_eq!(porcelain(&path), "", "the slot is clean");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), fixture.repo.head(), "at the source HEAD");
  assert!(fixture.repo.git(&["branch", "--list", "lane"]).contains("lane"), "the branch of the old lease stays");
  let dir = PathBuf::from(preserved["dir"].as_str().unwrap());
  let slot_record = slot_status(&fixture, &path);
  assert_eq!(slot_record["reclaimed"]["preserved"]["dir"], preserved["dir"], "{slot_record}");
  assert_matches_schema(&fixture, "lease_status", &fixture.status());
  let (exit_code, notes) = fixture.run(&["lease", "notes", "--lease-id", old["lease_id"].as_str().unwrap()]);
  assert_eq!(exit_code, 0, "{notes}");
  let record = &notes["notes"][0];
  assert_eq!(record["preserved"]["dir"], preserved["dir"], "{notes}");
  assert!(record["preserved_at"].is_u64() && record["expires_at"].is_u64(), "{notes}");
  assert_matches_schema(&fixture, "lease_notes", &notes);

  let base = preserved["base"].as_str().unwrap();
  assert_eq!(tree_of_patch(&path, base, &dir.join("changes.patch")), files_tree, "the whole tree of the files");
  assert_eq!(tree_of_patch(&path, base, &dir.join("staged.patch")), index_tree, "the tree of the index");
}

/// Local changes touched within the quiet time keep the lease and name when they are quiet, and so do changes beside a
/// commit that the home branch lacks, however quiet. With the key `0` no preserve runs, and once the key turns on again
/// the next get checks the kept lease anew and takes it over
#[test_log::test]
fn the_local_changes_of_an_idle_lease_stay_while_fresh_beside_a_commit_or_with_the_preserve_off() {
  let fixture = Fixture::new();
  fixture.repo.git(&["config", "branchdeck.leaseIdleChangesAfter", "1h"]);
  let fresh = path_of(&fixture.get(&["--holder", "fresh"]));
  std::fs::write(Path::new(&fresh).join("left.txt"), "left\n").unwrap();
  let with_commit = path_of(&fixture.get(&["--holder", "with-commit"]));
  let commit = commit_in(&with_commit, "(lane) Add c", "lane-c.txt");
  std::fs::write(Path::new(&with_commit).join("a.txt"), "a\nedited\n").unwrap();
  for path in [&fresh, &with_commit] {
    make_idle(&fixture, path);
  }

  let got = fixture.get(&[]);
  assert_eq!(got["reused"], json!(false), "{got}");
  let idle = |path: &str| slot_status(&fixture, path)["lease"]["idle"].clone();
  let fresh_idle = idle(&fresh);
  assert_eq!(
    (&fresh_idle["state"], &fresh_idle["paths"]),
    (&json!("idle_with_work"), &json!(["left.txt"])),
    "{fresh_idle}"
  );
  let quiet_at = fresh_idle["quiet_at"].as_u64().unwrap();
  let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
  assert!(quiet_at > now + 3000 && quiet_at <= now + 3600, "an hour after the change: {fresh_idle}");
  assert_eq!(idle(&with_commit)["commits"][0]["commit"], json!(commit));

  fixture.repo.git(&["config", "branchdeck.leaseIdleChangesAfter", "0"]);
  reclaim_keeps(&fixture);
  assert_eq!(idle(&fresh)["quiet_at"], Value::Null, "never quiet");

  fixture.repo.git(&["config", "branchdeck.leaseIdleChangesAfter", "0s"]);
  let taken = fixture.get(&[]);
  assert_eq!(path_of(&taken), fresh, "the key moved: {taken}");
  assert_eq!(taken["took_over"]["preserved"]["paths"], json!(1), "{taken}");
  assert_eq!(porcelain(&with_commit), "M a.txt", "the change beside a commit stays");
  assert_eq!(idle(&with_commit)["state"], json!("idle_with_work"));
}

/// Quiet local changes that a patch can't carry keep the lease, named why (`not_preserved`), and the slot as it was: an
/// unmerged entry, a staged gitlink, a nested repository, a path with a filter (Git LFS), an entry of the index that git
/// does not check (skip-worktree), and a lease without notes
#[test_log::test]
fn quiet_local_changes_that_a_patch_cannot_carry_keep_the_lease() {
  let fixture = quiet_fixture();
  let slot = |holder: &str| path_of(&fixture.get(&["--holder", holder]));
  let unmerged = slot("unmerged");
  let blob = slot_git(&unmerged, &["rev-parse", "HEAD:a.txt"]);
  let other = slot_git(&unmerged, &["rev-parse", "HEAD:b.txt"]);
  git_with_input(
    &unmerged,
    &["update-index", "--index-info"],
    &format!(
      "0 {zero}\ta.txt\n100644 {blob} 1\ta.txt\n100644 {other} 2\ta.txt\n100644 {blob} 3\ta.txt\n",
      zero = "0".repeat(blob.len())
    ),
  );
  let gitlink = slot("gitlink");
  slot_git(&gitlink, &["update-index", "--add", "--cacheinfo", &format!("160000,{},sub", fixture.repo.head())]);
  let nested = slot("nested");
  slot_git(&nested, &["init", "--quiet", "inner"]);
  let filtered = slot("filtered");
  std::fs::write(Path::new(&filtered).join(".gitattributes"), "*.bin filter=lfs\n").unwrap();
  std::fs::write(Path::new(&filtered).join("model.bin"), "model\n").unwrap();
  let hidden = slot("hidden");
  slot_git(&hidden, &["update-index", "--skip-worktree", "a.txt"]);
  std::fs::write(Path::new(&hidden).join("left.txt"), "left\n").unwrap();
  let without_notes = slot("without-notes");
  std::fs::write(Path::new(&without_notes).join("left.txt"), "left\n").unwrap();
  let link = Path::new(&without_notes).join("notes.local");
  let notes = std::fs::read_link(&link).unwrap();
  std::fs::remove_file(notes.with_extension("json")).unwrap();
  let all = [&unmerged, &gitlink, &nested, &filtered, &hidden, &without_notes];
  let before: Vec<String> = all.iter().map(|path| porcelain(path)).collect();
  for path in all {
    make_idle(&fixture, path);
  }

  let kept = reclaim_keeps(&fixture);
  let reason = |holder: &str| reason_of(&kept, holder);
  assert!(reason("unmerged").starts_with("an unmerged entry: a.txt"), "{kept:?}");
  assert!(reason("gitlink").starts_with("a gitlink or a nested repository: sub"), "{kept:?}");
  assert!(reason("nested").starts_with("a gitlink or a nested repository: inner"), "{kept:?}");
  assert!(reason("filtered").starts_with("a path with a filter: "), "{kept:?}");
  assert!(reason("hidden").contains("skip-worktree): a.txt"), "{kept:?}");
  assert_eq!(reason("without-notes"), "the lease has no notes", "{kept:?}");
  let after: Vec<String> = all.iter().map(|path| porcelain(path)).collect();
  assert_eq!(after, before, "every slot keeps its changes");
  for path in all {
    assert_eq!(slot_status(&fixture, path)["lease"]["idle"]["state"], json!("idle_with_work"), "{path}");
  }
}

/// `brd lease reclaim --idle` preserves the quiet local changes of an idle lease and returns it, its slot free and clean
/// with the record of the patches; `--dry-run` names where they would go and writes nothing. A command of the old
/// session in the slot is refused, its message naming the patches
#[test_log::test]
fn reclaim_preserves_the_quiet_local_changes_of_an_idle_lease_and_a_dry_run_names_them() {
  let fixture = quiet_fixture();
  let path = path_of(&fixture.get(&["--holder", "gone-agent"]));
  std::fs::write(Path::new(&path).join("left.txt"), "left\n").unwrap();
  make_idle(&fixture, &path);
  let dry_run = reclaim_dry_run(&fixture);
  assert_matches_schema(&fixture, "lease_reclaim", &dry_run);
  let planned = &dry_run["reclaimed"][0]["preserved"];
  assert_eq!(
    (&planned["paths"], &planned["base"], &planned["bytes"]),
    (&json!(1), &Value::Null, &Value::Null),
    "{dry_run}"
  );
  assert!(!Path::new(planned["dir"].as_str().unwrap()).exists(), "{dry_run}");

  let (exit_code, reclaimed) = fixture.run(&["lease", "reclaim", "--idle"]);
  assert_eq!(exit_code, 0, "{reclaimed}");
  assert_matches_schema(&fixture, "lease_reclaim", &reclaimed);
  let preserved = &reclaimed["reclaimed"][0]["preserved"];
  assert_eq!((&preserved["dir"], &preserved["paths"]), (&planned["dir"], &json!(1)), "{reclaimed}");
  assert!(preserved["base"].is_string() && preserved["bytes"].as_u64().unwrap() > 0, "{reclaimed}");
  let patch = std::fs::read_to_string(Path::new(preserved["dir"].as_str().unwrap()).join("changes.patch")).unwrap();
  assert!(patch.contains("+left"), "{patch}");
  let slot = slot_status(&fixture, &path);
  assert_eq!(
    (&slot["state"], &slot["dirty"], &slot["reclaimed"]["preserved"]["dir"]),
    (&json!("free"), &Value::Null, &preserved["dir"]),
    "{slot}"
  );
  assert_eq!(porcelain(&path), "", "the slot is clean");

  let state: Value = serde_json::from_slice(&std::fs::read(fixture.state_file()).unwrap()).unwrap();
  let record = state["slots"].as_array().unwrap().iter().find(|slot| slot["path"] == json!(slot_path(&path))).unwrap()["reclaimed"].clone();
  let message = lease_pool::lease::LeaseRefusal::LeaseReclaimed {
    path,
    reclaimed: Box::new(serde_json::from_value(record).unwrap()),
    in_progress: false,
  }
  .to_string();
  assert!(message.contains(preserved["dir"].as_str().unwrap()) && message.contains("changes.patch"), "{message}");
}

/// A get that found the local changes of an idle lease fresh checks them again once they were restored by hand, as the
/// stat of their paths moved, and takes the slot over without a patch
#[test_log::test]
fn local_changes_restored_by_hand_are_checked_again_and_the_slot_is_taken_over() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&["--holder", "gone-agent"]));
  std::fs::write(Path::new(&path).join("a.txt"), "a\nprobe\n").unwrap();
  make_idle(&fixture, &path);
  let first = fixture.get(&[]);
  assert_eq!(first["skipped"][0]["reason"], json!("idle_with_work"), "{first}");

  slot_git(&path, &["checkout", "--", "a.txt"]);
  let second = fixture.get(&[]);
  assert_eq!(path_of(&second), path, "{second}");
  assert_eq!(second["took_over"]["holder"], json!("gone-agent"), "{second}");
  assert_eq!(second["took_over"]["preserved"], Value::Null, "nothing to preserve: {second}");
}

/// The environment of brd in the repository of `fixture` with a git wrapped in `dir`, which runs `script` before the
/// real git for the arguments that `pattern` (a `case` pattern of ` <args> `) matches
fn wrapped_environment(fixture: &Fixture, dir: &Path, (pattern, script): (&str, &str)) -> Environment {
  let git = fake_executable(dir, "git", &format!("case \" $* \" in\n{pattern}) {script};;\nesac\nexec git \"$@\""));
  let mut environment = fixture.environment(fixture.repo.path());
  environment.git = Some(git.to_str().unwrap().to_string());
  environment
}

/// An edit of a changed file while the get writes the patches (here at their proof, `apply --cached`) moves its stat:
/// the second `status` sees it, the get keeps the lease and its files and adds a slot, and so does a reclaim
#[test_log::test]
fn an_edit_while_the_changes_are_preserved_keeps_the_lease_and_its_files() {
  let fixture = quiet_fixture();
  let path = path_of(&fixture.get(&["--holder", "gone-agent"]));
  let edited = Path::new(&path).join("a.txt");
  std::fs::write(&edited, "a\nchanged\n").unwrap();
  make_idle(&fixture, &path);
  let wrapper = tempfile::tempdir().unwrap();
  let environment = wrapped_environment(
    &fixture,
    wrapper.path(),
    ("*\" apply --cached \"*", &format!("printf 'meanwhile\\n' >> '{}'", shell_path(&edited))),
  );

  let (exit_code, got) = run_json(&environment, &["lease", "get"]);
  assert_eq!(exit_code, 0, "{got}");
  let (exit_code, reclaimed) = run_json(&environment, &["lease", "reclaim", "--idle"]);
  assert_eq!(exit_code, 0, "{reclaimed}");
  assert_eq!(reclaimed["skipped"][0]["reason"], json!("failed"), "{reclaimed}");
  assert_eq!(std::fs::read_to_string(&edited).unwrap(), "a\nchanged\nmeanwhile\nmeanwhile\n", "the files stay");
  assert_kept_beside(&fixture, &got, &path, "gone-agent");
}

/// A discard that fails after the commit point (here its `reset --hard`), as a get or a reclaim killed there: the slot
/// is free and dirty, its record and the notes name the patches, the files stay, and the next get passes over the slot
/// as dirty without a `status`
#[test_log::test]
fn a_discard_that_fails_after_the_commit_point_leaves_a_dirty_slot_and_the_patches() {
  let fixture = quiet_fixture();
  let by_get = path_of(&fixture.get(&["--holder", "by-get"]));
  let by_reclaim = path_of(&fixture.get(&["--holder", "by-reclaim"]));
  for path in [&by_get, &by_reclaim] {
    std::fs::write(Path::new(path).join("a.txt"), "a\nchanged\n").unwrap();
    make_idle(&fixture, path);
  }
  let wrapper = tempfile::tempdir().unwrap();
  let environment = wrapped_environment(&fixture, wrapper.path(), ("*\" reset --quiet --hard HEAD \"*", "exit 1"));

  // the get takes the lease checked longest ago: the first
  let (exit_code, got) = run_json(&environment, &["lease", "get", "--holder", "next"]);
  assert_ne!(exit_code, 0, "{got}");
  let (exit_code, reclaimed) = run_json(&environment, &["lease", "reclaim", "--idle"]);
  assert_eq!(exit_code, 0, "{reclaimed}");
  assert_eq!(reclaimed["skipped"][0]["reason"], json!("failed"), "{reclaimed}");
  for path in [&by_get, &by_reclaim] {
    let slot = slot_status(&fixture, path);
    assert_eq!((&slot["state"], &slot["dirty"]), (&json!("free"), &json!(true)), "{slot}");
    let dir = PathBuf::from(slot["reclaimed"]["preserved"]["dir"].as_str().unwrap());
    assert!(dir.join("changes.patch").is_file(), "{slot}");
    assert_eq!(porcelain(path), "M a.txt", "the files stay");
  }
  let next = fixture.get(&[]);
  let skipped: Vec<&Value> = next["skipped"].as_array().unwrap().iter().map(|skipped| &skipped["reason"]).collect();
  assert_eq!(skipped, [&json!("dirty"), &json!("dirty")], "{next}");
}

/// The `not_preserved` reason of the kept lease of `holder` among `kept`
fn reason_of(kept: &[(String, String)], holder: &str) -> String {
  kept.iter().find(|(kept, _)| kept == holder).map_or_default(|(_, reason)| reason.clone())
}

/// A file that only a change of a tracked `.gitignore` ignores is in no patch, and the discard would put the old rules
/// back and delete it: the lease stays with the file
#[test_log::test]
fn a_changed_tracked_gitignore_keeps_the_lease_and_the_file_it_ignores() {
  let fixture = quiet_fixture();
  fixture.repo.create_commit("Add the ignore rules", ".gitignore", "*.log\n");
  let path = path_of(&fixture.get(&["--holder", "ignores"]));
  std::fs::write(Path::new(&path).join(".gitignore"), "*.log\nlocal.properties\n").unwrap();
  std::fs::write(Path::new(&path).join("local.properties"), "sdk.dir=/sdk\n").unwrap();
  make_idle(&fixture, &path);

  let kept = reclaim_keeps(&fixture);
  assert!(
    reason_of(&kept, "ignores").starts_with("a changed ignore file, whose ignored files no patch holds: .gitignore"),
    "{kept:?}"
  );
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("local.properties")).unwrap(), "sdk.dir=/sdk\n");
}

/// A file that only an untracked `.gitignore` of a directory ignores: the discard would remove that `.gitignore` first
/// and then the file, which no patch holds. The lease stays with both
#[test_log::test]
fn an_untracked_gitignore_keeps_the_lease_and_the_file_it_ignores() {
  let fixture = quiet_fixture();
  let path = path_of(&fixture.get(&["--holder", "ignores"]));
  let sub = Path::new(&path).join("sub");
  std::fs::create_dir_all(&sub).unwrap();
  std::fs::write(sub.join(".gitignore"), ".env\n").unwrap();
  std::fs::write(sub.join(".env"), "TOKEN=1\n").unwrap();
  make_idle(&fixture, &path);

  let kept = reclaim_keeps(&fixture);
  assert!(reason_of(&kept, "ignores").ends_with(": sub/.gitignore"), "{kept:?}");
  assert_eq!(std::fs::read_to_string(sub.join(".env")).unwrap(), "TOKEN=1\n");
}

/// After `git rm --cached` of a file that stays, edited, and an ignore entry for it outside the tree (`info/exclude`),
/// no patch holds the file and `reset --hard` would write the version of HEAD over it: the lease stays with the edit
#[test_log::test]
fn a_staged_deletion_of_a_file_that_stays_keeps_the_lease_and_the_file() {
  let fixture = quiet_fixture();
  fixture.repo.create_commit("Add cfg", "cfg", "tracked\n");
  let path = path_of(&fixture.get(&["--holder", "untracks"]));
  slot_git(&path, &["rm", "--cached", "--quiet", "cfg"]);
  std::fs::write(Path::new(&path).join("cfg"), "edited\n").unwrap();
  let exclude = fixture.repo.path().join(".git").join("info").join("exclude");
  let rules = std::fs::read_to_string(&exclude).unwrap_or_default();
  std::fs::write(&exclude, format!("{rules}/cfg\n")).unwrap();
  make_idle(&fixture, &path);

  let kept = reclaim_keeps(&fixture);
  assert_eq!(reason_of(&kept, "untracks"), "a deleted or retyped path that something else stands in for: cfg", "{kept:?}");
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("cfg")).unwrap(), "edited\n");
}

/// A tracked file replaced by a directory that holds an ignored file, and a tracked directory replaced by an ignored
/// file: `status` names only the deletions, and `reset --hard` would remove what stands there to write the files of
/// HEAD. Both leases stay with the ignored files
#[test_log::test]
fn a_deleted_path_that_an_ignored_file_or_directory_stands_in_for_keeps_the_lease() {
  let fixture = quiet_fixture();
  fixture.repo.create_commit_with_files("Add x and d", &[("x", "x\n"), ("d/a.txt", "a\n")]);
  // after the tracked files: a rule that ignores `d` keeps the tracked `d/a.txt` tracked
  fixture.repo.create_commit("Ignore the env files and d", ".gitignore", "*.env\n/d\n");
  let as_directory = path_of(&fixture.get(&["--holder", "as-directory"]));
  std::fs::remove_file(Path::new(&as_directory).join("x")).unwrap();
  std::fs::create_dir_all(Path::new(&as_directory).join("x")).unwrap();
  std::fs::write(Path::new(&as_directory).join("x").join(".env"), "TOKEN=1\n").unwrap();
  let as_file = path_of(&fixture.get(&["--holder", "as-file"]));
  std::fs::remove_dir_all(Path::new(&as_file).join("d")).unwrap();
  std::fs::write(Path::new(&as_file).join("d"), "ignored\n").unwrap();
  for path in [&as_directory, &as_file] {
    make_idle(&fixture, path);
  }

  let kept = reclaim_keeps(&fixture);
  assert_eq!(
    reason_of(&kept, "as-directory"),
    "a deleted or retyped path that something else stands in for: x",
    "{kept:?}"
  );
  assert_eq!(
    reason_of(&kept, "as-file"),
    "a deleted or retyped path that something else stands in for: d/a.txt",
    "{kept:?}"
  );
  assert_eq!(std::fs::read_to_string(Path::new(&as_directory).join("x").join(".env")).unwrap(), "TOKEN=1\n");
  assert_eq!(std::fs::read_to_string(Path::new(&as_file).join("d")).unwrap(), "ignored\n");
}

/// A refusal of the index (skip-worktree) moves no file once it is lifted: the stat of the index in its proof ends the
/// verdict, and the next get takes the slot over
#[test_log::test]
fn a_refusal_of_the_index_ends_with_a_write_of_the_index() {
  let fixture = quiet_fixture();
  let path = path_of(&fixture.get(&["--holder", "hidden"]));
  slot_git(&path, &["update-index", "--skip-worktree", "a.txt"]);
  std::fs::write(Path::new(&path).join("left.txt"), "left\n").unwrap();
  make_idle(&fixture, &path);
  let first = fixture.get(&[]);
  assert!(
    first["skipped"][0]["idle"]["idle"]["not_preserved"].as_str().unwrap().ends_with("skip-worktree): a.txt"),
    "{first}"
  );

  slot_git(&path, &["update-index", "--no-skip-worktree", "a.txt"]);
  let second = fixture.get(&[]);
  assert_eq!(path_of(&second), path, "{second}");
  assert_eq!(second["took_over"]["preserved"]["paths"], json!(1), "{second}");
}

/// Changed files past `branchdeck.leasePreserveMax` keep the lease before `add --all` writes an object of them
#[test_log::test]
fn changes_past_the_bound_keep_the_lease_before_any_object_is_written() {
  let fixture = quiet_fixture();
  fixture.repo.git(&["config", "branchdeck.leasePreserveMax", "10"]);
  let path = path_of(&fixture.get(&["--holder", "large"]));
  let dump = Path::new(&path).join("dump.txt");
  std::fs::write(&dump, "a dump of more than ten bytes, of this test only\n").unwrap();
  make_idle(&fixture, &path);

  let kept = reclaim_keeps(&fixture);
  assert_eq!(
    reason_of(&kept, "large"),
    "the changes have more than 10 bytes (branchdeck.leasePreserveMax), the largest: dump.txt",
    "{kept:?}"
  );
  let id = slot_git(&path, &["hash-object", "dump.txt"]);
  let written = GitCommandExecutor::new().cmd(&["cat-file", "-e", &id], &path).status().unwrap().code == 0;
  assert!(!written, "no object of the dump");
}

/// The path that refuses comes first in the proof of the kept verdict: past the first 8 changes, its fix by hand still
/// ends the verdict, and the next get takes the slot over
#[test_log::test]
fn the_fix_of_the_path_that_refuses_ends_the_verdict_past_the_first_eight_changes() {
  let fixture = quiet_fixture();
  let path = path_of(&fixture.get(&["--holder", "refused"]));
  for n in 0..9 {
    std::fs::write(Path::new(&path).join(format!("a{n}.txt")), "a\n").unwrap();
  }
  std::fs::create_dir_all(Path::new(&path).join("zz")).unwrap();
  std::fs::write(Path::new(&path).join("zz").join(".gitignore"), "*.tmp\n").unwrap();
  make_idle(&fixture, &path);
  let first = fixture.get(&[]);
  assert!(
    first["skipped"][0]["idle"]["idle"]["not_preserved"].as_str().unwrap().ends_with(": zz/.gitignore"),
    "{first}"
  );

  std::fs::remove_file(Path::new(&path).join("zz").join(".gitignore")).unwrap();
  let second = fixture.get(&[]);
  assert_eq!(path_of(&second), path, "{second}");
  assert_eq!(second["took_over"]["preserved"]["paths"], json!(9), "{second}");
}

/// A process of another session that comes into the slot during the preserve (here at the proof of the patches) keeps
/// the lease: no patch and no record stay, the check records the failure, and the next get waits an hour for another
/// preserve rather than run it again
#[cfg(unix)]
#[test_log::test]
fn a_process_in_the_slot_at_the_commit_point_keeps_the_lease_and_the_next_get_waits() {
  let fixture = quiet_fixture();
  let old = fixture.get(&["--holder", "gone-agent"]);
  let path = path_of(&old);
  std::fs::write(Path::new(&path).join("a.txt"), "a\nchanged\n").unwrap();
  make_idle(&fixture, &path);
  let wrapper = tempfile::tempdir().unwrap();
  let pid_file = wrapper.path().join("pid");
  // a group of its own (`set -m`): the scan takes a process of the group of brd for one of its own
  let start = format!(
    "set -m; (cd '{}' && exec sleep 30) > /dev/null 2>&1 & echo $! > '{}'; set +m",
    shell_path(Path::new(&path)),
    shell_path(&pid_file)
  );
  let environment = wrapped_environment(&fixture, wrapper.path(), ("*\" apply --cached \"*", &start));

  let (exit_code, got) = run_json(&environment, &["lease", "get"]);
  let pid = std::fs::read_to_string(&pid_file).unwrap();
  std::process::Command::new("kill").arg(pid.trim()).status().unwrap();
  assert_eq!(exit_code, 0, "{got}");
  assert_kept_beside(&fixture, &got, &path, "gone-agent");
  let idle = slot_status(&fixture, &path)["lease"]["idle"].clone();
  assert!(idle["preserve_error"].as_str().unwrap().starts_with("A process of another session works in"), "{idle}");
  let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
  assert!(idle["quiet_at"].as_u64().unwrap() > now + 3000, "an hour on: {idle}");
  let notes = std::fs::read_link(Path::new(&path).join("notes.local")).unwrap();
  assert!(!notes.join(lease_pool::idle_changes::PRESERVED_DIR).exists(), "no patch stays");
  let (exit_code, record) = fixture.run(&["lease", "notes", "--lease-id", old["lease_id"].as_str().unwrap()]);
  assert_eq!((exit_code, &record["notes"][0]["preserved"]), (0, &Value::Null), "{record}");

  let next = fixture.get(&[]);
  assert_eq!(next["skipped"][0]["reason"], json!("idle_with_work"), "{next}");
  assert_eq!(slot_status(&fixture, &path)["lease"]["idle"]["checked_at"], idle["checked_at"], "no check again");
}
