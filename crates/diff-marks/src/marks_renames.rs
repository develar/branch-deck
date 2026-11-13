//! The renames of a diff (openspec/specs/commit/quiet-changes/spec.md, "Renames"; docs/design/diff-view.md, "Marks"): the
//! names a change renamed within a file, and the runs of the lines that changed in such a name alone (`marks.rs`).
//!
//! 1. **Candidates, from the tokens alone.** Where the alignment of the tokens of a block put a name of the new side
//!    at the place of a name of the old one (`marks_runs`), the two are a candidate when they stand alike in the tree
//!    (`Token::shape`), are spelled otherwise and are each one plain word (`words`). A block whose sides hold the same
//!    words has none (`may_hold_rename`), which is also the question the block is asked before the tokens are read.
//!    A file without a candidate is not parsed for its names.
//! 2. **The bindings, from one parse of each side** (`bindings::Bindings::names_at_within`, openspec/specs/commit/name-bindings/spec.md):
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
//!    that knows the other files (`marks_subject`).
//! 4. **The blocks are read again with the two names of a place read as one** (`Renames::cut`): the tokens of each
//!    place get one text on both sides (`Tokens::with_texts`), a text of that place alone, and the blocks that hold
//!    one are read as before (`runs_of_block`). Lines that are then the same tokens and hold a place are a run of
//!    the kind `Renamed`: the rename and nothing else, however the lines were wrapped. A place on a line with
//!    another change is in no run, and that line is read. Lines of the same tokens that hold no place are a run of
//!    formatting as they are without the rename, never lines of the rename beside them; and a run the first reading
//!    found stays where the second aligned its lines otherwise (`with_the_first`): the second reading adds runs and
//!    takes none.
//!    **And once more for the names the file declares none of** (`Renames::pending`): the blocks that hold a place of
//!    one, with those names read as one too. No run of that reading is a run of the pair: those of the kind `Renamed`
//!    that lean on such a name wait, each with the names it leans on, for the reader that knows whether another file
//!    of the subject declares them, which then needs no text (`leftovers`); the others are dropped, since a run of
//!    formatting that a name read as one made is a quiet run nothing of the pair proves.
//! 5. **A name bound inside a body is a rename only where a line says so** (`Renames::keep_the_named`): one of its
//!    places lies in a run of the kind `Renamed`, the same tokens but for the name. Two bindings whose every line
//!    changed otherwise too are two variables, one in the place of the other: nothing of them is marked, and the
//!    blocks are read again without them. Measured on real commits: half of the local renames the places alone
//!    made were such pairs (docs/design/diff-view.md, "Measured on real history").
//! 6. **A run is proven** (`Renames::prove`) when it holds only places of proven renames: a local value of its body
//!    alone, every name of which the scopes bind (`bindings::Local::of_body_alone`: a value no code outside its body
//!    may name, in a text the scopes read whole), in a member nothing else of which changed (`Found::code`: the
//!    member around the name with the heads of the types that hold it, the whole text for a name without a
//!    member, the bindings of its reader: `code_symbols::navigate`, "The statements of a name"). No line of that code
//!    changed in anything that says what a name is: each is in a run of formatting or of comments, in a run of the
//!    kind `Renamed` of proven renames, on a line that changed in the name its own declaration is known by alone,
//!    or holds no token of code (a blank line inside a string of several lines is a line of that string, and
//!    code). So the function that declares the name, gives it its value and reads it is the same code but for names
//!    that are the same variables, and the new name is the old variable. A declaration that changed otherwise, a
//!    lambda given to another call, a parameter of another type, a statement that writes a variable the name is
//!    made of: each may bind another value, and the lines that read the name, the same tokens but for it, are
//!    labels. What a local holds comes from the code of its function, and no part of that code is known to give it
//!    nothing: each review of a narrower rule (the lines between the places, the statements of the places) found a
//!    value given outside it. A place that makes more of the name than its binding (a shorthand property, a
//!    destructuring, a name only the types tell apart, a macro, a name that is a value where the code runs: any
//!    `Site` but `Plain`) and the declaration of a name that is not local never get the one text, so their lines
//!    are in no run and are read, and a rename with such a place is never proven. Whether a proven run is quiet
//!    `marks::is_quiet` says of its kind, which a measurement switched on (`marks_of`).
//! 7. **The name a declaration is known by** (`Renames::with_declarations`). The declaration of a name that is not
//!    local is on no line of a run of the marks. The proofs read the blocks that hold one once more, with the
//!    names of those declarations as one too: a line that changed in such a name alone says nothing of the locals
//!    of its member (a function that got another name with a local of its body), and is the same code of its own
//!    declaration. No run of that reading is a run of the marks.
//! 8. **A name that is not local is declared by the same code** (`Renames::read_declarations`, `Found::redeclared`)
//!    when no line of the statements of its declarations, each with its bodies but the members of a type, changed
//!    in anything but formatting, comments, this name and the locals the scopes prove. Only of such a name may the
//!    index prove the uses (`marks_subject`): a constant that got another value with its other name is another
//!    constant, and a line that reads it does something else.
//!
//! Nothing here is a rule of characters: a file of no grammar has no rename.

