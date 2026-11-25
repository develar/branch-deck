//! The landing plan (`landing_units`, `landing_plan`, `checks_price`): the units of the table, the waves of their edges,
//! and Land Together with its price

use crate::facts::Repository;
use crate::landing_plan::{PlanFacts, UnitFacts, checks_price, landing_plan, landing_units};
use crate::stack_test::{repository, stacks_of};
use domain_types::landing::{BranchFootprint, EdgeAction, LandingFix, LandingFootprints, LandingRefusal, NeedState, StackCommit};
use domain_types::landing_plan::{ChecksPrice, LandTogether, LandingPlan, LandingUnit, LandingWave, TogetherPrice, WaveKind};
use domain_types::next_step::LandingNodeKind;
use domain_types::repository_view::RobotOf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// The seven required checks any path of `plugins/air/**` starts in idea-1 (`UltimatePatronusConfigTest`)
const AIR: [&str; 7] = ["Air UI", "Air Tests", "Bazel", "Product Tests", "Smoke Tests", "Incremental", "Inspections"];

fn names(names: &[&str]) -> Vec<String> {
  names.iter().map(|name| (*name).to_string()).collect()
}

fn robot(branch: &str) -> RobotOf {
  RobotOf {
    branch: branch.to_string(),
    robot_id: format!("robot-{branch}"),
    whole_stack: false,
  }
}

/// A use in `branch` of a name a commit of `declared_in` adds, its fix the restack of `root` onto it
fn missing(branch: &str, root: &str, declared_in: &str, landing_in: Option<RobotOf>) -> LandingRefusal {
  LandingRefusal::MissingDeclaration {
    branch: branch.to_string(),
    identifier: "Effect".to_string(),
    file: "plugins/air/native/air-ide/src/acp_load_replay.rs".to_string(),
    line: 7,
    used_by: StackCommit {
      id: "u1".to_string(),
      subject: "Use".to_string(),
      branch: branch.to_string(),
    },
    declared_by: StackCommit {
      id: "d1".to_string(),
      subject: "Add".to_string(),
      branch: declared_in.to_string(),
    },
    fix: LandingFix::Restack {
      branch: root.to_string(),
      onto: declared_in.to_string(),
    },
    landing_in,
  }
}

/// A unit as the test reads it: its name, branches, node, kind and fixes
type UnitRow<'a> = (&'a str, &'a [String], &'a str, LandingNodeKind, &'a [String]);

