//! The usages of the API changes of a review (openspec/specs/branch/review-plan/spec.md, "Fan-in"): the usages of each removed or
//! changed declaration at the head of the range, from `accio references --at` (`file_diffs::accio::SymbolIndex`), split
//! into those the change updated, those left outside that can break, and those left outside that still bind.
//!
//! - **Asked by its place** (`ApiChange::at`): the line of its name at the head, of a removal at the parent of the
//!   commit that first touched it; accio answers only that declaration, every place of a plan in one process. Additions
//!   are never asked. Never an index build: an index that fails fails no plan (`Lenient`), and the answer of
//!   `add_usages` says why, as it does when accio is missing or has no usable index of the repository
//!   (`IndexUnavailable`). The caller's index cancels the query (`SearchCanceled`, a newer request of the window). A
//!   place accio finds no declaration at (`References::unavailable`) keeps its usages unknown.
//! - **A usage** is a hit in code tied to the declaration by a type or an import: no declaration, `import` or `doc`
//!   hit (`NOT_USES`). A hit only the name ties (`Resolved::Name`: a receiver nothing typed, another language, a
//!   namesake's) is no use and no proof of none: those left outside are counted apart (`Usages::by_name`). Of a type
//!   whose constructors are its only change, only the `call` hits (`Type(…)`, `new Type(…)`, a Kotlin
//!   `class Sub : Type(…)`) and the `supertype` hits (a Java `extends Type`, whose `super(…)` calls a constructor)
//!   count: a use of the type as a type or an import still compiles.
//! - **Inside** when a line of it is one the commits changed (`pair::ChangedLines`, read only when the index
//!   answered), a call from its first line to its last: this change wrote or rewrote it. A use on lines it left as they
//!   were is **outside**, in a changed file too.
//! - **Still bound** (`still_binds`): a call left outside of a declaration whose parameters or constructors are its only
//!   change, which accio says fits the new form (`fits`), and each argument of which binds a parameter of the same name
//!   and type as before the commits: by its name, by its position, as the trailing lambda. It neither breaks nor
//!   changes its meaning. A spread argument is never known to.
//! - The first `OUTSIDE_SITES` uses left outside that can break are kept, by path and line. The index answers at most
//!   100 hits per declaration, and a cut answer is `complete: false`.
//! - **The verdict** of the plan (`verdict_of`): `Breaks` when an entry has a use left outside that can break, else
//!   `Unknown` when the uses of one are unknown or not `unbroken`, else `NoUseBreaks`, also of an API of additions alone.

use crate::api::{ApiChange, ApiChangeKind, BreakVerdict, FormChange, UsageSite, Usages, rank};
use crate::pair::ChangedLines;
use anyhow::Result;
use file_diffs::accio::{Access, CallArgument, DeclarationAt, Lenient, ReferenceHit, References, Resolved, SymbolIndex};
use std::collections::BTreeMap;
use tracing::debug;
use trial_check::symbol_dependencies::declaration_checks::ParameterForm;

/// The usages left outside that an entry lists at most
pub const OUTSIDE_SITES: usize = 5;

/// Why no usages: this machine has no accio
pub const NO_ACCIO: &str = "accio is not installed";
/// Why no usages: the index answered none and gave no reason (a fixture without its file); accio gives its own
/// (`IndexUnavailable`). The review never builds an index
pub const NO_INDEX: &str = "accio has no index of this repository";

/// The kinds of accio hits that use nothing: the declaration itself, an import, a mention in a doc comment or in prose
pub const NOT_USES: [Access; 3] = [Access::Declaration, Access::Import, Access::Doc];

/// The kinds of accio hits that call a constructor of a type: a call, and a subtype, whose constructors call one
const CONSTRUCTOR_USES: [Access; 2] = [Access::Call, Access::Supertype];

/// The only change of `change` is its constructors: of its hits, only a call and a subtype are known to break
fn constructors_only(change: &ApiChange) -> bool {
  change.kind == ApiChangeKind::Changed && change.changes == [FormChange::Constructors]
}

