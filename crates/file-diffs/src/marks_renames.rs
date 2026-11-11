//! The renames of a diff (openspec/specs/commit/quiet-changes/spec.md, "Renames"; docs/design/diff-view.md, "Marks"): the
//! names a change renamed within a file, and the runs of the lines that changed in such a name alone (`marks.rs`).
//!
//! 1. **Candidates, from the tokens alone.** Where the alignment of the tokens of a block put a name of the new side
//!    at the place of a name of the old one (`marks_runs`), the two are a candidate when they stand alike in the tree
//!    (`Token::shape`), are spelled otherwise and are each one plain word (`words`). A block whose sides hold the same
//!    words has none (`may_hold_rename`), which is also the question the block is asked before the tokens are read.
//!    A file without a candidate is not parsed for its names.
//! 2. **The bindings, from one parse of each side** (`code_symbols::navigate::names_at`, openspec/specs/commit/name-bindings/spec.md):
//!    each candidate place of the old text is asked with the new name, each of the new text with the old one. The
//!    answers are bound by work (`MAX_RENAME_WORK`): a resolution of a name, one per spelling and one more per binding,
//!    costs the tokens of the text or an eighth of them by its grammar (`resolution_work`), and the first place of
//!    each pair of names is asked before the second of any.
//! 3. **A rename is one binding on each side with the same places** (`confirm`): as many names on both sides, name `i`
//!    of the old text in the block of name `i` of the new one and nowhere the alignment put another name, a
//!    declaration at the same places of both. So a use the change left behind, a use it took from another name and
//!    two bindings that became one are no rename. Of a declaration inside code (`local`) each side also says that the
//!    other name changes no binding (no clash): a rename that captures a name is a change of the code, not marked.
//!    Any other name is every name of its spelling that no such declaration binds; declared in the file it is a
//!    rename that is a label, declared elsewhere it is handed out unmarked (`Renames::undeclared`) for the reader
//!    that knows the other files.
//! 4. **The blocks are read again with the two names of a place read as one** (`Renames::cut`): the tokens of each
//!    place get one text on both sides (`Tokens::with_texts`), a text of that place alone, and the blocks that hold
//!    one are read as before (`runs_of_block`). Lines that are then the same tokens and hold a place are a run of
//!    the kind `Renamed`: the rename and nothing else, however the lines were wrapped. A place on a line with
//!    another change is in no run, and that line is read. Lines of the same tokens that hold no place are a run of
//!    formatting as they are without the rename, never lines of the rename beside them; and a run the first reading
//!    found stays where the second aligned its lines otherwise (`with_the_first`): the second reading adds runs and
//!    takes none.
//! 5. **A run is proven** when it holds only places of local values of their bodies alone, every name of which the
//!    scopes bind (`LocalBinding::is_of_body_alone`: a value no code outside its body may name, in a text the scopes
//!    read whole). A place that makes more of the name than its binding (a shorthand property, a destructuring, a
//!    name only the types tell apart, a macro, a name that is a value where the code runs: any `Site` but `Plain`)
//!    and the declaration of a name that is not local never get the one text, so their lines are in no run and are
//!    read. Whether a proven run is quiet `marks::is_quiet` says of its kind, which a measurement switches on
//!    (`marks_of`).
//!
//! Nothing here is a rule of characters: a file of no grammar has no rename.

use crate::marks::MAX_RENAME_WORK;
use crate::marks_runs::{Asked, BlockLines, runs_of_block};
use code_symbols::highlight::Grammar;
use code_symbols::navigate::{Access, NameAt, NameQuestion, NameUse, Position, Site, names_at_within, resolution_work};
use code_symbols::tokens::{Token, TokenClass, Tokens};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use text_diff::model::{BlockKind, DiffBlock, LineRun, Rename, RunKind};

/// The words of a text: its longest rows of letters, digits, `_` and `$` that do not start with a digit. A name
/// that is one word is a candidate of a rename; a name written with quotes or an escape is none
fn words(text: &str) -> impl Iterator<Item = &str> {
  text
    .split(|character: char| !(character.is_alphanumeric() || character == '_' || character == '$'))
    .filter(|word| !word.is_empty() && !word.starts_with(char::is_numeric))
}

