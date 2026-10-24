//! Which robot belongs to a virtual branch, and which to the whole stack of a root (its stack branch
//! `{prefix}/stack/<root>`): the one rule of `bd` and of the repository view (spec/branch/patronus.spec.md). A robot
//! started on the branch (`branch_name`), or a Safe Push of a temporary ref (the IDE, the CLI) on the local or the
//! pushed tip of the branch; the robot of the whole stack (`stack_root`) is the stack's, never a branch's, not even by
//! its revision. Of several, a running robot wins over a canceling one (a restart replaces it) over a finished one, then
//! the newest, then the first of the list. The page still matches them itself (`findBranchRobot` of
//! `src/lib/patronus.ts`); both pass the cases of `src/test-support/robot-match-cases.json`.

use domain_types::landing::Subject;
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};

/// The preferred robot of each branch, of each root of a whole stack and of each source revision of the robots of a
/// temporary ref, built once per list of robots: every branch of a table asks for its robot, and a scan of all the
/// robots for each made that O(branches × robots)
#[derive(Debug, Default)]
pub struct RobotIndex<'a> {
  branches: HashMap<&'a str, Ranked<'a>>,
  stacks: HashMap<&'a str, Ranked<'a>>,
  revisions: HashMap<&'a str, Ranked<'a>>,
}

/// A robot with its place in the list, which decides between equals
#[derive(Clone, Copy, Debug)]
struct Ranked<'a> {
  robot: &'a PatronusRobot,
  order: usize,
}

impl Ranked<'_> {
  /// The greater is preferred: running, canceling, finished; then the newest; then the first of the list
  const fn rank(&self) -> (u8, u32, Reverse<usize>) {
    (activity(self.robot.status), self.robot.start_time, Reverse(self.order))
  }
}

/// Running, then canceling (it goes away; its restart runs), then finished
const fn activity(status: PatronusRobotStatus) -> u8 {
  match status {
    PatronusRobotStatus::Running => 2,
    PatronusRobotStatus::Canceling => 1,
    _ => 0,
  }
}

impl<'a> RobotIndex<'a> {
  pub fn new(robots: &'a [PatronusRobot]) -> Self {
    let mut index = Self::default();
    for (order, robot) in robots.iter().enumerate() {
      let (by, key) = match (&robot.branch_name, &robot.stack_root, &robot.source_revision) {
        (Some(name), _, _) => (&mut index.branches, name),
        (None, Some(root), _) => (&mut index.stacks, root),
        (None, None, Some(revision)) => (&mut index.revisions, revision),
        (None, None, None) => continue,
      };
      let ranked = Ranked { robot, order };
      let current = by.entry(key.as_str()).or_insert(ranked);
      if ranked.rank() > current.rank() {
        *current = ranked;
      }
    }
    index
  }

  /// The robot of the virtual branch `name`, whose local and pushed tips are `local_tip` and `remote_tip`
  pub fn branch_robot(&self, name: &str, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a PatronusRobot> {
    self.preferred(self.branches.get(name), local_tip, remote_tip)
  }

  /// The robot of the whole stack of `root`, whose stack branch has the local and pushed tips `local_tip` and
  /// `remote_tip`
  pub fn stack_robot(&self, root: &str, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a PatronusRobot> {
    self.preferred(self.stacks.get(root), local_tip, remote_tip)
  }

  /// `own`, or a robot of a temporary ref on one of the tips that is preferred over it
  fn preferred(&self, own: Option<&Ranked<'a>>, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a PatronusRobot> {
    [local_tip, remote_tip]
      .into_iter()
      .flatten()
      .filter_map(|tip| self.revisions.get(tip))
      .chain(own)
      .max_by_key(|ranked| ranked.rank())
      .map(|ranked| ranked.robot)
  }
}

/// The robot was started on the branch of `subject`: the virtual branch `{prefix}/virtual/<name>` of a branch, the
/// stack branch `{prefix}/stack/<root>` of the whole stack of a root (every branch of the stack in one robot); a robot
/// of a temporary ref was started on neither
pub fn is_started_on(robot: &PatronusRobot, subject: &Subject) -> bool {
  match subject {
    Subject::Branch { name } => robot.branch_name.as_deref() == Some(name),
    Subject::WholeStack { root } => robot.stack_root.as_deref() == Some(root),
  }
}

/// The tips a robot of a temporary ref is matched by
#[derive(Clone, Copy, Debug)]
pub struct Tips<'t> {
  /// The name of the branch, or the root of the whole stack
  pub of: &'t str,
  /// The local tip of the branch, or of the stack branch of the whole stack
  pub local: Option<&'t str>,
  /// The tip of its remote branch
  pub remote: Option<&'t str>,
}

/// The robot of each branch and of each whole stack of a table, as robot ids
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RobotMatches {
  /// By branch name
  pub branches: BTreeMap<String, String>,
  /// By the root of the whole stack
  pub stacks: BTreeMap<String, String>,
}

/// The robots of `branches` and of the whole stacks of `stacks` in `index`; one without a robot is not listed
pub fn match_table<'t>(index: &RobotIndex<'_>, branches: impl IntoIterator<Item = Tips<'t>>, stacks: impl IntoIterator<Item = Tips<'t>>) -> RobotMatches {
  RobotMatches {
    branches: branches
      .into_iter()
      .filter_map(|tips| index.branch_robot(tips.of, tips.local, tips.remote).map(|robot| (tips.of.to_string(), robot.id.clone())))
      .collect(),
    stacks: stacks
      .into_iter()
      .filter_map(|tips| index.stack_robot(tips.of, tips.local, tips.remote).map(|robot| (tips.of.to_string(), robot.id.clone())))
      .collect(),
  }
}
