//! The input of the rules (docs/design/rust-domain-authority.md, s8-policy): the branch table as the rules read it,
//! borrowed from where it lives, never copied. Its one shape is `Repository`, read from the view of a session (`of_view`,
//! what `repository_model::derive` builds the view's structure with), or from a table a sync or a grouping left as its
//! grouped branches (`of_grouping`: `repository_model::branch_table::BranchTable` and `bd status`, until they read the
//! view of a session too). The rules of each family take it: `stack` (the stacks by declared parent, the ancestors by
//! effective base) and `parked`.

use domain_types::GroupedBranchInfo;
use domain_types::session::BranchView;
use domain_types::stack::BranchStack;
use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap};

/// A branch of the table as the rules read it: its name and where the sync placed it in its stack
#[derive(Clone, Copy, Debug)]
pub struct Placed<'a> {
  pub name: &'a str,
  /// `None` with stacked branches off
  pub stack: Option<&'a BranchStack>,
}

/// The branch table the rules decide over: its branches in the order of the table (newest first), where each is placed,
/// and the lines of the parked branches (`refs/branch-deck/parked`: branch -> note, also of branches without commits
/// here)
#[derive(Debug)]
pub struct Repository<'a> {
  branches: Vec<Placed<'a>>,
  index: HashMap<&'a str, usize>,
  parked: &'a BTreeMap<String, String>,
  /// The branches stacked directly on each branch, in the order of the table: built once, by the first rule that asks
  children: OnceCell<HashMap<&'a str, Vec<&'a str>>>,
}

impl<'a> Repository<'a> {
  /// `branches` in the order of the table, the first of a name counts
  pub fn new(branches: impl IntoIterator<Item = Placed<'a>>, parked: &'a BTreeMap<String, String>) -> Self {
    let mut placed = Vec::new();
    let mut index = HashMap::new();
    for branch in branches {
      if !index.contains_key(branch.name) {
        index.insert(branch.name, placed.len());
        placed.push(branch);
      }
    }
    Self {
      branches: placed,
      index,
      parked,
      children: OnceCell::new(),
    }
  }

  /// The table of a view: `order` names its rows, `branches` holds them
  pub fn of_view(order: &'a [String], branches: &'a BTreeMap<String, BranchView>, parked: &'a BTreeMap<String, String>) -> Self {
    let placed = order.iter().filter_map(|name| {
      let branch = branches.get(name)?;
      Some(Placed {
        name: &branch.head.name,
        stack: branch.head.stack.as_ref(),
      })
    });
    Self::new(placed, parked)
  }

  /// The table of the grouped branches of a sync or a grouping, in their order
  pub fn of_grouping(branches: &'a [GroupedBranchInfo], parked: &'a BTreeMap<String, String>) -> Self {
    let placed = branches.iter().map(|branch| Placed {
      name: &branch.name,
      stack: branch.stack.as_ref(),
    });
    Self::new(placed, parked)
  }

  /// The names of the branches, in the order of the table
  pub fn order(&self) -> impl Iterator<Item = &'a str> + '_ {
    self.branches.iter().map(|branch| branch.name)
  }

  /// The branch of the table named `name`
  #[must_use]
  pub fn branch(&self, name: &str) -> Option<Placed<'a>> {
    self.index.get(name).map(|&at| self.branches[at])
  }

  #[must_use]
  pub fn contains(&self, name: &str) -> bool {
    self.index.contains_key(name)
  }

  /// Where the sync placed `name`; `None` for a branch the table lacks, or with stacked branches off
  #[must_use]
  pub fn stack(&self, name: &str) -> Option<&'a BranchStack> {
    self.branch(name)?.stack
  }

  /// The note of the parked line of `name`; `None` when it has none
  #[must_use]
  pub fn parked_note(&self, name: &str) -> Option<&'a str> {
    self.parked.get(name).map(String::as_str)
  }

  /// Whether any branch has a parked line
  #[must_use]
  pub fn has_parked(&self) -> bool {
    !self.parked.is_empty()
  }

  /// The index of the children, built by `build` the first time a rule asks
  pub(crate) fn children_index(&self, build: impl FnOnce(&Self) -> HashMap<&'a str, Vec<&'a str>>) -> &HashMap<&'a str, Vec<&'a str>> {
    self.children.get_or_init(|| build(self))
  }
}
