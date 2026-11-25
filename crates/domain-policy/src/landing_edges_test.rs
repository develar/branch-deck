use crate::landing_edges::{edges_of, landing_edges, whole_stack_refusals};
use domain_types::landing::{BindingKind, EdgeKind, EdgeState, LandingEdge, LandingFix, LandingRefusal, NeedState, ReferenceKind, StackCommit, Subject};
use domain_types::repository_view::RobotOf;
use pretty_assertions::assert_eq;

const FILE: &str = "plugins/air/native/air-ide/src/acp_load_replay.rs";
const NAMES: [&str; 5] = ["step", "Effect", "Input", "ReplayRecord", "StepError"];

fn commit(id: &str, branch: &str) -> StackCommit {
  StackCommit {
    id: id.to_string(),
    subject: format!("Commit {id}"),
    branch: branch.to_string(),
  }
}

fn robot(branch: &str) -> RobotOf {
  RobotOf {
    branch: branch.to_string(),
    robot_id: format!("robot-{branch}"),
    whole_stack: false,
  }
}

fn restack(root: &str, onto: &str) -> LandingFix {
  LandingFix::Restack {
    branch: root.to_string(),
    onto: onto.to_string(),
  }
}

/// A use in `branch` of `name`, which a commit of `declared_in` adds
fn missing(branch: &str, name: &str, declared_in: &str, fix: LandingFix, landing_in: Option<RobotOf>) -> LandingRefusal {
  LandingRefusal::MissingDeclaration {
    branch: branch.to_string(),
    identifier: name.to_string(),
    file: FILE.to_string(),
    line: 7,
    used_by: commit("u1", branch),
    declared_by: commit("d1", declared_in),
    fix,
    landing_in,
  }
}

fn declared(branch: &str, needs: &str, inherited_from: Option<&str>) -> LandingRefusal {
  LandingRefusal::UnlandedNeed {
    branch: branch.to_string(),
    needs: needs.to_string(),
    state: NeedState::Active,
    inherited_from: inherited_from.map(str::to_string),
    fix: inherited_from.map(|from| LandingFix::Unstack {
      branch: branch.to_string(),
      from: from.to_string(),
    }),
    landing_in: None,
  }
}

/// The five names acp-load-replay adds that the stack of air-home-watch uses, as the gate refuses each start of it
fn idea_1_refusals(landing_in: Option<&RobotOf>) -> Vec<LandingRefusal> {
  NAMES
    .iter()
    .map(|name| missing("air-home-watch", name, "acp-load-replay", restack("air-home-watch", "acp-load-replay"), landing_in.cloned()))
    .collect()
}

fn edge(from: &str, needed: &str, kind: EdgeKind, state: EdgeState, refuses: &[&str], fix: Option<LandingFix>) -> LandingEdge {
  LandingEdge {
    from: from.to_string(),
    needed: needed.to_string(),
    kind,
    state,
    names: Vec::new(),
    files: Vec::new(),
    refusals: 1,
    refuses: refuses.iter().map(|branch| (*branch).to_string()).collect(),
    fix,
    actions: Vec::new(),
  }
}

fn none(_: &str) -> Option<RobotOf> {
  None
}

