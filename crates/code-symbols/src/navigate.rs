//! The name at a place of a file and what its syntax tells of it, for going to its declaration
//! (openspec/specs/commit/symbol-navigation/spec.md): the name, its qualifier (`Foo` of `Foo.bar`), whether it is called, a type
//! or a member, and its declaration in the same file when the scopes show one (a parameter, a local, a member of an
//! enclosing class, a declaration of the file). The rest is the caller's: the other files of a review, an index.
//!
//! The usages of a name for Find Usages are its names in the tree: those bound to the same declaration for a local
//! (a parameter, a local of a function), else every name of that text that no local binds, each with how it uses it.
//!
//! `names_at` answers many places of a text from one parse of it, for a reader that asks of several names at once (the
//! renames of a diff, openspec/specs/commit/name-bindings/spec.md): the names bound to the declaration of each place,
//! what a body declares it as (`LocalBinding`), what each site makes of the name beside its binding (`Site`), and what
//! another name for the binding would clash with (`Clash`). `usages_at` is `names_at` of one place.
//!
//! Kotlin, Java and C# answer from their PSI and Roslyn's tree (`kotlin::navigate`, `java::navigate`,
//! `csharp::navigate`: one pass over the leaves files each declaration under the scopes that see it, by the language's
//! own scope rules), TypeScript and JavaScript from oxc's tree (`js::navigate`), Rust from rust-analyzer's
//! (`rust::navigate`). Each gives the names of one spelling with their bindings (`Names::resolve`); the answers are
//! made here (`reference_answer`, `answer`), with the positions of the view.

use crate::highlight::Grammar;
use std::collections::HashMap;

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

/// What a body declares a local as
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalKind {
  /// A variable a body declares: a `val`, a `let`, a resource of a `try`, the variable of a pattern, a range variable
  Variable,
  /// A parameter of a function with a name, of a method, a constructor or an accessor
  Parameter,
  /// A parameter of a lambda, a closure or a function without a name
  LambdaParameter,
  /// The variable of a `for`
  LoopVariable,
  /// The parameter of a `catch`
  CatchParameter,
  /// Any other declaration inside code: a local function, a local type and its members, a parameter that declares a
  /// member, a type parameter, a label, a name of a function type
  Other,
}

impl LocalKind {
  /// Whether the declaration is a value of its body alone (not `Other`): a name read as a type, or called where a call
  /// names no value, is not its
  pub const fn is_value(self) -> bool {
    !matches!(self, Self::Other)
  }
}

/// A declaration inside code, which no other file sees: what declares it, and whether other code may still spell its name
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalBinding {
  pub kind: LocalKind,
  /// Code outside the body may name the parameter, so the uses the scopes bind are not all that spell the name: a
  /// call by a named argument (a function with a name, a method or a constructor of Kotlin, C# and Starlark), an
  /// attribute macro on a Rust function, a framework that reads the annotations of a Java method or its class and
  /// binds by the names of the parameters
  pub named_by_calls: bool,
}

impl LocalBinding {
  pub(crate) const fn of(kind: LocalKind) -> Self {
    Self { kind, named_by_calls: false }
  }

  pub(crate) const OTHER: Self = Self::of(LocalKind::Other);
}

/// What the site of a name makes of it beside the binding the scopes give it: another name there changes more than the
/// binding, or the binding is not what the site names. One per name, the first of this order that holds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
  /// The name of the binding and nothing else (a string template that reads it too)
  Plain,
  /// The name is also the property the site reads or sets: `{ total }` of an object, a record or a pattern of
  /// TypeScript, JavaScript and Rust, a member of an anonymous type or an element of a tuple of C# named by it
  Shorthand,
  /// The name of a named argument (an element of an annotation too): the parameter of what is called, which no scope
  /// binds to a local
  NamedArgument,
  /// A label, or the name of a jump to it
  Label,
  /// Only the types tell whether the name is the binding the scopes give it: a value called or written after `::` in
  /// Kotlin (its `invoke`, or a function of its spelling), a name that is all of a `case` label of Java (a constant of
  /// the enum the `switch` is over), a name in a Java class with a supertype inside the body of the binding (a member
  /// the class inherits hides it), a name that is all of the right side of a C# `is` (a type of that name, declared
  /// anywhere, comes before the constant)
  Namesake,
  /// The spelling of the name is a value there: `nameof(total)`
  Spelled,
  /// Among the tokens of a macro, which may read the name as it likes
  Macro,
  /// A pattern that takes a value apart declares or assigns the name: by its place there, as far as the syntax tells
  Destructured,
}

