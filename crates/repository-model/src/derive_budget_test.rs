//! The time the view derives what its rows show, on the branches of the `branch_table_large` template of `pnpm
//! perf:table` (120 lone branches and three stacks of seven that fork, 141 rows, robots running on ten): the next step
//! and the node of every subject, as a flush of the session derives them twice (the readiness it asks, then the patch),
//! and the whole first patch of the view. Budgets of the derivation (docs/design/rendering-performance.md, B10 of
//! docs/design/rust-domain-authority.md): the CPU time of the test thread decides each, the least of several runs, in
//! units of a reference work on that thread (`reference_work`). The derive runs on that thread alone (no pool, no task),
//! a loaded machine adds preemptions to the wall time of a run, not CPU time to its thread, and a slower machine does
//! the reference slower too, so neither fails a budget and a slower derive still does; the times are printed.

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
use std::time::Duration;
use test_utils::cpu_time::thread_cpu_time;

/// The derivations of all rows of one flush take at most this many times the reference work (`reference_work`) on the
/// same thread. On 2026-11-10 in a debug build on the developer Mac: 1.28–1.34 (about 2.5 ms), 1.23–1.70 for 180 copies,
/// 60 at once at background priority; a derive of twice the CPU time 3.26–3.44
const BUDGET: f64 = 2.2;
/// The whole first patch of the view (every part derived and compared from nothing, and the view moved to it) takes at
/// most this many times the reference work: B10, 2 ms in a release build on the Mac Studio. On 2026-11-10 in a debug
/// build on the developer Mac: 2.42–2.51 (about 4.5 ms), 2.20–3.17 for the 180 copies; a derive of twice the CPU time
/// 5.97–6.10
const PATCH_BUDGET: f64 = 4.0;
/// The rounds of the samples of the derives; the median decides
const RUNS: usize = 9;
/// Each sample runs its derive until it used this much CPU time of the thread, and counts the runs: the clock of
/// Windows counts whole ticks of about 15.6 ms, the one of macOS and Linux nanoseconds
const SAMPLE: Duration = if cfg!(windows) { Duration::from_millis(100) } else { Duration::ZERO };

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
    issue_groups: BTreeMap::new(),
    stack_parents: BTreeMap::new(),
    parked: BTreeMap::new(),
    needs: BTreeMap::new(),
    companions: Vec::new(),
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

/// The CPU time of this thread of one run of `derive`, over runs of at least `SAMPLE`
fn sample(derive: &mut dyn FnMut()) -> Duration {
  let start = thread_cpu_time();
  let mut runs = 0_u32;
  loop {
    derive();
    runs += 1;
    if thread_cpu_time().saturating_sub(start) >= SAMPLE {
      break thread_cpu_time().saturating_sub(start) / runs;
    }
  }
}

/// What the derives took in units of the reference work
struct Units {
  flush: f64,
  patch: f64,
  /// The CPU time of the median round, for the line of the test
  times: [Duration; 3],
}

/// The median over `RUNS` rounds of the CPU time of `flush` and of `patch`, each in units of the reference work of its
/// round: a round is short, so the reference and the derives of one round mostly run on one core. A machine of two
/// kinds of cores takes both about twice as long on the slower kind, and a round apart from the others changes no median
fn units(flush: &mut dyn FnMut(), patch: &mut dyn FnMut()) -> Units {
  let mut rounds: Vec<[Duration; 3]> = (0..RUNS).map(|_| [sample(&mut reference_work), sample(flush), sample(patch)]).collect();
  let ratio = |round: &[Duration; 3], at: usize| round[at].as_secs_f64() / round[0].as_secs_f64();
  let median = |rounds: &mut Vec<[Duration; 3]>, at: usize| {
    rounds.sort_by(|a, b| ratio(a, at).total_cmp(&ratio(b, at)));
    ratio(&rounds[RUNS / 2], at)
  };
  let flush_units = median(&mut rounds, 1);
  let patch_units = median(&mut rounds, 2);
  Units {
    flush: flush_units,
    patch: patch_units,
    times: rounds[RUNS / 2],
  }
}

/// The keys of `reference_work`
const REFERENCE_KEYS: usize = 2_000;

/// Work of a fixed size and of the kind a derive does, the unit of the budgets: branch names and commit ids as the keys
/// and values of a map, its clone and the comparison of the two. A machine or a build that derives slower does this
/// slower too, so a budget in this unit holds on every machine of the tests, and a slower derive fails it
fn reference_work() {
  let map: BTreeMap<String, Vec<String>> = (0..REFERENCE_KEYS)
    .map(|key| (format!("user/virtual/branch-{key:05}"), (0..4).map(|commit| format!("{key:08x}{commit:032x}")).collect()))
    .collect();
  let copy = std::hint::black_box(&map).clone();
  assert_eq!(std::hint::black_box(copy), map);
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
      github_layers: &BTreeMap::new(),
    };
    actions_of(&state, &facts, &sources)
  };
  assert!(actions(&steps).len() >= 141 + 3, "every row and every whole stack has its actions");
  let measured = units(
    &mut || {
      let _ = wanted_readiness(&state, &facts, "user", &demand);
      let steps = next_steps_of(&state, &facts, &open, &landed);
      let _ = actions(&steps);
    },
    &mut || {
      let mut view = View::new(1);
      let _ = view.diff(&state, &facts);
    },
  );
  let (Units { flush, patch, .. }, [unit, flush_time, patch_time]) = (&measured, measured.times);
  eprintln!(
    "CPU time of the thread, the median of {RUNS} rounds: the next steps and the actions of 141 rows per flush {flush:.2} units (budget {BUDGET}); the first patch of the view {patch:.2} units (budget {PATCH_BUDGET}); a round of the patch: the reference work {unit:?}, the flush {flush_time:?}, the patch {patch_time:?}"
  );
  assert!(*flush <= BUDGET, "the next steps of a flush took {flush:.2} units, over the budget of {BUDGET}");
  assert!(
    *patch <= PATCH_BUDGET,
    "the first patch of the view took {patch:.2} units, over the budget of {PATCH_BUDGET}"
  );
}
