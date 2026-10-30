//! The time the view derives what its rows show, on the branches of the `branch_table_large` template of `pnpm
//! perf:table` (120 lone branches and three stacks of seven that fork, 141 rows, robots running on ten): the next step
//! and the node of every subject, as a flush of the session derives them twice (the readiness it asks, then the patch),
//! and the whole first patch of the view. Budgets of the derivation (docs/design/rendering-performance.md, B10 of
//! docs/design/rust-domain-authority.md): the minimum of several runs decides each, so the preemption of a loaded
//! machine doesn't; the times are printed.

use crate::action_view::{ActionSources, actions_of};
use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::derive::{landed_states_of, next_steps_of, open_pull_requests_view, push_view, wanted_readiness};
use crate::facts::{FactResult, FactStore, Observation, RobotsRead, Seq};
use crate::state::View;
use crate::table::TableState;
use crate::table::table_test::{branch_status, grouped, run_sync, synced};
use domain_types::fixtures::{missing_approvals, pull_request, robot};
use domain_types::next_step::NextStepView;
use domain_types::patronus::{PatronusAvailability, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult, ReviewReadiness};
use domain_types::pull_request::{PullRequestAvailability, PullRequestsResult};
use domain_types::repository_view::{BranchPatronus, LandingNeeds, PatronusView};
use domain_types::session::Demand;
use domain_types::stack::{BranchStack, StackBranchState, StackBranchUpdate};
use domain_types::{BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, SyncEvent};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The derivations of all rows of one flush take at most this long, in a debug build
const BUDGET: Duration = Duration::from_millis(20);
/// The whole first patch of the view (every part derived and compared from nothing, and the view moved to it) takes at
/// most this long: B10, 2 ms in a release build; 40 ms in a debug build, which takes about five times as long, with the
/// margin `BUDGET` leaves a loaded machine
const PATCH_BUDGET: Duration = if cfg!(debug_assertions) { Duration::from_millis(40) } else { Duration::from_millis(2) };
const RUNS: usize = 9;

pub(crate) fn at(seq: u32) -> Observation {
  Observation {
    seq: Seq(seq),
    inputs: 0,
    at: Duration::from_secs(u64::from(seq)),
    observed_at: seq,
  }
}

/// The facts of a session whose last sync answered `table`, at the first observation
pub(crate) fn synced_facts(table: &BranchTable) -> FactStore {
  let mut facts = FactStore::new();
  let _ = facts.table.answer(at(1), Ok(Arc::new(table.clone())));
  facts
}

/// Patronus answered `view` at `seq`, and the landing needs of `table` are none: the part as the session publishes it
pub(crate) fn answer_patronus(facts: &mut FactStore, seq: u32, view: PatronusView, table: &BranchTable) {
  assert!(facts.patronus.begin(u64::from(seq), Duration::from_secs(u64::from(seq))));
  facts.patronus.answer(at(seq), Ok(view));
  if let Some((inputs, _)) = facts.patronus.publish(table) {
    let _ = facts.patronus.needs_answer(inputs, Ok(BTreeMap::new()));
    let _ = facts.patronus.publish(table);
  }
}

fn stacked(name: &str, parent: &str) -> GroupedBranchInfo {
  GroupedBranchInfo {
    stack: Some(BranchStack {
      declared_parent: Some(parent.to_owned()),
      effective_base: Some(parent.to_owned()),
      ..BranchStack::default()
    }),
    ..grouped(name, &[&format!("{name}-2"), &format!("{name}-1")])
  }
}

