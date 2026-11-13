//! What the index of a repository answers a diff (openspec/specs/commit/quiet-changes/spec.md, "Subject"), as accio
//! answers it: the contract of its hidden answers for a tool, which Branch Deck's reader asks and reads
//! (`file_diffs::accio`) and the marks of a subject decide by (`marks_subject`). Only the fields a diff decides by
//! are read; a field it does not know is left alone.
//!
//! - **`accio renames --base <revision> --ref <revision>`** (`RenamePair` asked, `RenameAnswer` answered, accio's
//!   decision 2026-11-11-rename-read-in-two-trees): whether two places are one declaration renamed, and its uses in
//!   both trees.
//! - **`accio moves --base <revision> --ref <revision>`** (`MovesAnswer`): the declarations a change moved whole to
//!   another file or under other owners, and those it put in another order among their own (`reordered`), with
//!   whether their order is free.
//! - **The columns** of the index (`Columns`): what an `--at` takes and a hit answers, by the version of accio.

use serde::Deserialize;

/// A declaration a change may have renamed, by the place of its name in the old tree and in the new one:
/// `<path>:<line>:<column>`, the path from the root of the repository, the line and the column from 1, the column in
/// characters, as `--at` takes a place (`Columns::Characters`)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenamePair {
  pub old: String,
  pub new: String,
}

/// What accio says of a `RenamePair` between two trees (`accio renames`, answer revision `RENAMES_ANSWER`; the
/// contract is accio's decision 2026-11-11-rename-read-in-two-trees). Only the fields a diff decides by are read
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[expect(clippy::struct_excessive_bools, reason = "the fields of the answer of accio, as it writes them")]
pub struct RenameAnswer {
  /// The pair could not be asked: a place names no declaration, or reads as none. Never "no use"
  #[serde(default)]
  pub unavailable: bool,
  /// The two places are one declaration: the same kind, language, owners and signature apart from the name
  #[serde(default)]
  pub same: bool,
  /// Every candidate file of both trees was read, every use listed and tied by more than its name, and every
  /// relation of the declaration read: only then are the uses all there are, and no conflict "none"
  #[serde(default)]
  pub complete: bool,
  #[serde(default)]
  pub old: Option<RenamedDeclaration>,
  #[serde(default)]
  pub new: Option<RenamedDeclaration>,
  /// The uses of the old declaration in the old tree, and of the new one in the new tree; absent when the budget of
  /// the call ran out first
  #[serde(default)]
  pub base: Option<RenameUses>,
  #[serde(default)]
  pub head: Option<RenameUses>,
  /// What the rename changed beyond the name
  #[serde(default)]
  pub conflicts: Vec<RenameConflict>,
  /// What cut the answer short, when something did: a bound of the work, the same on every run, or the clock of the
  /// call, an answer of its load (`RenameAnswer::is_final` says which a reader may keep)
  #[serde(default)]
  pub cut: Option<Cut>,
  /// The same two trees, asked the same pairs with the same work and limit, give this answer again
  /// (`RenameAnswer::is_final`); absent, as an accio before the field writes it, it is false
  #[serde(default, rename = "final")]
  pub is_final: bool,
}

impl RenameAnswer {
  /// Whether the answer is all the index says of the pair between its two trees, asked with the same pairs, as
  /// accio says it (`final`, its decision 2026-11-13-rename-answer-says-final): complete or not, no clock cut it.
  /// A pair it could not ask, two places that are not one declaration, a use tied by the name alone, a relation not
  /// read, a bound of the work and a list cut by the limit are final: no later query of the same trees reads more.
  /// The clock of a call (accio shares one between its pairs, and a cold index spends it) unsettles every pair of the
  /// call that read anything, since the relations of the call are shared: asked again, the index may say more, so such
  /// an answer is a function of the load of the machine, not of the two trees. An accio before the field writes none,
  /// and none of its answers is final
  pub const fn is_final(&self) -> bool {
    self.is_final
  }
}