/// A name bound to the declaration asked, or of the text asked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameUse {
  pub start: Position,
  pub end_column: u32,
  pub access: Access,
  pub site: Site,
}

/// What another name for a binding would change the meaning of
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClashKind {
  /// A name of the binding would be another declaration's: one of the new name is nearer to it
  Shadowed,
  /// A name of the text already spelled as the new name would become the binding's
  Captured,
  /// The language gives the new name a meaning no declaration shows (`it`, `field`, `value`): at the declaration
  Implicit,
}

/// A place a rename of a binding would change the meaning of
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clash {
  pub start: Position,
  pub end_column: u32,
  pub kind: ClashKind,
}

/// A place asked of `names_at`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameQuestion<'a> {
  pub position: Position,
  /// Another name for the binding of the place: the answer says what it would clash with (`NameAt::clashes`)
  pub renamed_to: Option<&'a str>,
}

impl NameQuestion<'_> {
  pub const fn at(position: Position) -> Self {
    Self { position, renamed_to: None }
  }
}

/// The name of a place: its binding, and every name of the text bound as it is
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameAt {
  pub name: String,
  /// The declaration inside code the name is bound to; `None` for any other name: a member, a declaration of the
  /// file, a name no declaration of the file binds
  pub local: Option<LocalBinding>,
  /// In the order of the text: for a local the names its scopes bind to it, its declaration among them; else every
  /// name of that text that no local binds (as `UsagesAt::usages`)
  pub uses: Vec<NameUse>,
  /// What the name asked as `NameQuestion::renamed_to` would clash with, in the order of the text; `None` where none
  /// was asked or the name is no local, empty where the rename changes no binding the scopes show
  pub clashes: Option<Vec<Clash>>,
}

/// The answers of `names_at`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamesAt {
  /// For each place asked, in the order asked, its entry of `names`; `None` where no name is. The places of one
  /// binding asked with the same new name share an entry, as the places of one text that no local binds
  pub asked: Vec<Option<usize>>,
  pub names: Vec<NameAt>,
}

impl NamesAt {
  /// The answer of the `index`-th place asked
  pub fn of(&self, index: usize) -> Option<&NameAt> {
    self.asked.get(index).copied().flatten().and_then(|it| self.names.get(it))
  }
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

/// The places of the bytes of a text asked in its order, in one pass over it, where `position_of` scans the text from
/// its start for each (a generated file of 400 KB has 14 000 names of a word)
pub(crate) struct Places<'t> {
  text: &'t str,
  last: (usize, Position),
}

impl<'t> Places<'t> {
  pub(crate) const fn new(text: &'t str) -> Self {
    Self {
      text,
      last: (0, Position { line: 0, column: 0 }),
    }
  }

  /// The place of the byte `start`: counted from the place asked before it, else from the start of the text
  pub(crate) fn at(&mut self, start: usize) -> Position {
    let position = match self.text.get(self.last.0..start) {
      Some(between) => advance(self.last.1, between, self.text.as_bytes()[..self.last.0].last() == Some(&b'\r')),
      None => position_of(self.text, start),
    };
    self.last = (start, position);
    position
  }
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

/// The names a resolution looks for: those spelled `name`, and those that start at a byte of `also` (sorted) whatever
/// they spell, which a rename would spell `name` (`clashes`)
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wanted<'a> {
  pub(crate) name: &'a str,
  pub(crate) also: &'a [usize],
}

impl<'a> Wanted<'a> {
  pub(crate) const fn named(name: &'a str) -> Self {
    Self { name, also: &[] }
  }

  /// Whether the name `text` that starts at the byte `start` is looked for
  pub(crate) fn takes(&self, text: &str, start: usize) -> bool {
    text == self.name || self.starts_at(start)
  }

  /// Whether a name that starts at the byte `start` is looked for whatever it spells
  pub(crate) fn starts_at(&self, start: usize) -> bool {
    !self.also.is_empty() && self.also.binary_search(&start).is_ok()
  }
}

/// The declaration a name is bound to: the byte where its name starts, and what it is when code declares it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bound {
  pub(crate) start: usize,
  pub(crate) local: Option<LocalBinding>,
}

