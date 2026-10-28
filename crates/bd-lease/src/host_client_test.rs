//! `bd host snapshot`, `pack` and `apply` end to end through the Cli: the commit a run ships with the local changes, the
//! candidates and a file of the commit, its thin pack that the host takes, the patch of a run applied to the files, every
//! output as `bd schema` names it, and the refusals by their codes

use crate::host_command_test::checked;
use crate::lease_test::Fixture;
use bd_cli::fixture_test::{assert_error, git};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::TestRepo;

/// `bd host <args>`, which must succeed, its output checked against `bd schema`
fn client(fixture: &Fixture, output: &str, args: &[&str]) -> Value {
  checked(fixture, output, &[&["host"][..], args].concat())
}

/// HEAD with its local changes named, the commit of the working tree with its file, and without the status only HEAD
/// and the candidates
#[test_log::test]
fn a_snapshot_names_the_commit_the_changes_and_the_candidates() {
  let fixture = Fixture::new();
  let head = fixture.repo.head();
  let clean = client(&fixture, "host_snapshot", &["snapshot", "--blob", "a.txt"]);
  let git_dir = std::fs::canonicalize(fixture.repo.path().join(".git")).unwrap();
  assert_eq!(
    (&clean["commit"], &clean["head"], &clean["changed"], &clean["blob"]["text"]),
    (&json!(head), &json!(head), &json!([]), &json!("a\n")),
    "{clean}"
  );
  assert_eq!(clean["candidates"][0], json!(head));
  assert_eq!(std::fs::canonicalize(clean["git_dir"].as_str().unwrap()).unwrap(), git_dir);
  assert_eq!(clean["patch_dir"].as_str().map(|dir| dir.ends_with("yonder")), Some(true));

  std::fs::write(fixture.repo.path().join("a.txt"), "a\nlocal\n").unwrap();
  let refused = client(&fixture, "host_snapshot", &["snapshot"]);
  assert_eq!((&refused["commit"], &refused["changed"]), (&json!(head), &json!([" M a.txt"])), "{refused}");
  let shipped = client(&fixture, "host_snapshot", &["snapshot", "--working-tree", "--blob", "a.txt"]);
  assert_ne!(shipped["commit"], json!(head), "{shipped}");
  assert_eq!((&shipped["head"], &shipped["blob"]["text"]), (&json!(head), &json!("a\nlocal\n")));
  assert_eq!(git(&fixture.repo, &["rev-parse", &format!("{}^", shipped["commit"].as_str().unwrap())]), head);
  let quick = client(&fixture, "host_snapshot", &["snapshot", "--no-status", "--farthest", "1"]);
  assert_eq!((&quick["commit"], quick.get("changed")), (&json!(head), None), "{quick}");
}

/// The thin pack of a commit lands on a host that has its base, through `bd lease host ingest`; at least one commit of
/// the host is required
#[test_log::test]
fn a_pack_of_the_client_lands_on_the_host() {
  let fixture = Fixture::new();
  let base = fixture.repo.head();
  let host_repo = TestRepo::new_empty();
  host_repo.clone_from(fixture.repo.path()).unwrap();
  let commit = fixture.repo.create_commit("Client change", "c.txt", "c\n");
  let pack = client(&fixture, "host_pack", &["pack", &commit, "--have", &base]);
  assert_eq!(pack["commits"], json!(1), "{pack}");
  let path = pack["path"].as_str().unwrap();
  assert_eq!(pack["bytes"], json!(std::fs::metadata(path).unwrap().len()));

  let host = Fixture {
    repo: host_repo,
    home: tempfile::tempdir().unwrap(),
    global_config: tempfile::NamedTempFile::new().unwrap(),
  };
  let (exit_code, ingested) = host.run(&["lease", "host", "ingest", "--ref", "run:r1", "--commit", &commit, "--pack", path]);
  assert_eq!((exit_code, &ingested["stored"]), (0, &json!("loose")), "{ingested}");

  let (exit_code, refused) = fixture.run(&["host", "pack", &commit]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
  let (exit_code, refused) = fixture.run(&["host", "pack", &commit[..8], "--have", &base]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
}

/// The patch of a run applies to the files, not the index; one that does not apply is `git_error` and changes nothing
#[test_log::test]
fn the_patch_of_a_run_applies_to_the_files_of_the_client() {
  let fixture = Fixture::new();
  let elsewhere = TestRepo::new_empty();
  elsewhere.clone_from(fixture.repo.path()).unwrap();
  std::fs::write(elsewhere.path().join("a.txt"), "a\nby the run\n").unwrap();
  let dir = tempfile::tempdir().unwrap();
  let patch = dir.path().join("run.patch");
  std::fs::write(&patch, format!("{}\n", elsewhere.git(&["diff", "--binary", "--full-index"]))).unwrap();

  let applied = client(&fixture, "host_apply", &["apply", patch.to_str().unwrap()]);
  assert_eq!(applied["files"], json!(1), "{applied}");
  assert_eq!(std::fs::read_to_string(fixture.repo.path().join("a.txt")).unwrap(), "a\nby the run\n");
  assert_eq!(git(&fixture.repo, &["diff", "--cached", "--name-only"]), "", "the index stays");

  let (exit_code, refused) = fixture.run(&["host", "apply", patch.to_str().unwrap()]);
  assert_error(&refused, exit_code, (5, "git_error"));
  let (exit_code, refused) = fixture.run(&["host", "apply", "missing.patch"]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
}