/// Whether `text` is one word
fn is_word(text: &str) -> bool {
  let mut found = words(text);
  found.next() == Some(text) && found.next().is_none()
}

/// Whether the lines of a block, the text of each side, may hold a candidate of a rename: one side has a word the
/// other has not, or has more often. Asked before the tokens are read, and of every block that is read: a block
/// whose sides hold the same words gives no candidate
pub(crate) fn may_hold_rename(old: &str, new: &str) -> bool {
  let mut counts: HashMap<&str, i32> = HashMap::new();
  for word in words(old) {
    *counts.entry(word).or_default() += 1;
  }
  for word in words(new) {
    *counts.entry(word).or_default() -= 1;
  }
  counts.values().any(|count| *count != 0)
}

/// The names of the two sides that the alignment of the tokens put at one place: what a rename is looked for in
#[derive(Debug, Default)]
pub(crate) struct Candidates {
  /// In the order of the texts, each the token of the old text and the one of the new text
  pairs: Vec<[Token; 2]>,
}

impl Candidates {
  /// Takes two tokens the alignment of a block put at one place when they are two names that stand alike in the
  /// tree and are spelled otherwise
  pub(crate) fn take(&mut self, old: &Token, new: &Token) {
    if old.class == TokenClass::Name && new.class == TokenClass::Name && old.shape() == new.shape() && old.text != new.text {
      self.pairs.push([*old, *new]);
    }
  }

  pub(crate) const fn is_empty(&self) -> bool {
    self.pairs.is_empty()
  }
}

/// A rename the diff marks
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
  pub(crate) rename: Rename,
  /// Its places are proven to change nothing: a value of its body alone, every name of which the scopes bind
  proven: bool,
  /// For each place that is the rename and nothing else, its tokens in the two texts (their places in
  /// `Tokens::all`), which are read as one; `None` for a place that is read
  places: Vec<Option<[usize; 2]>>,
}

/// The renames of a pair of texts
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Renames {
  /// The renames the diff marks, by their first place
  pub(crate) found: Vec<Found>,
  /// The names that changed at every place of the file and are declared in none of them, by their first place: the
  /// diff marks nothing of them
  pub(crate) undeclared: Vec<Rename>,
  /// The resolutions of a name each text took, of those the work paid for
  pub(crate) resolutions: [usize; 2],
}

/// The renames among `candidates` of two texts of `grammar` with the tokens `tokens` and the blocks of their diff
pub(crate) fn renames_of(grammar: Grammar, texts: [&str; 2], tokens: [&Tokens; 2], blocks: &[DiffBlock], candidates: &Candidates) -> Renames {
  renames_within(grammar, texts, tokens, blocks, candidates, MAX_RENAME_WORK)
}

