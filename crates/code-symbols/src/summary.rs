//! What one file declares, imports and references, made by `extract::summarize`.
//!
//! - **Names are interned**: every `u32` name is an index into `FileSummary::names`, which is sorted, so a summary is
//!   the same for the same bytes whatever order the walk met the names in, and `name_id` is a binary search.
//! - **Shapes are interned**: `Reference::shape` indexes into `FileSummary::shapes`, one entry per distinct shape.
//! - **References are sorted** by name, then line, role, qualifier, container, shape and receiver type, and exact
//!   duplicates are dropped: `references_to` is a range.
//! - **Lines are 1-based**; a reference's line is the line of its identifier.
//! - **Postcard**: the object cache stores a summary with postcard, which is not self-describing, so no field may be
//!   skipped or defaulted (`skip_serializing_if`, `default`); a new field changes `RULES_VERSION`.

use crate::language::Language;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSummary {
  pub language: Language,
  /// The JVM `package a.b.c`, dotted; `None` in other languages and in the default package
  pub package: Option<String>,
  /// The name of the JVM class of the top-level declarations of a Kotlin file, `@file:JvmName("Util")`; `None` without
  /// one, when it is the file's name and `Kt` (`UtilKt` of `util.kt`)
  pub jvm_name: Option<String>,
  /// The interned names, sorted; every `u32` name below indexes here
  pub names: Vec<String>,
  pub imports: Vec<Import>,
  /// In source order; `container` indexes into this list
  pub declarations: Vec<Declaration>,
  /// Sorted by name, line, role, qualifier, container, shape and receiver type, without duplicates
  pub references: Vec<Reference>,
  /// The shapes of the references, each once, in the order the walk met them; `Reference::shape` indexes here
  pub shapes: Vec<Shape>,
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
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a declaration")]
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
  /// The parameters of a Kotlin or Java function; `None` for other declarations and languages
  pub signature: Option<Signature>,
  /// A Kotlin `override` member, a Java method annotated `@Override`
  pub overrides: bool,
  /// A function or property that a subtype may override. Kotlin: an `open` or `abstract` member of a class or an enum,
  /// an `override` one without `final`, a member of an interface; never a `private` one or one of an object. The
  /// modality of a Kotlin class is not read: a member of a final class may be overridable. Java: a method that is not
  /// `final`, `private` or `static`, of a class that is not `final` (nor a record), of an enum or of an interface.
  /// Locals are never.
  pub overridable: bool,
  /// A Kotlin function or property annotated `@JvmStatic` (or `@kotlin.jvm.JvmStatic`): Java calls a member of a
  /// companion by the class around it too (`Owner.create()`). Never in Java
  pub jvm_static: bool,
  /// A Kotlin property annotated `@JvmField`: Java sees a field of its name, and no accessors
  pub jvm_field: bool,
  /// A Kotlin function or property annotated `@JvmName`, `@get:JvmName` or `@set:JvmName`: Java names it, or an
  /// accessor of it, otherwise
  pub jvm_name: bool,
  /// The direct supertypes of a Kotlin or Java type, as written without type arguments: `[a, Base]` of
  /// `: a.Base<T>()`, `extends a.Base<T>`; a Kotlin function type, which has no path, is an empty one
  pub supertypes: Vec<Vec<u32>>,
  /// The type of a Kotlin property or a Java field, as written without type arguments (`[a, Manager]` of
  /// `val m: a.Manager<T>?`), else the type its initializer constructs (`val m = Manager(…)`, a capitalized call;
  /// `new Manager(…)` without a body); `None` for a type parameter, a function type, an inferred type of any other
  /// expression, and other declarations and languages
  pub value_type: Option<Vec<u32>>,
  /// The `value_type` is the type the initializer constructs, a call of its name, which a factory or an `invoke` of its
  /// companion may answer with another type
  pub value_constructed: bool,
  /// The constructors of a Kotlin or Java class, never empty: those it declares (a Kotlin primary constructor and the
  /// secondary ones, the canonical constructor of a Java record, declared, compact or not at all, and its others), else
  /// the implicit `()`, public in Kotlin and of the reach of its class in Java, as javac makes it; two of the same
  /// parameter types as their `TypeRef`s have them may both be there. `None` for other declarations and languages, and
  /// for an interface, an object, a companion, an enum, an annotation, a type alias, a Kotlin type the grammar misread
  /// and a class with a parse error in it: a call of one chooses no constructor, or the summary doesn't know them
  pub constructors: Option<Vec<Constructor>>,
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
  /// A function or method (a constructor is none: it is one of the `constructors` of its type)
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
  /// What the reference needs of the declaration it means, an index into `FileSummary::shapes`; `None` when it says
  /// nothing more than its name and role
  pub shape: Option<u32>,
  /// The type of the receiver of a Kotlin or Java member, as written where it is declared (`value_type`): of a local, a
  /// parameter or a typed lambda parameter of that type whose name is the whole receiver (`m.foo()`, `m?.foo()`,
  /// `m!!.foo()`, whose `qualifier` is then empty), or of a property or field of a class around it named by the
  /// receiver (`[m]`, `[this, m]`; not in an enclosed reference, not past a nested class that isn't inner, a companion,
  /// a class with supertypes or an extension function or property of another receiver, and only of the innermost class,
  /// with no extension between, for `this`). A parameter of a primary constructor has its type only in the initializers
  /// of the class, not in a function or an accessor, and a destructuring declaration none. `None` when the name is an
  /// operand of a Kotlin `is`, `!is` or `as` (a `when` subject with an `is` branch) or a Java `instanceof` or cast in
  /// its scope or function: a smart cast may give it another type
  pub receiver_type: Option<Vec<u32>>,
  /// The `receiver_type` is the type a constructor call gave its local or property (`value_constructed`)
  pub receiver_constructed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Role {
  Value,
  /// The callee of a call, `new` or a Kotlin trailing lambda, and the type of a Kotlin constructor invocation of a
  /// supertype (`Base` of `: a.Base(…)`, whose qualifiers are types); a Kotlin or Java call (not an infix one), `new`
  /// and that invocation included, has its arguments as its shape, a `Shape::Call`
  Call,
  /// A type position, an annotation, a JSX element
  Type,
  /// The name an import or `use` brings in
  Import,
  /// A Rust macro invocation or attribute
  Macro,
  /// The name of a Kotlin `override` or Java `@Override` member: one reference per direct supertype of the class around
  /// it, an object expression, an anonymous class and an enum entry's body included, whose `qualifier` is the path of
  /// that supertype (the enum of an enum entry; empty for a function type); its `shape` is a `Shape::Override`
  Override,
}

/// What a reference needs of the declaration it means, beyond its name
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shape {
  /// The member an `Override` reference overrides: a function (else a property), its extension receiver (the last
  /// name, as `Declaration::receiver`) and the types of its parameters
  Override {
    function: bool,
    receiver: Option<u32>,
    parameters: Vec<Option<TypeRef>>,
  },
  /// The arguments of a Kotlin or Java call, on the reference of its callee: in source order, a named one with its
  /// name, a Kotlin trailing lambda last; `spread` when one spreads an array (Kotlin `*items`). `enclosed` when the
  /// call is in a Kotlin lambda or anonymous function, an object expression, a Java anonymous class or a local
  /// declaration inside the declaration around it (`Reference::container`): a receiver of its own, which the summary
  /// doesn't show, may take a bare name or `this` before the type of that declaration (`buildList { add(0, item) }`).
  /// The constructor invocation of an object expression's supertype and a `new` of an anonymous class are of the code
  /// around the object, not in it, as their arguments are
  Call {
    arguments: Vec<(Option<u32>, Argument)>,
    spread: bool,
    enclosed: bool,
  },
}

/// What an argument of a call is, as far as its syntax tells
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Argument {
  /// Anything else: a name, a call, `null`, a real, a character, an unsigned or suffixed literal (`1L`, `1u`)
  Expression,
  /// A lambda, an anonymous function
  Lambda,
  /// A string literal or template
  Text,
  /// An integer literal without a suffix, with a sign or not
  Number,
  /// `true`, `false`
  Boolean,
}

/// The parameters of a function, in order
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Signature {
  pub parameters: Vec<Parameter>,
}

/// A constructor of a class
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Constructor {
  pub signature: Signature,
  /// The reach of its modifiers, as of a member of the class: `Container` for `private constructor(…)`
  pub reach: Reach,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Parameter {
  pub name: u32,
  /// `None` when a type parameter is in the type: it matches any type
  pub type_ref: Option<TypeRef>,
  /// It has a default value (Kotlin `= …`)
  pub optional: bool,
  /// Kotlin `vararg`, Java `…`
  pub vararg: bool,
}

/// A type as the last name of each type in it, in source order: `[List, Window]` of `kotlin.collections.List<a.Window>?`,
/// `[String, Int, Unit]` of `String.(Int) -> Unit`, `[int]` of Java `int[]`. Type arguments are kept only as names, so
/// two types that differ in their structure alone are the same; so are two types of one last name in two packages
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeRef {
  pub names: Vec<u32>,
  /// Kotlin `T?`
  pub nullable: bool,
  /// The dimensions of the array the parameter is on the JVM: one per Java `[]`, and one for a vararg (`int...`, Kotlin
  /// `vararg`); those of a type argument are not counted
  pub arrays: u8,
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
