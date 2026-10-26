//! The session of one command (docs/design/rust-domain-authority.md, "Разделение между `bd` и приложением без
//! daemon"): the table of the command's own sync (`TableState` of `bd`'s sync, or an empty one beside the table of a
//! grouping), and the facts it reads once for one intent (`operations::facts::fill`) into the same `FactStore` a session
//! of the app holds, numbered as a session numbers them; no actor, no scheduler, no window. The Patronus view is derived
//! as a session's host derives it (`facts::derive_patronus`, on the caller's thread), its landing needs answered by the
//! caller (`bd` answers the landing gate of its subject), so `bd` reads the robots, what they land, landed and hold, the
//! pull requests, the profiles, the rules and the readiness through the rules of the view (`derive::LandingFacts`), as
//! the app's session derives them.

use crate::branch_table::BranchTable;
use crate::derive::LandingFacts;
use crate::facts::{FactResult, FactStore, Needs, Observation, Seq, derive_patronus, listed_robots};
use crate::patronus_view::lock_memo;
use crate::readiness_facts::ReadinessDigest;
use crate::session::SessionKey;
use crate::table::TableState;
use domain_types::error::ApiError;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{BranchPatronus, PatronusView, StackPatronus};
use domain_types::session::Demand;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::timestamp::unix_now;

/// The table of a command and the facts it read; `Default`: a command that asked nothing
#[derive(Debug, Default)]
pub struct OneShot {
  key: SessionKey,
  state: TableState,
  facts: FactStore,
  /// The robots the facts hold, newest first, as a list answers them
  robots: Vec<PatronusRobot>,
  /// The landing needs the last view wants, with its inputs and the commits it names landed
  needs_wanted: Option<(u64, HashSet<String>)>,
  seq: u32,
}

impl OneShot {
  /// The session of `key` over `state` and `table`, the table of the same sync (`BranchTable::from_report`), committed as
  /// a session commits the table of a run
  #[must_use]
  pub fn new(key: SessionKey, state: TableState, table: BranchTable) -> Self {
    let mut session = Self { key, state, ..Self::default() };
    let baseline = table.baseline.clone();
    let observation = session.observation();
    let _ = session.facts.table.answer(observation, Ok(Arc::new(table)));
    session.facts.table_committed(&baseline);
    session
  }

  #[must_use]
  pub const fn key(&self) -> &SessionKey {
    &self.key
  }

  #[must_use]
  pub const fn state(&self) -> &TableState {
    &self.state
  }

  #[must_use]
  pub const fn facts(&self) -> &FactStore {
    &self.facts
  }

  fn observation(&mut self) -> Observation {
    self.seq += 1;
    Observation {
      seq: Seq(self.seq),
      inputs: 0,
      at: Duration::ZERO,
      observed_at: unix_now(),
    }
  }

  /// Applies the answer of a read, newer than every answer before it; the details of a robot are demanded first
  pub fn apply(&mut self, result: FactResult) {
    if let FactResult::RobotDetails { id, .. } = &result {
      self.facts.demand_details(std::slice::from_ref(id));
    }
    let observation = self.observation();
    self.facts.apply(observation, result);
    self.robots = listed_robots(self.facts.robots.view());
  }

  /// The Patronus view of the table over the robots the facts hold now, derived with git (the content the hashes and
  /// the memo don't answer) when what it is of changed; nothing before an answer of the robots. Blocking
  pub fn derive_patronus(&mut self, git: &GitCommandExecutor) {
    let Some(derivation) = self.facts.begin_patronus(&self.state, Duration::ZERO) else {
      return;
    };
    let view = derive_patronus(git, &self.key.worktree, &derivation.of(), &mut lock_memo(&self.facts.memo));
    let observation = Observation {
      inputs: derivation.inputs,
      ..self.observation()
    };
    self.facts.patronus.answer(observation, Ok(view));
    self.needs_wanted = self.facts.patronus.publish(&derivation.table);
  }

  /// The landing needs of the view, which `compute` answers of the commits it names landed (the landing gate of the
  /// caller's subject, the trial session of every branch); nothing while no view wants them
  pub fn landing_needs(&mut self, compute: impl FnOnce(&HashSet<String>) -> Result<Needs, ApiError>) {
    let (Some((inputs, landed)), Some(table)) = (self.needs_wanted.take(), self.facts.table.value().map(Arc::clone)) else {
      return;
    };
    let _ = self.facts.patronus.needs_answer(inputs, compute(&landed));
    self.needs_wanted = self.facts.patronus.publish(&table);
  }

  /// The asks of the readiness of the subjects `keys` (a branch, `stack:<root>`) over the table, the pull requests and
  /// the robots the facts hold, as a session asks what a surface needs now; no other subject is asked
  pub fn readiness_asks(&mut self, keys: &[String]) -> ReadinessDigest {
    let demand = Demand {
      needed: keys.to_vec(),
      ..Demand::default()
    };
    let robots = self.facts.robots.view().head.value.as_ref();
    let pull_requests = self.facts.pull_requests.view();
    self.facts.readiness.update(&self.state, pull_requests, robots, &demand, &self.key.prefix, &HashSet::new());
    self.facts.readiness.digest().clone()
  }

  /// The user's robots, newest first
  #[must_use]
  pub fn robots(&self) -> &[PatronusRobot] {
    &self.robots
  }

  /// The Patronus view with the landing needs it has; empty before an answer of the robots
  #[must_use]
  pub const fn view(&self) -> &PatronusView {
    &self.facts.patronus.part().view
  }

  #[must_use]
  pub fn robot(&self, id: &str) -> Option<&PatronusRobot> {
    self.robots.iter().find(|robot| robot.id == id)
  }

  #[must_use]
  pub fn branch(&self, name: &str) -> Option<&BranchPatronus> {
    self.view().branches.get(name)
  }

  #[must_use]
  pub fn stack(&self, root: &str) -> Option<&StackPatronus> {
    self.view().stacks.get(root)
  }

  /// The robot of a branch, as the rows match it: started on it, or of a temporary ref at its tip
  #[must_use]
  pub fn branch_robot(&self, name: &str) -> Option<&PatronusRobot> {
    self.branch(name)?.robot_id.as_deref().and_then(|id| self.robot(id))
  }

  /// The robot of the whole stack of `root`
  #[must_use]
  pub fn stack_robot(&self, root: &str) -> Option<&PatronusRobot> {
    self.stack(root)?.robot_id.as_deref().and_then(|id| self.robot(id))
  }

  /// The robot of a subject: of the branch, or of the whole stack
  #[must_use]
  pub fn subject_robot(&self, subject: &Subject) -> Option<&PatronusRobot> {
    match subject {
      Subject::Branch { name } => self.branch_robot(name),
      Subject::WholeStack { root } => self.stack_robot(root),
    }
  }

  /// What the rules read of `table` with these facts, for an intent of the command (its push, its start)
  #[must_use]
  pub fn landing<'a>(&'a self, table: &'a BranchTable) -> LandingFacts<'a> {
    LandingFacts::new(table, &table.remote, self.view(), &self.robots, None)
  }
}
