//! The fan-in of the API changes of a review (spec/branch/review-plan.spec.md, "Fan-in"): the usages of each removed or
//! changed declaration at the head of the range, from `accio references` (`file_diffs::accio::SymbolIndex`).
//!
//! - At most `MAX_FANIN_QUERIES` queries per plan, for the first removals and changes in the order of the card, each on
//!   a thread of its own; the caller's index cancels them (`SearchCanceled`, a newer request of the window).
//! - An index that fails fails no plan (`Lenient`): the entries it did not answer keep no fan-in, and the answer of
//!   `add_fan_in` says why, as it does when accio is missing or has no index of the repository. Never an index build.
//! - A usage is a hit of any kind but the declaration itself; the index answers at most 100 of them, and a cut answer
//!   is `complete: false`.

use crate::api::{ApiChange, ApiChangeKind, FanIn, rank};
use anyhow::Result;
use file_diffs::accio::{Lenient, References, SymbolIndex};
use sync_utils::span::in_current_span;
use tracing::debug;

/// The accio queries of one plan at most
pub const MAX_FANIN_QUERIES: usize = 20;

/// Why no fan-in: this machine has no accio
pub const NO_ACCIO: &str = "accio is not installed";
/// Why no fan-in: accio has no index of the repository, which the review never builds
pub const NO_INDEX: &str = "accio has no index of this repository";

/// The usages of `references` but the declarations themselves
fn fan_in_of(references: &References) -> FanIn {
  let usages = references.hits.iter().filter(|hit| hit.kind != "declaration").count();
  FanIn {
    usages: u32::try_from(usages).unwrap_or(u32::MAX),
    complete: references.complete,
  }
}

/// Add the fan-in at `head` to the removals and changes of `api` (at most `MAX_FANIN_QUERIES`) and rank it again
/// (`api::rank`); the answer is why some have none, `None` when every query answered or none was asked. Fails only
/// when the index was canceled
pub fn add_fan_in(index: Option<&dyn SymbolIndex>, repository_path: &str, head: &str, api: &mut [ApiChange]) -> Result<Option<String>> {
  let asked: Vec<usize> = api
    .iter()
    .enumerate()
    .filter(|(_, change)| change.kind != ApiChangeKind::Added)
    .map(|(position, _)| position)
    .take(MAX_FANIN_QUERIES)
    .collect();
  if asked.is_empty() {
    return Ok(None);
  }
  let Some(index) = index else { return Ok(Some(NO_ACCIO.to_string())) };
  let lenient = Lenient::new(index);
  let answers: Vec<Result<Option<References>>> = std::thread::scope(|scope| {
    let threads: Vec<_> = asked
      .iter()
      .map(|&position| {
        let (lenient, name) = (&lenient, api[position].name.as_str());
        scope.spawn(in_current_span(move || lenient.indexed_references(repository_path, name, Some(head))))
      })
      .collect();
    threads
      .into_iter()
      .map(|thread| thread.join().unwrap_or_else(|_| Err(anyhow::anyhow!("a query of the index panicked"))))
      .collect()
  });
  let mut answered = 0;
  for (position, answer) in asked.iter().zip(answers) {
    if let Some(references) = answer? {
      api[*position].fan_in = Some(fan_in_of(&references));
      answered += 1;
    }
  }
  debug!(asked = asked.len(), answered, "fan-in of the API changes");
  rank(api);
  let error = lenient.error();
  Ok(match error {
    Some(error) => Some(error),
    None if answered == 0 => Some(NO_INDEX.to_string()),
    None => None,
  })
}
