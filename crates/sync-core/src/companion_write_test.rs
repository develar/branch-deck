use crate::companion_write::{COMPANION_LOCK, Written, companion_of_write, unlink_primary, unlink_primary_with, write_companion, write_companion_under};
use crate::companions::companions_test::{main_worktree as main, snapshot};
use crate::regroup_test::path;
use crate::repository_config::read_repository_config;
use history_rewrite::repository_lock::{RepositoryLock, lock_as_another_process};
use pretty_assertions::assert_eq;
use std::sync::mpsc;
use std::thread::JoinHandle;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

/// The companion lock of `repo` as another process (`brd companion`) holds it
fn held_by_another_process(repo: &TestRepo) -> RepositoryLock {
  let dir = repo.path().join(".git").join(COMPANION_LOCK);
  std::fs::create_dir_all(dir.parent().unwrap()).unwrap();
  lock_as_another_process(dir, "brd companion add").unwrap()
}

/// The write of the companion `ci` of `primary` (`path`, or `None` for a removal) on a thread of its own, with the
/// companion the write touches asked first, as `write_companion` asks it; answers once the write waits for a lock, or
/// fails the test when it ended without waiting
fn write_that_waits(primary: &TestRepo, path_of_ci: Option<&TestRepo>) -> JoinHandle<anyhow::Result<Written>> {
  let (git, primary) = (primary.git_executor().clone(), path(primary).to_owned());
  let ci = path_of_ci.map(|ci| path(ci).to_owned());
  let (waits, waiting) = mpsc::channel();
  let write = std::thread::spawn(move || {
    let companion = companion_of_write(&git, &primary, "ci", ci.as_deref())?;
    let on_wait = move || waits.send(()).unwrap();
    write_companion_under(&git, &primary, "ci", ci.as_deref(), companion.as_deref(), &on_wait)
  });
  waiting.recv().expect("the write waits for the companion lock another process holds");
  write
}

/// Writes `key` of the local config of `repo` as another process does (a `brd companion` of its own, a `git config`)
fn set(repo: &TestRepo, key: &str, value: &str) {
  repo.git(&["config", "--local", key, value]);
}

/// Two primaries add one companion from two processes at once: the add of the second waits for the companion lock the
/// first holds, then reads the configs the first wrote, so it is refused (a companion has one primary) and its primary
/// names no companion
#[test]
fn of_two_primaries_that_add_one_companion_from_two_processes_the_second_reads_the_first_and_is_refused() {
  let (first, second, ci) = (baseline_repo(), baseline_repo(), baseline_repo());
  let held = held_by_another_process(&ci);
  let add = write_that_waits(&second, Some(&ci));
  // the other process adds ci to the first while it holds the lock
  set(&first, "branchdeck.companion.ci.path", &main(&ci));
  set(&ci, "branchdeck.primary", &main(&first));
  drop(held);
  let refused = add.join().unwrap().unwrap_err().to_string();
  assert!(refused.contains("a companion has one primary"), "{refused}");
  assert_eq!(snapshot(&ci).primary(), Some(main(&first).as_str()));
  assert!(snapshot(&second).companions().is_empty(), "the refused primary names no companion");
}

/// The order of the review, across two processes: an add of a linked companion again waits while another process
/// removes it, then reads both configs as the removal left them and links both sides; an add that read them before the
/// removal would see its companion named already and write only the key of the primary: a half link
#[test]
fn an_add_that_waits_for_a_removal_of_another_process_links_both_sides_after_it() {
  let (app, ci) = (baseline_repo(), baseline_repo());
  write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap();
  let held = held_by_another_process(&ci);
  let add = write_that_waits(&app, Some(&ci));
  // the other process removes ci while it holds the lock: both sides
  app.git(&["config", "--local", "--unset-all", "branchdeck.companion.ci.path"]);
  ci.git(&["config", "--local", "--unset-all", "branchdeck.primary"]);
  drop(held);
  assert_eq!(add.join().unwrap().unwrap(), Written::Done);
  assert_eq!(snapshot(&app).companions().get("ci"), Some(&main(&ci)));
  assert_eq!(snapshot(&ci).primary(), Some(main(&app).as_str()), "both sides name the link");
}

