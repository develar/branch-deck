//! Find Usages of a name in a diff of a review (openspec/specs/commit/symbol-navigation/spec.md): its names in the trees of the
//! sides (`code_symbols::navigate`), then those outside the review that the index of the repository knows.
//!
//! - **A local** (a parameter, a local of a function) is used only in its file: the names its scopes bind to it.
//! - **Any other name** is looked for by its text: every name of that text that no local binds, in the file of the name
//!   and in the same side of the other files of the review in a language that sees it, from the texts the diffs read.
//! - **Outside the review**: with a declaration in the review, `accio references --at … --resolved` answers linked
//!   uses and counts name-only candidates; without one, `accio references <name>` keeps the name-based fallback. One
//!   `cat-file --batch-check` names the missing blobs and one `cat-file --batch` reads them; a failed index takes
//!   nothing from the review's usages.

use crate::accio::{Access as IndexAccess, Columns, DeclarationAt, Lenient, ReferenceHit, Resolved, SymbolIndex};
use crate::declarations::{Candidate, candidate, in_review, outside_file, revision_of, site_position};
use crate::model::{DeclarationTarget, DiffSide, FindDeclarationParams, FoundUsages, UsageTarget};
use crate::service::FileDiffs;
use anyhow::Result;
use code_symbols::highlight::grammar_of;
use code_symbols::language::{Language, language_of};
use code_symbols::navigate::{Access as NavigationAccess, Usage, usages_at, usages_of};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
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
      filtered_count: 0,
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
        filtered_count: 0,
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
        filtered_count: 0,
        index_error: None,
      });
    };
    let columns = index.columns();
    // the text of a file of the review: the site's, or another's the diffs read
    let text_of = |path: &str| {
      if path == params.site.path {
        return Some(text.as_str());
      }
      others
        .iter()
        .find(|other| other.path == path)
        .and_then(|other| texts.get(&other.request.id).map(String::as_str))
    };
    let declaration = self.declaration_at(git, params, columns, &found.usages, &text_of)?;
    let (hits, more_outside, filtered_count, index_error) = if let Some(declaration) = declaration {
      let index_at = Lenient::new(index);
      let references = index_at.references_at_resolved(&params.repository_path, std::slice::from_ref(&declaration), revision_of(params))?;
      let unavailable = index_at.unavailable();
      let error = index_at.error();
      if let Some(references) = references.filter(|references| references.iter().any(|it| !it.unavailable)) {
        let more_outside = references.iter().any(|it| !it.complete);
        let filtered_count = references.iter().fold(0u32, |count, it| {
          let by_name = u32::try_from(it.by_name).unwrap_or(u32::MAX);
          let returned = u32::try_from(it.hits.iter().filter(|hit| hit.resolved == Resolved::Name).count()).unwrap_or(u32::MAX);
          count.saturating_add(by_name.max(returned))
        });
        let hits = references
          .into_iter()
          .flat_map(|it| it.hits.into_iter().filter(|hit| hit.resolved != Resolved::Name))
          .collect();
        (hits, more_outside, filtered_count, error.or(unavailable))
      } else if let Some(unavailable) = unavailable {
        (Vec::new(), false, 0, Some(unavailable))
      } else {
        references_by_name(index, params, &found.name)?
      }
    } else {
      references_by_name(index, params, &found.name)?
    };
    usages.extend(self.outside_usages(git, params, &found.name, language, columns, hits)?);
    Ok(FoundUsages {
      name: found.name,
      usages,
      more_outside,
      filtered_count,
      index_error,
    })
  }

  fn declaration_at<'t>(
    &self,
    git: &GitCommandExecutor,
    params: &FindDeclarationParams,
    columns: Columns,
    usages: &[Usage],
    text_of: &dyn Fn(&str) -> Option<&'t str>,
  ) -> Result<Option<DeclarationAt>> {
    let position = site_position(params);
    if usages.iter().any(|usage| usage.access == NavigationAccess::Declaration && usage.start == position) {
      return Ok(Some(DeclarationAt {
        revision: None,
        path: params.site.path.clone(),
        line: position.line.saturating_add(1),
        column: text_of(&params.site.path).and_then(|text| columns.at(text, position.line, position.column)),
      }));
    }
    let declarations = self.find_declaration(git, &ObjectCache::disabled(), None, params)?;
    let Some(target) = declarations.targets.iter().find(|target| target.outside.is_none()) else {
      return Ok(None);
    };
    let column = text_of(&target.path).and_then(|text| columns.at(text, target.line, target.column));
    Ok(Some(DeclarationAt {
      revision: None,
      path: target.path.clone(),
      line: target.line.saturating_add(1),
      column,
    }))
  }

  /// The usages the index knows in the files the review lacks, in a language that sees the name, read at the revision
  fn outside_usages(
    &self,
    git: &GitCommandExecutor,
    params: &FindDeclarationParams,
    name: &str,
    language: Language,
    columns: Columns,
    hits: Vec<ReferenceHit>,
  ) -> Result<Vec<UsageTarget>> {
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
          // the column the index answers, in the units of the review (`Columns::hit`), never a word found on the line
          let column = columns.hit(text, line, hit.column)?;
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
              kind: access_name(hit.kind).to_string(),
              container: None,
              outside: Some(outside_file(revision, id, text)),
            },
          })
        })
        .collect(),
    )
  }
}

fn references_by_name(index: &dyn SymbolIndex, params: &FindDeclarationParams, name: &str) -> Result<(Vec<ReferenceHit>, bool, u32, Option<String>)> {
  let index = Lenient::new(index);
  let references = index.references(&params.repository_path, name, revision_of(params))?;
  let mut hits = references.hits;
  hits.sort_by_key(|hit| resolved_order(hit.resolved));
  let unavailable = index.unavailable();
  let index_error = index.error().or(unavailable);
  Ok((hits, !references.complete, 0, index_error))
}

const fn resolved_order(resolved: Resolved) -> u8 {
  match resolved {
    Resolved::Typed => 0,
    Resolved::Import => 1,
    Resolved::Name => 2,
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
const fn access_name(kind: IndexAccess) -> &'static str {
  match kind {
    IndexAccess::Declaration => "declaration",
    IndexAccess::Import => "import",
    IndexAccess::Call => "call",
    IndexAccess::Write => "write",
    IndexAccess::Read
    | IndexAccess::Argument
    | IndexAccess::Type
    | IndexAccess::Supertype
    | IndexAccess::Override
    | IndexAccess::Doc
    | IndexAccess::String
    | IndexAccess::Unknown => "read",
  }
}
