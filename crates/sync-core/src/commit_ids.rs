//! Commit ids in a request (a regroup plan, `bd commits <id>…`): full, or abbreviated to at least `MIN_ID_LENGTH`
//! characters, and resolved against the commits ahead of the baseline only

use std::collections::HashSet;

/// An abbreviated id needs at least this many characters
pub const MIN_ID_LENGTH: usize = 4;

/// How an id of a request matches the commits ahead of the baseline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdMatch<'a> {
  /// The full id of the one commit it names
  One(&'a str),
  /// No commit, or an id shorter than `MIN_ID_LENGTH`
  None,
  /// Several commits start with it
  Ambiguous,
}

/// The candidate whose id starts with `id`
pub fn match_commit_id<'a>(id: &str, candidates: impl IntoIterator<Item = &'a str>) -> IdMatch<'a> {
  if id.len() < MIN_ID_LENGTH {
    return IdMatch::None;
  }
  let mut matches = candidates.into_iter().filter(|candidate| candidate.starts_with(id));
  match (matches.next(), matches.next()) {
    (Some(full), None) => IdMatch::One(full),
    (None, _) => IdMatch::None,
    (Some(_), Some(_)) => IdMatch::Ambiguous,
  }
}

/// The ids of a request resolved against the commits ahead of the baseline: the full id of each, and the ids that name
/// no commit or several
#[derive(Debug, Default)]
pub struct ResolvedIds {
  pub full: HashSet<String>,
  pub unknown: Vec<String>,
  pub ambiguous: Vec<String>,
}

pub fn resolve_commit_ids(ids: &[String], candidates: &[&str]) -> ResolvedIds {
  let mut resolved = ResolvedIds::default();
  for id in ids {
    match match_commit_id(id, candidates.iter().copied()) {
      IdMatch::One(full) => {
        resolved.full.insert(full.to_string());
      }
      IdMatch::None => resolved.unknown.push(id.clone()),
      IdMatch::Ambiguous => resolved.ambiguous.push(id.clone()),
    }
  }
  resolved
}