/// A name of a resolution: the byte where its text starts, how it uses the name, what its site makes of it and the
/// declaration its scopes bind it to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Resolved {
  pub(crate) start: usize,
  pub(crate) access: Access,
  pub(crate) site: Site,
  pub(crate) binding: Option<Bound>,
}

/// The site of a name that its syntax alone does not give one (`syntactic`): a `Namesake` where the scopes bind the
/// name to a value and only the types tell whether the name is that value (`apart`: a call of Kotlin, a name before
/// its `::`)
pub(crate) fn site_by_binding(syntactic: Site, apart: bool, binding: Option<LocalBinding>) -> Site {
  if syntactic == Site::Plain && apart && binding.is_some_and(|it| it.kind.is_value()) {
    Site::Namesake
  } else {
    syntactic
  }
}

/// A parse of a text read for its names: what each language gives the answers of `names_at`, `usages_at` and `usages_of`
pub(crate) trait Names {
  /// The name whose text holds the byte `offset`, else the one that ends there (`foo|`): the byte where it starts and
  /// the name as names compare
  fn name_at(&self, offset: usize) -> Option<(usize, &str)>;

  /// Every name the resolution looks for, in the order of the text, each bound by the scopes of the language over the
  /// declarations among them
  fn resolve(&self, wanted: &Wanted<'_>) -> Vec<Resolved>;

  /// Whether the language gives a name a meaning no declaration shows (Kotlin's `it`)
  fn is_implicit(&self, name: &str) -> bool;
}

/// Whether a name is a use of the declaration `local`, else of the text that no local binds: the rule of Find Usages
fn is_use(name: &Resolved, local: Option<Bound>) -> bool {
  match local {
    Some(local) => name.binding.is_some_and(|it| it.start == local.start),
    None => name.binding.is_none_or(|it| it.local.is_none()),
  }
}

/// The answers of `names_at` from a parse: one resolution per spelling asked, one per binding asked with a new name
fn answer(names: &dyn Names, text: &str, questions: &[NameQuestion<'_>]) -> NamesAt {
  let mut resolutions: HashMap<&str, Vec<Resolved>> = HashMap::new();
  // the entry of a binding (the byte of its declaration) or of a text no local binds, asked with a new name or none
  let mut entries: HashMap<(Option<usize>, &str, Option<&str>), usize> = HashMap::new();
  let mut found = NamesAt::default();
  for question in questions {
    let entry = byte_offset(text, question.position).and_then(|offset| names.name_at(offset)).and_then(|(start, name)| {
      let resolved = resolutions.entry(name).or_insert_with(|| names.resolve(&Wanted::named(name)));
      let local = resolved.iter().find(|it| it.start == start)?.binding.filter(|it| it.local.is_some());
      let renamed_to = question.renamed_to.filter(|_| local.is_some());
      let key = (local.map(|it| it.start), name, renamed_to);
      Some(*entries.entry(key).or_insert_with(|| {
        found.names.push(entry_of(names, text, name, resolved, local, renamed_to));
        found.names.len() - 1
      }))
    });
    found.asked.push(entry);
  }
  found
}

/// The entry of a name: the uses of its declaration `local` (else of its text) among the names of its spelling, and
/// what the name `renamed_to` would clash with
fn entry_of(names: &dyn Names, text: &str, name: &str, resolved: &[Resolved], local: Option<Bound>, renamed_to: Option<&str>) -> NameAt {
  let length = utf16_length(name);
  let mut places = Places::new(text);
  let uses = resolved.iter().filter(|it| is_use(it, local)).map(|it| {
    let start = places.at(it.start);
    NameUse {
      start,
      end_column: start.column + length,
      access: it.access,
      site: it.site,
    }
  });
  NameAt {
    name: name.to_string(),
    local: local.and_then(|it| it.local),
    uses: uses.collect(),
    clashes: local.zip(renamed_to).map(|(local, new_name)| clashes(names, text, name, resolved, local, new_name)),
  }
}

/// What the name `new_name` for the declaration `local` of `name` would change the meaning of: the names of the
/// declaration that the scopes would bind to another declaration of the new name (`Shadowed`), the names already
/// spelled so that they would bind to it (`Captured`), and the new name itself where the language binds it without a
/// declaration (`Implicit`). The scopes are asked once more, of the names of the new spelling and the names of the
/// declaration as one name
fn clashes(names: &dyn Names, text: &str, name: &str, resolved: &[Resolved], local: Bound, new_name: &str) -> Vec<Clash> {
  if new_name == name {
    return Vec::new();
  }
  let own: Vec<usize> = resolved.iter().filter(|it| is_use(it, Some(local))).map(|it| it.start).collect();
  let renamed = names.resolve(&Wanted { name: new_name, also: &own });
  let (old_length, new_length) = (utf16_length(name), utf16_length(new_name));
  let mut places = Places::new(text);
  let implicit = names.is_implicit(new_name);
  let mut found = Vec::new();
  for it in &renamed {
    let is_own = own.binary_search(&it.start).is_ok();
    let bound_to_it = it.binding.is_some_and(|binding| binding.start == local.start);
    let kind = if is_own && it.start == local.start && implicit {
      ClashKind::Implicit
    } else if is_own && !bound_to_it {
      ClashKind::Shadowed
    } else if !is_own && bound_to_it {
      ClashKind::Captured
    } else {
      continue;
    };
    let start = places.at(it.start);
    found.push(Clash {
      start,
      end_column: start.column + if is_own { old_length } else { new_length },
      kind,
    });
  }
  found
}

/// The answer of `read` for the names of a parse of `text`; `None` where the grammar is not in this build or the text
/// has no tree
fn with_names<R: Send>(grammar: Grammar, text: &str, read: impl FnOnce(&dyn Names) -> R + Send) -> Option<R> {
  match grammar {
    #[cfg(feature = "kotlin")]
    Grammar::Kotlin | Grammar::KotlinScript => Some(crate::kotlin::navigate::with_names(crate::kotlin::file_kind(grammar), text, read)),
    #[cfg(feature = "java")]
    Grammar::Java => Some(crate::psi_navigate::with_names(&crate::java::navigate::NAVIGATION, text, read)),
    #[cfg(feature = "csharp")]
    Grammar::CSharp => Some(crate::psi_navigate::with_names(&crate::csharp::navigate::NAVIGATION, text, read)),
    #[cfg(feature = "starlark")]
    Grammar::Starlark | Grammar::StarlarkBuild => Some(crate::psi_navigate::with_names(&crate::starlark::navigate::NAVIGATION, text, read)),
    #[cfg(feature = "typescript")]
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => crate::js::navigate::with_names(crate::js::dialect_of_grammar(grammar), text, read),
    #[cfg(feature = "rust")]
    Grammar::Rust => crate::rust::navigate::with_names(text, read),
    #[cfg(not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "typescript", feature = "rust")))]
    _ => None,
  }
}

/// The names at the places asked of `text`, from one parse of it: for each its binding, the names bound as it is with
/// what their sites make of them, and what the new name asked would clash with. No place has an answer where the
/// grammar is not in this build or the text has no tree
pub fn names_at(grammar: Grammar, text: &str, questions: &[NameQuestion<'_>]) -> NamesAt {
  let none = || NamesAt {
    asked: vec![None; questions.len()],
    names: Vec::new(),
  };
  if questions.is_empty() {
    return none();
  }
  with_names(grammar, text, |names| answer(names, text, questions)).unwrap_or_else(none)
}

/// The usages of the name at `position`: those of its declaration for a local, else those of every name of that text that
/// no local binds; `None` where no name is
pub fn usages_at(grammar: Grammar, text: &str, position: Position) -> Option<UsagesAt> {
  let found = names_at(grammar, text, &[NameQuestion::at(position)]).names.pop()?;
  Some(UsagesAt {
    local: found.local.is_some(),
    usages: found.uses.iter().map(usage_of).collect(),
    name: found.name,
  })
}

/// The usages of `name` in a text that is not the one of the name: every name of that text that no local binds
pub fn usages_of(grammar: Grammar, text: &str, name: &str) -> Vec<Usage> {
  // a cheap test first: most files of a review do not have the word
  if !text.contains(name) {
    return Vec::new();
  }
  with_names(grammar, text, |names| {
    entry_of(names, text, name, &names.resolve(&Wanted::named(name)), None, None)
      .uses
      .iter()
      .map(usage_of)
      .collect()
  })
  .unwrap_or_default()
}

const fn usage_of(found: &NameUse) -> Usage {
  Usage {
    start: found.start,
    end_column: found.end_column,
    access: found.access,
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

#[cfg(all(test, feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "rust", feature = "typescript"))]
#[path = "names_at_test.rs"]
mod names_at_test;