use crate::bindings::{Access, Bindings, NameAt, NameQuestion, NameUse, Position, Site};
use crate::leftovers::{PendingBlock, PendingRun};
use crate::marks::MAX_RENAME_WORK;
use crate::marks_runs::{Asked, BlockLines, runs_of_block};
use code_tokens::Grammar;
use code_tokens::{Token, TokenClass, Tokens};
use line_diff::model::to_u32;
use line_diff::model::{BlockKind, DiffBlock, LineRun, Rename, RunKind};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

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
pub fn may_hold_rename(old: &str, new: &str) -> bool {
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
pub struct Candidates {
  /// In the order of the texts, each the token of the old text and the one of the new text
  pairs: Vec<[Token; 2]>,
}

impl Candidates {
  /// Takes two tokens the alignment of a block put at one place when they are two names that stand alike in the
  /// tree and are spelled otherwise
  pub fn take(&mut self, old: &Token, new: &Token) {
    if old.class == TokenClass::Name && new.class == TokenClass::Name && old.shape() == new.shape() && old.text != new.text {
      self.pairs.push([*old, *new]);
    }
  }

  pub const fn is_empty(&self) -> bool {
    self.pairs.is_empty()
  }
}

/// A rename the diff marks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
  pub rename: Rename,
  /// Its places are proven to change nothing: a value of its body alone, every name of which the scopes bind
  pub proven: bool,
  /// For each place that is the rename and nothing else, its tokens in the two texts (their places in
  /// `Tokens::all`), which are read as one; `None` for a place that is read
  places: Vec<Option<[usize; 2]>>,
  /// The lines of each side, the old one first, whose code says what the name is: the member around the uses of
  /// a local, the statements of the declarations of any other name (`NameAt::statements`); every line where the
  /// scopes gave none. From 0, in order
  code: [Vec<Range<u32>>; 2],
  /// Of a name that is not local: the tokens of each of its declarations in the two texts, which
  /// `read_declarations` reads as one; `None` where a declaration makes more of the name than its binding
  declarations: Option<Vec<[usize; 2]>>,
  /// Of a name that is not local: the code that declares it changed in more than names (`read_declarations`).
  /// Never of a local
  pub redeclared: bool,
}

/// The renames of a pair of texts
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Renames {
  /// The renames the diff marks, by their first place
  pub found: Vec<Found>,
  /// The names that changed at every place of the file and are declared in none of them, by their first place: the
  /// diff marks nothing of them. Never proven: only the file that declares a name says what was renamed
  pub undeclared: Vec<Found>,
  /// The resolutions of a name each text took, of those the work paid for
  pub resolutions: [usize; 2],
}

/// The renames among `candidates` of two texts of `grammar` with the tokens `tokens` and the blocks of their diff
pub fn renames_of(grammar: Grammar, texts: [&str; 2], tokens: [&Tokens; 2], blocks: &[DiffBlock], candidates: &Candidates, bindings: &dyn Bindings) -> Renames {
  renames_within(grammar, texts, tokens, blocks, candidates, MAX_RENAME_WORK, bindings)
}

