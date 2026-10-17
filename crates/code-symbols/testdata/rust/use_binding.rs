//! The control owner of a lane, re-exported for the crate.

pub(crate) use avl_wire::program::lane::ControlOwner;
pub use crate::lane::{Lane, LaneId as Id, self};
use std::collections::{HashMap, hash_map::Entry};
use serde::Serialize as _;
use super::*;
extern crate alloc as heap;

pub fn owner_of(lanes: &HashMap<Id, Lane>) -> Option<ControlOwner> {
  match lanes.entry(Id::default()) {
    Entry::Occupied(entry) => Some(ControlOwner::from(entry.get())),
    Entry::Vacant(_) => None,
  }
}