/// A removal waits while another process adds the companion again and finishes: it reads the link both sides name and
/// unsets both
#[test]
fn a_removal_that_waits_for_an_add_of_another_process_unsets_both_sides_after_it() {
  let (app, ci) = (baseline_repo(), baseline_repo());
  write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap();
  let held = held_by_another_process(&app);
  let removal = write_that_waits(&app, None);
  // the add of the other process writes the same link again
  set(&app, "branchdeck.companion.ci.path", &main(&ci));
  set(&ci, "branchdeck.primary", &main(&app));
  drop(held);
  assert_eq!(removal.join().unwrap().unwrap(), Written::Done);
  assert!(snapshot(&app).companions().is_empty());
  assert_eq!(snapshot(&ci).primary(), None);
}

/// A removal whose companion the primary named another path for before the write took its locks writes nothing and asks
/// again (`Written::Moved`); asked again, it removes the key of the primary and leaves the side of the companion it
/// locked first, which names this primary still: a half link the companion reports
#[test]
fn a_removal_whose_companion_moved_before_its_locks_writes_nothing() {
  let (app, ci, web) = (baseline_repo(), baseline_repo(), baseline_repo());
  write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap();
  let asked = companion_of_write(app.git_executor(), path(&app), "ci", None).unwrap();
  assert_eq!(asked, Some(main(&ci)));
  set(&app, "branchdeck.companion.ci.path", &main(&web));
  let written = write_companion_under(app.git_executor(), path(&app), "ci", None, asked.as_deref(), &|| {}).unwrap();
  assert_eq!(written, Written::Moved);
  assert_eq!(snapshot(&app).companions().get("ci"), Some(&main(&web)), "nothing written");

  write_companion(app.git_executor(), path(&app), "ci", None).unwrap();
  assert!(snapshot(&app).companions().is_empty());
  assert_eq!(snapshot(&ci).primary(), Some(main(&app).as_str()));
  let of_ci = read_repository_config(ci.git_executor(), path(&ci)).unwrap();
  assert_eq!(
    of_ci.primary_problem,
    Some(format!("{} does not list this repository as a companion: add it again there to link it", main(&app)))
  );
}

/// A half link shows on the side that has it, and the next add or removal of the pair repairs it: a companion that names
/// no primary, or another one that does not list it, is the problem of its row in the primary; a companion whose primary
/// does not list it, or is gone, has the problem of its primary; an add links both sides, a removal unlists the primary's
#[test]
fn a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it() {
  let (app, ci, other) = (baseline_repo(), baseline_repo(), baseline_repo());
  let link_problem = |repo: &TestRepo| -> Vec<Option<String>> {
    let config = read_repository_config(repo.git_executor(), path(repo)).unwrap();
    config.companions.into_iter().map(|companion| companion.link_problem).collect()
  };
  let primary_problem = |repo: &TestRepo| read_repository_config(repo.git_executor(), path(repo)).unwrap().primary_problem;
  let add_ci = || write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap();
  add_ci();
  assert_eq!((link_problem(&app), primary_problem(&ci)), (vec![None], None), "a link of both sides");

  // the companion names no primary: a `git config --unset` of the user, an add cut short
  ci.git(&["config", "--local", "--unset-all", "branchdeck.primary"]);
  assert_eq!(link_problem(&app), [Some(format!("{} names no primary: add it again to link it, or remove it", main(&ci)))]);
  add_ci();
  assert_eq!((link_problem(&app), primary_problem(&ci)), (vec![None], None), "the add links both sides");

  // it names another primary, which does not list it: the half of each side, the add takes it over
  set(&ci, "branchdeck.primary", &main(&other));
  assert_eq!(
    link_problem(&app),
    [Some(format!(
      "{} names {} as its primary: add it again to link it here, or remove it",
      main(&ci),
      main(&other)
    ))]
  );
  assert_eq!(
    primary_problem(&ci),
    Some(format!("{} does not list this repository as a companion: add it again there to link it", main(&other)))
  );
  add_ci();
  assert_eq!((link_problem(&app), primary_problem(&ci)), (vec![None], None));

  // a removal of a half link unlists the primary's side and leaves the companion's, which names another primary
  set(&ci, "branchdeck.primary", &main(&other));
  write_companion(app.git_executor(), path(&app), "ci", None).unwrap();
  assert!(snapshot(&app).companions().is_empty());
  assert_eq!(snapshot(&ci).primary(), Some(main(&other).as_str()));

  // a companion whose primary is gone (a disk not mounted now) is no companion of none: the add is refused with how to
  // unlink it, and once the companion unlinked it on its side the add links both
  let gone = tempfile::tempdir().unwrap();
  let gone_path = gone.path().join("app").to_str().unwrap().to_owned();
  set(&ci, "branchdeck.primary", &gone_path);
  assert_eq!(primary_problem(&ci), Some(format!("{gone_path} is gone")));
  let refused = write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap_err().to_string();
  assert!(
    refused.contains(&format!("names {gone_path} as its primary, which can not be read ({gone_path} is gone)")),
    "{refused}"
  );
  assert!(refused.contains("unlink it in the Settings of"), "{refused}");
  unlink_primary(ci.git_executor(), path(&ci)).unwrap();
  assert_eq!(snapshot(&ci).primary(), None);
  add_ci();
  assert_eq!((link_problem(&app), primary_problem(&ci)), (vec![None], None));
}