/// `renames_of` within `work` in place of `MAX_RENAME_WORK`
pub(crate) fn renames_within(grammar: Grammar, texts: [&str; 2], tokens: [&Tokens; 2], blocks: &[DiffBlock], candidates: &Candidates, work: u64) -> Renames {
  let pairs: Vec<&[Token; 2]> = candidates
    .pairs
    .iter()
    .filter(|[old, new]| is_word(old.text_in(texts[0])) && is_word(new.text_in(texts[1])))
    .collect();
  if pairs.is_empty() {
    return Renames::default();
  }
  let asked = in_the_order_asked(&pairs);
  let place = |token: &Token| Position {
    line: token.line,
    column: token.start,
  };
  let questions = |side: usize| -> Vec<NameQuestion<'_>> {
    asked
      .iter()
      .map(|pair| NameQuestion {
        position: place(&pairs[*pair][side]),
        renamed_to: Some(pairs[*pair][1 - side].text_in(texts[1 - side])),
      })
      .collect()
  };
  // as many resolutions of each text as the work pays for of both
  let cost = |side: usize| resolution_work(grammar, tokens[side].all().len()).max(1);
  let paid = usize::try_from(work / (cost(0) + cost(1))).unwrap_or(usize::MAX);
  let (old_names, new_names) = (
    names_at_within(grammar, texts[0], &questions(0), paid),
    names_at_within(grammar, texts[1], &questions(1), paid),
  );
  let aligned = Aligned::of(&pairs);

  let mut renames = Renames {
    resolutions: [old_names.resolutions, new_names.resolutions],
    ..Renames::default()
  };
  let mut seen = HashSet::new();
  for (old_entry, new_entry) in old_names.asked.iter().zip(&new_names.asked) {
    let (Some(old_entry), Some(new_entry)) = (*old_entry, *new_entry) else { continue };
    if !seen.insert((old_entry, new_entry)) {
      continue;
    }
    match confirm(&old_names.names[old_entry], &new_names.names[new_entry], tokens, blocks, &aligned) {
      Some(Confirmed::Marked(found)) => renames.found.push(found),
      Some(Confirmed::Undeclared(rename)) => renames.undeclared.push(rename),
      None => {}
    }
  }
  let first_place = |rename: &Rename| (rename.old_sites.first().copied(), rename.old_sites.get(1).copied());
  renames.found.sort_by_key(|found| first_place(&found.rename));
  renames.undeclared.sort_by_key(first_place);
  // a token is a place of one rename: renames that share one are told apart by nothing here, and none of them holds
  let mut holders: [HashMap<usize, u32>; 2] = [HashMap::new(), HashMap::new()];
  for places in renames.found.iter().flat_map(|found| found.places.iter().flatten()) {
    *holders[0].entry(places[0]).or_default() += 1;
    *holders[1].entry(places[1]).or_default() += 1;
  }
  renames
    .found
    .retain(|found| found.places.iter().flatten().all(|places| holders[0][&places[0]] == 1 && holders[1][&places[1]] == 1));
  renames
}

/// The candidates by their numbers, in the order they are asked: the first place of each pair of names in the order
/// of the text, then the second of each, and so on. So where the work ends before the places do, it has paid for a
/// binding of as many pairs of names as it can, before a second binding of any
fn in_the_order_asked(pairs: &[&[Token; 2]]) -> Vec<usize> {
  let mut places: HashMap<(u64, u64), usize> = HashMap::new();
  let mut order: Vec<(usize, usize)> = pairs
    .iter()
    .enumerate()
    .map(|(index, [old, new])| {
      let place = places.entry((old.text, new.text)).or_default();
      *place += 1;
      (*place, index)
    })
    .collect();
  order.sort_unstable();
  order.into_iter().map(|(_, index)| index).collect()
}

/// Where the alignment of the tokens put a name of one side at the place of a name of the other: every candidate,
/// asked or not, by its line and column
struct Aligned {
  new_of_old: HashMap<(u32, u32), (u32, u32)>,
  old_of_new: HashMap<(u32, u32), (u32, u32)>,
}

impl Aligned {
  fn of(pairs: &[&[Token; 2]]) -> Self {
    let place = |token: &Token| (token.line, token.start);
    Self {
      new_of_old: pairs.iter().map(|[old, new]| (place(old), place(new))).collect(),
      old_of_new: pairs.iter().map(|[old, new]| (place(new), place(old))).collect(),
    }
  }

  /// Whether the alignment says nothing else of the two places: it put neither at the place of another name
  fn allows(&self, old: Position, new: Position) -> bool {
    let (old, new) = ((old.line, old.column), (new.line, new.column));
    self.new_of_old.get(&old).is_none_or(|aligned| *aligned == new) && self.old_of_new.get(&new).is_none_or(|aligned| *aligned == old)
  }
}

/// What two bindings that hold are
enum Confirmed {
  Marked(Found),
  /// Every name of the spelling changed, and the file declares none of them
  Undeclared(Rename),
}