/// The branches of the template, newest first: three stacks of seven that fork at their root, then 120 lone branches
fn branches() -> (Vec<GroupedBranchInfo>, Vec<StackBranchUpdate>) {
  let mut branches = Vec::new();
  let mut stacks = Vec::new();
  for stack in 0..3 {
    let root = format!("stack{stack}");
    let members: Vec<String> = std::iter::once(root.clone()).chain((1..=6).map(|member| format!("{root}-{member}"))).collect();
    // two lines of three on the root
    for member in (1..=6).rev() {
      let parent = if member == 1 || member == 4 { root.clone() } else { format!("{root}-{}", member - 1) };
      branches.push(stacked(&format!("{root}-{member}"), &parent));
    }
    branches.push(GroupedBranchInfo {
      stack: Some(BranchStack::default()),
      ..grouped(&root, &[&format!("{root}-2"), &format!("{root}-1")])
    });
    stacks.push(StackBranchUpdate {
      root: root.clone(),
      branch_name: format!("user/stack/{root}"),
      members,
      commit_count: 14,
      my_email: None,
      state: StackBranchState::Built,
      tip: Some(format!("{root}-stack-tip")),
      blocked_by: None,
      conflict: None,
      error: None,
      remote: None,
    });
  }
  for lone in 0..120 {
    branches.push(GroupedBranchInfo {
      stack: Some(BranchStack::default()),
      ..grouped(&format!("lone{lone:03}"), &[&format!("lone{lone:03}-1")])
    });
  }
  (branches, stacks)
}

/// The table after a sync of the template, and the facts of a session that answered everything: robots running on
/// ten branches, a pull request of every other branch, the readiness of every subject (one in three misses approvals)
pub(crate) fn session() -> (TableState, FactStore) {
  let (branches, stacks) = branches();
  let mut events = vec![SyncEvent::BranchesGrouped {
    branches: branches.clone(),
    baseline_branch: "origin/master".to_owned(),
    stacks: true,
    issue_groups: BTreeMap::new(),
    stack_parents: BTreeMap::new(),
    parked: BTreeMap::new(),
    stack_branches: stacks.iter().map(|stack| stack.root.clone()).collect(),
    target_branch: None,
    head: String::new(),
    branch: String::new(),
    store_tips: domain_types::session::StoreTips::default(),
    landing: Box::default(),
    review_experimental: false,
  }];
  for branch in &branches {
    for commit in &branch.commits {
      events.push(synced(&branch.name, &commit.id, CommitSyncStatus::Created));
    }
    events.push(branch_status(&branch.name, BranchSyncStatus::Created));
  }
  events.extend(stacks.iter().cloned().map(|stack| SyncEvent::StackBranchUpdate(Box::new(stack))));
  let mut state = TableState::with_prefix("user");
  run_sync(&mut state, events);

  let tips: HashMap<String, String> = branches
    .iter()
    .map(|branch| (format!("user/virtual/{}", branch.name), format!("copy-{}", branch.commits[0].id)))
    .collect();
  let up_to_date = |tip: &str| RemoteFact {
    remote_tip: Some(tip.to_owned()),
    up_to_date: true,
    known: true,
    drops: 0,
    others: 0,
  };
  let remote = RemoteFacts {
    branches: branches
      .iter()
      .map(|branch| (branch.name.clone(), up_to_date(&format!("copy-{}", branch.commits[0].id))))
      .collect(),
    stacks: stacks
      .iter()
      .map(|stack| (stack.root.clone(), up_to_date(stack.tip.as_deref().unwrap_or_default())))
      .collect(),
  };
  let table = BranchTable {
    prefix: "user".to_owned(),
    baseline: "origin/master".to_owned(),
    branches: branches.clone(),
    tips,
    stack_branches: stacks,
    remote,
    ..BranchTable::default()
  };
  let mut facts = synced_facts(&table);
  let running: Vec<_> = branches
    .iter()
    .step_by(14)
    .take(10)
    .enumerate()
    .map(|(index, branch)| PatronusRobot {
      source_revision: Some(format!("copy-{}", branch.commits[0].id)),
      ..robot(&format!("r{index}"), Some(&branch.name), PatronusRobotStatus::Running, 10)
    })
    .collect();
  facts.apply(
    at(2),
    FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: PatronusRobotsResult {
        availability: PatronusAvailability::Available,
        robots: running.clone(),
        github_repository: Some("acme/monorepo".to_owned()),
      },
    },
  );
  let view = PatronusView {
    branches: running
      .iter()
      .map(|robot| {
        let facts = BranchPatronus {
          robot_id: Some(robot.id.clone()),
          landing_needs: LandingNeeds::Known { refusals: Vec::new() },
          ..BranchPatronus::default()
        };
        (robot.branch_name.clone().unwrap_or_default(), facts)
      })
      .collect(),
    ..PatronusView::default()
  };
  answer_patronus(&mut facts, 3, view, &table);
  facts.apply(
    at(4),
    FactResult::PullRequests(PullRequestsResult {
      viewer_permission: None,
      merge_methods: None,
      availability: PullRequestAvailability::Available {
        owner: "acme".to_owned(),
        name: "monorepo".to_owned(),
      },
      pull_requests: branches
        .iter()
        .step_by(2)
        .enumerate()
        .map(|(number, branch)| (branch.name.clone(), pull_request(&branch.name, 100 + u32::try_from(number).unwrap_or(0))))
        .collect(),
      stack_pull_requests: BTreeMap::new(),
      github_stacks: false,
    }),
  );
  let demand = Demand {
    visible_branches: branches.iter().map(|branch| branch.name.clone()).collect(),
    visible_stacks: (0..3).map(|stack| format!("stack{stack}")).collect(),
    ..Demand::default()
  };
  let wanted = wanted_readiness(&state, &facts, "user", &demand);
  let robots = facts.robots.view().head.value.clone();
  facts.readiness.update(&state, facts.pull_requests.view(), robots.as_ref(), &demand, "user", &wanted);
  let answers = facts
    .readiness
    .digest()
    .subjects
    .iter()
    .enumerate()
    .map(|(index, subject)| crate::readiness_facts::ReadinessAnswer {
      subject: subject.clone(),
      result: Ok(domain_types::patronus::BranchReviewReadiness {
        branch_name: subject.ask.branch_name.clone(),
        tip: subject.ask.tip.clone(),
        requirement: None,
        readiness: if index % 3 == 0 {
          missing_approvals("IJPL Code Platform", Some(1))
        } else {
          ReviewReadiness::NotRequired
        },
      }),
    })
    .collect();
  facts.apply(at(5), FactResult::Readiness(answers));
  (state, facts)
}