/// What the units and the plan are decided on, of one table
struct Table {
  kinds: BTreeMap<&'static str, LandingNodeKind>,
  footprints: LandingFootprints,
  needs: Vec<(&'static str, Vec<LandingRefusal>)>,
  failed: Vec<&'static str>,
  stale: bool,
  github: bool,
  pull_requests: BTreeMap<String, u32>,
}

impl Table {
  fn new(kinds: &[(&'static str, LandingNodeKind)]) -> Self {
    Self {
      kinds: kinds.iter().copied().collect(),
      footprints: LandingFootprints::default(),
      needs: Vec::new(),
      failed: Vec::new(),
      stale: false,
      github: false,
      pull_requests: BTreeMap::new(),
    }
  }

  fn units(&self, repo: &Repository<'_>) -> Vec<LandingUnit> {
    // a stack forks below its root when a branch has two children
    let whole_stack_key = |root: &str| {
      let mut forks = false;
      let mut pending = vec![root];
      let mut leaf = root;
      while let Some(next) = pending.pop() {
        let children = crate::stack::children(repo, next);
        forks |= children.len() > 1;
        for &child in children {
          pending.push(child);
          leaf = child;
        }
      }
      Some(if forks { format!("stack:{root}") } else { leaf.to_string() })
    };
    let node_kind = |key: &str| self.kinds.get(key).copied();
    landing_units(&UnitFacts {
      repo,
      whole_stack_key: &whole_stack_key,
      node_kind: &node_kind,
      commit_count: &|_| 2,
      footprints: &self.footprints,
    })
  }

  fn plan(&self, repo: &Repository<'_>) -> LandingPlan {
    let failed = |branch: &str| self.failed.contains(&branch).then(|| robot(branch));
    let open = |key: &str| self.pull_requests.get(key).copied();
    landing_plan(PlanFacts {
      repo,
      units: self.units(repo),
      needs: self.needs.iter().map(|(branch, refusals)| (*branch, refusals.as_slice())).collect(),
      stale: self.stale,
      failed_robot_of: &failed,
      footprints: &self.footprints,
      github: self.github,
      open_pull_request: &open,
    })
  }
}

fn footprint(checks: &[&str], owners: &[&str]) -> BranchFootprint {
  BranchFootprint {
    checks: names(checks),
    owners: names(owners),
  }
}

/// idea-1 on 2026-11-24: genui-host-crate (with its chain air-session-copy → air-ui-skill-policy) and acp-load-replay
/// land in their robots; air-tools-rust (→ air-ui-terminal-delivery) needs genui-host-crate, the stack air-home-watch
/// (hosted-exit, claude-folds) acp-load-replay; agent-id, IJAI-7037 and ci-pack-cache need nothing; three are parked
#[test]
fn the_plan_of_idea_1_lands_two_robots_now_three_branches_next_and_two_units_after_them() {
  let specs = [
    ("parked-1", None, None),
    ("ci-pack-cache", None, None),
    ("IJAI-7037", None, None),
    ("agent-id", None, None),
    ("claude-folds", Some("air-home-watch"), Some("air-home-watch")),
    ("hosted-exit", Some("air-home-watch"), Some("air-home-watch")),
    ("air-home-watch", None, None),
    ("air-ui-terminal-delivery", Some("air-tools-rust"), Some("air-tools-rust")),
    ("air-tools-rust", None, None),
    ("acp-load-replay", None, None),
    ("air-ui-skill-policy", Some("air-session-copy"), Some("air-session-copy")),
    ("air-session-copy", Some("genui-host-crate"), Some("genui-host-crate")),
    ("genui-host-crate", None, None),
    ("parked-2", None, None),
    ("parked-3", None, None),
  ];
  let stacks = stacks_of(&specs);
  let parked: BTreeMap<String, String> = ["parked-1", "parked-2", "parked-3"]
    .into_iter()
    .map(|name| (name.to_string(), "later".to_string()))
    .collect();
  let repo = repository(&stacks, &parked);
  let mut table = Table::new(&[
    ("ci-pack-cache", LandingNodeKind::Ready),
    ("IJAI-7037", LandingNodeKind::Ready),
    ("agent-id", LandingNodeKind::Ready),
    ("stack:air-home-watch", LandingNodeKind::Waiting),
    ("air-home-watch", LandingNodeKind::Waiting),
    ("hosted-exit", LandingNodeKind::NeedsFix),
    ("claude-folds", LandingNodeKind::Waiting),
    ("air-tools-rust", LandingNodeKind::Waiting),
    ("air-ui-terminal-delivery", LandingNodeKind::Waiting),
    ("acp-load-replay", LandingNodeKind::Landing),
    ("genui-host-crate", LandingNodeKind::Landing),
    ("air-session-copy", LandingNodeKind::Landing),
    ("air-ui-skill-policy", LandingNodeKind::Landing),
  ]);
  let acp = robot("acp-load-replay");
  let genui = robot("genui-host-crate");
  table.needs = vec![
    ("air-home-watch", vec![missing("air-home-watch", "air-home-watch", "acp-load-replay", Some(acp.clone()))]),
    ("hosted-exit", vec![missing("hosted-exit", "air-home-watch", "acp-load-replay", Some(acp.clone()))]),
    ("claude-folds", vec![missing("claude-folds", "air-home-watch", "acp-load-replay", Some(acp))]),
    (
      "air-ui-terminal-delivery",
      vec![missing("air-ui-terminal-delivery", "air-tools-rust", "genui-host-crate", Some(genui))],
    ),
  ];
  table.footprints.branches = specs.iter().map(|(name, _, _)| ((*name).to_string(), footprint(&AIR, &["Air"]))).collect();
  let plan = table.plan(&repo);
  let units: Vec<UnitRow<'_>> = plan
    .units
    .iter()
    .map(|unit| (unit.name.as_str(), unit.branches.as_slice(), unit.node.as_str(), unit.kind, unit.fixes.as_slice()))
    .collect();
  let none: &[String] = &[];
  assert_eq!(
    units,
    [
      ("ci-pack-cache", &names(&["ci-pack-cache"])[..], "ci-pack-cache", LandingNodeKind::Ready, none),
      ("IJAI-7037", &names(&["IJAI-7037"])[..], "IJAI-7037", LandingNodeKind::Ready, none),
      ("agent-id", &names(&["agent-id"])[..], "agent-id", LandingNodeKind::Ready, none),
      (
        "air-home-watch",
        &names(&["air-home-watch", "claude-folds", "hosted-exit"])[..],
        "stack:air-home-watch",
        LandingNodeKind::Waiting,
        &names(&["hosted-exit"])[..]
      ),
      (
        "air-tools-rust",
        &names(&["air-tools-rust", "air-ui-terminal-delivery"])[..],
        "air-ui-terminal-delivery",
        LandingNodeKind::Waiting,
        none
      ),
      ("acp-load-replay", &names(&["acp-load-replay"])[..], "acp-load-replay", LandingNodeKind::Landing, none),
      (
        "genui-host-crate",
        &names(&["genui-host-crate", "air-session-copy", "air-ui-skill-policy"])[..],
        "air-ui-skill-policy",
        LandingNodeKind::Landing,
        none
      ),
    ]
  );
  assert_eq!(plan.units[0].checks, Some(names(&AIR)), "the checks of every branch of the unit");
  assert_eq!(plan.units[0].commit_count, 2);
  assert_eq!(plan.units[3].commit_count, 6);
  assert_eq!(
    plan.waves,
    [
      LandingWave {
        kind: WaveKind::Landing,
        units: names(&["acp-load-replay", "genui-host-crate"]),
      },
      LandingWave {
        kind: WaveKind::Next,
        units: names(&["ci-pack-cache", "IJAI-7037", "agent-id"]),
      },
      LandingWave {
        kind: WaveKind::After {
          after: names(&["acp-load-replay", "genui-host-crate"]),
        },
        units: names(&["air-home-watch", "air-tools-rust"]),
      },
    ]
  );
  assert_eq!(plan.parked, names(&["parked-1", "parked-2", "parked-3"]));
  // fifteen refusals of the stack are one edge of its root, which waits: no action, no offer
  let edges: Vec<(&str, &str, usize)> = plan.edges.iter().map(|edge| (edge.from.as_str(), edge.needed.as_str(), edge.actions.len())).collect();
  assert_eq!(edges, [("air-home-watch", "acp-load-replay", 0), ("air-tools-rust", "genui-host-crate", 0)]);
  assert_eq!(plan.proposals, []);
}

/// A unit after a unit after a running robot lands a wave later; a unit needs nothing of a unit in master
#[test]
fn a_unit_lands_one_wave_after_the_deepest_unit_it_needs() {
  let (stacks, parked) = (
    stacks_of(&[("ui", None, None), ("flow", None, None), ("codec", None, None), ("old", None, None)]),
    BTreeMap::new(),
  );
  let repo = repository(&stacks, &parked);
  let mut table = Table::new(&[
    ("ui", LandingNodeKind::YourStep),
    ("flow", LandingNodeKind::Waiting),
    ("codec", LandingNodeKind::Landing),
    ("old", LandingNodeKind::InMaster),
  ]);
  table.needs = vec![
    ("ui", vec![missing("ui", "ui", "flow", None), missing("ui", "ui", "old", None)]),
    ("flow", vec![missing("flow", "flow", "codec", Some(robot("codec")))]),
  ];
  let waves = table.plan(&repo).waves;
  assert_eq!(
    waves,
    [
      LandingWave {
        kind: WaveKind::Landing,
        units: names(&["codec"]),
      },
      LandingWave {
        kind: WaveKind::After { after: names(&["codec"]) },
        units: names(&["flow"]),
      },
      LandingWave {
        kind: WaveKind::After { after: names(&["flow"]) },
        units: names(&["ui"]),
      },
    ]
  );
}

/// ci-pack-cache uses a name of agent-id, whose robot neither lands nor failed: Land Together restacks it on agent-id,
/// its price one robot instead of two, the checks of the build agent-id lacks, a failure that holds both, the pull
/// request of ci-pack-cache whose approvals count, the owners of agent-id; the edge offers Land agent-id First beside it
#[test]
fn land_together_restacks_onto_the_branch_a_need_of_the_build_nothing_lands_with_its_price() {
  let (stacks, parked) = (stacks_of(&[("ci-pack-cache", None, None), ("agent-id", None, None)]), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let mut table = Table::new(&[("ci-pack-cache", LandingNodeKind::YourStep), ("agent-id", LandingNodeKind::Ready)]);
  let refusal = missing("ci-pack-cache", "ci-pack-cache", "agent-id", None);
  table.needs = vec![("ci-pack-cache", vec![refusal.clone()])];
  table.footprints.branches = BTreeMap::from([
    ("ci-pack-cache".to_string(), footprint(&["Build Scripts", "Inspections"], &["Build"])),
    ("agent-id".to_string(), footprint(&["Air Tests", "Inspections"], &["Air", "Build"])),
  ]);
  table.pull_requests = BTreeMap::from([("ci-pack-cache".to_string(), 12), ("agent-id".to_string(), 13)]);
  let plan = table.plan(&repo);
  assert_eq!(
    plan.proposals,
    [LandTogether {
      from: "ci-pack-cache".to_string(),
      onto: "agent-id".to_string(),
      onto_unit: "agent-id".to_string(),
      fixes: refusal,
      forks: false,
      price: TogetherPrice {
        robots: 2,
        together: 1,
        checks: ChecksPrice::Added {
          count: 3,
          added: names(&["Build Scripts", "Air Tests"]),
        },
        failure_holds_both: true,
        approvals_in: Some(12),
        owners_added: Some(names(&["Air"])),
        github_stack: None,
      },
      command: "brd restack ci-pack-cache agent-id".to_string(),
    }]
  );
  assert_eq!(
    plan.edges[0].actions,
    [
      EdgeAction::LandTogether {
        branch: "ci-pack-cache".to_string(),
        onto: "agent-id".to_string(),
        forks: false,
      },
      EdgeAction::LandFirst { branch: "agent-id".to_string() },
    ]
  );
  // off Patronus the two pull requests become one stack of GitHub
  table.github = true;
  assert_eq!(table.plan(&repo).proposals[0].price.github_stack, Some(vec![13, 12]));
  // while the needs are computed, nothing is offered
  table.stale = true;
  let stale = table.plan(&repo);
  assert!(stale.stale);
  assert_eq!(stale.proposals, []);
}

/// Onto a branch with a branch stacked on it the stack forks: its whole stack lands both, and the approvals count in
/// the pull request of that stack
#[test]
fn land_together_onto_a_branch_with_a_child_forks_and_the_approvals_count_in_the_stack_pull_request() {
  let specs = [("ui", None, None), ("login", Some("auth"), Some("auth")), ("auth", None, None)];
  let (stacks, parked) = (stacks_of(&specs), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let mut table = Table::new(&[("ui", LandingNodeKind::YourStep), ("auth", LandingNodeKind::Ready), ("login", LandingNodeKind::Ready)]);
  table.needs = vec![("ui", vec![missing("ui", "ui", "auth", None)])];
  table.pull_requests = BTreeMap::from([("ui".to_string(), 7), ("stack:auth".to_string(), 21)]);
  let plan = table.plan(&repo);
  let [proposal] = plan.proposals.as_slice() else { panic!("{plan:?}") };
  assert!(proposal.forks);
  assert_eq!(proposal.price.approvals_in, Some(21));
  assert_eq!(proposal.price.checks, ChecksPrice::Same { count: 0 });
  assert!(matches!(plan.edges[0].actions[0], EdgeAction::LandTogether { forks: true, .. }));
}

/// No offer to land with a unit that lands, failed or is in master, nor of a declared need: the edge waits, restarts
/// the robot of what it needs, or lands it first
#[test]
fn land_together_is_never_offered_with_a_landing_or_failed_unit_nor_for_a_declared_need() {
  let specs = [("ui", None, None), ("flow", None, None), ("codec", None, None), ("docs", None, None)];
  let (stacks, parked) = (stacks_of(&specs), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let mut table = Table::new(&[
    ("ui", LandingNodeKind::YourStep),
    ("flow", LandingNodeKind::YourStep),
    ("codec", LandingNodeKind::NeedsFix),
    ("docs", LandingNodeKind::Ready),
  ]);
  let declared = LandingRefusal::UnlandedNeed {
    branch: "flow".to_string(),
    needs: "docs".to_string(),
    state: NeedState::Active,
    inherited_from: None,
    fix: None,
    landing_in: None,
  };
  table.needs = vec![("ui", vec![missing("ui", "ui", "codec", None)]), ("flow", vec![declared])];
  table.failed = vec!["codec"];
  let plan = table.plan(&repo);
  assert_eq!(plan.proposals, []);
  let actions: Vec<(&str, &[EdgeAction])> = plan.edges.iter().map(|edge| (edge.from.as_str(), edge.actions.as_slice())).collect();
  assert_eq!(
    actions,
    [
      ("ui", &[EdgeAction::Restart { robot: robot("codec") }][..]),
      ("flow", &[EdgeAction::LandFirst { branch: "docs".to_string() }][..]),
    ]
  );
}

/// A stack that holds a parked branch lands in pieces: the root's own node is the unit's, the parked branch in no unit
#[test]
fn a_stack_that_holds_a_parked_branch_takes_the_node_of_its_root() {
  let specs = [("draft", Some("auth"), Some("auth")), ("login", Some("auth"), Some("auth")), ("auth", None, None)];
  let stacks = stacks_of(&specs);
  let parked = BTreeMap::from([("draft".to_string(), "later".to_string())]);
  let repo = repository(&stacks, &parked);
  let table = Table::new(&[("auth", LandingNodeKind::Ready), ("login", LandingNodeKind::Ready), ("stack:auth", LandingNodeKind::Parked)]);
  let units = table.units(&repo);
  let [unit] = units.as_slice() else { panic!("{units:?}") };
  assert_eq!((unit.node.as_str(), unit.branches.clone()), ("auth", names(&["auth", "login"])));
  assert_eq!(unit.checks, Some(Vec::new()));
}

/// The checks of two units in one robot: the same when each starts them all, else those only one starts; unknown with
/// the reason
#[test]
fn the_checks_of_two_units_are_the_same_the_added_or_unknown() {
  let mut footprints = LandingFootprints {
    branches: BTreeMap::from([
      ("air".to_string(), footprint(&AIR, &[])),
      ("home".to_string(), footprint(&AIR, &[])),
      ("llm".to_string(), footprint(&["AI Assistant Tests", "Inspections"], &[])),
    ]),
    ..LandingFootprints::default()
  };
  let of = |names: &[&str]| names.iter().map(|name| (*name).to_string()).collect::<Vec<_>>();
  assert_eq!(checks_price(&footprints, &of(&["air"]), &of(&["home"])), ChecksPrice::Same { count: 7 });
  assert_eq!(
    checks_price(&footprints, &of(&["air"]), &of(&["llm"])),
    ChecksPrice::Added {
      count: 8,
      added: names(&["Air UI", "Air Tests", "Bazel", "Product Tests", "Smoke Tests", "Incremental", "AI Assistant Tests"]),
    }
  );
  footprints.checks_unknown = Some("the baseline has no .patronus/config.yaml".to_string());
  assert_eq!(
    checks_price(&footprints, &of(&["air"]), &of(&["home"])),
    ChecksPrice::Unknown {
      reason: "the baseline has no .patronus/config.yaml".to_string()
    }
  );
}
