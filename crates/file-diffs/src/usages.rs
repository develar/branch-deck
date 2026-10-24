//! Find Usages of a name in a diff of a review (spec/commit/symbol-navigation.spec.md): its names in the trees of the
//! sides (`code_symbols::navigate`), then those outside the review that the index of the repository knows.
//!
//! - **A local** (a parameter, a local of a function) is used only in its file: the names its scopes bind to it.
//! - **Any other name** is looked for by its text: every name of that text that no local binds, in the file of the name
//!   and in the same side of the other files of the review in a language that sees it, from the texts the diffs read.
//! - **Outside the review**: `accio references --ref <the commit of the side>` for the files the review lacks; one
//!   `cat-file --batch-check` names their blobs and one `cat-file --batch` reads them, for their lines and the view of
//!   each file. The index answers by the word, as the review does; one that fails takes nothing from the review's usages.

use crate::accio::{Lenient, ReferenceHit, SymbolIndex};
use crate::declarations::{Candidate, candidate, in_review, outside_file, revision_of, site_position, word_columns};
use crate::model::{DeclarationTarget, DiffSide, FindDeclarationParams, FoundUsages, UsageTarget};
use crate::service::FileDiffs;
use anyhow::Result;
use code_symbols::highlight::grammar_of;
use code_symbols::language::{Language, language_of};
use code_symbols::navigate::{Usage, usages_at, usages_of};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::instrument;

/// The usages in the review answered at most
const MAX_USAGES: usize = 500;

impl FileDiffs {
  /// The usages of the name at `params.site`: in the review, then outside it; none where no name is
  #[instrument(skip(self, git, index, params), fields(path = %params.site.path, line = params.site.line))]
  pub fn find_usages(&self, git: &GitCommandExecutor, index: Option<&dyn SymbolIndex>, params: &FindDeclarationParams) -> Result<FoundUsages> {
    let site = &params.site;
    let empty = |name: String| FoundUsages {
      name,
      usages: Vec::new(),
      more_outside: false,
      index_error: None,
    };
    let Some((grammar, language, text)) = self.site_text(git, params)? else {
      return Ok(empty(String::new()));
    };
    let Some(found) = usages_at(grammar, &text, site_position(params)) else {
      return Ok(empty(String::new()));
    };
    let mut usages = targets_of(&site.path, site.side, &text, &found.usages);
    if found.local {
      return Ok(FoundUsages {
        name: found.name,
        usages,
        more_outside: false,
        index_error: None,
      });
    }

    // the other files of the review, in its order
    let others: Vec<Candidate<'_>> = params
      .files
      .iter()
      .filter_map(|file| candidate(file, site.side, language))
      .filter(|it| it.path != site.path)
      .collect();
    let requests: Vec<_> = others.iter().map(|it| it.request.clone()).collect();
    let texts = self.texts_by_id(git, &params.repository_path, &requests)?;
    for other in &others {
      let (Some(text), Some(grammar)) = (texts.get(&other.request.id), grammar_of(other.path)) else {
        continue;
      };
      usages.extend(targets_of(other.path, site.side, text, &usages_of(grammar, text, &found.name)));
    }
    usages.truncate(MAX_USAGES);

    let Some(index) = index else {
      return Ok(FoundUsages {
        name: found.name,
        usages,
        more_outside: false,
        index_error: None,
      });
    };
    let index = Lenient::new(index);
    let references = index.references(&params.repository_path, &found.name, revision_of(params))?;
    let more_outside = !references.complete;
    usages.extend(self.outside_usages(git, params, &found.name, language, references.hits)?);
    Ok(FoundUsages {
      name: found.name,
      usages,
      more_outside,
      index_error: index.error(),
    })
  }

  /// The usages the index knows in the files the review lacks, in a language that sees the name, read at the revision
  fn outside_usages(&self, git: &GitCommandExecutor, params: &FindDeclarationParams, name: &str, language: Language, hits: Vec<ReferenceHit>) -> Result<Vec<UsageTarget>> {
    let hits: Vec<ReferenceHit> = hits
      .into_iter()
      .filter(|hit| in_review(params, &hit.path).is_none() && language_of(&hit.path).is_some_and(|it| it.family() == language.family()))
      .collect();
    let mut paths: Vec<&str> = hits.iter().map(|it| it.path.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    let revision = revision_of(params);
    let ids = self.outside_ids(git, &params.repository_path, revision, &paths)?;
    let requests: Vec<_> = ids.values().map(|id| crate::model::BlobRequest { id: id.clone(), path: None }).collect();
    let texts = self.texts_by_id(git, &params.repository_path, &requests)?;
    let width = u32::try_from(name.encode_utf16().count()).unwrap_or(0);
    Ok(
      hits
        .into_iter()
        .filter_map(|hit| {
          let id = ids.get(&hit.path)?;
          let text = texts.get(id)?;
          let line = hit.line.checked_sub(1)?;
          // the occurrence of the word nearest to the column of the index
          let near = hit.column.saturating_sub(1);
          let column = word_columns(text, line, name).min_by_key(|it| it.abs_diff(near))?;
          let (line_text, indent) = line_text(text, line);
          Some(UsageTarget {
            text: line_text,
            text_column: column.saturating_sub(indent),
            target: DeclarationTarget {
              path: hit.path,
              side: params.site.side,
              line,
              column,
              end_column: column + width,
              kind: access_name(&hit.kind).to_string(),
              container: None,
              outside: Some(outside_file(revision, id, text)),
            },
          })
        })
        .collect(),
    )
  }
}

/// The usages of a text of the review as targets with their lines
fn targets_of(path: &str, side: DiffSide, text: &str, usages: &[Usage]) -> Vec<UsageTarget> {
  let lines: HashMap<u32, (String, u32)> = usages.iter().map(|it| (it.start.line, line_text(text, it.start.line))).collect();
  usages
    .iter()
    .map(|usage| UsageTarget {
      text: lines.get(&usage.start.line).map_or_default(|it| it.0.clone()),
      text_column: usage.start.column.saturating_sub(lines.get(&usage.start.line).map_or(0, |it| it.1)),
      target: DeclarationTarget {
        path: path.to_string(),
        side,
        line: usage.start.line,
        column: usage.start.column,
        end_column: usage.end_column,
        kind: usage.access.name().to_string(),
        container: None,
        outside: None,
      },
    })
    .collect()
}

/// A line of a text without its indent and its line end, and the UTF-16 length of the indent
fn line_text(text: &str, line: u32) -> (String, u32) {
  let content = text.split('\n').nth(line as usize).unwrap_or_default().trim_end_matches('\r');
  let trimmed = content.trim_start();
  let indent = u32::try_from(content[..content.len() - trimmed.len()].encode_utf16().count()).unwrap_or(0);
  (trimmed.to_string(), indent)
}

/// How the index names a usage, as the review does: an argument is read
fn access_name(kind: &str) -> &str {
  match kind {
    "argument" => "read",
    other => other,
  }
}