/// `renames_of` within `work` in place of `MAX_RENAME_WORK`
pub fn renames_within(grammar: Grammar, texts: [&str; 2], tokens: [&Tokens; 2], blocks: &[DiffBlock], candidates: &Candidates, work: u64, bindings: &dyn Bindings) -> Renames {
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
  let cost = |side: usize| bindings.resolution_work(grammar, tokens[side].all().len()).max(1);
  let paid = usize::try_from(work / (cost(0) + cost(1))).unwrap_or(usize::MAX);
  let (old_names, new_names) = (
    bindings.names_at_within(grammar, texts[0], &questions(0), paid),
    bindings.names_at_within(grammar, texts[1], &questions(1), paid),
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
      Some(Confirmed::Undeclared(found)) => renames.undeclared.push(found),
      None => {}
    }
  }
  let first_place = |rename: &Rename| (rename.old_sites.first().copied(), rename.old_sites.get(1).copied());
  renames.found.sort_by_key(|found| first_place(&found.rename));
  renames.undeclared.sort_by_key(|found| first_place(&found.rename));
  // a token is a place of one rename: renames that share one are told apart by nothing here, and none of them holds
  let mut holders: [HashMap<usize, u32>; 2] = [HashMap::new(), HashMap::new()];
  hold(&mut holders, &renames.found);
  renames.found.retain(|found| holds_alone(&holders, found));
  // and of one name the file declares none of, beside every rename found: its places are read as one beside theirs
  hold(&mut holders, &renames.undeclared);
  renames.undeclared.retain(|found| holds_alone(&holders, found));
  renames
}

/// Counts, for each token of each text, the renames of `renames` it is a place of
fn hold(holders: &mut [HashMap<usize, u32>; 2], renames: &[Found]) {
  for places in renames.iter().flat_map(|found| found.places.iter().flatten()) {
    *holders[0].entry(places[0]).or_default() += 1;
    *holders[1].entry(places[1]).or_default() += 1;
  }
}

