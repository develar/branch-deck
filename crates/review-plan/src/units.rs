//! The units of a review (spec/branch/review-units.spec.md): the changes of the pair a review shows (`pair`), each one a
//! reviewer reads once, in the order of the story.
//!
//! - **A block** of a file goes to one unit: the repeated edit of its file's group when it is that edit, else the
//!   imports or the whitespace when it changes only those, else the declaration that holds most of its changed lines
//!   (on the new side, on the old one for a removal alone; the innermost one, never a local), else a change outside
//!   any declaration. A file whose change is mechanical (`files::mechanical_kind`), added, without text or of no known
//!   declarations is one unit as a whole.
//! - **A declaration** is paired with itself across the two sides by its key and, among overloads, by an equal form,
//!   then in order: a pair whose form differs is `Shape`, one whose form is the same and whose text is not (without
//!   whitespace) `Behaviour`, one whose text is the same too whitespace; a declaration of one side alone, its lines
//!   changed, is `Added` or `Removed`, also when another unit holds its block.
//!   A declaration of one side alone inside another of that side alone is part of the unit of the outermost one.
//! - **A group** is one unit over its files: a kind of mechanical change, a repeated edit.
//! - **The order**: the files in the order of the plan (`order`), each unit at its first block; a repeated edit at the
//!   place of its first file; the mechanical units last.
//! - **Identity**: `id` names a unit within a plan; `fingerprint` names the change across plans, from the key, the
//!   signature and the text of a declaration without whitespace on both sides, or the text of the blocks of a slice,
//!   or the blobs of a file taken whole. Neither depends on the whitespace policy of the window.

use crate::api::{FormChange, form_changes};
use crate::files::{ChangedFile, FileKinds, MechanicalKind, indentation_matters, lines_edit_key, without_whitespace};
use crate::pair::{LineBlock, PairBlocks};
use code_symbols::language::language_of;
use file_diffs::service::imports_only;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::ops::Range;
use trial_check::symbol_dependencies::declaration_checks::{FileDeclarations, SpannedDeclaration, spanned_declarations};
use trial_check::symbol_scan::SymbolInput;

/// What a unit changes
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UnitKind {
  /// The form of a declaration, as a use sees it: these parts of it
  Shape { changes: Vec<FormChange> },
  /// The body of a declaration alone: its form is the same
  Behaviour,
  /// A declaration the commits add to a file that was there
  Added,
  /// A declaration the commits remove from a file that stays
  Removed,
  /// A file the commits add
  NewFile,
  /// A change the review folds: of one kind, over every file that has it
  Mechanical { mechanical: MechanicalKind },
  /// The same edit in 3 or more files: the group of `ReviewPlan::repeated_edits` it is
  RepeatedEdit { group: u32 },
  /// A change no declaration holds, or of a file whose declarations are not known
  Other { reason: OtherReason },
}

/// Why a change is of no declaration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum OtherReason {
  /// Lines outside any declaration of a source file
  Outside,
  /// A file of no language the review parses
  NoLanguage,
  /// A source file whose summary is missing, degraded or truncated on a side
  Unparsed,
  /// Binary, over 8 MiB, or unreadable
  NoText,
  /// Only the line ends changed, in a file where the indentation has a meaning
  LineEnds,
  /// Its type changed: a symlink, a submodule
  TypeChanged,
}

/// A block of the diff of a file, as the diffs of the pair under `Default` have it: its place there, its lines on each
/// side from 0, the end excluded
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitBlock {
  pub index: u32,
  pub old_start: u32,
  pub old_end: u32,
  pub new_start: u32,
  pub new_end: u32,
}

/// What a unit changes in one file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnitSlice {
  /// The path after the commits; before them for a deleted file
  pub path: String,
  /// The path before the first commit that changes it, when the commits rename it
  pub old_path: Option<String>,
  /// The unit takes the file whole: all of its blocks, or a file that has none (added, deleted, renamed, binary)
  pub whole: bool,
  /// Its blocks, in order: every block of the file for `whole`; none for a declaration whose block another unit holds
  pub blocks: Vec<UnitBlock>,
  /// The change this slice makes, by its text without whitespace or its blobs (`units`, "Identity")
  pub fingerprint: String,
}

