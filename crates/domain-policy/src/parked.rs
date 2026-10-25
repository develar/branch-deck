//! Parked branches for the app and `bd` alike (spec/branch/parked-branches.spec.md, R3): the branches kept on the local
//! branch, never pushed or landed, until unparked.
//!
//! - `parked_state`: a branch is parked by its own line, else by the nearest branch it is built on (`stack::build_parent`,
//!   up the chain) that has one, whose commits it carries. The chain is the one the commits are built on, not the
//!   declared one: a branch whose declared parent has no commits is built on the parent's parent, and carries its
//!   commits
//! - `whole_stack_parked`: the first member of a whole stack that is parked keeps its stack branch local

use crate::facts::Repository;
use crate::stack::build_parent;
use domain_types::session::{Parked, ParkedMember};

/// Why `name` stays local: its own line, else the nearest branch it is built on with one; `None` when neither parks it
/// (or the chain of its bases loops)
#[must_use]
pub fn parked_state(repo: &Repository<'_>, name: &str) -> Option<Parked> {
  if let Some(note) = repo.parked_note(name) {
    return Some(Parked {
      note: note.to_string(),
      with: None,
    });
  }
  if !repo.has_parked() {
    return None;
  }
  let mut seen = vec![name];
  let mut parent = build_parent(repo, name);
  while let Some(ancestor) = parent {
    if seen.contains(&ancestor) {
      return None;
    }
    if let Some(note) = repo.parked_note(ancestor) {
      return Some(Parked {
        note: note.to_string(),
        with: Some(ancestor.to_string()),
      });
    }
    seen.push(ancestor);
    parent = build_parent(repo, ancestor);
  }
  None
}

/// The first of `members` (the branches of a stack branch, root first) that is parked, and why: what keeps the stack
/// branch, which holds its commits, local
#[must_use]
pub fn whole_stack_parked<'m>(repo: &Repository<'_>, members: impl IntoIterator<Item = &'m str>) -> Option<ParkedMember> {
  members.into_iter().find_map(|member| {
    parked_state(repo, member).map(|parked| ParkedMember {
      member: member.to_string(),
      parked,
    })
  })
}
