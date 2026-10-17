//! What one file declares, imports and references, made by `extract::summarize`.
//!
//! - **Names are interned**: every `u32` name is an index into `FileSummary::names`, which is sorted, so a summary is
//!   the same for the same bytes whatever order the walk met the names in, and `name_id` is a binary search.
//! - **References are sorted** by name, then line, and exact duplicates are dropped: `references_to` is a range.
//! - **Lines are 1-based**; a reference's line is the line of its identifier.

use crate::language::Language;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSummary {
  pub language: Language,
  /// The JVM `package a.b.c`, dotted; `None` in other languages and in the default package
  pub package: Option<String>,
  /// The interned names, sorted; every `u32` name below indexes here
  pub names: Vec<String>,
  pub imports: Vec<Import>,
  /// In source order; `container` indexes into this list
  pub declarations: Vec<Declaration>,
  /// Sorted by name, line, role, qualifier and container, without duplicates
  pub references: Vec<Reference>,
  /// Bytes under ERROR nodes, plus one per MISSING node (which has no bytes)
  pub error_bytes: u32,
  pub total_bytes: u32,
  pub lines: u32,
  /// The file was not summarized: it is over a size limit (`MAX_BYTES`, `MAX_LINES`, `MAX_AVERAGE_LINE`) or its parse
  /// ran out of `PARSE_BUDGET`. Such a summary has no imports, declarations or references, so a caller must not take
  /// it for a file without them, and must not cache it.
  pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
  /// The segments before the target: `a.b` of `import a.b.C`, `std::collections` of `use std::collections::HashMap`;
  /// empty for TypeScript, whose module is `specifier`
  pub path: Vec<u32>,
  pub target: ImportTarget,
  /// The local name: `D` of `import a.b.C as D`, `X` of `import X from "m"`
  pub alias: Option<u32>,
  /// Rust `pub use`, TypeScript `export … from`: the file passes the name on
  pub exported: bool,
  /// The TypeScript module string, as written: `./m` of `from "./m"`
  pub specifier: Option<String>,
  pub line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportTarget {
  Name(u32),
  /// `import a.b.*`, `use a::b::*`, `export * from "m"`
  All,
  /// TypeScript `import X from "m"`
  Default,
  /// TypeScript `import * as N from "m"`, `export * as N from "m"`, `import N = require("m")`
  Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
  pub name: u32,
  pub kind: DeclKind,
  /// The declaration this one is a member of. A member of a Rust `impl` points to the declaration of the impl's type
  /// when this file declares it, which may come later in the file.
  pub container: Option<u32>,
  /// The type a Rust `impl` or a Kotlin extension adds this member to (its last name: `Foo` of `impl<T> a::Foo<T>`,
  /// `String` of `fun String.trim2()`), whether or not this file declares it
  pub receiver: Option<u32>,
  pub reach: Reach,
  pub member: MemberKind,
  /// The first line, with the doc comments and attributes directly above
  pub start_line: u32,
  pub end_line: u32,
  pub name_line: u32,
  /// Inside an ERROR node: the parser recovered around it, so it may be misread
  pub in_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeclKind {
  /// A class, interface, object, enum, record, annotation, struct, union or trait
  Type,
  /// A function or method (constructors are no declarations: they carry the name of their type)
  Function,
  /// A property, field, variable or Rust `static`
  Value,
  /// Kotlin `const val`, Java `static final` and interface fields, Rust `const`, a TypeScript `UPPER_CASE` const
  Constant,
  EnumEntry,
  TypeAlias,
  /// A Rust `mod`, a TypeScript namespace or ambient module
  Module,
  /// Rust `macro_rules!`
  Macro,
  /// A name an import binds in this file: Rust `use` and `pub use`, a Kotlin import alias, TypeScript imports and
  /// `export … from`; the `Import` with this line and name (or alias) says what it binds
  Binding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Reach {
  Public,
  /// Kotlin `internal`, Rust `pub(crate)`, Java package-private
  Module,
  /// Rust `pub(super)`
  Parent,
  /// Private at the top level of the file (Kotlin, TypeScript) or of a Rust module file
  File,
  /// A private member, visible in its container
  Container,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemberKind {
  TopLevel,
  /// Java `static`, a member of a Kotlin object or companion, an enum entry, a nested (not inner) type, a Rust associated
  /// function without `self`
  Static,
  Instance,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Reference {
  pub name: u32,
  pub line: u32,
  pub role: Role,
  /// The segments before `.name` or `::name`: `[a, b, C]` of `a.b.C.name`, `this`, `self`, `super` and `crate` kept as
  /// segments. `None` for a bare name; empty when the receiver is an expression or a local (`foo().name`, `list.add`).
  pub qualifier: Option<Vec<u32>>,
  /// The innermost declaration around it
  pub container: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Role {
  Value,
  /// The callee of a call, `new` or a Kotlin trailing lambda
  Call,
  /// A type position, an annotation, a JSX element
  Type,
  /// The name an import or `use` brings in
  Import,
  /// A Rust macro invocation or attribute
  Macro,
}

impl FileSummary {
  pub fn name(&self, id: u32) -> &str {
    self.names.get(id as usize).map_or("", String::as_str)
  }

  pub fn name_id(&self, name: &str) -> Option<u32> {
    let index = self.names.binary_search_by(|it| it.as_str().cmp(name)).ok()?;
    u32::try_from(index).ok()
  }

  pub fn references_to(&self, name: &str) -> impl Iterator<Item = &Reference> {
    let range = match self.name_id(name) {
      Some(id) => {
        let start = self.references.partition_point(|it| it.name < id);
        let end = self.references.partition_point(|it| it.name <= id);
        start..end
      }
      None => 0..0,
    };
    self.references[range].iter()
  }

  /// Whether any declaration, a `Binding` included, has this name
  pub fn declares(&self, name: &str) -> bool {
    self.name_id(name).is_some_and(|id| self.declarations.iter().any(|it| it.name == id))
  }

  /// At least 5% of the bytes are under ERROR nodes: the parse is unreliable
  pub fn is_degraded(&self) -> bool {
    self.error_bytes > 0 && u64::from(self.error_bytes) * 100 >= u64::from(self.total_bytes) * 5
  }
}
