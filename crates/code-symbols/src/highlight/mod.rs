//! The syntax highlighting of one file, as the IntelliJ editor colours it without resolving names
//! (docs/design/diff-view.md, "Syntax highlighting"): the role of each piece of text, from its syntax tree. Kotlin,
//! Java, C# and Starlark from their PSI and Roslyn's tree (`kotlin::highlight`, `java::highlight`, `csharp::highlight`,
//! `starlark::highlight`: one pass over the leaves of psi-syntax's tree, whose tokens are the lexer's, soft keywords
//! included), Rust from rust-analyzer's tree (`rust::highlight`, one pass over its tokens), TypeScript, TSX and
//! JavaScript from oxc's tokens and tree (`js::highlight`: a walk of the tree marks the names, a pass over the tokens
//! and the comments paints them).
//!
//! - **One walk, painted spans**: a preorder walk gives each piece of text its style; a node with a style paints its
//!   text, an inner node paints over it (an escape in a string, the code of a template), and a node without one takes
//!   the style of the node around it. So the spans come out in the order of the text, with no sort.
//! - **The format of the view**: the lines are split as `buildLineTable` of the view splits them (`\n`, `\r\n`, a lone
//!   `\r`), each line has the runs of its styles, a pair (UTF-16 offset in the line, style) where the style changes, none
//!   while it is the default foreground, as the Shiki worker sends them (`src/lib/diff/highlight/protocol.ts`).
//! - **A style** is a `Role` in its low 6 bits and the font style above them: italic 1, bold 2, underline 4.
//! - **Bounded**: over `MAX_BYTES` there is no highlighting, nor for a Rust or a script file nested deeper than its
//!   parse reads; the view falls back to Shiki for the file. psi-syntax parses any
//!   text, a construct past its nesting limit as an error element. A parse error colours what its leaves are (a
//!   keyword, a literal), the rest stays plain; a region oxc's recovery blanked has no tokens and stays plain.
//!
//! A pure function of the text, like the summary (`extract.rs`), but with its own parse: the summary is cached by blob,
//! a Kotlin script (`.kts`) is parsed as a script here; a script is read in the dialect of its grammar, as the summary
//! reads it in the dialect of its path.

pub(crate) mod lex;
pub mod lines;
pub(crate) mod paint;
pub(crate) mod scopes;

use lines::LineRuns;

/// The version of the highlighting rules; bump it on any rule change, so that cached highlightings are made again
pub const RULES_VERSION: u32 = 8;
/// Files above this size are not highlighted: `MAX_BLOB_BYTES` of the texts the view reads
pub const MAX_BYTES: usize = 8 << 20;

/// The bits of the role in a style
pub const ROLE_BITS: u32 = 6;
pub const ITALIC: u32 = 1 << ROLE_BITS;
pub const BOLD: u32 = 2 << ROLE_BITS;
/// A mutable binding (a Kotlin `var`, a Rust `mut`): underlined, as IntelliJ's mutable variables
pub const UNDERLINE: u32 = 4 << ROLE_BITS;

/// A comment; the view gives it the font style of the scheme (italic in the light one)
pub(crate) const COMMENT: u32 = Role::Comment.style();
/// An instance field or property
pub(crate) const FIELD: u32 = Role::Field.style();
/// A static field, a constant, an enum entry: italic
pub(crate) const STATIC_FIELD: u32 = Role::Field.style() | ITALIC;

/// The roles of the highlighting, by their ids in the view: `SYNTAX_ROLES` of `src/lib/diff/islands-themes.ts` is this
/// order after the default foreground (`roles_match_the_view` checks it)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Role {
  Foreground = 0,
  Keyword,
  String,
  Escape,
  Number,
  /// A line or block comment: italic in the light scheme only, as IntelliJ's
  Comment,
  FunctionDeclaration,
  Field,
  Annotation,
  Tag,
  Attribute,
  Label,
  DocComment,
  DocTag,
  DocTagValue,
  NamedArgument,
  Macro,
  Lifetime,
  TypeParameter,
  /// Rust's `self`, coloured apart from the keywords in `RustRover`
  SelfValue,
  EnumVariant,
  /// Rust's `?`
  QuestionOperator,
  /// A label of JavaScript (`DEFAULT_LABEL`): underlined in the light scheme only
  DefaultLabel,
  /// The `<`, `>` and `/>` of a JSX element: the text colour in the light scheme, the tag colour in the dark one
  TagBracket,
  /// A JSX element of a component (`<Button>`, `<Menu.Item>`), not of HTML
  ComponentTag,
  Regexp,
  /// A method of JavaScript: its declaration and `.f(`
  Method,
  /// A variable that the file declares and does not export
  LocalVariable,
  /// A variable that the file exports or imports, or one it does not declare (`console`): italic in the dark scheme
  GlobalVariable,
  /// A parameter of JavaScript: underlined in the light scheme only
  Parameter,
  /// A function that the file exports or imports, or one it does not declare (`setTimeout`): italic in the light scheme
  GlobalFunction,
}