/// idea-1 on 2026-11-24: the stack of air-home-watch (hosted-exit and claude-folds built on it) needs the five names of
/// acp-load-replay, which is landing in its own Safe Push. The gate refuses each of the three starts five times: fifteen
/// refusals are one edge on the root of the stack, which waits for that robot
#[test]
fn fifteen_refusals_of_a_stack_are_one_edge_on_its_root_that_waits_for_the_running_safe_push() {
  let acp = robot("acp-load-replay");
  let refusals = idea_1_refusals(Some(&acp));
  let needs = [
    ("air-home-watch", "air-home-watch", refusals.as_slice()),
    ("hosted-exit", "air-home-watch", refusals.as_slice()),
    ("claude-folds", "air-home-watch", refusals.as_slice()),
  ];
  let stack_root_of = |branch: &str| match branch {
    "air-home-watch" | "hosted-exit" | "claude-folds" => Some("air-home-watch"),
    "acp-load-replay" => Some("acp-load-replay"),
    _ => None,
  };
  let edges = landing_edges(needs, stack_root_of, none);
  let expected = LandingEdge {
    names: vec!["Effect", "Input", "ReplayRecord", "StepError", "step"].into_iter().map(str::to_string).collect(),
    files: vec![FILE.to_string()],
    refusals: 15,
    ..edge(
      "air-home-watch",
      "acp-load-replay",
      EdgeKind::Build,
      EdgeState::Waits { robot: acp },
      &["air-home-watch", "hosted-exit", "claude-folds"],
      Some(restack("air-home-watch", "acp-load-replay")),
    )
  };
  assert_eq!(edges, vec![expected.clone()]);
  // the start of a branch of the stack: one edge of its own, from the root of what it lands
  assert_eq!(
    edges_of("hosted-exit", "air-home-watch", &refusals, none),
    vec![LandingEdge {
      refusals: 5,
      refuses: vec!["hosted-exit".to_string()],
      ..expected
    }]
  );
}

/// The state of an edge, by where the branch it needs stands: landing, its robot failed, nothing lands it
#[test]
fn an_edge_waits_while_what_it_needs_is_landing_needs_a_restart_of_a_failed_robot_else_is_open() {
  let acp = robot("acp-load-replay");
  let state = |landing_in: Option<&RobotOf>, failed: Option<&RobotOf>| {
    let edges = edges_of("air-home-watch", "air-home-watch", &idea_1_refusals(landing_in), |branch| {
      failed.filter(|_| branch == "acp-load-replay").cloned()
    });
    assert_eq!(edges.len(), 1);
    edges[0].state.clone()
  };
  assert_eq!(state(Some(&acp), None), EdgeState::Waits { robot: acp.clone() });
  // a running Safe Push is newer than the failed one
  assert_eq!(state(Some(&acp), Some(&acp)), EdgeState::Waits { robot: acp.clone() });
  assert_eq!(state(None, Some(&acp)), EdgeState::NeededFailed { robot: acp.clone() });
  assert_eq!(state(None, None), EdgeState::Open);
}

/// A dangling reference is a need of the build too: it names the running Safe Push of the branch of the commit that
/// changes the use, and its edge waits for it
#[test]
fn a_dangling_reference_is_an_edge_of_the_build_that_waits_for_the_branch_changing_the_use() {
  let acp = robot("acp-load-replay");
  let dangling = LandingRefusal::DanglingReference {
    branch: "air-home-watch".to_string(),
    reference: ReferenceKind::Removed,
    identifier: "replay".to_string(),
    file: "src/lib.rs".to_string(),
    line: 3,
    removed_by: commit("r1", "air-home-watch"),
    fixed_by: commit("f1", "acp-load-replay"),
    fix: restack("air-home-watch", "acp-load-replay"),
    landing_in: Some(acp.clone()),
  };
  let mut refusals = idea_1_refusals(None);
  refusals.push(dangling);
  let edges = edges_of("air-home-watch", "air-home-watch", &refusals, none);
  assert_eq!(edges.len(), 1);
  assert_eq!(edges[0].state, EdgeState::Waits { robot: acp });
  assert_eq!(edges[0].refusals, 6);
  assert_eq!(edges[0].files, vec![FILE.to_string(), "src/lib.rs".to_string()]);
  assert!(edges[0].names.contains(&"replay".to_string()));
}

