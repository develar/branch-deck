use crate::branch_table::BranchTable;
use crate::branch_table::branch_table_test::placed;
use crate::patronus_view::{ContentMemo, match_robots, patronus_view};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::repository_view::{Carried, HoldKind, PatronusView, RemoteFact, RemoteFacts, RobotOf, StartRefusal};
use domain_types::stack::{BlockReason, StackBranchState, StackBranchUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use pretty_assertions::assert_eq;

/// A commit of the checked-out branch whose mapping note names its virtual copy `copy`
fn commit(id: &str, copy: &str) -> Commit {
  Commit {
    id: id.to_string(),
    stripped_subject: id.to_string(),
    message: id.to_string(),
    author_name: "Test".to_string(),
    author_email: "test@example.com".to_string(),
    author_timestamp: 1,
    committer_timestamp: 1,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject: id.to_string(),
    parent_id: None,
    tree_id: format!("tree-{id}"),
    note: None,
    mapped_commit_id: Some(copy.to_string()),
    mapped_source: None,
    mapped_resolution: None,
  }
}

/// A branch of a test table: its name, its base, its commits newest first as `(id, copy)`
type BranchSpec<'a> = (&'a str, Option<&'a str>, &'a [(&'a str, &'a str)]);

/// A table of `branches`, in this order, each virtual branch at the copy of its newest commit
fn table(branches: &[BranchSpec<'_>]) -> BranchTable {
  let mut table = BranchTable {
    prefix: "test".to_string(),
    baseline: "origin/master".to_string(),
    ..BranchTable::default()
  };
  for (name, base, commits) in branches {
    let mut branch = placed(name, *base);
    branch.commits = commits.iter().map(|(id, copy)| commit(id, copy)).collect();
    if let Some((_, tip)) = commits.first() {
      table.tips.insert(format!("test/virtual/{name}"), (*tip).to_string());
    }
    table.branches.push(branch);
  }
  table
}

/// auth with login built on it and oauth on login, oauth first in the table as the newest
fn chain() -> BranchTable {
  table(&[
    ("oauth", Some("login"), &[("o1", "vo1")]),
    ("login", Some("auth"), &[("l1", "vl1")]),
    ("auth", None, &[("a1", "va1")]),
  ])
}

/// `chain` with a new commit of oauth on the one a robot may have taken
fn grown_chain() -> BranchTable {
  table(&[
    ("oauth", Some("login"), &[("o2", "vo2"), ("o1", "vo1")]),
    ("login", Some("auth"), &[("l1", "vl1")]),
    ("auth", None, &[("a1", "va1")]),
  ])
}

fn robot(id: &str, branch: &str, status: PatronusRobotStatus, source: &str) -> PatronusRobot {
  PatronusRobot {
    id: id.to_string(),
    name: "IJPL-1 fix".to_string(),
    status,
    push_mode: PatronusPushMode::RebaseAutosquash,
    branch_name: Some(branch.to_string()),
    stack_root: None,
    source_revision: Some(source.to_string()),
    target_branch: "master".to_string(),
    start_time: 100,
    finish_time: (status == PatronusRobotStatus::Success).then_some(200),
    cancellation_reason: None,
    url: format!("https://patronus/robot/{id}"),
    pull_request: None,
    profile: None,
  }
}

fn running(id: &str, branch: &str, source: &str) -> PatronusRobot {
  robot(id, branch, PatronusRobotStatus::Running, source)
}

fn merged(id: &str, branch: &str, source: &str, finish_time: u32) -> PatronusRobot {
  PatronusRobot {
    finish_time: Some(finish_time),
    ..robot(id, branch, PatronusRobotStatus::Success, source)
  }
}

fn stack_robot(id: &str, root: &str, source: &str) -> PatronusRobot {
  PatronusRobot {
    branch_name: None,
    stack_root: Some(root.to_string()),
    ..running(id, root, source)
  }
}

/// The view of `table` with `robots`, the remote facts `remote`, matched as the rows match them; git is never asked (a
/// repository that is not one: what the content would tell is unknown)
fn view_with(table: &BranchTable, remote: &RemoteFacts, robots: &[PatronusRobot]) -> PatronusView {
  let matched = match_robots(table, remote, robots);
  patronus_view(&GitCommandExecutor::new(), "/nonexistent", table, remote, robots, &matched, &mut ContentMemo::default())
}

fn view_of(table: &BranchTable, robots: &[PatronusRobot]) -> PatronusView {
  view_with(table, &RemoteFacts::default(), robots)
}

fn of(branch: &str, robot_id: &str) -> RobotOf {
  RobotOf {
    branch: branch.to_string(),
    robot_id: robot_id.to_string(),
    whole_stack: false,
  }
}

fn carried(commits: &[&str]) -> Carried {
  Carried {
    commits: commits.iter().map(|id| (*id).to_string()).collect(),
    amended: Vec::new(),
  }
}

/// The kind and the robot of the hold of each branch
fn holds(view: &PatronusView, names: &[&str]) -> Vec<Option<(HoldKind, String)>> {
  names
    .iter()
    .map(|name| {
      view
        .branches
        .get(*name)
        .and_then(|facts| facts.hold.as_ref())
        .map(|hold| (hold.kind, hold.by.robot_id.clone()))
    })
    .collect()
}

/// A running robot at the tip of oauth lands auth and login with it: they are held, say so and wait with their starts;
/// oauth is held by its own robot, which runs on its last commit; no branch counts its own robot as an ancestor's
#[test]
fn a_running_robot_at_its_tip_lands_the_branches_it_is_built_on() {
  let table = chain();
  let view = view_of(&table, &[running("r", "oauth", "vo1")]);

  assert_eq!(
    holds(&view, &["auth", "login", "oauth"]),
    [
      Some((HoldKind::Span, "r".to_string())),
      Some((HoldKind::Span, "r".to_string())),
      Some((HoldKind::Own, "r".to_string()))
    ]
  );
  let auth = &view.branches["auth"];
  assert_eq!(auth.span.as_ref().map(|span| (&span.by, &span.carried)), Some((&of("oauth", "r"), &Some(carried(&["a1"])))));
  assert_eq!(
    view.branches["login"].start,
    Some(StartRefusal::Descendant {
      span: view.branches["login"].span.clone().unwrap()
    })
  );
  let oauth = &view.branches["oauth"];
  assert!(oauth.start_blocked);
  assert_eq!(oauth.start, Some(StartRefusal::LastCommit { robot_id: "r".to_string() }));
  assert_eq!(oauth.dry_run, oauth.start);
  // its own robot lands login and auth: the start replaces it, so it is no landing ancestor of oauth
  assert_eq!(oauth.landing_ancestor, None);
}

/// A branch the sync did not build keeps the ref of an older build: a robot at that ref took commits the branch may no
/// longer have, so it is no robot of the branch's last commit, and a start does not wait for it as for one
#[test]
fn the_tip_of_a_branch_the_sync_did_not_build_is_no_build_of_its_commits() {
  let mut table = chain();
  table.not_built.insert("oauth".to_string(), BlockReason::Conflict);
  assert_eq!(table.tip_of("oauth"), None);
  assert_eq!(table.tip_of("login"), Some("vl1"));
  let view = view_of(&table, &[running("r", "oauth", "vo1")]);
  assert_ne!(view.branches["oauth"].start, Some(StartRefusal::LastCommit { robot_id: "r".to_string() }));
}

/// New commits on top of the commit the robot took: the copies below it are the ones it took, so the hashes tell; the
/// robot holds its branch, a start no longer waits for it as for the last commit
#[test]
fn new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling() {
  let table = grown_chain();
  let view = view_of(&table, &[running("r", "oauth", "vo1")]);
  assert_eq!(view.branches["login"].span.as_ref().and_then(|span| span.carried.clone()), Some(carried(&["l1"])));
  let oauth = &view.branches["oauth"];
  assert!(!oauth.start_blocked);
  assert_eq!(oauth.start, None);
  assert_eq!(oauth.hold.as_ref().map(|hold| hold.kind), Some(HoldKind::Own));

  // merged so: oauth's new commit is the work left of it, the rest landed
  let view = view_of(&table, &[merged("m", "oauth", "vo1", 200)]);
  let landed = |name: &str| view.branches[name].landed_by.as_ref().map(|landed| (landed.own, landed.after.clone()));
  assert_eq!(landed("auth"), Some((false, Vec::new())));
  assert_eq!(landed("oauth"), Some((true, vec!["o2".to_string()])));
  assert_eq!(
    holds(&view, &["auth", "login", "oauth"]),
    [Some((HoldKind::Landed, "m".to_string())), Some((HoldKind::Landed, "m".to_string())), None]
  );
  assert_eq!(
    view.landed_by_robot["m"].iter().map(|branch| branch.branch.as_str()).collect::<Vec<_>>(),
    ["auth", "login", "oauth"]
  );
}

/// A robot on the remote branch, which has the commits of the branch by content, only rebuilt since on a newer baseline:
/// the hashes tell, and a start would test the same commits again
#[test]
fn a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is() {
  let table = chain();
  let mut remote = RemoteFacts::default();
  remote.branches.insert(
    "oauth".to_string(),
    RemoteFact {
      remote_tip: Some("pushed".to_string()),
      up_to_date: true,
    },
  );
  let view = view_with(&table, &remote, &[running("r", "oauth", "pushed")]);
  assert_eq!(view.branches["auth"].span.as_ref().and_then(|span| span.carried.clone()), Some(carried(&["a1"])));
  assert!(view.branches["oauth"].start_blocked);

  // not up to date: only the content tells, which this repository can't: the stack as it is now, nothing counted
  remote.branches.get_mut("oauth").unwrap().up_to_date = false;
  let view = view_with(&table, &remote, &[running("r", "oauth", "pushed")]);
  let span = view.branches["auth"].span.clone().unwrap();
  assert_eq!((span.by, span.carried), (of("oauth", "r"), None));
  assert_eq!(view.branches["auth"].hold.as_ref().map(|hold| hold.kind), Some(HoldKind::Span));
}

/// Of two robots built on a branch, the first of the table lands it; a canceling one still lands it (its row says
/// so), but only a running one holds its push
#[test]
fn the_first_robot_of_the_table_lands_a_branch_and_only_a_running_one_holds_it() {
  let table = chain();
  let both = [running("o", "oauth", "vo1"), running("l", "login", "vl1")];
  assert_eq!(view_of(&table, &both).branches["auth"].span.as_ref().map(|span| span.by.clone()), Some(of("oauth", "o")));

  let table = table_in_order(&["login", "oauth", "auth"]);
  let robots = [robot("l", "login", PatronusRobotStatus::Canceling, "vl1"), running("o", "oauth", "vo1")];
  let view = view_of(&table, &robots);
  assert_eq!(view.branches["auth"].span.as_ref().map(|span| span.by.clone()), Some(of("login", "l")));
  assert_eq!(holds(&view, &["auth"]), [Some((HoldKind::Span, "o".to_string()))]);
}

/// `chain` with its branches in this order
fn table_in_order(order: &[&str]) -> BranchTable {
  let chain = chain();
  BranchTable {
    branches: order.iter().map(|name| chain.branch(name).unwrap().clone()).collect(),
    ..chain
  }
}

/// The robot of the whole stack lands every branch of it a robot of a branch doesn't; it holds the stack branch, a
/// start of it waits for a robot of a branch of it, and a start of a branch for it
#[test]
fn the_robot_of_the_whole_stack_lands_every_branch_of_it() {
  let mut table = table(&[
    ("session", Some("auth"), &[("s1", "vs1")]),
    ("login", Some("auth"), &[("l1", "vl1")]),
    ("auth", None, &[("a1", "va1")]),
  ]);
  table.stack_branches.push(StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "test/stack/auth".to_string(),
    members: vec!["auth".to_string(), "login".to_string(), "session".to_string()],
    commit_count: 3,
    state: StackBranchState::Built,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  });
  let whole = stack_robot("w", "auth", "stack-tip");
  let view = view_of(&table, std::slice::from_ref(&whole));
  for name in ["auth", "login", "session"] {
    let hold = view.branches[name].hold.clone().unwrap();
    assert_eq!((hold.kind, hold.by.whole_stack, hold.by.branch.as_str()), (HoldKind::Span, true, "auth"), "{name}");
  }
  let stack = &view.stacks["auth"];
  assert_eq!((stack.hold_robot_id.as_deref(), stack.start_blocked), (Some("w"), true));
  assert_eq!(stack.start, Some(StartRefusal::LastCommit { robot_id: "w".to_string() }));

  // a robot of login lands auth first, and the whole stack waits for it
  let view = view_of(&table, &[whole, running("l", "login", "vl1")]);
  assert_eq!(view.branches["auth"].span.as_ref().map(|span| span.by.clone()), Some(of("login", "l")));
  assert_eq!(view.branches["session"].span.as_ref().map(|span| span.by.whole_stack), Some(true));
  assert_eq!(view.stacks["auth"].landing_member, Some(of("login", "l")));
}