/// A change a reviewer reads once
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewUnit {
  /// Names it in the plan: of a declaration its file, key and signature
  pub id: String,
  pub kind: UnitKind,
  /// Of a declaration: its name; of a new file or a change of no declaration: the name of its file
  pub name: Option<String>,
  /// Of a declaration: namespace, containers and name, as `ApiChange::key` (a public one has its entry by key and path)
  pub key: Option<String>,
  /// Of a declaration: what tells it from an overload of its key
  pub signature: Option<String>,
  /// Of a declaration: another module may name it
  pub public: bool,
  /// Of a declaration: the line of its name after the commits, before them for a removal, from 1
  pub line: Option<u32>,
  /// One per file, in the order of the files; of a group, every file of it
  pub slices: Vec<UnitSlice>,
  /// The change across plans: the slice's, or of a group the set of its slices' (`units`, "Identity")
  pub fingerprint: String,
}

/// The order of the mechanical units, as the tree of the review lists the kinds: the generated files last, which a
/// reviewer skips whole
const MECHANICAL_ORDER: [MechanicalKind; 6] = [
  MechanicalKind::BuildModel,
  MechanicalKind::ImportsOnly,
  MechanicalKind::Whitespace,
  MechanicalKind::Renamed,
  MechanicalKind::Deleted,
  MechanicalKind::Generated,
];

/// A stable digest of `parts`: the first 16 bytes of their SHA-256, in hex, each part ended by a NUL
fn digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
  let mut hasher = Sha256::new();
  for part in parts {
    hasher.update(part.as_bytes());
    hasher.update([0]);
  }
  hasher.finalize()[..16].iter().fold(String::with_capacity(32), |mut hex, byte| {
    let _ = write!(hex, "{byte:02x}");
    hex
  })
}

/// The text of `lines` (by line from 0, the end excluded) of `text`, without whitespace
fn text_of(text: &[&str], lines: Range<u32>) -> String {
  let (start, end) = ((lines.start as usize).min(text.len()), (lines.end as usize).min(text.len()));
  without_whitespace(&text[start..end])
}

/// The lines of `declaration` from 0, the end excluded
const fn lines_of(declaration: &SpannedDeclaration) -> Range<u32> {
  declaration.start_line.saturating_sub(1)..declaration.end_line
}

fn unit_block(index: usize, block: &LineBlock) -> UnitBlock {
  UnitBlock {
    index: u32::try_from(index).unwrap_or(u32::MAX),
    old_start: block.old.start,
    old_end: block.old.end,
    new_start: block.new.start,
    new_end: block.new.end,
  }
}

/// The innermost declaration of `declarations` that holds most of `lines` (from 0), the first of a tie
fn owner_of(declarations: &[SpannedDeclaration], lines: Range<u32>) -> Option<usize> {
  let mut counts: BTreeMap<usize, (u32, u32)> = BTreeMap::new();
  for line in lines {
    let at = line + 1;
    let innermost = declarations
      .iter()
      .enumerate()
      .filter(|(_, declaration)| declaration.start_line <= at && at <= declaration.end_line)
      .min_by_key(|(_, declaration)| (declaration.end_line - declaration.start_line, std::cmp::Reverse(declaration.start_line)))
      .map(|(index, _)| index);
    if let Some(index) = innermost {
      let entry = counts.entry(index).or_insert((0, line));
      entry.0 += 1;
    }
  }
  counts
    .into_iter()
    .max_by(|(_, (a, first_a)), (_, (b, first_b))| a.cmp(b).then(first_b.cmp(first_a)))
    .map(|(index, _)| index)
}

/// A declaration across the two sides of a file: its place on each side it is on
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Paired {
  before: Option<usize>,
  after: Option<usize>,
}

/// The places of `declarations` by their key, in order
fn by_key(declarations: &[SpannedDeclaration]) -> BTreeMap<&str, Vec<usize>> {
  let mut keys: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
  for (index, declaration) in declarations.iter().enumerate() {
    keys.entry(declaration.form.key.as_str()).or_default().push(index);
  }
  keys
}