impl Role {
  /// The names of the view, in the order of the ids after the default foreground
  pub const NAMES: [&'static str; 30] = [
    "keyword",
    "string",
    "escape",
    "number",
    "comment",
    "functionDeclaration",
    "field",
    "annotation",
    "tag",
    "attribute",
    "label",
    "docComment",
    "docTag",
    "docTagValue",
    "namedArgument",
    "macro",
    "lifetime",
    "typeParameter",
    "selfValue",
    "enumVariant",
    "questionOperator",
    "defaultLabel",
    "tagBracket",
    "componentTag",
    "regexp",
    "method",
    "localVariable",
    "globalVariable",
    "parameter",
    "globalFunction",
  ];

  pub const fn style(self) -> u32 {
    self as u32
  }
}

/// The grammar a file is highlighted with; the discriminant of each is its stable id (`id`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Grammar {
  Kotlin = 0,
  /// A Kotlin script (`.kts`), whose top-level statements the compiler parses as a script's
  KotlinScript = 5,
  Java = 1,
  Rust = 2,
  TypeScript = 3,
  /// TSX: `.tsx`
  Tsx = 4,
  /// JavaScript: `.js`, `.jsx`, `.mjs`, `.cjs`, JSX included
  JavaScript = 9,
  CSharp = 6,
  /// Starlark: Bazel's `.bzl`, WORKSPACE and REPO.bazel files
  Starlark = 7,
  /// A Starlark BUILD or MODULE.bazel file, whose top-level calls declare (a target, a module or a dependency)
  StarlarkBuild = 8,
}

impl Grammar {
  /// A stable number for compact keys
  pub const fn id(self) -> u8 {
    self as u8
  }

  /// Whether this build reads files of the grammar: its language is a cargo feature
  const fn is_enabled(self) -> bool {
    match self {
      Self::Kotlin | Self::KotlinScript => cfg!(feature = "kotlin"),
      Self::Java => cfg!(feature = "java"),
      Self::Rust => cfg!(feature = "rust"),
      Self::TypeScript | Self::Tsx | Self::JavaScript => cfg!(feature = "typescript"),
      Self::CSharp => cfg!(feature = "csharp"),
      Self::Starlark | Self::StarlarkBuild => cfg!(feature = "starlark"),
    }
  }
}

/// The grammar of a file by its name or extension, if this build can highlight it
pub fn grammar_of(path: &str) -> Option<Grammar> {
  let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  #[cfg(feature = "starlark")]
  if let Some(kind) = starlark_syntax::FileKind::from_path(path) {
    let grammar = if kind.top_level_calls_declare() { Grammar::StarlarkBuild } else { Grammar::Starlark };
    return grammar.is_enabled().then_some(grammar);
  }
  let (_, extension) = file_name.rsplit_once('.')?;
  let grammar = match extension {
    "kt" => Grammar::Kotlin,
    "kts" => Grammar::KotlinScript,
    "java" => Grammar::Java,
    "cs" => Grammar::CSharp,
    "rs" => Grammar::Rust,
    "ts" | "mts" | "cts" => Grammar::TypeScript,
    "tsx" => Grammar::Tsx,
    "js" | "jsx" | "mjs" | "cjs" => Grammar::JavaScript,
    _ => return None,
  };
  grammar.is_enabled().then_some(grammar)
}

/// A stable hash of the rules and the grammars of this build: a cached highlighting is valid only for the same value
pub fn highlighter_version() -> u32 {
  crate::extract::extractor_version() ^ RULES_VERSION.wrapping_mul(0x9e37_79b9)
}

/// A name in the style of a constant or an enum entry: upper case, digits and underscores
pub(crate) fn is_constant_name(name: &str) -> bool {
  name.len() > 1 && name.bytes().next().is_some_and(|it| it.is_ascii_uppercase()) && name.bytes().all(|it| it.is_ascii_uppercase() || it.is_ascii_digit() || it == b'_')
}

/// A name in the style of a type: `Foo`, not `FOO` or `foo`
pub(crate) fn is_type_name(name: &str) -> bool {
  name.bytes().next().is_some_and(|it| it.is_ascii_uppercase()) && name.bytes().any(|it| it.is_ascii_lowercase())
}

/// The highlighting of a text; `None` when it is over `MAX_BYTES`, its grammar is not in this build or it has no tree.
/// Kotlin, Java, C# and Starlark by psi-syntax's trees (`kotlin`, `java`, `csharp`, `starlark`), TypeScript, TSX and
/// JavaScript by oxc's (`js`), Rust by rust-analyzer's (`rust`)
pub fn highlight(grammar: Grammar, text: &str) -> Option<LineRuns> {
  if text.len() > MAX_BYTES {
    return None;
  }
  match grammar {
    #[cfg(feature = "kotlin")]
    Grammar::Kotlin | Grammar::KotlinScript => Some(crate::kotlin::highlight::highlight(crate::kotlin::file_kind(grammar), text)),
    #[cfg(feature = "java")]
    Grammar::Java => Some(crate::java::highlight::highlight(text)),
    #[cfg(feature = "csharp")]
    Grammar::CSharp => Some(crate::csharp::highlight::highlight(text)),
    #[cfg(feature = "starlark")]
    Grammar::Starlark | Grammar::StarlarkBuild => Some(crate::starlark::highlight::highlight(text, crate::starlark::file_kind(grammar))),
    #[cfg(feature = "rust")]
    Grammar::Rust => crate::rust::highlight::highlight(text),
    #[cfg(feature = "typescript")]
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => crate::js::highlight::highlight(crate::js::dialect_of_grammar(grammar), text),
    #[cfg(not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "rust", feature = "typescript")))]
    _ => None,
  }
}
