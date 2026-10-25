//! The Patronus section of the repository view for `bd` (`repository_model::facts::derive_patronus`, the rule the
//! session of the app derives too): the user's robots, the robot of each branch of the table as the rows match it, and
//! what they land, landed and hold, with what the rules decide over that (`derive::LandingFacts`). `bd push`, `bd
//! safe-push` and `bd status` read it; the words stay theirs.

use bd_core::context::RepoContext;
use bd_core::sync_command::SyncOutput;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{BranchPatronus, PatronusView, StackPatronus};
use repository_model::branch_table::BranchTable;
use repository_model::derive::LandingFacts;
use repository_model::facts::{PatronusOf, derive_patronus};
use repository_model::patronus_view::ContentMemo;
use std::collections::BTreeMap;

/// The robots and the Patronus view of a table; none without an answer of Patronus
#[derive(Default)]
pub struct PatronusFacts {
  pub robots: Vec<PatronusRobot>,
  pub view: PatronusView,
}

impl PatronusFacts {
  /// The view of `table` with the user's `robots`, on the remote facts of the table: by the hashes where they tell,
  /// else by content (git)
  pub fn read(context: &RepoContext, table: &BranchTable, robots: Vec<PatronusRobot>) -> Self {
    // no robot is known to need approvals: `bd` offers no restart of its own
    let of = PatronusOf {
      table,
      remote: &table.remote,
      robots: &robots,
      needs_review: &BTreeMap::new(),
    };
    let view = derive_patronus(&context.git, &context.repository, &of, &mut ContentMemo::default());
    Self { robots, view }
  }

  /// What the rules read of `table` with these facts, for an intent of `bd`'s own (its push, its start)
  pub fn landing<'a>(&'a self, table: &'a BranchTable) -> LandingFacts<'a> {
    LandingFacts::new(table, &table.remote, &self.view, &self.robots, None)
  }

  pub fn robot(&self, id: &str) -> Option<&PatronusRobot> {
    self.robots.iter().find(|robot| robot.id == id)
  }

  /// The robot of a branch, as the rows match it: started on it, or of a temporary ref at its tip
  pub fn branch_robot(&self, name: &str) -> Option<&PatronusRobot> {
    self.branch(name)?.robot_id.as_deref().and_then(|id| self.robot(id))
  }

  /// The robot of the whole stack of `root`
  pub fn stack_robot(&self, root: &str) -> Option<&PatronusRobot> {
    self.stack(root)?.robot_id.as_deref().and_then(|id| self.robot(id))
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
