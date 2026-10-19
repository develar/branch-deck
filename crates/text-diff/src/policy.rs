// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and
// contributors): `community/platform/util/diff/src/com/intellij/diff/comparison/ComparisonPolicy.kt`,
// `InnerFragmentsPolicy` of `ComparisonManager`, and `IgnorePolicy.java` and `HighlightPolicy.java` of
// `community/platform/diff-impl/src/com/intellij/diff/tools/util/base/`. Left out: `IgnorePolicy.FORMATTING` and
// `IGNORE_LANGUAGE_SPECIFIC_CHANGES`, which need a language model, and `HighlightPolicy.DO_NOT_HIGHLIGHT`.

//! How the comparison treats whitespace, and how fine its results are.

/// How two pieces of text compare
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComparisonPolicy {
  /// Every char counts
  Default,
  /// The whitespace at the start and the end of a line does not count
  TrimWhitespaces,
  /// No whitespace counts
  IgnoreWhitespaces,
}

/// The changes inside a changed block of lines
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InnerFragmentsPolicy {
  None,
  Words,
  Chars,
}

/// The whitespace option of a diff view (`IgnorePolicy`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum WhitespacePolicy {
  /// Every char counts
  #[default]
  Default,
  /// The whitespace at the start and the end of a line does not count
  Trim,
  /// No whitespace counts
  Ignore,
  /// No whitespace counts, and a block that changed only in whitespace or empty lines at its start or end is cut off
  /// (`IGNORE_WHITESPACES_CHUNKS`)
  IgnoreBlankLines,
}

impl WhitespacePolicy {
  pub const fn comparison_policy(self) -> ComparisonPolicy {
    match self {
      Self::Default => ComparisonPolicy::Default,
      Self::Trim => ComparisonPolicy::TrimWhitespaces,
      Self::Ignore | Self::IgnoreBlankLines => ComparisonPolicy::IgnoreWhitespaces,
    }
  }

  /// Adjoining blocks merge into one; every policy here squashes, only the left out language-specific one does not
  pub const fn should_squash(self) -> bool {
    true
  }

  /// The blocks at the ends of a run of adjoining blocks that changed only in whitespace are cut off
  pub const fn should_trim_chunks(self) -> bool {
    matches!(self, Self::IgnoreBlankLines)
  }
}

/// How fine the changes of a diff view are (`HighlightPolicy`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HighlightPolicy {
  /// Changed lines only
  ByLine,
  /// The changed words inside the changed lines
  #[default]
  ByWord,
  /// The changed words, and a changed block split at its unchanged lines
  ByWordSplit,
  /// The changed chars inside the changed lines
  ByChar,
}

impl HighlightPolicy {
  /// Adjoining blocks merge into one, except when the user asked for split blocks
  pub const fn should_squash(self) -> bool {
    !matches!(self, Self::ByWordSplit)
  }

  pub const fn fragments_policy(self) -> InnerFragmentsPolicy {
    match self {
      Self::ByWord | Self::ByWordSplit => InnerFragmentsPolicy::Words,
      Self::ByChar => InnerFragmentsPolicy::Chars,
      Self::ByLine => InnerFragmentsPolicy::None,
    }
  }

  pub const fn is_fine_fragments(self) -> bool {
    !matches!(self.fragments_policy(), InnerFragmentsPolicy::None)
  }
}