/// A declared need is an edge of its own, never merged with a need of the build of the same pair (the need is what
/// keeps them in two robots); a need of a branch of a companion is a companion edge, whose robots are its own. A lost
/// binding is no edge: no branch helps
#[test]
fn a_declared_need_is_never_merged_with_a_need_of_the_build_and_a_lost_binding_is_no_edge() {
  let acp = robot("acp-load-replay");
  let lost = LandingRefusal::LostBinding {
    branch: "air-home-watch".to_string(),
    identifier: "Effect".to_string(),
    binding: BindingKind::Import,
    file: FILE.to_string(),
    line: 1,
    removed_by: commit("r1", "air-home-watch"),
  };
  let refusals = vec![
    missing("air-home-watch", "Effect", "acp-load-replay", restack("air-home-watch", "acp-load-replay"), None),
    declared("air-home-watch", "acp-load-replay", None),
    declared("air-home-watch", "docs:guide", None),
    lost,
  ];
  let failed = |branch: &str| (branch != "air-home-watch").then(|| acp.clone());
  let kinds: Vec<(String, EdgeKind, EdgeState)> = edges_of("air-home-watch", "air-home-watch", &refusals, failed)
    .into_iter()
    .map(|edge| (edge.needed, edge.kind, edge.state))
    .collect();
  assert_eq!(
    kinds,
    vec![
      ("acp-load-replay".to_string(), EdgeKind::Build, EdgeState::NeededFailed { robot: acp.clone() }),
      ("acp-load-replay".to_string(), EdgeKind::Declared, EdgeState::NeededFailed { robot: acp }),
      ("docs:guide".to_string(), EdgeKind::Companion, EdgeState::Open),
    ]
  );
}

/// A declared need a branch inherits lifts to the root of its stack, where it is the root's own need: its fix is the
/// root's (none, only its landing clears it), while the edge of the start of the branch keeps its unstack
#[test]
fn an_inherited_declared_need_lifts_to_the_root_with_the_fix_of_the_root() {
  let root = [declared("auth", "docs", None)];
  let member = [declared("ui", "docs", Some("auth"))];
  let needs = [("auth", "auth", &root[..]), ("ui", "auth", &member[..])];
  let stack_root_of = |branch: &str| match branch {
    "auth" | "ui" => Some("auth"),
    "docs" => Some("docs"),
    _ => None,
  };
  let lifted = landing_edges(needs, stack_root_of, none);
  assert_eq!(lifted.len(), 1);
  assert_eq!(lifted[0].from, "auth");
  assert_eq!(lifted[0].kind, EdgeKind::Declared);
  assert_eq!(lifted[0].refuses, vec!["auth".to_string(), "ui".to_string()]);
  assert_eq!(lifted[0].fix, None);
  let own = edges_of("ui", "auth", &member, none);
  assert_eq!(
    own[0].fix,
    Some(LandingFix::Unstack {
      branch: "ui".to_string(),
      from: "auth".to_string()
    })
  );
}

/// The branch a start needs is built on what lands: the edge is a cycle of the branch itself, whose fix starts what lands
/// both (never a move), and the whole stack meets it, so it is no edge of the stack. An unassigned commit is the only
/// move: its group into the branch that needs it
#[test]
fn a_cycle_starts_what_lands_both_and_an_unassigned_commit_is_grouped_into_the_branch_that_needs_it() {
  let whole = LandingFix::Start {
    subject: Subject::whole_stack("air-home-watch"),
  };
  let group = LandingFix::MoveCommit {
    commit: commit("d1", ""),
    into: "claude-folds".to_string(),
  };
  let refusals = vec![
    missing("claude-folds", "Fold", "hosted-exit", whole.clone(), None),
    missing("claude-folds", "Fold", "", group.clone(), None),
  ];
  let edges = edges_of("claude-folds", "air-home-watch", &refusals, none);
  assert_eq!(
    edges,
    vec![
      LandingEdge {
        names: vec!["Fold".to_string()],
        files: vec![FILE.to_string()],
        ..edge("claude-folds", "hosted-exit", EdgeKind::Build, EdgeState::Cycle, &["claude-folds"], Some(whole))
      },
      LandingEdge {
        names: vec!["Fold".to_string()],
        files: vec![FILE.to_string()],
        ..edge("air-home-watch", "d1", EdgeKind::Build, EdgeState::Unassigned, &["claude-folds"], Some(group))
      },
    ]
  );
  // the stack: the cycle is met inside it, the unassigned commit stays its need
  let stack_root_of = |branch: &str| matches!(branch, "air-home-watch" | "hosted-exit" | "claude-folds").then_some("air-home-watch");
  let lifted: Vec<(String, String)> = landing_edges([("claude-folds", "air-home-watch", refusals.as_slice())], stack_root_of, none)
    .into_iter()
    .map(|edge| (edge.from, edge.needed))
    .collect();
  assert_eq!(lifted, vec![("air-home-watch".to_string(), "d1".to_string())]);
}

