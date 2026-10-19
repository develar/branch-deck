//! Go to the declaration of a name in a diff of a review (spec/commit/symbol-navigation.spec.md): what its syntax tells
//! (`code_symbols::navigate`), its declaration in the same file by the scopes, else the declarations of that name in
//! the sides of the review's files, ranked by what the name says of its owner.
//!
//! - **The texts** come from the blobs the diffs read (the memory cache), so a click costs no git process; the
//!   summaries from the `blob_summary` store, which the sync fills, else a summary made now and stored.
//! - **The side**: the new side of a file is searched in the new sides of the review, the old side in the old ones.
//! - **The rank**: a member reached through a type (`Box.LIMIT`), `this` or `self`, or a property of a known type
//!   (`items.add` of `val items: MutableList`) prefers the declarations of that type; then the file of the name, the
//!   files that import its package, the files of its directory.
//! - **Outside the review**: a name the files of the review do not declare is looked for in the index of the
//!   repository (`accio`, at the revision of the side), `Owner.name` when the owner is known; a declaration in a file of
//!   the review becomes one of the review, any other one names its blob (one `cat-file --batch-check`) for the view of
//!   its file.

use crate::accio::{SymbolHit, SymbolIndex};
use crate::model::{BlobRequest, BlobText, DeclarationTarget, DiffSide, FindDeclarationParams, FoundDeclarations, OutsideFile, ReviewFileSides};
use crate::service::{FileDiffs, read_summaries, write_summaries};
use crate::worktree::read_worktree_blob;
use anyhow::Result;
use code_symbols::extract::summarize;
use code_symbols::highlight::grammar_of;
use code_symbols::language::{Language, language_of};
use code_symbols::navigate::{Position, ReferenceKind, reference_at};
use code_symbols::summary::{DeclKind, FileSummary};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::object_ids_and_types;
use object_cache::cache::ObjectCache;
use std::collections::HashMap;
use tracing::instrument;

/// The targets answered at most
const MAX_TARGETS: usize = 20;

/// A side of a file of the review to search
struct Candidate<'a> {
  path: &'a str,
  request: BlobRequest,
  language: Language,
}

impl FileDiffs {
  /// The declarations the name at `params.site` may mean, the most likely first; none where no name is
  #[instrument(skip(self, git, cache, index, params), fields(path = %params.site.path, line = params.site.line))]
  pub fn find_declaration(&self, git: &GitCommandExecutor, cache: &ObjectCache, index: Option<&dyn SymbolIndex>, params: &FindDeclarationParams) -> Result<FoundDeclarations> {
    let site = &params.site;
    let empty = |name: String| FoundDeclarations { name, targets: Vec::new() };
    let (Some(grammar), Some(language)) = (grammar_of(&site.path), language_of(&site.path)) else {
      return Ok(empty(String::new()));
    };
    let Some(text) = self.text_of(git, &params.repository_path, &site.blob)? else {
      return Ok(empty(String::new()));
    };
    let Some(reference) = reference_at(
      grammar,
      &text,
      Position {
        line: site.line,
        column: site.column,
      },
    ) else {
      return Ok(empty(String::new()));
    };
    if reference.kind == ReferenceKind::Declaration {
      return Ok(empty(reference.name));
    }
    if let Some(local) = reference.local {
      let end_column = local.column + u32::try_from(reference.name.encode_utf16().count()).unwrap_or(0);
      let target = DeclarationTarget {
        path: site.path.clone(),
        side: site.side,
        line: local.line,
        column: local.column,
        end_column,
        kind: "local".to_string(),
        container: None,
        outside: None,
      };
      return Ok(FoundDeclarations {
        name: reference.name,
        targets: vec![target],
      });
    }

    let candidates: Vec<Candidate<'_>> = params.files.iter().filter_map(|file| candidate(file, site.side, language)).collect();
    let summaries = self.summaries_of(git, cache, &params.repository_path, &candidates)?;
    let site_summary = summarize(language, text.as_bytes());
    let owner = expected_owner(&site_summary, reference.qualifier.as_deref(), site.line);
    let mut scored: Vec<(i32, &Candidate<'_>, u32, Option<String>, DeclKind)> = Vec::new();
    for candidate in &candidates {
      let Some(summary) = summaries.get(&candidate.request.id) else { continue };
      for declaration in &summary.declarations {
        if summary.name(declaration.name) != reference.name || !fits(reference.kind, declaration.kind) {
          continue;
        }
        let container = declaration
          .container
          .and_then(|index| summary.declarations.get(index as usize))
          .map(|it| summary.name(it.name).to_string());
        let receiver = declaration.receiver.map(|id| summary.name(id).to_string());
        let mut score = 0;
        if candidate.path == site.path {
          score += 40;
        }
        if let Some(owner) = &owner {
          if container.as_deref() == Some(owner) || receiver.as_deref() == Some(owner) {
            score += 100;
          } else if container.is_some() {
            score -= 30;
          }
        }
        if imports(&site_summary, summary, &reference.name) {
          score += 20;
        }
        if directory_of(candidate.path) == directory_of(&site.path) {
          score += 10;
        }
        scored.push((score, candidate, declaration.name_line, container, declaration.kind));
      }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(b.1.path)).then(a.2.cmp(&b.2)));
    scored.truncate(MAX_TARGETS);
    // the column of each name, from the texts of the files with a target
    let requests: Vec<BlobRequest> = scored.iter().map(|it| it.1.request.clone()).collect();
    let texts = self.texts_by_id(git, &params.repository_path, &requests)?;
    let targets = scored
      .into_iter()
      .map(|(_, candidate, name_line, container, kind)| {
        let line = name_line.saturating_sub(1);
        let column = texts.get(&candidate.request.id).and_then(|text| column_of(text, line, &reference.name)).unwrap_or(0);
        DeclarationTarget {
          path: candidate.path.to_string(),
          side: site.side,
          line,
          column,
          end_column: column + u32::try_from(reference.name.encode_utf16().count()).unwrap_or(0),
          kind: kind_name(kind).to_string(),
          container,
          outside: None,
        }
      })
      .collect::<Vec<_>>();
    if targets.is_empty()
      && let Some(index) = index
    {
      let targets = self.outside_targets(git, index, params, &reference.name, reference.kind, owner.as_deref(), &site_summary, language)?;
      return Ok(FoundDeclarations { name: reference.name, targets });
    }
    Ok(FoundDeclarations { name: reference.name, targets })
  }

