//! The languages a summary can be made of, chosen by file extension. A language whose grammar feature is off is not
//! offered by `language_of`, so a caller never asks for a summary it cannot get.

use serde::{Deserialize, Serialize};

/// The discriminant of each is its stable id (`id`), in the order of the variants, which postcard writes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Language {
  Kotlin = 0,
  Java = 1,
  Rust = 2,
  /// TypeScript and JavaScript, both parsed by the TSX grammar (a superset of both, JSX included)
  TypeScript = 3,
  CSharp = 4,
}

/// Languages whose files see each other's declarations: Kotlin and Java call each other on the JVM; C# sees only C#
/// (a .NET assembly of another language is a reference, never a source of the branch)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
  Jvm,
  Rust,
  Script,
  Dotnet,
}

impl Language {
  pub const ALL: [Self; 5] = [Self::Kotlin, Self::Java, Self::Rust, Self::TypeScript, Self::CSharp];

  pub const fn family(self) -> Family {
    match self {
      Self::Kotlin | Self::Java => Family::Jvm,
      Self::Rust => Family::Rust,
      Self::TypeScript => Family::Script,
      Self::CSharp => Family::Dotnet,
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
    }
  }
}

/// The language of a file by its extension, if this build can parse it
pub fn language_of(path: &str) -> Option<Language> {
  let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
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
