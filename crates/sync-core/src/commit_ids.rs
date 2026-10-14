//! Commit ids in a request (a regroup plan, `bd commits <id>…`): full, or abbreviated to at least `MIN_ID_LENGTH`
//! characters, and resolved against the commits ahead of the baseline only

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