/// The declarations of the two sides paired: per key, an equal form first, then in order; each one once
fn pair_declarations(before: &[SpannedDeclaration], after: &[SpannedDeclaration]) -> Vec<Paired> {
  let (old_keys, mut new_keys) = (by_key(before), by_key(after));
  let mut pairs: Vec<Paired> = Vec::new();
  for (key, olds) in old_keys {
    let mut news = new_keys.remove(key).unwrap_or_default();
    let mut left: Vec<usize> = Vec::new();
    for old in olds {
      match news
        .iter()
        .position(|&new| after[new].form.form == before[old].form.form && after[new].signature == before[old].signature)
      {
        Some(position) => pairs.push(Paired {
          before: Some(old),
          after: Some(news.remove(position)),
        }),
        None => left.push(old),
      }
    }
    let mut news = news.into_iter();
    for old in left {
      pairs.push(Paired {
        before: Some(old),
        after: news.next(),
      });
    }
    pairs.extend(news.map(|new| Paired { before: None, after: Some(new) }));
  }
  pairs.extend(new_keys.into_values().flatten().map(|new| Paired { before: None, after: Some(new) }));
  pairs
}

/// Where a block of a source file goes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
  Repeated(u32),
  Mechanical(MechanicalKind),
  Declaration(usize),
  Outside,
}

/// A place in the story: the place of a file, then the block a unit starts at, then 1 after the unit of that block
type Place = (usize, usize, u8);

/// A unit and its place in the story
struct Placed {
  at: Place,
  unit: ReviewUnit,
}

/// The units being cut: those of one file each, and the slices of the groups
#[derive(Default)]
struct Cut {
  placed: Vec<Placed>,
  mechanical: BTreeMap<usize, Vec<UnitSlice>>,
  /// group -> its place in the story and its slices
  repeated: BTreeMap<u32, (Place, Vec<UnitSlice>)>,
}

/// One file of the pair, with what the units read of it
struct PairFile<'a> {
  file: &'a ChangedFile<'a>,
  /// Its place in the order of the plan
  at: usize,
  old_path: Option<String>,
}

impl PairFile<'_> {
  fn slice(&self, whole: bool, blocks: Vec<UnitBlock>, fingerprint: String) -> UnitSlice {
    UnitSlice {
      path: self.file.path.to_string(),
      old_path: self.old_path.clone(),
      whole,
      blocks,
      fingerprint,
    }
  }

  /// The slice of the whole file, its blocks when it has them, named by its blobs
  fn whole(&self, pair: &PairBlocks, tag: &str) -> UnitSlice {
    let blocks = match pair {
      PairBlocks::Blocks(blocks) => blocks.iter().enumerate().map(|(index, block)| unit_block(index, block)).collect(),
      _ => Vec::new(),
    };
    let fingerprint = digest([tag, self.file.path, self.file.before.unwrap_or_default(), self.file.after.unwrap_or_default()]);
    self.slice(true, blocks, fingerprint)
  }

  /// A unit of the whole file
  fn whole_unit(&self, pair: &PairBlocks, kind: UnitKind, tag: &str) -> Placed {
    let slice = self.whole(pair, tag);
    Placed {
      at: (self.at, 0, 0),
      unit: ReviewUnit {
        id: format!("{tag}:{}", self.file.path),
        kind,
        name: Some(file_name(self.file.path).to_string()),
        key: None,
        signature: None,
        public: false,
        line: None,
        fingerprint: slice.fingerprint.clone(),
        slices: vec![slice],
      },
    }
  }
}

fn file_name(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}