/// What a hit is to the declaration asked about
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Use {
  /// The declaration, an import, a doc, or a use of a type a constructor change cannot break
  None,
  /// Only the name ties it
  ByName,
  /// A use that may break
  Breaks,
  /// A call that still binds the parameters it bound before the commits
  StillBound,
}

fn use_of(change: &ApiChange, hit: &ReferenceHit) -> Use {
  if NOT_USES.contains(&hit.kind) {
    return Use::None;
  }
  if hit.resolved == Resolved::Name {
    return Use::ByName;
  }
  if constructors_only(change) && !CONSTRUCTOR_USES.contains(&hit.kind) {
    return Use::None;
  }
  if still_binds(change, hit) { Use::StillBound } else { Use::Breaks }
}

/// `hit` is a call that still binds what it bound before the commits: its declaration changed only its parameters or
/// its constructors, accio says the call fits the new form, and each argument binds a parameter of the same name and type
/// before and after the commits
fn still_binds(change: &ApiChange, hit: &ReferenceHit) -> bool {
  let only_parameters =
    change.kind == ApiChangeKind::Changed && !change.changes.is_empty() && change.changes.iter().all(|it| matches!(it, FormChange::Parameters | FormChange::Constructors));
  if !only_parameters || !CONSTRUCTOR_USES.contains(&hit.kind) || hit.fits != Some(true) {
    return false;
  }
  let Some(arguments) = &hit.arguments else { return false };
  arguments
    .iter()
    .all(|argument| binds_the_same(argument, &change.signatures_before, &change.signatures_after))
}

/// The one type of the parameter `name` across `signatures`; `None` when none of them has it or two give it different
/// types
fn type_of<'a>(signatures: &'a [Vec<ParameterForm>], name: &str) -> Option<&'a str> {
  let mut types = signatures
    .iter()
    .flatten()
    .filter(|parameter| parameter.name == name)
    .map(|parameter| parameter.type_text.as_str());
  let first = types.next()?;
  types.all(|other| other == first).then_some(first)
}

/// The parameter an argument at `position` binds in `signature`: the vararg takes every position from its own
fn at_position(signature: &[ParameterForm], position: usize) -> Option<&ParameterForm> {
  match signature.iter().position(|parameter| parameter.vararg) {
    Some(vararg) if position >= vararg => signature.get(vararg),
    _ => signature.get(position),
  }
}

/// `argument` binds, before the commits, the parameter it binds after them (`CallArgument::parameter`), of the same type:
/// a named one by its name, a positional one by its position in every list before that is long enough, a trailing lambda
/// as the last parameter of every list before
fn binds_the_same(argument: &CallArgument, before: &[Vec<ParameterForm>], after: &[Vec<ParameterForm>]) -> bool {
  let Some(parameter) = argument.parameter.as_deref() else { return false };
  if argument.spread || argument.unbound {
    return false;
  }
  let Some(new_type) = type_of(after, parameter) else { return false };
  if type_of(before, parameter) != Some(new_type) {
    return false;
  }
  if argument.name.is_some() {
    return argument.name.as_deref() == Some(parameter);
  }
  let bound_before: Vec<Option<&ParameterForm>> = if argument.trailing_lambda {
    before.iter().map(|signature| signature.last()).collect()
  } else if let Some(position) = argument.position {
    before
      .iter()
      .map(|signature| at_position(signature, usize::try_from(position).unwrap_or(usize::MAX)))
      .collect()
  } else {
    return false;
  };
  let bound: Vec<&ParameterForm> = bound_before.into_iter().flatten().collect();
  !bound.is_empty() && bound.iter().all(|old| old.name == parameter)
}

