use crate::branch_table::branch_table_test::placed;
use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::facts::PatronusOf;
use crate::patronus_view::{ContentMemo, match_robots, patronus_view};
use domain_types::commit::{Commit, SubjectParts};
use domain_types::fixtures::{carried, robot_of};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, PatronusView, RestartAction};
use domain_types::stack::{BlockReason, StackBranchState, StackBranchUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// A commit of the checked-out branch whose mapping note names its virtual copy `copy`
fn commit(id: &str, copy: &str) -> Commit {
  Commit {
    id: id.to_string(),
    stripped_subject: id.to_string(),
    message: id.to_string(),
    has_body: false,
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
    mapped_signed: false,
    subject_parts: SubjectParts::default(),
  }
}

/// A branch of a test table: its name, its base, its commits newest first as `(id, copy)`
pub(crate) type BranchSpec<'a> = (&'a str, Option<&'a str>, &'a [(&'a str, &'a str)]);

/// A table of `branches`, in this order, each virtual branch at the copy of its newest commit
pub(crate) fn table(branches: &[BranchSpec<'_>]) -> BranchTable {
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
pub(crate) fn chain() -> BranchTable {
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

pub(crate) fn robot(id: &str, branch: &str, status: PatronusRobotStatus, source: &str) -> PatronusRobot {
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

pub(crate) fn running(id: &str, branch: &str, source: &str) -> PatronusRobot {
  robot(id, branch, PatronusRobotStatus::Running, source)
}

pub(crate) fn merged(id: &str, branch: &str, source: &str, finish_time: u32) -> PatronusRobot {
  PatronusRobot {
    finish_time: Some(finish_time),
    ..robot(id, branch, PatronusRobotStatus::Success, source)
  }
}

pub(crate) fn stack_robot(id: &str, root: &str, source: &str) -> PatronusRobot {
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
  let of = PatronusOf {
    table,
    remote,
    robots,
    needs_review: &BTreeMap::new(),
    failing: &[],
  };
  patronus_view(&GitCommandExecutor::new(), "/nonexistent", &of, &matched, &ContentMemo::default()).unwrap_or_else(|failed| failed.view)
}

fn view_of(table: &BranchTable, robots: &[PatronusRobot]) -> PatronusView {
  view_with(table, &RemoteFacts::default(), robots)
}

/// A Safe Push of the robot `robot_id` runs on the last commit
fn last_commit(robot_id: &str) -> Reason {
  Reason::LastCommit {
    robot_id: robot_id.to_string(),
    mode: PatronusPushMode::RebaseAutosquash,
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
        .and_then(|facts| facts.landing.hold.as_ref())
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
  assert_eq!(
    auth.span.as_ref().map(|span| (&span.by, &span.carried)),
    Some((&robot_of("oauth", "r"), &Some(carried(&["a1"]))))
  );
  let span = view.branches["login"].span.clone().unwrap();
  assert_eq!(
    view.branches["login"].landing.start,
    Some(Reason::Descendant {
      branch: "login".to_string(),
      by: span.by,
      mode: PatronusPushMode::RebaseAutosquash,
      carried: span.carried
    })
  );
  let oauth = &view.branches["oauth"];
  assert_eq!(oauth.landing.start, Some(last_commit("r")));
  assert_eq!(oauth.landing.dry_run, oauth.landing.start);
  // its own robot runs on the last commit: a start replaces nothing
  assert_eq!(oauth.landing.replaces, None);
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
  assert_ne!(view.branches["oauth"].landing.start, Some(last_commit("r")));
}

/// New commits on top of the commit the robot took: the copies below it are the ones it took, so the hashes tell; the
/// robot holds its branch, a start no longer waits for it as for the last commit
#[test]
fn new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling() {
  let table = grown_chain();
  let view = view_of(&table, &[running("r", "oauth", "vo1")]);
  assert_eq!(view.branches["login"].span.as_ref().and_then(|span| span.carried.clone()), Some(carried(&["l1"])));
  let oauth = &view.branches["oauth"];
  assert_eq!(oauth.landing.start, None);
  assert_eq!(oauth.landing.hold.as_ref().map(|hold| hold.kind), Some(HoldKind::Own));
  // the Safe Push start replaces its robot of the older commit, which its push cancels
  assert_eq!(oauth.landing.replaces.as_deref(), Some("r"));

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
      known: true,
      drops: 0,
      others: 0,
    },
  );
  let view = view_with(&table, &remote, &[running("r", "oauth", "pushed")]);
  assert_eq!(view.branches["auth"].span.as_ref().and_then(|span| span.carried.clone()), Some(carried(&["a1"])));
  assert_eq!(view.branches["oauth"].landing.start, Some(last_commit("r")));

  // not up to date: only the content tells, which this repository can't: the stack as it is now, nothing counted
  remote.branches.get_mut("oauth").unwrap().up_to_date = false;
  let view = view_with(&table, &remote, &[running("r", "oauth", "pushed")]);
  let span = view.branches["auth"].span.clone().unwrap();
  assert_eq!((span.by, span.carried), (robot_of("oauth", "r"), None));
  assert_eq!(view.branches["auth"].landing.hold.as_ref().map(|hold| hold.kind), Some(HoldKind::Span));
}

/// Of two robots built on a branch, the first of the table lands it; a canceling one still lands it (its row says
/// so), but only a running one holds its push
#[test]
fn the_first_robot_of_the_table_lands_a_branch_and_only_a_running_one_holds_it() {
  let table = chain();
  let both = [running("o", "oauth", "vo1"), running("l", "login", "vl1")];
  assert_eq!(
    view_of(&table, &both).branches["auth"].span.as_ref().map(|span| span.by.clone()),
    Some(robot_of("oauth", "o"))
  );

  let table = table_in_order(&["login", "oauth", "auth"]);
  let robots = [robot("l", "login", PatronusRobotStatus::Canceling, "vl1"), running("o", "oauth", "vo1")];
  let view = view_of(&table, &robots);
  assert_eq!(view.branches["auth"].span.as_ref().map(|span| span.by.clone()), Some(robot_of("login", "l")));
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

/// login and session on auth, the stack of auth built as `test/stack/auth` at `stack-tip`
fn forked_stack() -> BranchTable {
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
    my_email: None,
    state: StackBranchState::Built,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  });
  table
}

/// The robot of the whole stack lands every branch of it a robot of a branch doesn't; it holds the stack branch, a
/// start of it waits for a robot of a branch of it, and a start of a branch for it
#[test]
fn the_robot_of_the_whole_stack_lands_every_branch_of_it() {
  let table = forked_stack();
  let whole = stack_robot("w", "auth", "stack-tip");
  let view = view_of(&table, std::slice::from_ref(&whole));
  for name in ["auth", "login", "session"] {
    let hold = view.branches[name].landing.hold.clone().unwrap();
    assert_eq!((hold.kind, hold.by.whole_stack, hold.by.branch.as_str()), (HoldKind::Span, true, "auth"), "{name}");
  }
  let stack = &view.stacks["auth"];
  assert_eq!(
    stack.landing.hold,
    Some(Reason::StackHeld {
      root: "auth".to_string(),
      robot_id: "w".to_string(),
      mode: PatronusPushMode::RebaseAutosquash
    })
  );
  assert_eq!(stack.landing.start, Some(last_commit("w")));

  // a robot of login lands auth first, and the whole stack waits for it
  let view = view_of(&table, &[whole, running("l", "login", "vl1")]);
  assert_eq!(view.branches["auth"].span.as_ref().map(|span| span.by.clone()), Some(robot_of("login", "l")));
  assert_eq!(view.branches["session"].span.as_ref().map(|span| span.by.whole_stack), Some(true));
  assert_eq!(view.stacks["auth"].landing_member, Some(robot_of("login", "l")));
}

/// A robot that finished on the last commit of a branch or of the whole stack blocks no start: a failed Safe Push is
/// started again from the same commit; while it runs, or cancels, the start waits for it
#[test]
fn a_finished_robot_on_the_last_commit_blocks_no_start_a_canceling_one_still_does() {
  let table = chain();
  let start = |status: PatronusRobotStatus| {
    let view = view_of(&table, &[robot("r", "oauth", status, "vo1")]);
    let oauth = view.branches.get("oauth").cloned().unwrap_or_default();
    (oauth.landing.start, oauth.landing.dry_run)
  };
  let waits = (Some(last_commit("r")), Some(last_commit("r")));
  assert_eq!(start(PatronusRobotStatus::Running), waits);
  assert_eq!(start(PatronusRobotStatus::Canceling), waits);
  assert_eq!(start(PatronusRobotStatus::Failure), (None, None));
  assert_eq!(start(PatronusRobotStatus::Canceled), (None, None));

  let table = forked_stack();
  let stack_start = |status: PatronusRobotStatus| {
    let view = view_of(
      &table,
      &[PatronusRobot {
        status,
        ..stack_robot("w", "auth", "stack-tip")
      }],
    );
    let stack = view.stacks.get("auth").cloned().unwrap_or_default();
    (stack.landing.start, stack.landing.dry_run)
  };
  let waits = (Some(last_commit("w")), Some(last_commit("w")));
  assert_eq!(stack_start(PatronusRobotStatus::Running), waits);
  assert_eq!(stack_start(PatronusRobotStatus::Canceling), waits);
  assert_eq!(stack_start(PatronusRobotStatus::Failure), (None, None));
  assert_eq!(stack_start(PatronusRobotStatus::Canceled), (None, None));
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
  assert_eq!((oauth.landing.hold.clone(), oauth.span.clone()), (None, None));
  assert_eq!(
    oauth.landing.dry_run,
    Some(Reason::LastCommit {
      robot_id: "d".to_string(),
      mode: PatronusPushMode::DryRun
    })
  );
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
  assert_eq!(side_fix.landing_ancestor, Some(robot_of("runtime", "rt")));
  assert_eq!(
    side_fix.landing.start,
    Some(Reason::Ancestor {
      landing: robot_of("runtime", "rt"),
      mode: PatronusPushMode::RebaseAutosquash
    })
  );
  // its own robot lands nothing of its own ancestors twice: runtime waits for none
  assert_eq!(view.branches["runtime"].landing_ancestor, None);

  let view = view_of(&table, &[running("b", "bazel", "vb1")]);
  assert_eq!(view.branches["runtime"].landing_ancestor, Some(robot_of("bazel", "b")));
  let dry = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..running("b", "bazel", "vb1")
  };
  assert_eq!(view_of(&table, &[dry]).branches.get("runtime").and_then(|facts| facts.landing_ancestor.clone()), None);
}

/// The remote facts of `names`: up to date or with work, as a remote status said
fn remote_of(names: &[(&str, bool)]) -> RemoteFacts {
  let mut remote = RemoteFacts::default();
  for (name, up_to_date) in names {
    remote.branches.insert(
      (*name).to_string(),
      RemoteFact {
        remote_tip: Some(format!("remote-{name}")),
        up_to_date: *up_to_date,
        known: true,
        drops: 0,
        others: 0,
      },
    );
  }
  remote
}

/// A Dry Run of a branch whose push goes into what a running Safe Push of a branch built on it lands: refused by that
/// hold while it pushes (D15), not when the remote has the branch and its ancestors
#[test]
fn a_dry_run_whose_push_goes_into_a_running_safe_push_is_held() {
  let table = chain();
  let robots = [running("r", "oauth", "vo1")];
  let view = view_with(&table, &remote_of(&[("login", false), ("auth", true)]), &robots);
  let login = &view.branches["login"].landing;
  assert!(
    matches!(&login.dry_run, Some(Reason::Held { branch, hold, .. }) if branch == "login" && hold.kind == HoldKind::Span),
    "{login:?}"
  );
  let view = view_with(&table, &remote_of(&[("login", true), ("auth", true)]), &robots);
  assert_eq!(view.branches["login"].landing.dry_run, None);
}

/// The restart of a robot of the user: of a failed one, on the remote while it has the branch, with a push first while
/// it lacks commits of it, refused while a robot lands an ancestor; none of one that failed for missing approvals; of a
/// running one only while a required check of it failed
#[test]
fn a_robot_offers_its_restart_with_why_it_cannot_start_now() {
  let table = chain();
  let failed = robot("f", "oauth", PatronusRobotStatus::Failure, "vo1");
  let view = view_with(&table, &remote_of(&[("oauth", true)]), std::slice::from_ref(&failed));
  let restart = view.restarts.get("f").cloned();
  assert_eq!(restart.map(|restart| (restart.action, restart.refusal)), Some((RestartAction::Restart, None)));

  // on an older commit of oauth, which the remote lacks the newer one of
  let older = robot("f", "oauth", PatronusRobotStatus::Failure, "older");
  let robots = [older, running("a", "auth", "va1")];
  let view = view_with(&table, &remote_of(&[("oauth", false)]), &robots);
  let restart = &view.restarts["f"];
  assert_eq!(restart.action, RestartAction::PushAndRestart);
  assert!(
    matches!(&restart.refusal, Some(Reason::Ancestor { landing, .. }) if landing.branch == "auth"),
    "{restart:?}"
  );

  let matched = match_robots(&table, &RemoteFacts::default(), std::slice::from_ref(&failed));
  let needs_review = BTreeMap::from([("f".to_string(), true)]);
  let of = PatronusOf {
    table: &table,
    remote: &RemoteFacts::default(),
    robots: std::slice::from_ref(&failed),
    needs_review: &needs_review,
    failing: &[],
  };
  let view = patronus_view(&GitCommandExecutor::new(), "/nonexistent", &of, &matched, &ContentMemo::default()).expect("the hashes tell");
  assert!(view.restarts.is_empty());

  // a running one on the commit of oauth only once a required check of it failed: its restart cancels it first
  let doomed = running("r", "oauth", "vo1");
  let remote = remote_of(&[("oauth", true)]);
  assert!(view_with(&table, &remote, std::slice::from_ref(&doomed)).restarts.is_empty());
  let matched = match_robots(&table, &remote, std::slice::from_ref(&doomed));
  let failing = ["r".to_string()];
  let of = PatronusOf {
    table: &table,
    remote: &remote,
    robots: std::slice::from_ref(&doomed),
    needs_review: &BTreeMap::new(),
    failing: &failing,
  };
  let view = patronus_view(&GitCommandExecutor::new(), "/nonexistent", &of, &matched, &ContentMemo::default()).expect("the hashes tell");
  let restart = view.restarts.get("r").cloned();
  assert_eq!(restart.map(|restart| (restart.action, restart.refusal)), Some((RestartAction::Restart, None)));
}

/// What the content told of a merged robot is read by its id and the commits it covered (`ContentMemo`): of the branches
/// as they are, its answer of them; of a state not asked yet (auth has a commit since), its latest answer, of the
/// branches as they were, which the readers judge against the branches as they are (`LandedByView::knows`: the view
/// doesn't know auth as it is, so its step waits for the derivation of it); without an answer, nothing by content
#[test]
fn the_view_reads_what_the_content_told_of_a_robot_by_its_id_and_the_commits_it_covered() {
  use crate::patronus_view::{merged_plans, view_of as view_of_memo};
  use domain_types::robot_landing::BranchLanding;

  // the robot took a commit no copy of auth is: only the content tells what it landed
  let robots = [merged("r", "auth", "taken", 20)];
  let as_it_was = table(&[("auth", None, &[("a1", "va1")])]);
  let grown = table(&[("auth", None, &[("a2", "va2"), ("a1", "va1")])]);
  let remote = RemoteFacts::default();
  let key_of = |table: &BranchTable| {
    let plans = merged_plans(table, &table.repository(), &remote, &robots);
    assert!(plans[0].hashes.is_none(), "the hashes told");
    plans[0].question.key.clone()
  };
  let memo = ContentMemo::default();
  let landed = |table: &BranchTable, memo: &ContentMemo| {
    let of = PatronusOf {
      table,
      remote: &remote,
      robots: &robots,
      needs_review: &BTreeMap::new(),
      failing: &[],
    };
    let view = view_of_memo(&of, &match_robots(table, &remote, &robots), memo);
    view.branches.get("auth").and_then(|facts| facts.landed_by.clone())
  };
  assert_eq!(landed(&as_it_was, &memo), None, "no answer: nothing by content");
  memo.keep_landings([(
    "r".to_string(),
    key_of(&as_it_was),
    vec![BranchLanding {
      branch: "auth".to_string(),
      landed: vec!["a1".to_string()],
      after: Vec::new(),
    }],
  )]);
  assert_ne!(key_of(&as_it_was), key_of(&grown));
  let of_it = landed(&as_it_was, &memo).unwrap();
  assert_eq!(of_it.landed, ["a1"]);
  assert!(as_it_was.view_knows("auth", Some(&of_it)));
  let of_grown = landed(&grown, &memo).unwrap();
  assert_eq!(of_grown.landed, ["a1"], "its latest answer");
  assert!(!grown.view_knows("auth", Some(&of_grown)), "of the branch as it was");

  // a robot that took auth as it was: the hashes told it, and a derivation keeps that as its latest answer, which a view
  // of auth reworded since (no copy is the commit it took) reads
  let at_tip = [merged("h", "auth", "va1", 20)];
  let needs_review = BTreeMap::new();
  let of = |table| PatronusOf {
    table,
    remote: &remote,
    robots: &at_tip,
    needs_review: &needs_review,
    failing: &[],
  };
  let memo = ContentMemo::default();
  super::learn_content(
    &GitCommandExecutor::new(),
    "/nonexistent",
    &of(&as_it_was),
    &match_robots(&as_it_was, &remote, &at_tip),
    &memo,
  )
  .expect("the hashes tell");
  let reworded = table(&[("auth", None, &[("a3", "va3")])]);
  let view = view_of_memo(&of(&reworded), &match_robots(&reworded, &remote, &at_tip), &memo);
  let told = view.branches.get("auth").and_then(|facts| facts.landed_by.clone());
  assert_eq!(
    told.as_ref().map(|landed| landed.landed.clone()),
    Some(vec!["a1".to_string()]),
    "what the hashes told of auth as it was"
  );
  assert!(!reworded.view_knows("auth", told.as_ref()));
}

/// What the content told the merged robot `robot` landed of its branch as `table` has it (`landed`, the commits it
/// landed, none after), kept in `memo` as a derivation keeps it (`learn_content`)
pub(crate) fn content_told(memo: &ContentMemo, table: &BranchTable, robot: &PatronusRobot, landed: &[&str]) {
  use crate::patronus_view::merged_plans;
  use domain_types::robot_landing::BranchLanding;
  let plans = merged_plans(table, &table.repository(), &RemoteFacts::default(), std::slice::from_ref(robot));
  let branch = robot.branch_name.clone().unwrap_or_default();
  memo.keep_landings([(
    robot.id.clone(),
    plans[0].question.key.clone(),
    vec![BranchLanding {
      branch,
      landed: landed.iter().map(ToString::to_string).collect(),
      after: Vec::new(),
    }],
  )]);
}

/// The view of `table` with `robots` over what `memo` keeps, no git (`view_of`)
fn view_over(table: &BranchTable, robots: &[PatronusRobot], memo: &ContentMemo) -> PatronusView {
  let remote = RemoteFacts::default();
  let of = PatronusOf {
    table,
    remote: &remote,
    robots,
    needs_review: &BTreeMap::new(),
    failing: &[],
  };
  super::view_of(&of, &match_robots(table, &remote, robots), memo)
}

/// A merged robot holds as landed only the branch it landed as the table has it: the latest answer of the content, of
/// auth as it was, names what landed it, but auth has a commit since, which nothing landed, so no hold keeps its push or
/// its start (`LandingFacts::new` judges the answer against the branch, `BranchTable::landed_whole`)
#[test]
fn a_merged_robot_holds_as_landed_only_the_branch_as_it_landed_it() {
  // the robot took a commit no copy of auth is: only the content tells what it landed
  let robots = [merged("r", "auth", "taken", 20)];
  let as_it_was = table(&[("auth", None, &[("a1", "va1")])]);
  let grown = table(&[("auth", None, &[("a2", "va2"), ("a1", "va1")])]);
  let memo = ContentMemo::default();
  content_told(&memo, &as_it_was, &robots[0], &["a1"]);
  assert_eq!(holds(&view_over(&as_it_was, &robots, &memo), &["auth"]), [Some((HoldKind::Landed, "r".to_string()))]);
  let view = view_over(&grown, &robots, &memo);
  assert_eq!(
    view.branches["auth"].landed_by.as_ref().map(|landed| landed.landed.clone()),
    Some(vec!["a1".to_string()]),
    "its latest answer, of auth as it was"
  );
  assert_eq!(holds(&view, &["auth"]), [None], "a2 landed with nothing");
}

/// A derivation git fails to tell the content to fails with the error and the view of what the memo has
/// (`ContentFailed`), keeping what the hashes told: a robot that took auth as it is and one that took a commit no copy is
/// (an object id git is asked of, in a repository that is not one); the failed question stays unasked, the next
/// derivation asks it again
#[test]
fn a_derivation_git_fails_to_tell_the_content_to_fails_with_the_view_of_what_the_memo_has() {
  use crate::patronus_view::merged_plans;
  let auth = table(&[("login", Some("auth"), &[("l1", "vl1")]), ("auth", None, &[("a1", "va1")])]);
  let robots = [merged("h", "auth", "va1", 10), merged("c", "login", &"c".repeat(40), 20)];
  let remote = RemoteFacts::default();
  let of = PatronusOf {
    table: &auth,
    remote: &remote,
    robots: &robots,
    needs_review: &BTreeMap::new(),
    failing: &[],
  };
  let memo = ContentMemo::default();
  let failed = patronus_view(&GitCommandExecutor::new(), "/nonexistent", &of, &match_robots(&auth, &remote, &robots), &memo).expect_err("git failed");
  assert!(
    format!("{:#}", failed.error).starts_with("Cannot tell what the merged Safe Pushes landed"),
    "{:#}",
    failed.error
  );
  assert_eq!(failed.view, view_over(&auth, &robots, &memo), "the view of what the memo has");
  assert_eq!(
    failed.view.branches["auth"].landed_by.as_ref().map(|landed| landed.by.robot_id.clone()),
    Some("h".to_string()),
    "the hashes told"
  );
  let plans = merged_plans(&auth, &auth.repository(), &remote, &robots);
  let kept = memo.answers();
  assert!(kept.latest_landing.contains_key("h"));
  assert!(!kept.landings.contains_key(&plans[1].question.key), "asked again by the next derivation");
}

/// A frame keeps of the content what the view of its table reads: the answers of the questions of the listed robots over
/// it (`content_keys`) and the latest answer of each listed robot; not an answer of a state the table left, nor of a
/// robot the list no longer has
#[test]
fn a_frame_keeps_the_answers_the_view_of_its_table_reads() {
  use crate::patronus_view::content_keys;
  use std::collections::HashSet;
  let (robot, gone) = (merged("r", "auth", "taken", 20), merged("g", "auth", "other", 10));
  let as_it_was = table(&[("auth", None, &[("a1", "va1")])]);
  let grown = table(&[("auth", None, &[("a2", "va2"), ("a1", "va1")])]);
  let reworded = table(&[("auth", None, &[("a3", "va3")])]);
  let memo = ContentMemo::default();
  content_told(&memo, &as_it_was, &robot, &["a1"]);
  content_told(&memo, &grown, &robot, &["a2", "a1"]);
  content_told(&memo, &as_it_was, &gone, &["a1"]);
  let robots = [robot];
  let remote = RemoteFacts::default();
  let frame = |table: &BranchTable| {
    let keys = content_keys(&PatronusOf {
      table,
      remote: &remote,
      robots: &robots,
      needs_review: &BTreeMap::new(),
      failing: &[],
    });
    memo.frame_of(&keys, &HashSet::from(["r"]))
  };
  let landed = |answers: &super::MemoAnswers| {
    let mut landed: Vec<Vec<String>> = answers.landings.values().map(|branches| branches[0].landed.clone()).collect();
    landed.sort();
    (landed, answers.latest_landing.keys().cloned().collect::<Vec<_>>())
  };
  let r = vec!["r".to_string()];
  assert_eq!(landed(&frame(&grown)), (vec![vec!["a2".to_string(), "a1".to_string()]], r.clone()), "of grown, its latest");
  assert_eq!(
    landed(&frame(&as_it_was)),
    (vec![vec!["a1".to_string()], vec!["a2".to_string(), "a1".to_string()]], r.clone()),
    "of auth as it was, and the latest"
  );
  assert_eq!(
    landed(&frame(&reworded)),
    (vec![vec!["a2".to_string(), "a1".to_string()]], r),
    "a state no answer is of: the latest"
  );
}

/// What a frame keeps of the content on the rows of `pnpm perf:table` (`derive_budget_test::session`: 141 rows, robots
/// running on ten) with 33 merged robots only the content tells: of a memo that answered each of them for ten earlier
/// states of the branches too, the answers of the table as it is: 6.7 of 51 KB on 2026-11-14 (the latest answer of each
/// robot names its key once more); the bytes and the time of the frame printed
#[test]
fn a_frame_of_the_large_table_keeps_the_content_of_its_table_only() {
  use crate::derive::derive_budget_test::{at, session};
  use crate::facts::{FactResult, RobotsRead, listed_robots};
  use domain_types::patronus::{PatronusAvailability, PatronusRobotsResult};
  use std::sync::Arc;

  let (state, mut facts) = session();
  let table = facts.table.value().map(Arc::clone).unwrap();
  let merged_robots: Vec<PatronusRobot> = (0u32..)
    .zip(table.branches.iter().skip(1).step_by(4).take(33))
    .map(|(index, branch)| merged(&format!("m{index}"), &branch.name, &format!("taken-{index}"), 20 + index))
    .collect();
  let mut robots = listed_robots(facts.robots.view());
  robots.extend(merged_robots.iter().cloned());
  facts.apply(
    at(10),
    FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: PatronusRobotsResult {
        availability: PatronusAvailability::Available,
        robots,
        github_repository: None,
      },
    },
  );
  let landed_of = |table: &BranchTable, robot: &PatronusRobot| -> Vec<String> {
    table
      .branch(robot.branch_name.as_deref().unwrap_or_default())
      .map_or_else(Vec::new, |branch| branch.commits.iter().map(|commit| commit.id.clone()).collect())
  };
  for earlier in 0..10 {
    let mut past = BranchTable::clone(&table);
    for branch in &mut past.branches {
      branch.commits[0].id = format!("{}-v{earlier}", branch.commits[0].id);
    }
    for robot in &merged_robots {
      let landed = landed_of(&past, robot);
      content_told(&facts.memo, &past, robot, &landed.iter().map(String::as_str).collect::<Vec<_>>());
    }
  }
  for robot in &merged_robots {
    let landed = landed_of(&table, robot);
    content_told(&facts.memo, &table, robot, &landed.iter().map(String::as_str).collect::<Vec<_>>());
  }
  let whole = serde_json::to_string(&*facts.memo.answers()).unwrap().len();
  let started = std::time::Instant::now();
  let framed = facts.frame(&state).content;
  let elapsed = started.elapsed();
  let kept = serde_json::to_string(&framed).unwrap().len();
  eprintln!(
    "the content of a frame of 141 rows and 33 merged robots: {whole} bytes of every answer the memo kept (11 states), {kept} bytes of the table as it is; the frame in {elapsed:?}"
  );
  assert_eq!((framed.landings.len(), framed.latest_landing.len()), (33, 33));
  assert!(kept * 7 <= whole, "{kept} of {whole} bytes");
}
