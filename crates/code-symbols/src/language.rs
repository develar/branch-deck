//! The languages a summary can be made of, chosen by file extension. A language whose grammar feature is off is not
//! offered by `language_of`, so a caller never asks for a summary it cannot get.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Language {
  Kotlin,
  Java,
  Rust,
  /// TypeScript and JavaScript, both parsed by the TSX grammar (a superset of both, JSX included)
  TypeScript,
}

/// Languages whose files see each other's declarations: Kotlin and Java call each other on the JVM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
  Jvm,
  Rust,
  Script,
}

impl Language {
  pub const ALL: [Self; 4] = [Self::Kotlin, Self::Java, Self::Rust, Self::TypeScript];

  pub const fn family(self) -> Family {
    match self {
      Self::Kotlin | Self::Java => Family::Jvm,
      Self::Rust => Family::Rust,
      Self::TypeScript => Family::Script,
    }
  }

  /// A stable number for compact keys
  pub const fn id(self) -> u8 {
    match self {
      Self::Kotlin => 0,
      Self::Java => 1,
      Self::Rust => 2,
      Self::TypeScript => 3,
    }
  }

  /// Whether this build has the grammar (the cargo feature of the language)
  pub const fn is_enabled(self) -> bool {
    match self {
      Self::Kotlin => cfg!(feature = "kotlin"),
      Self::Java => cfg!(feature = "java"),
      Self::Rust => cfg!(feature = "rust"),
      Self::TypeScript => cfg!(feature = "typescript"),
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
    "rs" => Language::Rust,
    "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" => Language::TypeScript,
    _ => return None,
  };
  language.is_enabled().then_some(language)
}