/// What cut an answer of `renames` short (`RenameAnswer::cut`), the worse of its two sides and its relations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cut {
  /// A bound of the work: the candidate files of a side, a bound of the relations; the same answer on every run
  Work,
  /// The clock of the call: a pair not asked before it ran out, or a scan it stopped; an answer of the load of the run
  Time,
  /// A word of a later accio
  #[serde(other)]
  Unknown,
}

/// A declaration of a `RenameAnswer`
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RenamedDeclaration {
  /// accio's name of its language: `kotlin`, `java`, …
  #[serde(default)]
  pub language: String,
}

/// The uses of a declaration in one tree
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RenameUses {
  /// By file, line and column, the declaration itself among them; at most the limit of the call
  #[serde(default)]
  pub uses: Vec<RenameUse>,
  /// The uses found: more than `uses` holds when the limit cut them
  #[serde(default)]
  pub total: usize,
}

/// A use of a declaration: a path of its tree, a line from 1 and the zero-based UTF-16 columns of the word written
/// there, the two the same for a use that writes no word (a positional argument that sets a property, a constructor
/// delegation)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RenameUse {
  #[serde(rename = "file")]
  pub path: String,
  pub line: u32,
  pub column: u32,
  pub end_column: u32,
  pub kind: Access,
  /// What ties it to the declaration: a use tied by its name alone proves nothing
  pub resolved: Resolved,
  /// The word written there when it is not the name of the declaration: an alias of an import, the accessor of a
  /// property in another language. A rename of the declaration asks nothing of such a word
  #[serde(default)]
  pub text: Option<String>,
}

/// What a rename changed beyond the name: `left` (a use the change did not rename, in a file it left alone),
/// `captured`, `overrides`, `overridden`, `overload`, `hides`, `hidden`, `namesake`
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RenameConflict {
  pub kind: String,
}

/// How an indexed usage accesses a name; unknown kinds display as `read`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
  Declaration,
  Import,
  Call,
  Write,
  Read,
  Argument,
  Type,
  Supertype,
  Override,
  Doc,
  String,
  #[serde(other)]
  Unknown,
}

/// What ties a usage to the declaration asked about
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolved {
  /// A type the index knows: the receiver, the argument or the type is the declaration's
  Typed,
  /// An import or a `use` of the declaration or of its module
  Import,
  /// The name alone: a receiver nothing typed, another language, prose or a namesake's
  Name,
}

/// How accio counts the column it takes in `--at` and answers in a `references` hit. An accio before 0.21 counts bytes
/// from 1 in both; from 0.21 (accio's decision 2026-11-09-one-column-unit) `--at` counts characters from 1 and a hit's
/// `column` UTF-16 units from 0, as a review does. Every column a review sends to the index or reads from it goes
/// through these two conversions, so no reader guesses a column from the words of a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Columns {
  Bytes,
  Characters,
}

impl Columns {
  /// The units of the accio whose `--version` printed `version` (`accio 0.21.0`, `accio 0.21.0 (source)`): `Bytes` for
  /// one before 0.21 or one it cannot read
  pub fn of_version(version: &str) -> Self {
    let number = version.split_whitespace().nth(1).unwrap_or_default();
    let mut parts = number.split('.').map(|it| it.parse::<u32>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    if (major, minor) >= (0, 21) { Self::Characters } else { Self::Bytes }
  }

  /// The `--at` column of the zero-based UTF-16 `column` on line `line` (from 0) of `text`; `None` past the line's end
  /// or inside a character
  pub fn at(self, text: &str, line: u32, column: u32) -> Option<u32> {
    let (byte, character, _) = boundaries(line_of(text, line)?).find(|&(_, _, utf16)| utf16 == column)?;
    u32::try_from(1 + if self == Self::Bytes { byte } else { character }).ok()
  }

  /// The zero-based UTF-16 column of a `references` hit's `column` on line `line` (from 0) of `text`; `None` past the
  /// line's end or inside a character (a hit of another text)
  pub fn hit(self, text: &str, line: u32, column: u32) -> Option<u32> {
    let mut found = boundaries(line_of(text, line)?);
    match self {
      Self::Bytes => found.find(|&(byte, _, _)| byte + 1 == column as usize).map(|(_, _, utf16)| utf16),
      Self::Characters => found.find(|&(_, _, utf16)| utf16 == column).map(|(_, _, utf16)| utf16),
    }
  }
}

/// Line `line` (from 0) of `text`, without its line end
fn line_of(text: &str, line: u32) -> Option<&str> {
  text.split('\n').nth(line as usize).map(|it| it.trim_end_matches('\r'))
}

/// Each character boundary of `line`, its end included: the bytes, the characters and the UTF-16 units before it
fn boundaries(line: &str) -> impl Iterator<Item = (usize, usize, u32)> + '_ {
  let mut utf16 = 0u32;
  line.char_indices().map(Some).chain(std::iter::once(None)).enumerate().map(move |(character, it)| {
    let here = (it.map_or(line.len(), |(byte, _)| byte), character, utf16);
    utf16 += it.map_or(0, |(_, c)| u32::try_from(c.len_utf16()).unwrap_or(0));
    here
  })
}

