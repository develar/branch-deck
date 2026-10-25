//! What a session derives of its table for the view by the rules of `domain-policy` (docs/design/rust-domain-authority.md,
//! s8-policy), so the page and `bd` read one answer and the page decides nothing: `repository` is the input of the rules
//! read from a table (`domain_policy::facts::Repository`), `place` the structure of each branch (its stack by declared
//! parent, its ancestors by effective base, its main line, whether it is parked and its section) and the rows of each
//! section. The table places its branches once per grouping (`TableState::apply`), the root of a whole stack again when
//! its stack branch is reported; a patch carries a structure only when it changed.

use crate::table::{TableParts, TableState};
use domain_policy::facts::Repository;
use domain_policy::parked::{parked_state, whole_stack_parked};
use domain_policy::stack::{build_ancestors, children, declared_parent, descendants, main_line};
use domain_types::session::{BranchSection, BranchStructure, ParkedMember, RowsView};
use domain_types::stack::StackBranchUpdate;
use std::collections::BTreeMap;

/// The input of the rules over the table of `table`, as its view has it
#[must_use]
pub fn repository(table: &TableState) -> Repository<'_> {
  parts_repository(&table.parts)
}

fn parts_repository(parts: &TableParts) -> Repository<'_> {
  Repository::of_view(&parts.order, &parts.branches, &parts.header.parked)
}

/// The structure of every branch of `parts` and the rows of each section, written into them
pub(crate) fn place(parts: &mut TableParts) {
  let (structures, rows) = {
    let repo = parts_repository(parts);
    let structures: BTreeMap<String, BranchStructure> = repo.order().map(|name| (name.to_string(), structure_of(&repo, &parts.stacks, name))).collect();
    let (parked, main): (Vec<&String>, Vec<&String>) = parts
      .order
      .iter()
      .filter(|name| structures.contains_key(*name))
      .partition(|name| structures[*name].section == BranchSection::Parked);
    let rows = RowsView {
      main: main.into_iter().cloned().collect(),
      parked: parked.into_iter().cloned().collect(),
    };
    (structures, rows)
  };
  for (name, structure) in structures {
    if let Some(branch) = parts.branches.get_mut(&name) {
      branch.structure = structure;
    }
  }
  parts.rows = rows;
}

/// The parked member of the stack branch of `root` written into the structure of `root`, once its stack branch is
/// reported
pub(crate) fn place_stack(parts: &mut TableParts, root: &str) {
  let stack_parked = stack_parked(&parts_repository(parts), parts.stacks.get(root));
  if let Some(branch) = parts.branches.get_mut(root) {
    branch.structure.stack_parked = stack_parked;
  }
}

fn structure_of(repo: &Repository<'_>, stacks: &BTreeMap<String, StackBranchUpdate>, name: &str) -> BranchStructure {
  let owned = |names: Vec<&str>| names.into_iter().map(str::to_string).collect();
  let parked = parked_state(repo, name);
  let main_line = repo.branch(name).map_or_default(|branch| owned(main_line(repo, branch.name)));
  BranchStructure {
    parent: declared_parent(repo, name).map(str::to_string),
    children: owned(children(repo, name).to_vec()),
    ancestors: owned(build_ancestors(repo, name)),
    descendants: owned(descendants(repo, name)),
    main_line,
    section: if parked.is_some() { BranchSection::Parked } else { BranchSection::Main },
    parked,
    stack_parked: stack_parked(repo, stacks.get(name)),
  }
}

fn stack_parked(repo: &Repository<'_>, stack: Option<&StackBranchUpdate>) -> Option<ParkedMember> {
  whole_stack_parked(repo, stack?.members.iter().map(String::as_str))
}

#[cfg(test)]
#[path = "derive_test.rs"]
mod derive_test;
