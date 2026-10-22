use crate::branch_table::BranchTable;
use crate::patronus_view::match_robots;
use crate::patronus_view::patronus_view_content_test::repo;
use crate::views::RepositoryViews;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use sync_core::sync::{SyncOptions, sync_branches};
use sync_test_utils::TestReporter;
use sync_types::landing::{LandingFix, LandingRefusal};
use sync_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use sync_types::repository_view::{GetRepositoryViewParams, LandingNeeds, RemoteFacts, RepositoryView, RobotMatch, RobotOf, WaitLandingNeedsParams};
use test_utils::git_test_utils::TestRepo;

fn params(revision: u32) -> GetRepositoryViewParams {
  GetRepositoryViewParams {
    repository_path: "/repo".to_string(),
    branch_prefix: "test".to_string(),
    revision,
    robots: Vec::new(),
    matched: RobotMatch::default(),
    remote: RemoteFacts::default(),
  }
}

/// Before the first sync the view is empty, of table 0; each finished sync numbers its table; the asker's revision comes
/// back; another branch prefix has its own table
#[test]
fn a_view_is_of_the_table_the_last_sync_finished() {
  let views = RepositoryViews::default();
  let git = GitCommandExecutor::new();
  let cache = ObjectCache::disabled();
  let view = views.view(&git, &cache, &params(7));
  assert_eq!((view.revision, view.table_generation, view.patronus.branches.len()), (7, 0, 0));

  views.keep("/repo", "test", BranchTable::default());
  assert_eq!(views.view(&git, &cache, &params(8)).table_generation, 1);
  views.keep("/repo", "test", BranchTable::default());
  assert_eq!(views.view(&git, &cache, &params(9)).table_generation, 2);
  let other = GetRepositoryViewParams {
    branch_prefix: "other".to_string(),
    ..params(9)
  };
  assert_eq!(views.view(&git, &cache, &other).table_generation, 0);
}

/// Stacked branches synced: `util` adds `controlUrl`, `flow` uses it; the views keep the table of the sync
async fn synced_views() -> (TestRepo, RepositoryViews, BranchTable) {
  let repo = repo();
  repo.create_commit_with_files("(util) Add controlUrl", &[("src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n")]);
  repo.create_commit_with_files(
    "(flow) Start with controlUrl",
    &[("src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n")],
  );
  let report = sync_branches(&GitCommandExecutor::new(), repo.path_str(), "test", TestReporter::new(), SyncOptions::default())
    .await
    .unwrap();
  let table = BranchTable::from_report("test", &report);
  let views = RepositoryViews::default();
  views.keep(repo.path_str(), "test", table.clone());
  (repo, views, table)
}

fn repo_params(repo: &TestRepo, robots: Vec<PatronusRobot>, matched: RobotMatch) -> GetRepositoryViewParams {
  GetRepositoryViewParams {
    repository_path: repo.path_str().to_string(),
    robots,
    matched,
    ..params(1)
  }
}

fn needs<'a>(view: &'a RepositoryView, branch: &str) -> &'a LandingNeeds {
  view.patronus.branches.get(branch).map_or(&LandingNeeds::Unknown, |facts| &facts.landing_needs)
}

/// The landing needs are unknown until the background computation of the view's inputs finished, never an empty list
/// before; then known for every branch, and a view of the same inputs computes nothing again
#[tokio::test]
async fn the_landing_needs_are_unknown_until_computed_then_kept_while_the_inputs_stay() {
  let (repo, views, _) = synced_views().await;
  let git = GitCommandExecutor::new().with_process_stats("test");
  let cache = ObjectCache::disabled();
  let first = views.view(&git, &cache, &repo_params(&repo, Vec::new(), RobotMatch::default()));
  assert_eq!(
    (needs(&first, "flow"), needs(&first, "util"), first.landing_generation),
    (&LandingNeeds::Unknown, &LandingNeeds::Unknown, 0)
  );

  let wait = WaitLandingNeedsParams {
    repository_path: repo.path_str().to_string(),
    branch_prefix: "test".to_string(),
    after: first.landing_generation,
  };
  assert_eq!(views.wait_landing_needs(&wait), 1);
  let processes = git.process_stats().unwrap().by_command().len();
  assert!(processes > 0, "the computation ran on the view's executor");

  let known = views.view(&git, &cache, &repo_params(&repo, Vec::new(), RobotMatch::default()));
  assert_eq!(known.landing_generation, 1);
  let LandingNeeds::Known { refusals } = needs(&known, "flow") else { panic!("{known:?}") };
  let [LandingRefusal::MissingDeclaration { fix, landing_in: None, .. }] = &refusals[..] else {
    panic!("{refusals:?}")
  };
  assert_eq!(
    fix,
    &LandingFix::Restack {
      branch: "flow".to_string(),
      onto: "util".to_string()
    }
  );
  assert_eq!(needs(&known, "util"), &LandingNeeds::Known { refusals: Vec::new() });

  let before = git.process_stats().unwrap().total();
  views.view(&git, &cache, &repo_params(&repo, Vec::new(), RobotMatch::default()));
  assert_eq!(views.wait_landing_needs(&WaitLandingNeedsParams { after: 1, ..wait }), 1, "nothing computed again");
  assert_eq!(git.process_stats().unwrap().total(), before);
}

/// While the Safe Push of `util` runs, the need of `flow` names it: its robot tests `util` without `flow`
#[tokio::test]
async fn a_running_safe_push_of_the_branch_the_start_needs_is_named() {
  let (repo, views, table) = synced_views().await;
  let robot = PatronusRobot {
    id: "robot-1".to_string(),
    name: "util".to_string(),
    status: PatronusRobotStatus::Running,
    push_mode: PatronusPushMode::RebaseAutosquash,
    branch_name: Some("util".to_string()),
    stack_root: None,
    source_revision: table.tip_of("util").map(str::to_string),
    target_branch: "master".to_string(),
    start_time: 100,
    finish_time: None,
    cancellation_reason: None,
    url: "https://patronus/robot/robot-1".to_string(),
    pull_request: None,
    profile: None,
  };
  let robots = vec![robot];
  let matched = match_robots(&table, &RemoteFacts::default(), &robots);
  let git = GitCommandExecutor::new();
  let cache = ObjectCache::disabled();
  let first = views.view(&git, &cache, &repo_params(&repo, robots.clone(), matched.clone()));
  views.wait_landing_needs(&WaitLandingNeedsParams {
    repository_path: repo.path_str().to_string(),
    branch_prefix: "test".to_string(),
    after: first.landing_generation,
  });
  let view = views.view(&git, &cache, &repo_params(&repo, robots, matched));
  let LandingNeeds::Known { refusals } = needs(&view, "flow") else { panic!("{view:?}") };
  let [LandingRefusal::MissingDeclaration { landing_in, .. }] = &refusals[..] else {
    panic!("{refusals:?}")
  };
  assert_eq!(
    landing_in.as_ref(),
    Some(&RobotOf {
      branch: "util".to_string(),
      robot_id: "robot-1".to_string(),
      whole_stack: false,
    })
  );
}
