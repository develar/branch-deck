use crate::companions::{CompanionNeed, companion_needs, need_repositories, read_companion_needs, repository_links};
use crate::config_snapshot::ConfigSnapshot;
use crate::regroup_test::{commits, path, sync};
use crate::repository_config::{finish_companion_of_primary, read_repository_config, write_companion, write_companion_of_primary, write_companion_side};
use branch_integration::detection_notes::{CacheOps, CachedDetection};
use domain_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use domain_types::landing::NeedState;
use domain_types::session::{LinkRole, RepositoryLink};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};
use test_utils::process_budget::{assert_processes, counting};

fn snapshot(repo: &TestRepo) -> ConfigSnapshot {
  ConfigSnapshot::read(repo.git_executor(), path(repo)).unwrap()
}

/// `ci` with the branch `prepare` built (`test-prefix/virtual/prepare`), `drop` unapplied, `gone` archived and
/// integrated, `left` archived and not integrated
fn companion() -> TestRepo {
  let ci = baseline_repo();
  let ids = commits(&ci, &["(prepare) Prepare the data", "(drop) Drop the step"]);
  sync(&ci);
  let git = ci.git_executor();
  git.execute_command(&["update-ref", "refs/heads/test-prefix/unapplied/drop", &ids[1]], path(&ci)).unwrap();
  git.execute_command(&["update-ref", "-d", "refs/heads/test-prefix/virtual/drop"], path(&ci)).unwrap();
  for (name, tip) in [("gone", &ids[0]), ("left", &ids[1])] {
    git
      .execute_command(&["update-ref", &format!("refs/heads/test-prefix/archived/2026-11-01/{name}"), tip], path(&ci))
      .unwrap();
  }
  let integrated = CachedDetection {
    info: BranchIntegrationInfo {
      name: "gone".to_string(),
      summary: String::new(),
      status: BranchIntegrationStatus::Integrated {
        integrated_at: None,
        confidence: IntegrationConfidence::Exact,
        commit_count: 1,
        commit: None,
      },
    },
    check: None,
  };
  CacheOps::new(git, path(&ci)).write(&ids[0], &integrated).unwrap();
  ci
}

#[test]
fn a_branch_of_a_companion_is_active_unapplied_integrated_or_missing_as_its_last_sync_left_it() {
  let ci = companion();
  let repositories = BTreeMap::from([("ci".to_string(), path(&ci).to_string())]);
  let git = counting();
  let states = companion_needs(&git, &repositories, "test-prefix", ["ci:prepare", "ci:drop", "ci:gone", "ci:left", "ci:never"]);
  assert_eq!(
    states,
    BTreeMap::from([
      ("ci:drop".to_string(), CompanionNeed::Unlanded(NeedState::Unapplied)),
      ("ci:gone".to_string(), CompanionNeed::Integrated),
      ("ci:left".to_string(), CompanionNeed::Unlanded(NeedState::Missing)),
      ("ci:never".to_string(), CompanionNeed::Unlanded(NeedState::Missing)),
      ("ci:prepare".to_string(), CompanionNeed::Unlanded(NeedState::Active)),
    ])
  );
  // its config, its branches under the prefix, the notes of their tips: one of each, for every need of the companion
  assert_processes(&git, &[("config", 1), ("for-each-ref", 1), ("log", 1)]);
}

#[test]
fn a_companion_that_is_gone_or_unknown_makes_its_needs_unreadable_and_nothing_fails() {
  let gone = tempfile::tempdir().unwrap();
  let missing = gone.path().join("ci-config").to_str().unwrap().to_string();
  let repositories = BTreeMap::from([("ci".to_string(), missing.clone())]);
  let repo = baseline_repo();
  let states = companion_needs(repo.git_executor(), &repositories, "test-prefix", ["ci:prepare", "web:deploy"]);
  let unreadable = |companion: &str, reason: String| {
    CompanionNeed::Unlanded(NeedState::CompanionUnreadable {
      companion: companion.to_string(),
      reason,
    })
  };
  assert_eq!(
    states,
    BTreeMap::from([
      ("ci:prepare".to_string(), unreadable("ci", format!("{missing} is gone"))),
      ("web:deploy".to_string(), unreadable("web", "no companion repository is named web".to_string())),
    ])
  );
  // without a need of a companion nothing is read
  let git = counting();
  assert_eq!(read_companion_needs(&git, path(&repo), &snapshot(&repo), "test-prefix", &[]), BTreeMap::new());
  assert_processes(&git, &[]);
}

