//! The git attributes that mark a file generated: `linguist-generated` (the convention of GitHub, which folds them in a
//! pull request too) or `-diff` (which `binary` implies). The rule alone: git reads the attributes
//! (`git_ops::generated`, one `check-attr` for all the paths)

/// The attributes the rule reads, as a `check-attr` asks for them
pub const GENERATED_ATTRIBUTES: [&str; 2] = ["linguist-generated", "diff"];

/// Whether `attribute` with `value` (`set`, `unset`, `unspecified` or its value, as `check-attr` prints it) marks a file
/// generated
pub fn marks_generated(attribute: &str, value: &str) -> bool {
  matches!((attribute, value), ("linguist-generated", "set" | "true") | ("diff", "unset"))
}