/// The usages of `change` in `references` (`use_of`), on lines the commits changed (`changed`) or not
fn usages_of(change: &ApiChange, references: &References, changed: &ChangedLines) -> Usages {
  let mut usages = Usages {
    inside: 0,
    outside: 0,
    outside_sites: Vec::new(),
    still_bound: 0,
    by_name: 0,
    complete: references.complete,
    unbroken: false,
  };
  for hit in &references.hits {
    let kind = use_of(change, hit);
    if kind == Use::None {
      continue;
    }
    if changed.touches(&hit.path, hit.line..=hit.end_line.unwrap_or(hit.line).max(hit.line)) {
      if kind != Use::ByName {
        usages.inside += 1;
      }
      continue;
    }
    match kind {
      Use::ByName => usages.by_name += 1,
      Use::StillBound => usages.still_bound += 1,
      Use::Breaks => {
        usages.outside += 1;
        if usages.outside_sites.len() < OUTSIDE_SITES {
          usages.outside_sites.push(UsageSite {
            path: hit.path.clone(),
            line: hit.line,
          });
        }
      }
      Use::None => {}
    }
  }
  usages.unbroken = usages.complete && usages.outside == 0 && usages.by_name == 0;
  usages
}

/// The answer of one query of `index` through `Lenient`, else why there is none: accio is missing, has no usable index of
/// the repository, or failed. Fails only when the query was canceled
pub(crate) fn ask_index<T>(index: Option<&dyn SymbolIndex>, ask: impl FnOnce(&Lenient<'_>) -> Result<Option<T>>) -> Result<Result<T, String>> {
  let Some(index) = index else { return Ok(Err(NO_ACCIO.to_string())) };
  let lenient = Lenient::new(index);
  let answer = ask(&lenient)?;
  let unavailable = lenient.unavailable();
  let error = lenient.error();
  Ok(answer.ok_or_else(|| error.or(unavailable).unwrap_or_else(|| NO_INDEX.to_string())))
}

/// Add the usages at `head` to the removals and changes of `api`, asked by their places in one query, split by the
/// lines the commits changed (`changed`, called once the index answered), and rank it again (`api::rank`); the answer
/// is why they have none, `None` when the index answered or none was asked. Fails when the index was canceled or
/// `changed` fails
pub fn add_usages(
  index: Option<&dyn SymbolIndex>,
  repository_path: &str,
  head: &str,
  changed: impl FnOnce() -> Result<ChangedLines>,
  api: &mut [ApiChange],
) -> Result<Option<String>> {
  // place -> the entries it answers, in the order of the card
  let mut asked: BTreeMap<&DeclarationAt, Vec<usize>> = BTreeMap::new();
  for (position, change) in api.iter().enumerate() {
    if change.kind != ApiChangeKind::Added {
      asked.entry(&change.at).or_default().push(position);
    }
  }
  if asked.is_empty() {
    return Ok(None);
  }
  let places: Vec<DeclarationAt> = asked.keys().map(|at| (*at).clone()).collect();
  let answers = match ask_index(index, |lenient| lenient.references_at(repository_path, &places, head))? {
    Ok(answers) => answers,
    Err(note) => return Ok(Some(note)),
  };
  let changed = changed()?;
  let entries: Vec<Vec<usize>> = asked.into_values().collect();
  for (positions, references) in entries.into_iter().zip(&answers) {
    // a place accio found no declaration at: its uses stay unknown, never "none left outside"
    if references.unavailable {
      continue;
    }
    for position in positions {
      api[position].usages = Some(usages_of(&api[position], references, &changed));
    }
  }
  debug!(asked = places.len(), "usages of the API changes");
  rank(api);
  Ok(None)
}

/// Whether a use left outside breaks (`BreakVerdict`), by the usages of the removed and changed entries of `api`: an
/// entry with one that can break makes it `Breaks`, whatever the others; else an entry whose uses are unknown, cut at
/// the limit or tied by the name alone makes it `Unknown`; else no use breaks, also when every entry is an addition
pub fn verdict_of(api: &[ApiChange]) -> BreakVerdict {
  let mut verdict = BreakVerdict::NoUseBreaks;
  for change in api.iter().filter(|change| change.kind != ApiChangeKind::Added) {
    match &change.usages {
      Some(usages) if usages.outside > 0 => return BreakVerdict::Breaks,
      Some(usages) if usages.unbroken => {}
      _ => verdict = BreakVerdict::Unknown,
    }
  }
  verdict
}