/// The fix of a need of a start, by where the branch of the commit it needs stands: a restack of the root onto it, the
/// Safe Push of that branch when it is built on the branch itself, the whole stack when it is built on the root only,
/// the group of an unassigned commit
#[test]
fn the_fix_of_a_need_restacks_starts_what_lands_both_or_groups_an_unassigned_commit() {
  let lands = ["air-home-watch", "claude-folds"];
  let cases: [(&str, &[&str], LandingFix); 4] = [
    ("acp-load-replay", &[], restack("air-home-watch", "acp-load-replay")),
    ("deep", &["claude-folds", "air-home-watch"], LandingFix::Start { subject: Subject::branch("deep") }),
    (
      "hosted-exit",
      &["air-home-watch"],
      LandingFix::Start {
        subject: Subject::whole_stack("air-home-watch"),
      },
    ),
    (
      "",
      &[],
      LandingFix::MoveCommit {
        commit: commit("d1", ""),
        into: "claude-folds".to_string(),
      },
    ),
  ];
  for (onto, ancestors, expected) in cases {
    let fix = LandingFix::for_need(&commit("d1", onto), "claude-folds", "air-home-watch", LandingFix::cycle_start(onto, ancestors, &lands));
    assert_eq!(fix, expected, "{onto}");
  }
}

/// The start of a whole stack is refused by what its branches need outside it, each need once whichever start of a
/// branch of it found it: a need of a branch of the stack is met by the whole stack (a cycle too), a declared need one
/// branch only inherits from another one of it is that one's own; a lost binding stays, its commit lands
#[test]
fn the_whole_stack_is_refused_by_what_its_branches_need_outside_it_each_need_once() {
  let members = ["lane-run", "lane-docs", "ui-perf-nightly"];
  let stills = restack("lane-run", "stills-codec");
  let lost = LandingRefusal::LostBinding {
    branch: "lane-docs".to_string(),
    identifier: "Stills".to_string(),
    binding: BindingKind::Import,
    file: FILE.to_string(),
    line: 3,
    removed_by: commit("r1", "lane-docs"),
  };
  let refusals = [
    // the use of lane-run, as the start of each branch of the stack found it
    missing("lane-run", "Stills", "stills-codec", stills.clone(), None),
    of_start(missing("lane-run", "Stills", "stills-codec", stills.clone(), None), "lane-docs"),
    of_start(missing("lane-run", "Stills", "stills-codec", stills.clone(), None), "ui-perf-nightly"),
    // lane-docs uses what ui-perf-nightly adds: the whole stack lands both
    missing("lane-docs", "Nightly", "ui-perf-nightly", restack("lane-docs", "ui-perf-nightly"), None),
    declared("lane-run", "release-notes", None),
    declared("lane-docs", "release-notes", Some("lane-run")),
    declared("ui-perf-nightly", "lane-docs", None),
    lost.clone(),
  ];
  assert_eq!(
    whole_stack_refusals(&members, refusals.iter()),
    vec![
      missing("lane-run", "Stills", "stills-codec", stills, None),
      declared("lane-run", "release-notes", None),
      lost
    ]
  );
}

/// `refusal` as the start of `branch` finds it
fn of_start(refusal: LandingRefusal, branch: &str) -> LandingRefusal {
  match refusal {
    LandingRefusal::MissingDeclaration {
      identifier,
      file,
      line,
      used_by,
      declared_by,
      fix,
      landing_in,
      ..
    } => LandingRefusal::MissingDeclaration {
      branch: branch.to_string(),
      identifier,
      file,
      line,
      used_by,
      declared_by,
      fix,
      landing_in,
    },
    other => other,
  }
}