/// A primary whose config fails to read is unknown, not absent (the probe of the review: the config of the first primary
/// broken, the second adds the companion): the add is refused with the primary it names and how to unlink it, and the
/// companion stays the first one's
#[test]
fn a_companion_of_a_primary_whose_config_fails_to_read_is_not_taken_over() {
  let (first, ci, second) = (baseline_repo(), baseline_repo(), baseline_repo());
  write_companion(first.git_executor(), path(&first), "ci", Some(path(&ci))).unwrap();
  let first_main = main(&first);
  let config = first.path().join(".git/config");
  let mut text = std::fs::read_to_string(&config).unwrap();
  text.push_str("\n[broken\n");
  std::fs::write(&config, text).unwrap();
  let refused = write_companion(second.git_executor(), path(&second), "ci", Some(path(&ci))).unwrap_err().to_string();
  assert!(refused.contains(&format!("names {first_main} as its primary, which can not be read")), "{refused}");
  assert!(refused.contains("brd companion unlink"), "{refused}");
  assert_eq!(snapshot(&ci).primary(), Some(first_main.as_str()), "not taken over");
  assert!(snapshot(&second).companions().is_empty());
}

/// Unlink of a companion takes back the side a half link left on it (its `branchdeck.primary`): of a primary that does
/// not list it, or can not be read. A link of both sides goes by a removal in the primary, and a repository that names no
/// primary has nothing to unlink
#[test]
fn unlink_takes_back_the_side_of_a_half_link_on_the_companion_only() {
  let (app, ci, other) = (baseline_repo(), baseline_repo(), baseline_repo());
  write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap();
  let refused = unlink_primary(ci.git_executor(), path(&ci)).unwrap_err().to_string();
  assert!(
    refused.contains(&format!("{} lists {} as its companion: remove it there", main(&app), main(&ci))),
    "{refused}"
  );
  assert_eq!(snapshot(&ci).primary(), Some(main(&app).as_str()));

  set(&ci, "branchdeck.primary", &main(&other));
  unlink_primary(ci.git_executor(), path(&ci)).unwrap();
  assert_eq!(snapshot(&ci).primary(), None, "a primary that does not list it");
  let none = unlink_primary(ci.git_executor(), path(&ci)).unwrap_err().to_string();
  assert!(none.contains("names no primary"), "{none}");
}

/// An unlink waits for the companion lock another process holds, then asks again which primary the companion names: a
/// primary that came meanwhile and lists it keeps it (refused), where an unlink of what it read before the lock would
/// leave that primary listing a companion that names none
#[test]
fn an_unlink_that_waits_for_another_process_asks_again_which_primary_the_companion_names() {
  let (app, ci, other) = (baseline_repo(), baseline_repo(), baseline_repo());
  set(&ci, "branchdeck.primary", &main(&other));
  let held = held_by_another_process(&ci);
  let (waits, waiting) = mpsc::channel();
  let unlink = std::thread::spawn({
    let (git, ci_path) = (ci.git_executor().clone(), path(&ci).to_owned());
    move || unlink_primary_with(&git, &ci_path, &move || waits.send(()).unwrap())
  });
  waiting.recv().expect("the unlink waits for the companion lock another process holds");
  // the other process adds ci to app while it holds the lock: both sides
  set(&app, "branchdeck.companion.ci.path", &main(&ci));
  set(&ci, "branchdeck.primary", &main(&app));
  drop(held);
  let refused = unlink.join().unwrap().unwrap_err().to_string();
  assert!(refused.contains("lists"), "{refused}");
  assert_eq!(snapshot(&ci).primary(), Some(main(&app).as_str()));
}