/// Whether the names `old` binds in the old text and the names `new` binds in the new one are one binding under two
/// names, and what the rename is then; `None` when the two are not the same places of the texts, or another name
/// for either changes a binding of its text
fn confirm(old: &NameAt, new: &NameAt, tokens: [&Tokens; 2], blocks: &[DiffBlock], aligned: &Aligned) -> Option<Confirmed> {
  let count = old.uses.len();
  if count == 0 || new.uses.len() != count || old.name == new.name {
    return None;
  }
  let (local, proven) = match (old.local, new.local) {
    (Some(old_local), Some(new_local)) => {
      let binds_alike = |name: &NameAt| name.clashes.as_ref().is_some_and(Vec::is_empty);
      if old_local.kind != new_local.kind || !binds_alike(old) || !binds_alike(new) {
        return None;
      }
      (true, old_local.is_of_body_alone() && new_local.is_of_body_alone())
    }
    (None, None) => (false, false),
    _ => return None,
  };
  let mut rename = Rename {
    old: old.name.clone(),
    new: new.name.clone(),
    sites: u32::try_from(count).ok()?,
    local,
    ..Rename::default()
  };
  let mut places = Vec::with_capacity(count);
  // the spelling of the name in each text: every place that is read as one has the same
  let mut spelled: Option<[u64; 2]> = None;
  for (index, (old_use, new_use)) in old.uses.iter().zip(&new.uses).enumerate() {
    let declares = old_use.access == Access::Declaration;
    if declares != (new_use.access == Access::Declaration) || !in_one_block(blocks, old_use.start.line, new_use.start.line) || !aligned.allows(old_use.start, new_use.start) {
      return None;
    }
    if declares {
      rename.declared.push(u32::try_from(index).ok()?);
    }
    rename.old_sites.extend([old_use.start.line, old_use.start.column, old_use.end_column]);
    rename.new_sites.extend([new_use.start.line, new_use.start.column, new_use.end_column]);
    // the declaration of a name other files may see is always read; so is a place that makes more of the name
    let alone = old_use.site == Site::Plain && new_use.site == Site::Plain && (local || !declares);
    places.push(if alone {
      let found = [name_token(tokens[0], old_use)?, name_token(tokens[1], new_use)?];
      let texts = [tokens[0].all()[found[0]].text, tokens[1].all()[found[1]].text];
      if texts[0] == texts[1] || *spelled.get_or_insert(texts) != texts {
        return None;
      }
      Some(found)
    } else {
      None
    });
  }
  if rename.declared.is_empty() {
    return (!local).then_some(Confirmed::Undeclared(rename));
  }
  Some(Confirmed::Marked(Found { rename, proven, places }))
}

/// The place in `Tokens::all` of the token that is the name `used`: a name that starts and ends where it does
fn name_token(tokens: &Tokens, used: &NameUse) -> Option<usize> {
  let place = tokens.place_at(used.start.line, used.start.column)?;
  let token = &tokens.all()[place];
  (token.class == TokenClass::Name && token.end_line == token.line && token.end == used.end_column).then_some(place)
}

/// Whether one block that is not cut off holds the old line `old` and the new line `new`
fn in_one_block(blocks: &[DiffBlock], old: u32, new: u32) -> bool {
  blocks
    .get(blocks.partition_point(|block| block.old_end <= old))
    .is_some_and(|block| block.kind != BlockKind::Ignored && block.old_start <= old && block.new_start <= new && new < block.new_end)
}