/// The primary lists its companions; a companion names its primary by the name of its directory and the other
/// companions of the primary by their names, never itself
#[test]
fn a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings() {
  let (app, ci, web) = (baseline_repo(), baseline_repo(), baseline_repo());
  write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap();
  write_companion(app.git_executor(), path(&app), "web", Some(path(&web))).unwrap();
  let main = |repo: &TestRepo| main_worktree(repo);
  let config = read_repository_config(app.git_executor(), path(&app)).unwrap();
  let listed: Vec<(String, String, Option<String>)> = config.companions.into_iter().map(|it| (it.name, it.path, it.problem)).collect();
  assert_eq!(listed, [("ci".to_string(), main(&ci), None), ("web".to_string(), main(&web), None)]);
  assert_eq!(snapshot(&ci).primary(), Some(main(&app).as_str()));

  let from_ci = need_repositories(ci.git_executor(), path(&ci), &snapshot(&ci));
  let app_name = std::path::Path::new(&main(&app)).file_name().unwrap().to_str().unwrap().to_string();
  assert_eq!(from_ci, BTreeMap::from([(app_name.clone(), main(&app)), ("web".to_string(), main(&web))]));

  let from_app = repository_links(app.git_executor(), path(&app), &snapshot(&app));
  assert_eq!(
    from_app,
    [
      RepositoryLink {
        name: "ci".to_string(),
        path: main(&ci),
        role: LinkRole::Companion,
        problem: None
      },
      RepositoryLink {
        name: "web".to_string(),
        path: main(&web),
        role: LinkRole::Companion,
        problem: None
      },
    ]
  );
  let from_ci = repository_links(ci.git_executor(), path(&ci), &snapshot(&ci));
  assert_eq!(
    from_ci,
    [
      RepositoryLink {
        name: app_name,
        path: main(&app),
        role: LinkRole::Primary,
        problem: None
      },
      RepositoryLink {
        name: "web".to_string(),
        path: main(&web),
        role: LinkRole::Companion,
        problem: None
      },
    ]
  );

  // a companion has one primary, a primary is no companion, a repository is no companion of itself
  let other = baseline_repo();
  let refused = |repo: &TestRepo, name: &str, of: &TestRepo| write_companion(repo.git_executor(), path(repo), name, Some(path(of))).unwrap_err().to_string();
  assert!(refused(&other, "ci", &ci).contains("a companion has one primary"));
  assert!(refused(&ci, "other", &other).contains("only a primary has companions"));
  assert!(refused(&app, "self", &app).contains("can not be a companion of itself"));
  assert!(refused(&app, "ci:config", &other).contains("is no name of a companion"));

  // a removal unsets both sides
  write_companion(app.git_executor(), path(&app), "ci", None).unwrap();
  assert_eq!(snapshot(&app).companions().keys().collect::<Vec<_>>(), ["web"]);
  assert_eq!(snapshot(&ci).primary(), None);
}

