//! The name at a place of a file and what its syntax tells of it, for going to its declaration
//! (openspec/specs/commit/symbol-navigation/spec.md): the name, its qualifier (`Foo` of `Foo.bar`), whether it is called, a type
//! or a member, and its declaration in the same file when the scopes show one (a parameter, a local, a member of an
//! enclosing class, a declaration of the file). The rest is the caller's: the other files of a review, an index.
//!
//! The usages of a name for Find Usages are its names in the tree: those bound to the same declaration for a local
//! (a parameter, a local of a function), else every name of that text that no local binds, each with how it uses it.
//!
//! Kotlin, Java and C# answer from their PSI and Roslyn's tree (`kotlin::navigate`, `java::navigate`,
//! `csharp::navigate`: one pass over the leaves files each declaration under the scopes that see it, by the language's
//! own scope rules), TypeScript and JavaScript from oxc's tree (`js::navigate`), Rust from rust-analyzer's
//! (`rust::navigate`). All give their answers through `reference_answer` and `usage_answers`, with the positions of the
//! view.

use crate::highlight::Grammar;

/// A place in a text: a line from 0 and a UTF-16 column, as the view counts them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
  pub line: u32,
  pub column: u32,
}

/// What the syntax says the name is
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
  /// A value: a variable, a property, a constant
  Value,
  /// The callee of a call, unqualified
  Call,
  /// A member after a qualifier, `foo.bar`
  Member,
  /// A called member, `foo.bar()`
  MemberCall,
  /// A type
  Type,
  /// The name of a declaration itself
  Declaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceAt {
  pub name: String,
  /// The text before the dot or the `::`: `Foo` of `Foo.bar`, `a.b` of `a.b.c`
  pub qualifier: Option<String>,
  pub kind: ReferenceKind,
  /// Where the name starts and ends on its line
  pub start: Position,
  pub end_column: u32,
  /// Its declaration in this file, by the scopes around it: where the declared name starts
  pub local: Option<Position>,
}

/// How a usage uses its name, as IntelliJ labels it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
  Declaration,
  Import,
  Call,
  Write,
  Read,
}

impl Access {
  pub const fn name(self) -> &'static str {
    match self {
      Self::Declaration => "declaration",
      Self::Import => "import",
      Self::Call => "call",
      Self::Write => "write",
      Self::Read => "read",
    }
  }
}

/// A name in a text, for Find Usages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
  pub start: Position,
  pub end_column: u32,
  pub access: Access,
}

/// The usages of the name at a place
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsagesAt {
  pub name: String,
  /// The name is a local: a parameter or a local of a function, which no other file sees
  pub local: bool,
  /// In the order of the text
  pub usages: Vec<Usage>,
}

/// The name at `position` of `text` and what its syntax says; `None` where no name is or the grammar is not in this
/// build
pub fn reference_at(grammar: Grammar, text: &str, position: Position) -> Option<ReferenceAt> {
  match grammar {
    #[cfg(feature = "kotlin")]
    Grammar::Kotlin | Grammar::KotlinScript => crate::kotlin::navigate::reference_at(crate::kotlin::file_kind(grammar), text, position),
    #[cfg(feature = "java")]
    Grammar::Java => crate::psi_navigate::reference_at(&crate::java::navigate::NAVIGATION, text, position),
    #[cfg(feature = "csharp")]
    Grammar::CSharp => crate::psi_navigate::reference_at(&crate::csharp::navigate::NAVIGATION, text, position),
    #[cfg(feature = "starlark")]
    Grammar::Starlark | Grammar::StarlarkBuild => crate::psi_navigate::reference_at(&crate::starlark::navigate::NAVIGATION, text, position),
    #[cfg(feature = "typescript")]
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => crate::js::navigate::reference_at(crate::js::dialect_of_grammar(grammar), text, position),
    #[cfg(feature = "rust")]
    Grammar::Rust => crate::rust::navigate::reference_at(text, position),
    #[cfg(not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "typescript", feature = "rust")))]
    _ => None,
  }
}

/// The answer of `reference_at` of any parser: the name `name` that starts at byte `start` of `text`, what the syntax
/// says it is, and the byte where its declaration in this file starts (`local`)
pub(crate) fn reference_answer(text: &str, start: usize, name: String, kind: ReferenceKind, qualifier: Option<String>, local: Option<usize>) -> ReferenceAt {
  let start = position_of(text, start);
  let end_column = start.column + utf16_length(&name);
  ReferenceAt {
    name,
    qualifier,
    kind,
    start,
    end_column,
    local: local.map(|it| position_of(text, it)),
  }
}

/// A usage of any parser: the name `name` that starts at byte `start` of `text`, used as `access`; `usage_answers`
/// answers many in one pass
#[cfg(test)]
pub(crate) fn usage_answer(text: &str, start: usize, name: &str, access: Access) -> Usage {
  let start = position_of(text, start);
  Usage {
    start,
    end_column: start.column + utf16_length(name),
    access,
  }
}