/// The text the tokens of place `place` of rename `rename` are read by on both sides: of that place alone, so no
/// other token is the same as either
const fn text_of_place(rename: usize, place: usize) -> u64 {
  // the finalizer of SplitMix64 over the two numbers
  let mut mixed = ((rename as u64) << 32 ^ place as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
  mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
  mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
  mixed ^ (mixed >> 31)
}

impl Renames {
  /// The two texts with the tokens of each place that is a rename and nothing else read as one
  fn read_as_one(&self, tokens: [&Tokens; 2]) -> [Tokens; 2] {
    let side = |side: usize| {
      tokens[side].with_texts(self.found.iter().enumerate().flat_map(|(rename, found)| {
        found
          .places
          .iter()
          .enumerate()
          .filter_map(move |(place, places)| Some(((*places)?[side], text_of_place(rename, place))))
      }))
    };
    [side(0), side(1)]
  }

  /// Reads the blocks that hold a place of a rename again, with the two names of each place read as one: `runs` are
  /// the runs of each block of `blocks` from its first reading, `tokens` the tokens of the two texts, `has_text` says
  /// whether lines of a side hold a character that is no blank, and `work` is what the first reading of the blocks
  /// left of the file's (`runs_of_block`): a block it does not pay for keeps the runs it has.
  ///
  /// Lines that are the same tokens then and hold a place are a run of the kind `Renamed`, proven when every place in
  /// it is; the lines of the same tokens beside them that hold none are the run of formatting they are without the
  /// rename, and lines that hold a place and differ in their comments are no run. The reading adds runs and takes
  /// none: a run the first reading found is in the runs after it (`with_the_first`)
  pub(crate) fn cut(&self, blocks: &[DiffBlock], tokens: [&Tokens; 2], has_text: &dyn Fn(bool, Range<u32>) -> bool, work: &mut u64, runs: &mut [Vec<LineRun>]) {
    // the old line of each place that is a rename and nothing else, and whether it is proven
    let mut places: Vec<(u32, bool)> = self
      .found
      .iter()
      .flat_map(|found| found.places.iter().flatten().map(|places| (tokens[0].all()[places[0]].line, found.proven)))
      .collect();
    if places.is_empty() {
      return;
    }
    places.sort_unstable();
    let places_in = |lines: Range<u32>| &places[places.partition_point(|place| place.0 < lines.start)..places.partition_point(|place| place.0 < lines.end)];
    let asked = Asked {
      has_text,
      renamed: &|lines| !places_in(lines).is_empty(),
    };
    let [old, new] = self.read_as_one(tokens);
    for (block, runs) in blocks.iter().zip(runs) {
      if places_in(block.old_start..block.old_end).is_empty() {
        continue;
      }
      let lines = BlockLines {
        old: block.old_start..block.old_end,
        new: block.new_start..block.new_end,
      };
      // over a limit of work: what the block has stays, none of it a run of a rename
      let Some(mut second) = runs_of_block(&old, &new, &lines, &asked, work, &mut |_, _| {}) else {
        continue;
      };
      for run in second.iter_mut().filter(|run| run.kind == RunKind::Renamed) {
        run.quiet = places_in(run.old_start..run.old_end).iter().all(|place| place.1);
      }
      *runs = with_the_first(std::mem::take(runs), second);
    }
  }
}

/// The runs of a block after its two readings, in the order of the lines: those of the second reading (`second`,
/// with the names of a rename read as one), and of the first reading (`first`) every run the second does not hold
/// in a run of its kind, in place of the runs of the second that share a line with it or stand before it on one side
/// and after it on the other. The two readings align the tokens of the block each by itself, and put the same lines
/// beside other lines where a line stands twice: what the first reading proved of the texts as they are stays, so a
/// rename never takes the quiet of a line that changed in its formatting alone. The runs of each reading stand in
/// the order of both sides, so the work is of their number, not of its square
pub(crate) fn with_the_first(first: Vec<LineRun>, mut second: Vec<LineRun>) -> Vec<LineRun> {
  let holds = |held: &LineRun, run: &LineRun| {
    held.kind == run.kind && held.old_start <= run.old_start && run.old_end <= held.old_end && held.new_start <= run.new_start && run.new_end <= held.new_end
  };
  // `other` ends before `run` starts, on both sides
  let follows = |run: &LineRun, other: &LineRun| other.old_end <= run.old_start && other.new_end <= run.new_start;
  second.retain(|held| {
    // the runs of the first reading that neither end before this one nor start after it: a row of them
    let before = first.partition_point(|run| follows(held, run));
    let beside = first[before..].partition_point(|run| !follows(run, held));
    first[before..before + beside].iter().all(|run| holds(held, run))
  });
  // the runs that are left share no line, so only the last one that starts at or before a run can hold it
  let kept: Vec<LineRun> = first
    .into_iter()
    .filter(|run| {
      let after = second.partition_point(|held| held.old_start <= run.old_start);
      !after.checked_sub(1).is_some_and(|last| holds(&second[last], run))
    })
    .collect();
  second.extend(kept);
  second.sort_unstable_by_key(|run| run.old_start);
  second
}