/// The least time `derive` took of `RUNS` runs
fn least(mut derive: impl FnMut()) -> Duration {
  (0..RUNS)
    .map(|_| {
      let start = Instant::now();
      derive();
      start.elapsed()
    })
    .min()
    .unwrap_or_default()
}

/// A flush derives the next step, the node and the actions of every row, the readiness it asks and the patch, within its
/// budget, and the whole first patch of the view within its own
#[test]
fn a_flush_derives_the_next_steps_of_the_141_rows_of_the_large_table_within_its_budget() {
  let (state, facts) = session();
  let open = open_pull_requests_view(
    "master",
    |name| state.branch(name).is_some(),
    |root| state.stack(root).is_some(),
    facts.pull_requests.view(),
    |_| None,
    "user",
    |_| None,
  );
  let landed = landed_states_of(&state, &facts);
  let steps = next_steps_of(&state, &facts, &open, &landed);
  assert_eq!(steps.len(), 141 + 3, "every row and every whole stack has a step");
  assert!(steps.values().any(|view| view.step.needs_readiness), "the readiness was asked and answered");
  let demand = Demand {
    visible_branches: state.order().to_vec(),
    ..Demand::default()
  };
  let pushes = push_view(&state.parts, &facts);
  let actions = |steps: &BTreeMap<String, NextStepView>| {
    let sources = ActionSources {
      prefix: "user",
      steps,
      pushes: &pushes,
      lands: &BTreeMap::new(),
      open_pull_requests: &open,
      landed: &landed,
    };
    actions_of(&state, &facts, &sources)
  };
  assert!(actions(&steps).len() >= 141 + 3, "every row and every whole stack has its actions");
  let flush = least(|| {
    let _ = wanted_readiness(&state, &facts, "user", &demand);
    let steps = next_steps_of(&state, &facts, &open, &landed);
    let _ = actions(&steps);
  });
  let patch = least(|| {
    let mut view = View::new(1);
    let _ = view.diff(&state, &facts);
  });
  eprintln!("the next steps and the actions of 141 rows per flush: {flush:?} (budget {BUDGET:?}); the first patch of the view: {patch:?} (budget {PATCH_BUDGET:?})");
  assert!(flush <= BUDGET, "the next steps of a flush took {flush:?}, over the budget of {BUDGET:?}");
  assert!(patch <= PATCH_BUDGET, "the first patch of the view took {patch:?}, over the budget of {PATCH_BUDGET:?}");
}
