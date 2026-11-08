//! The declarations of a text as the scopes of its lines, for the labels of the hunks: `CommitPatch › read_patch`.

#[cfg(test)]
use code_symbols::language::Language;
use code_symbols::summary::{DeclKind, Declaration, FileSummary};
use text_diff::model::Scope;

/// Between the names of a scope and the scopes around it
const SEPARATOR: &str = " › ";

/// The scopes of the file at `path`, in source order; empty when the language has no grammar here or the summary is
/// truncated
#[cfg(test)]
pub(crate) fn scopes_of(language: Language, path: &str, text: &str) -> Vec<Scope> {
  scopes_of_summary(&code_symbols::extract::summarize(language, path, text.as_bytes()))
}

/// The types, functions, modules and Bazel targets of a summary, in source order; empty for a truncated one. A target
/// is a scope so that the folded lines inside its call (`deps = […]`) carry its name
pub(crate) fn scopes_of_summary(summary: &FileSummary) -> Vec<Scope> {
  if summary.truncated {
    return Vec::new();
  }
  summary
    .declarations
    .iter()
    .filter(|declaration| matches!(declaration.kind, DeclKind::Type | DeclKind::Function | DeclKind::Module | DeclKind::Target) && !declaration.in_error)
    .map(|declaration| Scope {
      // the summary has 1-based lines, the last one included
      start_line: declaration.start_line.saturating_sub(1),
      end_line: declaration.end_line,
      label: label(summary, declaration),
    })
    .collect()
}

/// The names of the declaration and its containers, the outermost first
fn label(summary: &FileSummary, declaration: &Declaration) -> String {
  let mut names = vec![summary.names[declaration.name as usize].as_str()];
  let mut container = declaration.container;
  // a container comes from the same summary; the bound keeps a malformed one from looping
  while let Some(index) = container
    && names.len() < summary.declarations.len()
  {
    let outer = &summary.declarations[index as usize];
    names.push(&summary.names[outer.name as usize]);
    container = outer.container;
  }
  names.reverse();
  names.join(SEPARATOR)
}