/// Whether no token of a place of `found` is a place of another rename counted in `holders`
fn holds_alone(holders: &[HashMap<usize, u32>; 2], found: &Found) -> bool {
  found.places.iter().flatten().all(|places| holders[0][&places[0]] == 1 && holders[1][&places[1]] == 1)
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
  Undeclared(Found),
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
      let binds_alike = |name: &NameAt| name.clashes == Some(0);
      if old_local.kind != new_local.kind || !binds_alike(old) || !binds_alike(new) {
        return None;
      }
      (true, old_local.of_body_alone && new_local.of_body_alone)
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
  let mut declarations = (!local).then(Vec::new);
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
    let plain = old_use.site == Site::Plain && new_use.site == Site::Plain;
    let alone = plain && (local || !declares);
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
    if declares && !local {
      // what `read_declarations` reads as one; a declaration that cannot be is one that changed otherwise
      let token = plain.then(|| Some([name_token(tokens[0], old_use)?, name_token(tokens[1], new_use)?])).flatten();
      declarations = declarations.zip(token).map(|(mut declarations, token)| {
        declarations.push(token);
        declarations
      });
    }
  }
  // where the scopes gave no statement, nothing says where the code of the name ends
  let code = |name: &NameAt| name.statements.clone().unwrap_or_else(|| std::iter::once(0..u32::MAX).collect());
  let found = Found {
    rename,
    proven,
    places,
    code: [code(old), code(new)],
    redeclared: !local && declarations.is_none(),
    declarations,
  };
  if found.rename.declared.is_empty() {
    return (!local).then_some(Confirmed::Undeclared(found));
  }
  Some(Confirmed::Marked(found))
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
  /// The renames whose places a reading reads as one: those the diff marks, and with `undeclared` the names the
  /// file declares none of after them
  fn read(&self, undeclared: bool) -> impl Iterator<Item = &Found> {
    self.found.iter().chain(self.undeclared.iter().filter(move |_| undeclared))
  }

  /// The two texts with the tokens of each place that is a rename and nothing else read as one; with `undeclared`
  /// also those of the names the file declares none of
  fn read_as_one(&self, tokens: [&Tokens; 2], undeclared: bool) -> [Tokens; 2] {
    let side = |side: usize| {
      tokens[side].with_texts(self.read(undeclared).enumerate().flat_map(|(rename, found)| {
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
  pub fn cut(&self, blocks: &[DiffBlock], tokens: [&Tokens; 2], has_text: &dyn Fn(bool, Range<u32>) -> bool, work: &mut u64, runs: &mut [Vec<LineRun>]) {
    self.read_again(false, blocks, tokens, has_text, work, &mut |block, second, _| {
      runs[block] = with_the_first(std::mem::take(&mut runs[block]), second);
    });
  }

  /// Reads the blocks that hold a place of a name the file declares none of once more, with those names read as one
  /// too, beside the renames of the diff (`tokens`, `has_text` and `work` as for `cut`): of the runs of each such
  /// block, those of the kind `Renamed` that lean on such a name, each with the names it leans on. It leans on a
  /// name that has a place in its lines, and, where it reaches the end of its block, on the name whose place is the
  /// first token of the code after the block: a run is a run there only while the code after the block reads the
  /// same (`same_after`), and this reading made the two names of that place one. Never proven here: only the index
  /// proves a name another file declares.
  ///
  /// No other run of the reading is kept. A run of formatting or of comments holds no place, so it differs from the
  /// runs of the pair only where a name read as one let it reach an end of its block; it would be a quiet run that
  /// leans on a name, added once the files of the subject are put together, after the rows of the file were drawn.
  /// A block the work does not pay for, and one without such a run, is left out: the runs of the pair say all there
  /// is of it
  pub fn pending(&self, blocks: &[DiffBlock], tokens: [&Tokens; 2], has_text: &dyn Fn(bool, Range<u32>) -> bool, work: &mut u64) -> Vec<PendingBlock> {
    // the name of each place of such a name, by its token in the old text
    let named: HashMap<usize, u32> = (0u32..)
      .zip(&self.undeclared)
      .flat_map(|(name, found)| found.places.iter().flatten().map(move |place| (place[0], name)))
      .collect();
    let mut pending = Vec::new();
    self.read_again(true, blocks, tokens, has_text, work, &mut |block, read, places| {
      let lines = &blocks[block];
      // the name the code after the block reads the same by
      let after = {
        let end = tokens[0].places_in_lines(lines.old_start..lines.old_end).end;
        let first = tokens[0].all()[end..].iter().position(|token| !token.is_comment());
        first.and_then(|first| named.get(&(end + first)).copied())
      };
      let runs: Vec<PendingRun> = read
        .into_iter()
        .filter(|run| run.kind == RunKind::Renamed)
        .filter_map(|run| {
          let mut names: Vec<u32> = places.in_lines(run.old_start..run.old_end).iter().filter_map(|place| place.name).collect();
          if run.old_end == lines.old_end && run.new_end == lines.new_end {
            names.extend(after);
          }
          names.sort_unstable();
          names.dedup();
          (!names.is_empty()).then_some(PendingRun {
            run: LineRun { quiet: false, ..run },
            names,
          })
        })
        .collect();
      if !runs.is_empty() {
        pending.push(PendingBlock { block: to_u32(block), runs });
      }
    });
    pending
  }

  /// Takes out the renames of a name bound inside a body none of whose places changed in the name alone: `runs` are
  /// the runs of each block after `cut`, `old` the tokens of the old text. Two bindings with as many names at the
  /// same places are one variable under two names only where a line says so: the same tokens but for the name. Where
  /// every line of them changed otherwise too (`val isRoot = …` and `val nameComponent = …`, each read where the
  /// other was), one variable took the place of another, and nothing is called a rename. Whether one was taken out:
  /// the blocks are then read again without it, since a run may lean on a name read as one beside it
  pub fn keep_the_named(&mut self, runs: &[Vec<LineRun>], old: &Tokens) -> bool {
    let renamed = RenamedRuns::of(runs);
    let before = self.found.len();
    self
      .found
      .retain(|found| !found.rename.local || found.places.iter().flatten().any(|place| renamed.at(old.all()[place[0]].line).is_some()));
    self.found.len() != before
  }

  /// The blocks that hold the declaration of a name that is not local, read once more with the names of those
  /// declarations read as one too, beside every other place of a rename (`tokens`, `has_text` and `work` as for
  /// `cut`). In the runs of the marks such a declaration is on no line of a run: it is read. This reading says
  /// whether its line changed in the name alone, which the proofs ask (`prove`, `read_declarations`); no run of it
  /// is a run of the marks
  pub fn with_declarations(&self, blocks: &[DiffBlock], tokens: [&Tokens; 2], has_text: &dyn Fn(bool, Range<u32>) -> bool, work: &mut u64) -> DeclarationsRead {
    let mut read = DeclarationsRead::default();
    fn declared(found: &Found) -> impl Iterator<Item = &[usize; 2]> {
      found.declarations.iter().flatten()
    }
    if !self.found.iter().any(|found| declared(found).next().is_some()) {
      return read;
    }
    // the tokens with every place and every declaration read as one
    let as_one = |side: usize| {
      let places = self.found.iter().enumerate().flat_map(|(rename, found)| {
        let declarations = declared(found).enumerate().map(move |(place, tokens)| (tokens[side], found.places.len() + place));
        let used = found.places.iter().enumerate().filter_map(move |(place, tokens)| Some(((*tokens)?[side], place)));
        used.chain(declarations).map(move |(token, place)| (token, text_of_place(rename, place)))
      });
      tokens[side].with_texts(places)
    };
    let [old, new] = [as_one(0), as_one(1)];
    let line_of = |place: &[usize; 2]| tokens[0].all()[place[0]].line;
    // the old lines that hold a name read as one, and those of the declarations among them
    let mut named: Vec<u32> = self
      .found
      .iter()
      .flat_map(|found| found.places.iter().flatten().chain(declared(found)))
      .map(line_of)
      .collect();
    named.sort_unstable();
    let holds_name = |lines: Range<u32>| named.partition_point(|line| *line < lines.start) < named.partition_point(|line| *line < lines.end);
    let asked = Asked { has_text, renamed: &holds_name };
    for line in self.found.iter().flat_map(declared).map(line_of) {
      let index = blocks.partition_point(|block| block.old_end <= line);
      let Some(block) = blocks.get(index).filter(|block| block.kind != BlockKind::Ignored && block.old_start <= line) else {
        continue;
      };
      read.blocks.entry(index).or_insert_with(|| {
        let lines = BlockLines {
          old: block.old_start..block.old_end,
          new: block.new_start..block.new_end,
        };
        runs_of_block(&old, &new, &lines, &asked, work, &mut |_, _| {})
      });
    }
    read
  }

  /// The old lines of the places a reading of the marks reads as one, each with the number of its rename, in
  /// their order: the declaration of a name that is not local is none of them
  fn place_lines(&self, old: &Tokens) -> Vec<(u32, usize)> {
    let places = self.found.iter().enumerate();
    let mut lines: Vec<(u32, usize)> = places
      .flat_map(|(number, found)| found.places.iter().flatten().map(move |place| (old.all()[place[0]].line, number)))
      .collect();
    lines.sort_unstable();
    lines
  }

  /// Says which renames are proven, and makes the runs of the kind `Renamed` among `runs` (those of each block of
  /// `blocks` after `cut`) quiet where every place in them is of a proven one; `tokens` are those of the two texts,
  /// `declarations` the blocks read with the declared names as one (`with_declarations`).
  ///
  /// A rename is proven when its name is a value of its body alone (`Found::proven` as `confirm` left it) and no
  /// line of its code changed in anything that says what a name is (`Found::code`: the member around it; on both
  /// sides): each is in a run of formatting or of comments, in a run of the kind `Renamed` whose places are all of
  /// proven renames, on a line that changed in the name some declaration is known by alone, or holds no token of
  /// code. So the function that declares the name, gives it its value and reads it is the same code but for names
  /// that are the same variables: the new name is the old variable, and a line that changed in it alone is the
  /// line it was. A declaration that got another initializer or another type, a lambda given to another call or
  /// to a call on another value, a parameter of another type, a statement that gives another value to a variable
  /// the name is made of: each binds another value under the new name, and a line that reads it does something
  /// else though its tokens are the same. Every run of such a rename is a label
  pub fn prove(&mut self, blocks: &[DiffBlock], tokens: [&Tokens; 2], runs: &mut [Vec<LineRun>], declarations: &DeclarationsRead) {
    let places = self.place_lines(tokens[0]);
    let in_run = |run: &LineRun| &places[places.partition_point(|place| place.0 < run.old_start)..places.partition_point(|place| place.0 < run.old_end)];
    // the lines of each rename that changed, by their block and their side
    let changed: Vec<Vec<Changed>> = self
      .found
      .iter()
      .map(|found| {
        let hull = found.rename.spans();
        let lines = |side: usize| found.code[side].iter().chain(std::iter::once(&hull[side]));
        changed_lines(blocks, [&mut lines(0), &mut lines(1)])
      })
      .collect();
    let mut proven: Vec<bool> = self.found.iter().map(|found| found.proven).collect();
    // a rename leans on those in the lines of its code: taken out one after another until none is left
    loop {
      // a run of either reading passes when the names read in it are all of proven renames; a name a declaration
      // is known by gives no value to what is declared
      let passes = |run: &LineRun| in_run(run).iter().all(|place| proven[place.1]);
      let quiet = |lines: &Changed| declarations.runs_of(lines.block, runs).is_some_and(|runs| lines.is_quiet(runs, tokens, &passes));
      let unproven = (0..proven.len()).find(|number| proven[*number] && !changed[*number].iter().all(quiet));
      let Some(unproven) = unproven else { break };
      proven[unproven] = false;
    }
    for (found, proven) in self.found.iter_mut().zip(&proven) {
      found.proven = *proven;
    }
    for run in runs.iter_mut().flatten().filter(|run| run.kind == RunKind::Renamed) {
      let held = in_run(run);
      run.quiet = !held.is_empty() && held.iter().all(|place| proven[place.1]);
    }
  }

  /// Says of each rename that is not local whether the code that declares the name changed in more than names
  /// (`Found::redeclared`): `runs` are the runs of each block of `blocks` after `prove`, `tokens` those of the two
  /// texts, `declarations` the blocks read with the declared names as one (`with_declarations`).
  ///
  /// The name is declared by the same code when no line of the statements of its declarations (`Found::code`)
  /// changed in anything that says what a name is: each is in a run of formatting or of comments, in a run of the
  /// kind `Renamed` that holds only places of this rename and of locals the scopes prove, or holds no token of
  /// code. A block the work did not pay for says nothing, and the name is declared anew
  pub fn read_declarations(&mut self, blocks: &[DiffBlock], tokens: [&Tokens; 2], runs: &[Vec<LineRun>], declarations: &DeclarationsRead) {
    let places = self.place_lines(tokens[0]);
    let in_run = |run: &LineRun| &places[places.partition_point(|place| place.0 < run.old_start)..places.partition_point(|place| place.0 < run.old_end)];
    let mut redeclared = Vec::new();
    for (number, found) in self.found.iter().enumerate().filter(|(_, found)| found.declarations.is_some()) {
      let passes = |run: &LineRun| in_run(run).iter().all(|place| place.1 == number || self.found[place.1].proven);
      let quiet = |lines: &Changed| declarations.runs_of(lines.block, runs).is_some_and(|runs| lines.is_quiet(runs, tokens, &passes));
      if !changed_lines(blocks, [&mut found.code[0].iter(), &mut found.code[1].iter()]).iter().all(quiet) {
        redeclared.push(number);
      }
    }
    for number in redeclared {
      self.found[number].redeclared = true;
    }
  }

  /// One more reading of the blocks: with the places of the renames of the diff read as one, and with `undeclared`
  /// those of the names the file declares none of too. A block is read when it holds a place of the kind the
  /// reading is for (of such a name with `undeclared`, of any rename without) and the work pays for it; `read` gets
  /// its number, its runs, a run of the kind `Renamed` proven when every place in it is, and the places
  fn read_again(
    &self,
    undeclared: bool,
    blocks: &[DiffBlock],
    tokens: [&Tokens; 2],
    has_text: &dyn Fn(bool, Range<u32>) -> bool,
    work: &mut u64,
    read: &mut dyn FnMut(usize, Vec<LineRun>, &Places),
  ) {
    let places = Places::of(self, tokens[0], undeclared);
    let wanted = |lines: Range<u32>| places.in_lines(lines).iter().any(|place| !undeclared || place.name.is_some());
    if !wanted(0..u32::MAX) {
      return;
    }
    let asked = Asked {
      has_text,
      renamed: &|lines| !places.in_lines(lines).is_empty(),
    };
    let [old, new] = self.read_as_one(tokens, undeclared);
    for (index, block) in blocks.iter().enumerate() {
      if !wanted(block.old_start..block.old_end) {
        continue;
      }
      let lines = BlockLines {
        old: block.old_start..block.old_end,
        new: block.new_start..block.new_end,
      };
      // over a limit of work: what the block has stays, none of it a run of a rename
      let Some(mut runs) = runs_of_block(&old, &new, &lines, &asked, work, &mut |_, _| {}) else {
        continue;
      };
      for run in runs.iter_mut().filter(|run| run.kind == RunKind::Renamed) {
        run.quiet = places.in_lines(run.old_start..run.old_end).iter().all(|place| place.proven);
      }
      read(index, runs, &places);
    }
  }
}

/// Lines of a side of a block that changed and belong to the code of a rename
#[derive(Debug, Clone)]
struct Changed {
  /// The block, by its number among the blocks of the diff
  block: usize,
  new_side: bool,
  lines: Range<u32>,
}

impl Changed {
  /// Whether the lines changed in nothing that says what a name is: each is in a run of `runs`, the runs of its
  /// block, of formatting, of comments (the same code beside them), or of the kind `Renamed` that `passes`, or is
  /// a line no token of code stands on: a blank line, a line of a comment. A comment is read where it stands and
  /// gives no name a value. A line inside a string of several lines has the token of that string, and is code
  fn is_quiet(&self, runs: &[LineRun], tokens: [&Tokens; 2], passes: &dyn Fn(&LineRun) -> bool) -> bool {
    let side = tokens[usize::from(self.new_side)];
    let blank = |lines: Range<u32>| lines.is_empty() || side.in_lines(lines).iter().all(Token::is_comment);
    let mut line = self.lines.start;
    for run in runs {
      let of_run = if self.new_side { run.new_start..run.new_end } else { run.old_start..run.old_end };
      if of_run.end <= line {
        continue;
      }
      if of_run.start >= self.lines.end {
        break;
      }
      // the lines before the run are in none: without code on this side, or read
      if !blank(line..of_run.start) {
        return false;
      }
      let passed = match run.kind {
        RunKind::Formatting => run.quiet,
        RunKind::Renamed => passes(run),
        RunKind::Comments => true,
      };
      if !passed {
        return false;
      }
      line = line.max(of_run.end);
    }
    line >= self.lines.end || blank(line..self.lines.end)
  }
}

/// The blocks of a diff read with the names of the declarations that are not local as one too
/// (`Renames::with_declarations`), by their numbers; `None` for a block the work did not pay for
#[derive(Debug, Default)]
pub struct DeclarationsRead {
  blocks: HashMap<usize, Option<Vec<LineRun>>>,
}

impl DeclarationsRead {
  /// The runs of the block `block` as the proofs read them: those of this reading where it read the block, else
  /// those of the marks (`runs`, the runs of each block); `None` for a block the work did not pay for
  fn runs_of<'a>(&'a self, block: usize, runs: &'a [Vec<LineRun>]) -> Option<&'a [LineRun]> {
    match self.blocks.get(&block) {
      Some(read) => read.as_deref(),
      None => runs.get(block).map(Vec::as_slice),
    }
  }
}

/// The lines of `lines` (rows of each side, the old one first, in any order) that stand in a block of `blocks` that
/// is not cut off: the lines of the code of a rename that changed
fn changed_lines(blocks: &[DiffBlock], lines: [&mut dyn Iterator<Item = &Range<u32>>; 2]) -> Vec<Changed> {
  let mut changed = Vec::new();
  for (new_side, rows) in [false, true].into_iter().zip(lines) {
    let of_block = |block: &DiffBlock| if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
    for row in rows {
      let from = blocks.partition_point(|block| of_block(block).end <= row.start);
      for (index, block) in blocks.iter().enumerate().skip(from).take_while(|(_, block)| of_block(block).start < row.end) {
        let within = of_block(block).start.max(row.start)..of_block(block).end.min(row.end);
        if block.kind != BlockKind::Ignored && !within.is_empty() {
          changed.push(Changed {
            block: index,
            new_side,
            lines: within,
          });
        }
      }
    }
  }
  changed
}

/// The old lines of the runs of the kind `Renamed` of a diff, in their order
struct RenamedRuns {
  lines: Vec<Range<u32>>,
}

impl RenamedRuns {
  /// Of `runs`, the runs of each block in the order of the blocks
  fn of(runs: &[Vec<LineRun>]) -> Self {
    Self {
      lines: runs
        .iter()
        .flatten()
        .filter(|run| run.kind == RunKind::Renamed)
        .map(|run| run.old_start..run.old_end)
        .collect(),
    }
  }

  /// The number of the run that holds the old line `line`
  fn at(&self, line: u32) -> Option<usize> {
    let at = self.lines.partition_point(|run| run.end <= line);
    self.lines.get(at).is_some_and(|run| run.start <= line).then_some(at)
  }
}

/// A place of a rename that a reading reads as one
struct Place {
  /// Its line in the old text
  line: u32,
  /// Its rename is proven; never a name the file declares none of
  proven: bool,
  /// The name it is of among those the file declares none of, by its number
  name: Option<u32>,
}

/// The places a reading reads as one, by their old lines
struct Places(Vec<Place>);

impl Places {
  /// The places of the renames of the diff that are a rename and nothing else, and with `undeclared` those of the
  /// names the file declares none of; `old` are the tokens of the old text
  fn of(renames: &Renames, old: &Tokens, undeclared: bool) -> Self {
    let mut places = Vec::new();
    for (index, found) in renames.read(undeclared).enumerate() {
      let name = index.checked_sub(renames.found.len()).map(to_u32);
      places.extend(found.places.iter().flatten().map(|place| Place {
        line: old.all()[place[0]].line,
        proven: found.proven && name.is_none(),
        name,
      }));
    }
    places.sort_unstable_by_key(|place| place.line);
    Self(places)
  }

  /// The places on `lines` of the old text
  fn in_lines(&self, lines: Range<u32>) -> &[Place] {
    let Self(places) = self;
    &places[places.partition_point(|place| place.line < lines.start)..places.partition_point(|place| place.line < lines.end)]
  }
}

/// The runs of a block after its two readings, in the order of the lines: those of the second reading (`second`,
/// with the names of a rename read as one), and of the first reading (`first`) every run the second does not hold
/// in a run of its kind, in place of the runs of the second that share a line with it or stand before it on one side
/// and after it on the other. The two readings align the tokens of the block each by itself, and put the same lines
/// beside other lines where a line stands twice: what the first reading proved of the texts as they are stays, so a
/// rename never takes the quiet of a line that changed in its formatting alone. The runs of each reading stand in
/// the order of both sides, so the work is of their number, not of its square
pub fn with_the_first(first: Vec<LineRun>, mut second: Vec<LineRun>) -> Vec<LineRun> {
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