/// Two primaries that add one companion at once both pass the check of their own config, which writes nothing yet; the
/// check that a companion has one primary is made with the write of its side, so the second side is refused and its
/// primary never names the companion
#[test]
fn of_two_primaries_that_add_one_companion_at_once_the_side_of_the_second_is_refused() {
  let (first, second, ci) = (baseline_repo(), baseline_repo(), baseline_repo());
  let begin = |primary: &TestRepo| write_companion_of_primary(primary.git_executor(), path(primary), "ci", Some(path(&ci))).unwrap().unwrap();
  let (of_first, of_second) = (begin(&first), begin(&second));
  assert!(write_companion_side(ci.git_executor(), &of_first).unwrap(), "the first names its primary");
  let refused = write_companion_side(ci.git_executor(), &of_second).unwrap_err().to_string();
  assert!(refused.contains("a companion has one primary"), "{refused}");
  finish_companion_of_primary(first.git_executor(), &of_first).unwrap();
  assert_eq!(snapshot(&ci).primary(), Some(main_worktree(&first).as_str()));
  assert_eq!(snapshot(&first).companions().keys().collect::<Vec<_>>(), ["ci"]);
  assert!(snapshot(&second).companions().is_empty(), "the second primary names no companion");
}

/// An add whose primary became a companion between the claim of the companion and the key of the primary is refused,
/// and the claim it made is taken back: the companion names no primary, the primary no companion
#[test]
fn an_add_whose_primary_became_a_companion_meanwhile_takes_back_its_claim() {
  let (app, ci, other) = (baseline_repo(), baseline_repo(), baseline_repo());
  let side = write_companion_of_primary(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap().unwrap();
  assert!(write_companion_side(ci.git_executor(), &side).unwrap());
  write_companion(other.git_executor(), path(&other), "app", Some(path(&app))).unwrap();
  let refused = finish_companion_of_primary(app.git_executor(), &side).unwrap_err().to_string();
  assert!(refused.contains("only a primary has companions"), "{refused}");
  write_companion_side(ci.git_executor(), &side.undo().unwrap()).unwrap();
  assert_eq!(snapshot(&ci).primary(), None, "the claim is taken back");
  assert!(snapshot(&app).companions().is_empty());
}

/// An add whose key of the primary cannot be written (another process holds the lock of its config past the wait of a
/// write) fails, and the primary the companion named just now is taken back: no companion names a primary that does
/// not name it
#[test]
fn an_add_whose_key_of_the_primary_cannot_be_written_takes_back_the_primary_of_the_companion() {
  let (app, ci) = (baseline_repo(), baseline_repo());
  std::fs::write(app.path().join(".git/config.lock"), "").unwrap();
  let failed = write_companion(app.git_executor(), path(&app), "ci", Some(path(&ci))).unwrap_err().to_string();
  assert!(failed.contains("config.lock"), "{failed}");
  assert_eq!(snapshot(&ci).primary(), None, "the claim is taken back");
}

#[test]
fn repository_links_report_missing_and_non_repository_paths() {
  let app = baseline_repo();
  let temporary = tempfile::tempdir().unwrap();
  let missing = temporary.path().join("gone").to_str().unwrap().to_string();
  let not_repository = temporary.path().join("not-a-repository").to_str().unwrap().to_string();
  std::fs::create_dir_all(&not_repository).unwrap();
  app.git(&["config", "--local", "--replace-all", "branchdeck.companion.gone.path", &missing]);
  app.git(&["config", "--local", "--replace-all", "branchdeck.companion.not-a-repository.path", &not_repository]);

  let config = snapshot(&app);
  let git = counting();
  let links = repository_links(&git, path(&app), &config);

  assert_eq!(
    links,
    [
      RepositoryLink {
        name: "gone".to_string(),
        path: missing.clone(),
        role: LinkRole::Companion,
        problem: Some(format!("{missing} is gone")),
      },
      RepositoryLink {
        name: "not-a-repository".to_string(),
        path: not_repository.clone(),
        role: LinkRole::Companion,
        problem: Some(format!("{not_repository} is no git repository")),
      },
    ]
  );
  assert_processes(&git, &[("config", 1)]);
}

/// The main worktree of `repo` as git names it (the parent of its common git dir)
fn main_worktree(repo: &TestRepo) -> String {
  let common = repo
    .git_executor()
    .execute_command(&["rev-parse", "--path-format=absolute", "--git-common-dir"], path(repo))
    .unwrap();
  std::path::Path::new(common.trim()).parent().unwrap().to_str().unwrap().to_string()
}
