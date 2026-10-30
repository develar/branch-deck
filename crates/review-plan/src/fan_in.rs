//! The usages of the API changes of a review (spec/branch/review-plan.spec.md, "Fan-in"): the usages of each removed or
//! changed declaration at the head of the range, from `accio references` (`file_diffs::accio::SymbolIndex`), split into
//! those in the files the reviewed commits change (updated in this change) and those left outside.
//!
//! - **The query** is `ApiChange::query`: `Owner.name` for a member, so that a use on another type drops out,
//!   `package.name` for a top-level JVM declaration, else the bare name. A name of `STOP_NAMES`, which nearly every type
//!   declares, is never asked bare: its usages stay unknown (`None`), not a wrong count. A member of that name is asked
//!   by its owner, but accio keeps every call of the name whose receiver it cannot type, so only the hits whose line
//!   names the owner count (`Store::new(…)`, `Store.of(…)`), and the usages are a lower bound (`complete: false`).
//!   Additions are never asked.
//! - At most `MAX_USAGE_QUERIES` queries per plan, for the first removals and changes in the order of the card, one per
//!   distinct query, each on a thread of its own; the caller's index cancels them (`SearchCanceled`, a newer request
//!   of the window).
//! - An index that fails fails no plan (`Lenient`): the entries it did not answer keep no usages, and the answer of
//!   `add_usages` says why, as it does when accio is missing or has no index of the repository. Never an index build.
//! - **A usage** is a hit in code: no declaration, `import` or `doc` hit (`NOT_USES`), none in a file of another
//!   family of languages than the declaration (a Python `.get(` is no use of a Kotlin `Key.get`). Of a type whose
//!   constructors are its only change, only a call (`Type(…)`, `new Type(…)`) can break, so only `call` hits count. It
//!   is **inside** when its path is one of the changed paths after the commits, else **outside**; the first
//!   `OUTSIDE_SITES` outside ones are kept, by path and line. The index answers at most 100 hits, and a cut answer is
//!   `complete: false`.

use crate::api::{ApiChange, ApiChangeKind, FormChange, UsageSite, Usages, rank};
use anyhow::Result;
use code_symbols::language::language_of;
use file_diffs::accio::{Lenient, ReferenceHit, References, SymbolIndex};
use std::collections::{BTreeMap, HashSet};
use sync_utils::span::in_current_span;
use tracing::debug;

/// The accio queries of one plan at most
pub const MAX_USAGE_QUERIES: usize = 20;

/// The usages left outside the change that an entry lists at most
pub const OUTSIDE_SITES: usize = 5;

/// The names that nearly every type declares or that a convention calls on anything: the members of `Object` and
/// `Any`, the operators of Kotlin, the common traits of Rust and the methods of the collections. An index answers
/// their usages on every type, so they are never asked
pub const STOP_NAMES: [&str; 32] = [
  "equals",
  "hashCode",
  "toString",
  "compareTo",
  "clone",
  "finalize", // Object, Any
  "get",
  "set",
  "invoke",
  "contains",
  "iterator",
  "next",
  "hasNext",
  "getValue",
  "setValue",
  "provideDelegate", // operators
  "run",
  "apply",
  "call",
  "close",
  "dispose", // Runnable, Callable, Closeable, Disposable
  "new",
  "default",
  "from",
  "into",
  "fmt",
  "eq",
  "hash",
  "drop", // Rust traits
  "size",
  "add",
  "remove", // collections
];

/// Why no usages: this machine has no accio
pub const NO_ACCIO: &str = "accio is not installed";
/// Why no usages: accio has no index of the repository, which the review never builds
pub const NO_INDEX: &str = "accio has no index of this repository";

/// The kinds of accio hits that use nothing: the declaration itself, an import, a mention in a doc comment
pub const NOT_USES: [&str; 3] = ["declaration", "import", "doc"];

/// A name of `STOP_NAMES` that is asked bare: never asked
fn stopped(change: &ApiChange) -> bool {
  change.owner.is_none() && STOP_NAMES.contains(&change.name.as_str())
}

/// A member of a name of `STOP_NAMES`: only the hits that name its owner count
fn by_owner(change: &ApiChange) -> Option<&str> {
  change.owner.as_deref().filter(|_| STOP_NAMES.contains(&change.name.as_str()))
}