  /// The declarations the index of the repository knows, at the revision of the side, ranked by the imports of the file
  #[expect(clippy::too_many_arguments, reason = "the facts of one lookup")]
  fn outside_targets(
    &self,
    git: &GitCommandExecutor,
    index: &dyn SymbolIndex,
    params: &FindDeclarationParams,
    name: &str,
    kind: ReferenceKind,
    owner: Option<&str>,
    site_summary: &FileSummary,
    language: Language,
  ) -> Result<Vec<DeclarationTarget>> {
    let site = &params.site;
    let revision = match site.side {
      DiffSide::Old => params.revisions.old.as_deref(),
      DiffSide::New => params.revisions.new.as_deref(),
    };
    let qualified = owner.map(|owner| format!("{owner}.{name}"));
    let mut hits = match &qualified {
      Some(query) => index.symbols(&params.repository_path, query, revision)?.unwrap_or_default(),
      None => Vec::new(),
    };
    if hits.is_empty() {
      hits = index.symbols(&params.repository_path, name, revision)?.unwrap_or_default();
    }
    let packages = imported_directories(site_summary, name);
    let mut hits: Vec<(i32, SymbolHit)> = hits
      .into_iter()
      .filter(|hit| language_of(&hit.path).is_some_and(|it| it.family() == language.family()) && fits_hit(kind, &hit.kind))
      .map(|hit| {
        let mut score = 0;
        if packages.iter().any(|directory| hit.path.contains(directory.as_str())) {
          score += 50;
        }
        if owner.is_some() && hit.parent.as_deref() == owner {
          score += 20;
        }
        if hit.path.split('/').next() == site.path.split('/').next() {
          score += 5;
        }
        (score, hit)
      })
      .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    hits.truncate(MAX_TARGETS);
    // the files of the review keep their place in it; the others are read at the revision
    let in_review = |path: &str| {
      params.files.iter().find(|file| match site.side {
        DiffSide::Old => file.old_id.is_some() && file.old_path.as_deref().unwrap_or(&file.path) == path,
        DiffSide::New => file.new_id.is_some() && file.path == path,
      })
    };
    let outside_paths: Vec<&str> = hits.iter().map(|it| it.1.path.as_str()).filter(|path| in_review(path).is_none()).collect();
    let ids = self.outside_ids(git, &params.repository_path, revision, &outside_paths)?;
    // the blob of the side of a file of the review, else the one at the revision
    let id_of = |path: &str| {
      in_review(path)
        .and_then(|it| if site.side == DiffSide::Old { it.old_id.clone() } else { it.new_id.clone() })
        .or_else(|| ids.get(path).cloned())
    };
    let requests: Vec<BlobRequest> = hits
      .iter()
      .filter_map(|(_, hit)| {
        Some(BlobRequest {
          id: id_of(&hit.path)?,
          path: None,
        })
      })
      .collect();
    let texts = self.texts_by_id(git, &params.repository_path, &requests)?;
    Ok(
      hits
        .into_iter()
        .filter_map(|(_, hit)| {
          let file = in_review(&hit.path);
          let id = id_of(&hit.path)?;
          let text = texts.get(&id)?;
          let line = hit.line.saturating_sub(1);
          let column = column_of(text, line, name).unwrap_or(0);
          let outside = file.is_none().then(|| OutsideFile {
            revision: revision.map(str::to_string),
            blob_id: id.clone(),
            line_count: u32::try_from(text.split('\n').count()).unwrap_or(u32::MAX),
          });
          Some(DeclarationTarget {
            path: file.map_or_else(|| hit.path.clone(), |it| it.path.clone()),
            side: site.side,
            line,
            column,
            end_column: column + u32::try_from(name.encode_utf16().count()).unwrap_or(0),
            kind: hit.kind,
            container: hit.parent,
            outside,
          })
        })
        .collect(),
    )
  }

