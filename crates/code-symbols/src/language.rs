//! The languages a summary can be made of, chosen by file name or extension. A language whose grammar feature is off is not
//! offered by `language_of`, so a caller never asks for a summary it cannot get.

use serde::{Deserialize, Serialize};

/// The discriminant of each is its stable id (`id`), in the order of the variants, which postcard writes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Language {
  Kotlin = 0,
  Java = 1,
  Rust = 2,
  /// TypeScript, TSX and JavaScript, each read by oxc in the dialect of its extension (`js::dialect_of_path`)
  TypeScript = 3,
  CSharp = 4,
  Starlark = 5,
}

/// Languages whose files see each other's declarations: Kotlin and Java call each other on the JVM; C# sees only C#;
/// Bazel Starlark files see declarations loaded from other Bazel Starlark files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
  Jvm,
  Rust,
  Script,
  Dotnet,
  Bazel,
}

impl Language {
  pub const ALL: [Self; 6] = [Self::Kotlin, Self::Java, Self::Rust, Self::TypeScript, Self::CSharp, Self::Starlark];

  pub const fn family(self) -> Family {
    match self {
      Self::Kotlin | Self::Java => Family::Jvm,
      Self::Rust => Family::Rust,
      Self::TypeScript => Family::Script,
      Self::CSharp => Family::Dotnet,
      Self::Starlark => Family::Bazel,
    }
  }

  /// A stable number for compact keys
  pub const fn id(self) -> u8 {
    self as u8
  }

  /// Whether this build has the grammar (the cargo feature of the language)
  pub const fn is_enabled(self) -> bool {
    match self {
      Self::Kotlin => cfg!(feature = "kotlin"),
      Self::Java => cfg!(feature = "java"),
      Self::Rust => cfg!(feature = "rust"),
      Self::TypeScript => cfg!(feature = "typescript"),
      Self::CSharp => cfg!(feature = "csharp"),
      Self::Starlark => cfg!(feature = "starlark"),
    }
  }
}

/// The bit of a summary key of a Starlark file whose top-level calls declare targets
const DECLARES_TARGETS: u8 = 0x80;
/// The bits of a summary key of a TypeScript file read as TSX and as JavaScript, beside the TypeScript of `.ts`
const TSX: u8 = 0x40;
const JAVASCRIPT: u8 = 0x20;

/// The key a cached summary of the file at `path` is kept under, beside the id of its bytes: the language, for
/// Starlark whether the file declares targets — the same bytes summarize differently as a `BUILD` and as a `.bzl` — and
/// for TypeScript the dialect of the extension, as `<T>x` is an assertion in a `.ts` file and an element in a `.tsx`
pub fn summary_key(language: Language, path: &str) -> u8 {
  #[cfg(feature = "starlark")]
  if language == Language::Starlark && starlark_syntax::FileKind::from_path(path).is_some_and(starlark_syntax::FileKind::top_level_calls_declare) {
    return language.id() | DECLARES_TARGETS;
  }
  #[cfg(feature = "typescript")]
  if language == Language::TypeScript {
    return language.id()
      | match crate::js::dialect_of_path(path) {
        js_syntax::Dialect::TypeScript => 0,
        js_syntax::Dialect::Tsx => TSX,
        js_syntax::Dialect::JavaScript => JAVASCRIPT,
      };
  }
  let _ = path;
  language.id()
}

/// The language of a file by its name or extension, if this build can parse it
pub fn language_of(path: &str) -> Option<Language> {
  let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  #[cfg(feature = "starlark")]
  if starlark_syntax::FileKind::from_path(path).is_some() {
    return Some(Language::Starlark);
  }
  let (_, extension) = file_name.rsplit_once('.')?;
  let language = match extension {
    "kt" | "kts" => Language::Kotlin,
    "java" => Language::Java,
    "cs" => Language::CSharp,
    "rs" => Language::Rust,
    "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" => Language::TypeScript,
    _ => return None,
  };
  language.is_enabled().then_some(language)
}