/// What accio says of the declarations a change moved between two trees (`accio moves`, answer revision 2 with
/// `reordered`): those it took whole to another file or under other owners (`moved`), and those it put in another
/// order among the declarations of their owner, which stayed (`reordered`). An accio of revision 1 answers no
/// `reordered`: an empty list then says nothing of an order
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MovesAnswer {
  /// The index could not be asked: no index of the repository, or one that is not ready. Never "no move"
  #[serde(default)]
  pub unavailable: bool,
  /// By the old place, at most the limit of the call
  #[serde(default)]
  pub moved: Vec<MovedDeclaration>,
  /// The declarations put in another order among their own, by the old place
  #[serde(default)]
  pub reordered: Vec<Reordered>,
  /// The moves found: more than `moved` holds when the limit cut the list
  #[serde(default)]
  pub total: usize,
  /// Every changed file was read and the lists are whole
  #[serde(default)]
  pub complete: bool,
}

/// A declaration of a tree, as `accio moves` names its place: the path from the root of the repository, its lines from
/// 1 (both ends included) and the zero-based UTF-16 columns of its name
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DeclaredAt {
  pub file: String,
  pub line: u32,
  pub end_line: u32,
  #[serde(default)]
  pub column: Option<u32>,
  #[serde(default)]
  pub end_column: Option<u32>,
  /// accio's kind of it: `function`, `class`, …
  #[serde(default)]
  pub kind: String,
  pub name: String,
  /// The declaration around it, by its name
  #[serde(default)]
  pub parent: Option<String>,
  /// accio's name of its language: `kotlin`, `java`, …
  #[serde(default)]
  pub language: String,
}

/// A declaration a change took whole to another place, with the same words: in another file (`file`) or under other
/// owners (`owner`); the outermost only, so a type moved whole is one move
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MovedDeclaration {
  pub old: DeclaredAt,
  pub new: DeclaredAt,
  #[serde(default)]
  pub file: bool,
  #[serde(default)]
  pub owner: bool,
}

/// A declaration a change put in another order among the declarations of its owner, its own text the same apart
/// from its layout, and whether that order is free (`ReorderMark`)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Reordered {
  pub old: DeclaredAt,
  pub new: DeclaredAt,
  pub order: Order,
  /// Why the order is what it is, in accio's words: the rule of the language that holds
  #[serde(default)]
  pub why: String,
}

/// Whether the order of a declaration among those of its owner says anything to the code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Order {
  /// No reader of the code sees the order: the members of a type of Kotlin or Java but an enum entry and what an
  /// initializer reads, the items of a Rust module, the functions a script hoists
  Free,
  /// The order is behaviour: the statements of a body, an enum entry, a decorator, an initializer
  Matters,
  /// A word of a later accio: read as `Matters`
  #[serde(other)]
  Unknown,
}

impl Order {
  /// Whether a reader may pass the new place of the declaration as the old one moved: only a free order
  pub const fn is_free(self) -> bool {
    matches!(self, Self::Free)
  }
}