/// The usages of any parser in the order of the text: the name `name` at each byte with its access, in one pass over the
/// text, where `usage_answer` scans it from its start for each (a generated file of 400 KB has 14 000 names of a word)
pub(crate) fn usage_answers(text: &str, name: &str, usages: impl IntoIterator<Item = (usize, Access)>) -> Vec<Usage> {
  let length = utf16_length(name);
  let mut last = (0, Position { line: 0, column: 0 });
  usages
    .into_iter()
    .map(|(start, access)| {
      let position = match text.get(last.0..start) {
        Some(between) => advance(last.1, between, text.as_bytes()[..last.0].last() == Some(&b'\r')),
        None => position_of(text, start),
      };
      last = (start, position);
      Usage {
        start: position,
        end_column: position.column + length,
        access,
      }
    })
    .collect()
}

/// The place after `text` that follows `from`, with the lines as the view splits them; `after_cr` when the text before
/// it ends with a `\r`, whose `\n` ends no other line
fn advance(from: Position, text: &str, after_cr: bool) -> Position {
  let mut position = from;
  let mut previous_cr = after_cr;
  for character in text.chars() {
    match character {
      '\n' if previous_cr => {}
      '\n' | '\r' => {
        position.line += 1;
        position.column = 0;
      }
      _ => position.column += utf16_length_of(character),
    }
    previous_cr = character == '\r';
  }
  position
}

/// The usages of the name at `position`: those of its declaration for a local, else those of every name of that text that
/// no local binds; `None` where no name is
pub fn usages_at(grammar: Grammar, text: &str, position: Position) -> Option<UsagesAt> {
  match grammar {
    #[cfg(feature = "kotlin")]
    Grammar::Kotlin | Grammar::KotlinScript => crate::kotlin::navigate::usages_at(crate::kotlin::file_kind(grammar), text, position),
    #[cfg(feature = "java")]
    Grammar::Java => crate::psi_navigate::usages_at(&crate::java::navigate::NAVIGATION, text, position),
    #[cfg(feature = "csharp")]
    Grammar::CSharp => crate::psi_navigate::usages_at(&crate::csharp::navigate::NAVIGATION, text, position),
    #[cfg(feature = "starlark")]
    Grammar::Starlark | Grammar::StarlarkBuild => crate::psi_navigate::usages_at(&crate::starlark::navigate::NAVIGATION, text, position),
    #[cfg(feature = "typescript")]
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => crate::js::navigate::usages_at(crate::js::dialect_of_grammar(grammar), text, position),
    #[cfg(feature = "rust")]
    Grammar::Rust => crate::rust::navigate::usages_at(text, position),
    #[cfg(not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "typescript", feature = "rust")))]
    _ => None,
  }
}

/// The usages of `name` in a text that is not the one of the name: every name of that text that no local binds
pub fn usages_of(grammar: Grammar, text: &str, name: &str) -> Vec<Usage> {
  match grammar {
    #[cfg(feature = "kotlin")]
    Grammar::Kotlin | Grammar::KotlinScript => crate::kotlin::navigate::usages_of(crate::kotlin::file_kind(grammar), text, name),
    #[cfg(feature = "java")]
    Grammar::Java => crate::psi_navigate::usages_of(&crate::java::navigate::NAVIGATION, text, name),
    #[cfg(feature = "csharp")]
    Grammar::CSharp => crate::psi_navigate::usages_of(&crate::csharp::navigate::NAVIGATION, text, name),
    #[cfg(feature = "starlark")]
    Grammar::Starlark | Grammar::StarlarkBuild => crate::psi_navigate::usages_of(&crate::starlark::navigate::NAVIGATION, text, name),
    #[cfg(feature = "typescript")]
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => crate::js::navigate::usages_of(crate::js::dialect_of_grammar(grammar), text, name),
    #[cfg(feature = "rust")]
    Grammar::Rust => crate::rust::navigate::usages_of(text, name),
    #[cfg(not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "typescript", feature = "rust")))]
    _ => Vec::new(),
  }
}

/// The byte offset of a line and UTF-16 column, with the lines as the view splits them; `None` past the last line
pub(crate) fn byte_offset(text: &str, position: Position) -> Option<usize> {
  let bytes = text.as_bytes();
  let mut line = 0;
  let mut index = 0;
  while line < position.line {
    let next = index + bytes.get(index..)?.iter().position(|it| *it == b'\n' || *it == b'\r')?;
    index = next + if bytes[next] == b'\r' && bytes.get(next + 1) == Some(&b'\n') { 2 } else { 1 };
    line += 1;
  }
  let mut column = 0;
  for (offset, character) in text.get(index..)?.char_indices() {
    if column >= position.column || character == '\n' || character == '\r' {
      return Some(index + offset);
    }
    column += utf16_length_of(character);
  }
  Some(text.len())
}

/// The line and UTF-16 column of a byte offset, with the lines as the view splits them (`\n`, `\r\n`, a lone `\r`)
pub(crate) fn position_of(text: &str, offset: usize) -> Position {
  let before = text.get(..offset).unwrap_or(text);
  let bytes = before.as_bytes();
  let mut line = 0u32;
  let mut start = 0;
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] == b'\n' || bytes[index] == b'\r' {
      index += if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') { 2 } else { 1 };
      line += 1;
      start = index;
    } else {
      index += 1;
    }
  }
  Position {
    line,
    column: utf16_length(before.get(start..).unwrap_or_default()),
  }
}

/// The length of a text in UTF-16 code units, as the view counts columns
pub(crate) fn utf16_length(text: &str) -> u32 {
  u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

const fn utf16_length_of(character: char) -> u32 {
  if character.len_utf16() == 2 { 2 } else { 1 }
}