/// Of two merged robots that landed a branch, the one that left the fewest commits after it, of equals the first merge;
/// a branch with commits after the landed ones is held by no landing
#[test]
fn the_merged_robot_that_left_the_least_work_names_the_landing() {
  let table = grown_chain();
  // the robot of login merged after the one of oauth's older commit: both landed auth and login whole
  let robots = [merged("o", "oauth", "vo1", 200), merged("l", "login", "vl1", 300)];
  let view = view_of(&table, &robots);
  assert_eq!(view.branches["auth"].landed_by.as_ref().map(|landed| landed.by.robot_id.as_str()), Some("o"));
  assert_eq!(
    view.branches["login"].landed_by.as_ref().map(|landed| (landed.by.robot_id.as_str(), landed.own)),
    Some(("o", false))
  );
  assert_eq!(view.branches["oauth"].landed_by.as_ref().map(|landed| landed.after.clone()), Some(vec!["o2".to_string()]));
  assert_eq!(view.landed_by_robot.keys().collect::<Vec<_>>(), ["l", "o"]);
}

/// A dry run lands nothing and holds nothing; on the last commit it still refuses a start of the same commit
#[test]
fn a_dry_run_lands_nothing() {
  let table = chain();
  let dry = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..running("d", "oauth", "vo1")
  };
  let view = view_of(&table, &[dry]);
  assert_eq!(view.branches.keys().collect::<Vec<_>>(), ["oauth"]);
  let oauth = &view.branches["oauth"];
  assert_eq!((oauth.hold.clone(), oauth.span.clone()), (None, None));
  assert_eq!(oauth.dry_run, Some(StartRefusal::LastCommit { robot_id: "d".to_string() }));
}