  /// The blob ids of `paths` at `revision` (one `cat-file --batch-check`), or of the files of the working tree
  fn outside_ids(&self, git: &GitCommandExecutor, repository_path: &str, revision: Option<&str>, paths: &[&str]) -> Result<HashMap<String, String>> {
    if paths.is_empty() {
      return Ok(HashMap::new());
    }
    let Some(revision) = revision else {
      let mut ids = HashMap::new();
      for path in paths {
        // the file of the working tree as a blob, its id what git would give its bytes; kept for the view of the file
        if let Some(blob) = read_worktree_blob(repository_path, path, &"0".repeat(40))? {
          ids.insert((*path).to_string(), self.keep_blob(blob).id().to_string());
        }
      }
      return Ok(ids);
    };
    let names: Vec<String> = paths.iter().map(|path| format!("{revision}:{path}")).collect();
    Ok(
      paths
        .iter()
        .zip(object_ids_and_types(git, repository_path, &names)?)
        .filter_map(|(path, found)| found.filter(|(_, kind)| *kind == "blob").map(|(id, _)| ((*path).to_string(), id)))
        .collect(),
    )
  }

  /// The texts of the blobs of `requests` by id, without the binary and missing ones
  fn texts_by_id(&self, git: &GitCommandExecutor, repository_path: &str, requests: &[BlobRequest]) -> Result<HashMap<String, String>> {
    Ok(
      self
        .blob_texts(git, repository_path, requests)?
        .into_iter()
        .filter_map(|blob| match blob {
          BlobText::Text { id, text } => Some((id, text)),
          _ => None,
        })
        .collect(),
    )
  }

  /// The text of a blob, from memory, the working tree or one `cat-file`
  fn text_of(&self, git: &GitCommandExecutor, repository_path: &str, blob: &BlobRequest) -> Result<Option<String>> {
    Ok(self.blob_texts(git, repository_path, std::slice::from_ref(blob))?.into_iter().find_map(|it| match it {
      BlobText::Text { text, .. } => Some(text),
      _ => None,
    }))
  }

  /// The summaries of the candidates by blob id: from the store, else made from their texts and stored
  fn summaries_of(&self, git: &GitCommandExecutor, cache: &ObjectCache, repository_path: &str, candidates: &[Candidate<'_>]) -> Result<HashMap<String, FileSummary>> {
    let keys: Vec<(u8, &str)> = candidates.iter().map(|it| (it.language.id(), it.request.id.as_str())).collect();
    let mut summaries: HashMap<String, FileSummary> = read_summaries(cache, &keys).into_iter().map(|((_, id), summary)| (id, summary)).collect();
    let missing: Vec<&Candidate<'_>> = candidates.iter().filter(|it| !summaries.contains_key(&it.request.id)).collect();
    if missing.is_empty() {
      return Ok(summaries);
    }
    let requests: Vec<BlobRequest> = missing.iter().map(|it| it.request.clone()).collect();
    let texts = self.blob_texts(git, repository_path, &requests)?;
    let mut made: Vec<((u8, String), FileSummary)> = Vec::new();
    for (candidate, blob) in missing.into_iter().zip(texts) {
      if let BlobText::Text { text, .. } = blob {
        let summary = summarize(candidate.language, text.as_bytes());
        if !summary.truncated {
          made.push(((candidate.language.id(), candidate.request.id.clone()), summary.clone()));
        }
        summaries.insert(candidate.request.id.clone(), summary);
      }
    }
    write_summaries(cache, &made);
    Ok(summaries)
  }
}

/// The side of a file to search: on the side of the name, in a language that sees its declarations
fn candidate(file: &ReviewFileSides, side: DiffSide, language: Language) -> Option<Candidate<'_>> {
  let (path, id, worktree) = match side {
    DiffSide::Old => (file.old_path.as_deref().unwrap_or(&file.path), file.old_id.as_ref()?, false),
    DiffSide::New => (file.path.as_str(), file.new_id.as_ref()?, file.worktree),
  };
  let candidate_language = language_of(path)?;
  (candidate_language.family() == language.family()).then(|| Candidate {
    path,
    request: BlobRequest {
      id: id.clone(),
      path: worktree.then(|| path.to_string()),
    },
    language: candidate_language,
  })
}