/// `word` stands in `text` as a whole word
fn names(text: &str, word: &str) -> bool {
  let part = |char: Option<char>| char.is_some_and(|char| char.is_alphanumeric() || char == '_' || char == '$');
  text
    .match_indices(word)
    .any(|(at, _)| !part(text[..at].chars().next_back()) && !part(text[at + word.len()..].chars().next()))
}

/// `hit` is a use of `change`: a hit in code of its family of languages; only a call of a type whose constructors are its
/// only change; one that names the owner of a member of `STOP_NAMES`
fn uses(change: &ApiChange, hit: &ReferenceHit) -> bool {
  if NOT_USES.contains(&hit.kind.as_str()) {
    return false;
  }
  if let (Some(declared), Some(used)) = (language_of(&change.file), language_of(&hit.path))
    && declared.family() != used.family()
  {
    return false;
  }
  if change.kind == ApiChangeKind::Changed && change.changes == [FormChange::Constructors] && hit.kind != "call" {
    return false;
  }
  by_owner(change).is_none_or(|owner| names(&hit.snippet, owner))
}

/// The usages of `change` in `references` (`uses`), inside `changed` (the paths after the commits) or not
fn usages_of(change: &ApiChange, references: &References, changed: &HashSet<&str>) -> Usages {
  let mut usages = Usages {
    inside: 0,
    outside: 0,
    outside_sites: Vec::new(),
    complete: references.complete && by_owner(change).is_none(),
  };
  for hit in references.hits.iter().filter(|hit| uses(change, hit)) {
    if changed.contains(hit.path.as_str()) {
      usages.inside += 1;
      continue;
    }
    usages.outside += 1;
    if usages.outside_sites.len() < OUTSIDE_SITES {
      usages.outside_sites.push(UsageSite {
        path: hit.path.clone(),
        line: hit.line,
      });
    }
  }
  usages
}

/// Add the usages at `head` to the removals and changes of `api` (at most `MAX_USAGE_QUERIES` distinct queries, no name
/// of `STOP_NAMES` asked bare), split by `changed` (the paths after the commits), and rank it again (`api::rank`); the answer is
/// why some have none, `None` when every query answered or none was asked. Fails only when the index was canceled
pub fn add_usages(index: Option<&dyn SymbolIndex>, repository_path: &str, head: &str, changed: &HashSet<&str>, api: &mut [ApiChange]) -> Result<Option<String>> {
  // query -> the entries it answers, in the order of the card
  let mut asked: BTreeMap<String, Vec<usize>> = BTreeMap::new();
  let mut queries: Vec<String> = Vec::new();
  for (position, change) in api.iter().enumerate() {
    if change.kind == ApiChangeKind::Added || stopped(change) {
      continue;
    }
    if !asked.contains_key(&change.query) {
      if queries.len() == MAX_USAGE_QUERIES {
        continue;
      }
      queries.push(change.query.clone());
    }
    asked.entry(change.query.clone()).or_default().push(position);
  }
  if queries.is_empty() {
    return Ok(None);
  }
  let Some(index) = index else { return Ok(Some(NO_ACCIO.to_string())) };
  let lenient = Lenient::new(index);
  let answers: Vec<Result<Option<References>>> = std::thread::scope(|scope| {
    let threads: Vec<_> = queries
      .iter()
      .map(|query| {
        let (lenient, query) = (&lenient, query.as_str());
        scope.spawn(in_current_span(move || lenient.indexed_references(repository_path, query, Some(head))))
      })
      .collect();
    threads
      .into_iter()
      .map(|thread| thread.join().unwrap_or_else(|_| Err(anyhow::anyhow!("a query of the index panicked"))))
      .collect()
  });
  let mut answered = 0;
  for (query, answer) in queries.iter().zip(answers) {
    if let Some(references) = answer? {
      for &position in &asked[query] {
        api[position].usages = Some(usages_of(&api[position], &references, changed));
      }
      answered += 1;
    }
  }
  debug!(asked = queries.len(), answered, "usages of the API changes");
  rank(api);
  let error = lenient.error();
  Ok(match error {
    Some(error) => Some(error),
    None if answered == 0 => Some(NO_INDEX.to_string()),
    None => None,
  })
}