/// The units of the pair of `files` (`pair::pair_blocks`, by place), in the order of the story: their kinds (`kinds`),
/// the summaries of `input`, the hunk text `texts` (by blob) and the order of the plan's files `order`
pub(crate) fn review_units(
  input: &SymbolInput,
  files: &[ChangedFile<'_>],
  pairs: &[PairBlocks],
  texts: &HashMap<String, String>,
  kinds: &FileKinds,
  order: &[String],
) -> Vec<ReviewUnit> {
  let place: HashMap<&str, usize> = order.iter().enumerate().map(|(index, path)| (path.as_str(), index)).collect();
  // file -> its group of repeated edits and the edit
  let mut group_of: HashMap<usize, (u32, &(String, String))> = HashMap::new();
  for (group, edit) in kinds.edits.iter().enumerate() {
    for &member in &edit.members {
      group_of.insert(member, (u32::try_from(group).unwrap_or(u32::MAX), &edit.key));
    }
  }
  let mut cut = Cut::default();
  for (index, (file, pair)) in files.iter().zip(pairs).enumerate() {
    if file.before == file.after && file.first_path == file.path {
      continue;
    }
    let pair_file = PairFile {
      file,
      at: place.get(file.path).copied().unwrap_or(order.len() + index),
      old_path: (file.first_path != file.path).then(|| file.first_path.to_string()),
    };
    if let Some(kind) = kinds.mechanical[index] {
      let slice = pair_file.whole(pair, "mechanical");
      cut.mechanical.entry(mechanical_place(kind)).or_default().push(slice);
      continue;
    }
    let other = |reason: OtherReason| pair_file.whole_unit(pair, UnitKind::Other { reason }, "other");
    match pair {
      PairBlocks::Added => cut.placed.push(pair_file.whole_unit(pair, UnitKind::NewFile, "new")),
      PairBlocks::Deleted => {
        let slice = pair_file.whole(pair, "mechanical");
        cut.mechanical.entry(mechanical_place(MechanicalKind::Deleted)).or_default().push(slice);
      }
      PairBlocks::TypeChanged => cut.placed.push(other(OtherReason::TypeChanged)),
      PairBlocks::NoText => cut.placed.push(other(OtherReason::NoText)),
      PairBlocks::LineEnds => cut.placed.push(other(OtherReason::LineEnds)),
      PairBlocks::Blocks(blocks) => {
        let sides = (
          file.before.and_then(|blob| texts.get(blob)),
          file.after.and_then(|blob| texts.get(blob)),
          file.before.and_then(|blob| spanned_declarations(input, file.first_path, blob)),
          file.after.and_then(|blob| spanned_declarations(input, file.path, blob)),
        );
        match sides {
          _ if language_of(file.path).is_none() => cut.placed.push(other(OtherReason::NoLanguage)),
          (Some(old_text), Some(new_text), Some(before), Some(after)) => {
            let source = SourceFile {
              pair_file: &pair_file,
              old: old_text.split('\n').collect(),
              new: new_text.split('\n').collect(),
              before,
              after,
              repeated: group_of.get(&index).copied(),
            };
            source.cut(blocks, &mut cut);
          }
          _ => cut.placed.push(other(OtherReason::Unparsed)),
        }
      }
    }
  }
  let Cut { mut placed, mechanical, repeated } = cut;
  for (group, (at, slices)) in repeated {
    let fingerprint = group_fingerprint(&slices);
    placed.push(Placed {
      at,
      unit: ReviewUnit {
        id: format!("repeated:{fingerprint}"),
        kind: UnitKind::RepeatedEdit { group },
        name: None,
        key: None,
        signature: None,
        public: false,
        line: None,
        slices,
        fingerprint,
      },
    });
  }
  placed.sort_by_key(|placed| placed.at);
  let mut units: Vec<ReviewUnit> = placed.into_iter().map(|placed| placed.unit).collect();
  for (kind, mut slices) in mechanical {
    slices.sort_by_key(|slice| place.get(slice.path.as_str()).copied().unwrap_or(usize::MAX));
    let mechanical = MECHANICAL_ORDER[kind];
    let fingerprint = group_fingerprint(&slices);
    units.push(ReviewUnit {
      id: format!("mechanical:{}", mechanical_tag(mechanical)),
      kind: UnitKind::Mechanical { mechanical },
      name: None,
      key: None,
      signature: None,
      public: false,
      line: None,
      slices,
      fingerprint,
    });
  }
  debug_assert!(every_block_once(files, pairs, &units), "a block in no unit or in two: {units:#?}");
  units
}

/// Every block of every file of `pairs` is in exactly one of `units`, and every file of the pair in one at least
fn every_block_once(files: &[ChangedFile<'_>], pairs: &[PairBlocks], units: &[ReviewUnit]) -> bool {
  let mut seen: HashMap<(&str, u32), usize> = HashMap::new();
  let mut in_unit: HashMap<&str, bool> = HashMap::new();
  for slice in units.iter().flat_map(|unit| &unit.slices) {
    in_unit.insert(slice.path.as_str(), true);
    for block in &slice.blocks {
      *seen.entry((slice.path.as_str(), block.index)).or_default() += 1;
    }
  }
  files.iter().zip(pairs).all(|(file, pair)| {
    if file.before == file.after && file.first_path == file.path {
      return true;
    }
    let blocks = match pair {
      PairBlocks::Blocks(blocks) => blocks.len(),
      _ => 0,
    };
    in_unit.contains_key(file.path) && (0..blocks).all(|index| seen.get(&(file.path, u32::try_from(index).unwrap_or(u32::MAX))) == Some(&1))
  })
}

/// The place of `kind` in `MECHANICAL_ORDER`
fn mechanical_place(kind: MechanicalKind) -> usize {
  MECHANICAL_ORDER.iter().position(|known| *known == kind).unwrap_or(MECHANICAL_ORDER.len())
}

const fn mechanical_tag(kind: MechanicalKind) -> &'static str {
  match kind {
    MechanicalKind::BuildModel => "buildModel",
    MechanicalKind::ImportsOnly => "importsOnly",
    MechanicalKind::Whitespace => "whitespace",
    MechanicalKind::Renamed => "renamed",
    MechanicalKind::Deleted => "deleted",
    MechanicalKind::Generated => "generated",
  }
}

/// The fingerprint of a group: of the set of its slices'
fn group_fingerprint(slices: &[UnitSlice]) -> String {
  let mut parts: Vec<&str> = slices.iter().map(|slice| slice.fingerprint.as_str()).collect();
  parts.sort_unstable();
  digest(parts)
}

/// A source file of the pair whose two sides have text and known declarations
struct SourceFile<'a> {
  pair_file: &'a PairFile<'a>,
  old: Vec<&'a str>,
  new: Vec<&'a str>,
  before: FileDeclarations,
  after: FileDeclarations,
  /// Its group of repeated edits and the edit, when it is in one
  repeated: Option<(u32, &'a (String, String))>,
}

impl SourceFile<'_> {
  /// The fingerprint of `blocks` of this file: its path and their text without whitespace
  fn blocks_fingerprint(&self, tag: &str, blocks: &[UnitBlock]) -> String {
    let texts: Vec<String> = blocks
      .iter()
      .flat_map(|block| [text_of(&self.old, block.old_start..block.old_end), text_of(&self.new, block.new_start..block.new_end)])
      .collect();
    digest([tag, self.pair_file.file.path].into_iter().chain(texts.iter().map(String::as_str)))
  }

  /// Where `block` goes (`units`, "A block")
  fn owner(&self, block: &LineBlock, of_before: &HashMap<usize, usize>, of_after: &HashMap<usize, usize>, folded: &[usize]) -> Owner {
    let removed = &self.old[(block.old.start as usize).min(self.old.len())..(block.old.end as usize).min(self.old.len())];
    let added = &self.new[(block.new.start as usize).min(self.new.len())..(block.new.end as usize).min(self.new.len())];
    if let Some((group, key)) = self.repeated
      && lines_edit_key(removed, added).as_ref() == Some(key)
    {
      return Owner::Repeated(group);
    }
    let imports = [Some(self.before.import_lines.as_slice()), Some(self.after.import_lines.as_slice())];
    if imports_only([block.old.clone(), block.new.clone()], [&self.old, &self.new], imports) {
      return Owner::Mechanical(MechanicalKind::ImportsOnly);
    }
    if !indentation_matters(self.pair_file.file.path) && without_whitespace(removed) == without_whitespace(added) {
      return Owner::Mechanical(MechanicalKind::Whitespace);
    }
    let owner = if block.new.is_empty() {
      owner_of(&self.before.declarations, block.old.clone()).and_then(|index| of_before.get(&index))
    } else {
      owner_of(&self.after.declarations, block.new.clone()).and_then(|index| of_after.get(&index))
    };
    owner.map_or(Owner::Outside, |&pair| Owner::Declaration(folded[pair]))
  }

  /// Per pair, the pair whose unit it is part of: itself, or for a declaration of one side alone the outermost
  /// declaration around it that is of that side alone too (a member of a new type is part of the new type)
  fn folded(&self, pairs: &[Paired]) -> Vec<usize> {
    let keys = |declarations: &[SpannedDeclaration], side: fn(&Paired) -> Option<usize>| -> HashMap<String, usize> {
      let mut keys: HashMap<String, usize> = HashMap::new();
      for (pair, paired) in pairs.iter().enumerate() {
        if let Some(index) = side(paired) {
          keys.entry(declarations[index].form.key.clone()).or_insert(pair);
        }
      }
      keys
    };
    let (old_keys, new_keys) = (
      keys(&self.before.declarations, |paired| paired.before),
      keys(&self.after.declarations, |paired| paired.after),
    );
    let one_side = |pair: usize| pairs[pair].before.is_none() != pairs[pair].after.is_none();
    (0..pairs.len())
      .map(|pair| {
        let (declaration, keys) = match (pairs[pair].before, pairs[pair].after) {
          (None, Some(after)) => (&self.after.declarations[after], &new_keys),
          (Some(before), None) => (&self.before.declarations[before], &old_keys),
          _ => return pair,
        };
        let mut folded = pair;
        let mut key = declaration.form.key.as_str();
        while let Some((container, _)) = key.rsplit_once('.') {
          match keys.get(container) {
            Some(&outer) if one_side(outer) && pairs[outer].after.is_some() == pairs[pair].after.is_some() => folded = outer,
            Some(_) => break,
            None => {}
          }
          key = container;
        }
        folded
      })
      .collect()
  }

  /// Cut `blocks` into the units of this file and the slices of the groups
  fn cut(&self, blocks: &[LineBlock], cut: &mut Cut) {
    let pair_file = self.pair_file;
    let at = pair_file.at;
    let pairs = pair_declarations(&self.before.declarations, &self.after.declarations);
    let of_before: HashMap<usize, usize> = pairs.iter().enumerate().filter_map(|(pair, paired)| Some((paired.before?, pair))).collect();
    let of_after: HashMap<usize, usize> = pairs.iter().enumerate().filter_map(|(pair, paired)| Some((paired.after?, pair))).collect();
    let folded = self.folded(&pairs);

    // the blocks of each declaration, of each group, and the runs of blocks outside any declaration
    let mut declared: BTreeMap<usize, Vec<UnitBlock>> = BTreeMap::new();
    let mut repeated: BTreeMap<u32, Vec<UnitBlock>> = BTreeMap::new();
    let mut mechanical: BTreeMap<usize, Vec<UnitBlock>> = BTreeMap::new();
    let mut outside: Vec<Vec<UnitBlock>> = Vec::new();
    let mut last_outside: Option<usize> = None;
    for (index, block) in blocks.iter().enumerate() {
      let unit_block = unit_block(index, block);
      match self.owner(block, &of_before, &of_after, &folded) {
        Owner::Declaration(pair) => declared.entry(pair).or_default().push(unit_block),
        Owner::Outside => {
          if last_outside == Some(index.wrapping_sub(1))
            && let Some(run) = outside.last_mut()
          {
            run.push(unit_block);
          } else {
            outside.push(vec![unit_block]);
          }
          last_outside = Some(index);
        }
        Owner::Repeated(group) => repeated.entry(group).or_default().push(unit_block),
        Owner::Mechanical(kind) => mechanical.entry(mechanical_place(kind)).or_default().push(unit_block),
      }
    }

    // a declaration of one side alone whose lines the commits changed is a unit, with no block when another holds it
    let touched = |declaration: &SpannedDeclaration, old_side: bool| {
      let lines = lines_of(declaration);
      blocks.iter().any(|block| {
        let changed = if old_side { &block.old } else { &block.new };
        changed.start < lines.end && lines.start < changed.end
      })
    };
    for (pair, paired) in pairs.iter().enumerate() {
      let one_side = match (paired.before, paired.after) {
        (Some(before), None) => touched(&self.before.declarations[before], true),
        (None, Some(after)) => touched(&self.after.declarations[after], false),
        _ => false,
      };
      if one_side {
        declared.entry(folded[pair]).or_default();
      }
    }

    for (pair, unit_blocks) in declared {
      let paired = pairs[pair];
      let before = paired.before.map(|index| &self.before.declarations[index]);
      let after = paired.after.map(|index| &self.after.declarations[index]);
      let old_text = before.map_or_else(String::new, |declaration| text_of(&self.old, lines_of(declaration)));
      let new_text = after.map_or_else(String::new, |declaration| text_of(&self.new, lines_of(declaration)));
      let kind = match (before, after) {
        (Some(before), Some(after)) if before.form.form != after.form.form => UnitKind::Shape {
          changes: form_changes(&before.form.form, &after.form.form),
        },
        (Some(_), Some(_)) if old_text != new_text || indentation_matters(pair_file.file.path) => UnitKind::Behaviour,
        (Some(_), Some(_)) => {
          // the same text without whitespace: a change of whitespace across lines
          mechanical.entry(mechanical_place(MechanicalKind::Whitespace)).or_default().extend(unit_blocks);
          continue;
        }
        (None, _) => UnitKind::Added,
        (_, None) => UnitKind::Removed,
      };
      let Some(declaration) = after.or(before) else { continue };
      let form = &declaration.form;
      let fingerprint = digest([form.key.as_str(), declaration.signature.as_str(), old_text.as_str(), new_text.as_str()]);
      // a unit without a block follows the block that changed its lines
      let lines = lines_of(declaration);
      let first = unit_blocks.first().map_or_else(
        || {
          let old_side = after.is_none();
          let index = blocks.iter().position(|block| {
            let changed = if old_side { &block.old } else { &block.new };
            changed.start < lines.end && lines.start < changed.end
          });
          (index.unwrap_or(0), 1)
        },
        |block| (block.index as usize, 0),
      );
      cut.placed.push(Placed {
        at: (at, first.0, first.1),
        unit: ReviewUnit {
          id: format!("declaration:{}:{}:{}", pair_file.file.path, form.key, declaration.signature),
          kind,
          name: Some(form.name.clone()),
          key: Some(form.key.clone()),
          signature: Some(declaration.signature.clone()),
          public: form.public,
          line: Some(form.line),
          slices: vec![pair_file.slice(false, unit_blocks, fingerprint.clone())],
          fingerprint,
        },
      });
    }

    for run in outside {
      let fingerprint = self.blocks_fingerprint("outside", &run);
      let first = run.first().map_or(0, |block| block.index as usize);
      cut.placed.push(Placed {
        at: (at, first, 0),
        unit: ReviewUnit {
          id: format!("outside:{}:{first}", pair_file.file.path),
          kind: UnitKind::Other { reason: OtherReason::Outside },
          name: Some(file_name(pair_file.file.path).to_string()),
          key: None,
          signature: None,
          public: false,
          line: None,
          slices: vec![pair_file.slice(false, run, fingerprint.clone())],
          fingerprint,
        },
      });
    }

    for (group, unit_blocks) in repeated {
      let first = unit_blocks.first().map_or(0, |block| block.index as usize);
      let fingerprint = self.blocks_fingerprint("repeated", &unit_blocks);
      let slice = pair_file.slice(false, unit_blocks, fingerprint);
      cut.repeated.entry(group).or_insert_with(|| ((at, first, 0), Vec::new())).1.push(slice);
    }
    for (kind, mut unit_blocks) in mechanical {
      unit_blocks.sort_by_key(|block| block.index);
      let fingerprint = self.blocks_fingerprint("mechanical", &unit_blocks);
      cut.mechanical.entry(kind).or_default().push(pair_file.slice(false, unit_blocks, fingerprint));
    }
  }
}
