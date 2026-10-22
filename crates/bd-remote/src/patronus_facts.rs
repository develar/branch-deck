//! The Patronus section of the repository view for `bd` (`repository_view::patronus_view`, the rule the app reads too):
//! the user's robots, matched to the branches of the table as the rows match them, and what they land, landed and hold.
//! `bd push`, `bd safe-push` and `bd status` read it; the words stay theirs.

use bd_core::context::RepoContext;
use bd_core::sync_command::SyncOutput;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{BranchPatronus, PatronusView, RobotMatch, StackPatronus};
use repository_view::branch_table::BranchTable;
use repository_view::patronus_view::{ContentMemo, match_robots, patronus_view};

/// The robots and the Patronus view of a table
pub struct PatronusFacts {
  pub robots: Vec<PatronusRobot>,
  pub matched: RobotMatch,
  pub view: PatronusView,
}

impl PatronusFacts {
  /// The view of `table` with the user's `robots`, on the remote facts of the table: by the hashes where they tell,
  /// else by content (git)
  pub fn read(context: &RepoContext, table: &BranchTable, robots: Vec<PatronusRobot>) -> Self {
    let remote = &table.remote;
    let matched = match_robots(table, remote, &robots);
    let view = patronus_view(&context.git, &context.repository, table, remote, &robots, &matched, &mut ContentMemo::default());
    Self { robots, matched, view }
  }

  pub fn robot(&self, id: &str) -> Option<&PatronusRobot> {
    self.robots.iter().find(|robot| robot.id == id)
  }

  /// The robot of a branch, as the rows match it: started on it, or of a temporary ref at its tip
  pub fn branch_robot(&self, name: &str) -> Option<&PatronusRobot> {
    self.matched.branches.get(name).and_then(|id| self.robot(id))
  }

  /// The robot of the whole stack of `root`
  pub fn stack_robot(&self, root: &str) -> Option<&PatronusRobot> {
    self.matched.stacks.get(root).and_then(|id| self.robot(id))
  }

  pub fn branch(&self, name: &str) -> Option<&BranchPatronus> {
    self.view.branches.get(name)
  }

  pub fn stack(&self, root: &str) -> Option<&StackPatronus> {
    self.view.stacks.get(root)
  }
}

/// The table of `bd`'s sync, with the remote statuses it emitted
pub fn table_of(context: &RepoContext, synced: &SyncOutput) -> BranchTable {
  BranchTable::from_report(&context.prefix, &synced.report, &synced.remote_statuses)
}