/// The type whose member the name is, by its qualifier: a type (`Box.LIMIT`), `this` or `self` (the class around the
/// line), or a property or local of the file whose type is written (`items.add` of `val items: List<T>`)
fn expected_owner(summary: &FileSummary, qualifier: Option<&str>, line: u32) -> Option<String> {
  let qualifier = qualifier?;
  let last = qualifier.rsplit(['.', ':']).next().unwrap_or(qualifier).trim();
  if matches!(last, "this" | "self" | "Self") {
    let line = line + 1;
    return summary
      .declarations
      .iter()
      .filter(|it| it.kind == DeclKind::Type && it.start_line <= line && line <= it.end_line)
      .min_by_key(|it| it.end_line - it.start_line)
      .map(|it| summary.name(it.name).to_string());
  }
  if last.starts_with(|it: char| it.is_ascii_uppercase()) {
    return Some(last.to_string());
  }
  summary
    .declarations
    .iter()
    .find(|it| summary.name(it.name) == last && it.value_type.is_some())
    .and_then(|it| it.value_type.as_ref()?.last().map(|id| summary.name(*id).to_string()))
}

/// Whether a declaration can be what the syntax says the name is
const fn fits(reference: ReferenceKind, declaration: DeclKind) -> bool {
  match reference {
    ReferenceKind::Type => matches!(declaration, DeclKind::Type | DeclKind::TypeAlias),
    // a call of a class is its constructor
    ReferenceKind::Call => matches!(declaration, DeclKind::Function | DeclKind::Type | DeclKind::Macro),
    ReferenceKind::MemberCall => matches!(declaration, DeclKind::Function | DeclKind::Type),
    ReferenceKind::Member | ReferenceKind::Value => !matches!(declaration, DeclKind::Binding),
    ReferenceKind::Declaration => false,
  }
}

/// Whether the file of the name imports the declaration's file by the name: `import a.b.Name` of `package a.b`
fn imports(site: &FileSummary, summary: &FileSummary, name: &str) -> bool {
  let Some(package) = &summary.package else { return false };
  site.imports.iter().any(|import| {
    let path: Vec<&str> = import.path.iter().map(|id| site.name(*id)).collect();
    let joined = path.join(".");
    joined == *package || joined == format!("{package}.{name}")
  })
}

fn directory_of(path: &str) -> &str {
  path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

/// The UTF-16 column of `name` as a whole word on `line` of `text`
fn column_of(text: &str, line: u32, name: &str) -> Option<u32> {
  let content = text.split('\n').nth(line as usize)?;
  let is_word = |it: char| it.is_alphanumeric() || it == '_' || it == '$';
  content.match_indices(name).find_map(|(index, _)| {
    let before = content[..index].chars().next_back();
    let after = content[index + name.len()..].chars().next();
    (!before.is_some_and(is_word) && !after.is_some_and(is_word)).then(|| u32::try_from(content[..index].encode_utf16().count()).unwrap_or(0))
  })
}

/// The directories of the packages the file imports the name from: `a/b/` of `import a.b.Name`
fn imported_directories(site: &FileSummary, name: &str) -> Vec<String> {
  site
    .imports
    .iter()
    .filter_map(|import| {
      let path: Vec<&str> = import.path.iter().map(|id| site.name(*id)).collect();
      let (last, package) = path.split_last()?;
      (*last == name && !package.is_empty()).then(|| format!("{}/", package.join("/")))
    })
    .collect()
}

/// Whether a declaration of an index (its kind as accio names it) can be what the syntax says the name is
fn fits_hit(reference: ReferenceKind, kind: &str) -> bool {
  let is_type = matches!(
    kind,
    "class" | "interface" | "enum" | "object" | "trait" | "struct" | "union" | "record" | "annotation" | "type" | "typealias" | "type_alias"
  );
  match reference {
    ReferenceKind::Type => is_type,
    ReferenceKind::Call | ReferenceKind::MemberCall => is_type || matches!(kind, "method" | "function" | "constructor" | "macro"),
    ReferenceKind::Member | ReferenceKind::Value => true,
    ReferenceKind::Declaration => false,
  }
}

const fn kind_name(kind: DeclKind) -> &'static str {
  match kind {
    DeclKind::Type => "class",
    DeclKind::Function => "function",
    DeclKind::Value => "property",
    DeclKind::Constant => "constant",
    DeclKind::EnumEntry => "enumEntry",
    DeclKind::TypeAlias => "typeAlias",
    DeclKind::Module => "module",
    DeclKind::Macro => "macro",
    DeclKind::Binding => "local",
  }
}