/// A Safe Push of a stacked branch waits for a robot that lands an ancestor: of the ancestor itself, or of a side branch
/// built on it, which lands it too; a dry run lands nothing, so nothing waits for it
#[test]
fn a_branch_waits_for_a_robot_that_lands_an_ancestor_not_for_a_dry_run() {
  // runtime and side-fix are built on bazel, bazel on air
  let table = table(&[
    ("side-fix", Some("bazel"), &[("f1", "vf1")]),
    ("runtime", Some("bazel"), &[("r1", "vr1")]),
    ("bazel", Some("air"), &[("b1", "vb1")]),
    ("air", None, &[("a1", "va1")]),
  ]);
  let view = view_of(&table, &[running("rt", "runtime", "vr1")]);
  let side_fix = &view.branches["side-fix"];
  assert_eq!(side_fix.landing_ancestor, Some(of("runtime", "rt")));
  assert_eq!(side_fix.start, Some(StartRefusal::Ancestor { landing: of("runtime", "rt") }));
  // its own robot lands nothing of its own ancestors twice: runtime waits for none
  assert_eq!(view.branches["runtime"].landing_ancestor, None);

  let view = view_of(&table, &[running("b", "bazel", "vb1")]);
  assert_eq!(view.branches["runtime"].landing_ancestor, Some(of("bazel", "b")));
  let dry = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..running("b", "bazel", "vb1")
  };
  assert_eq!(view_of(&table, &[dry]).branches.get("runtime").and_then(|facts| facts.landing_ancestor.clone()), None);
}
