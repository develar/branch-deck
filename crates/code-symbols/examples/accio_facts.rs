//! Compare the file-local answers of `code-symbols` with accio's indexed symbols and references.
//!
//! ```text
//! cargo run --release -p code-symbols --example accio_facts -- [--probes K] [--root DIR] [--parse-only] (--list FILE | <files…>) > facts.txt
//! cargo run --release -p code-symbols --example accio_facts -- --compare before.txt after.txt [--show N] [--family F]
//! ```
//!
//! A line is `path<TAB>family<TAB>fact`. Local probes are word starts, selected as in `examples/facts.rs`; accio symbol
//! and reference queries are batched across small groups of files. Accio's symbol columns and `code-symbols` positions are zero-based UTF-16;
//! the `references --at` command takes one-based columns, which are converted at the process boundary.
//!
//! Every declaration accio's extractor reads of a file (`accio parse`, no index) is set against the declarations of the
//! summary by name and line: `declaration-set-diff` names one that only one side declares, `end-line-diff` a pair whose
//! spans end apart, and `kind-pair` counts the kind words of the pairs, so that a table of `--compare` shows which kinds
//! of the two sides meet. `--parse-only` asks only that, for a corpus accio has no index of.

use code_symbols::extract::summarize;
use code_symbols::language::Language;
use code_symbols::language::language_of;
use code_symbols::navigate::{Position, reference_at, usages_at};
use code_symbols::summary::{DeclKind, MemberKind};
use code_tokens::{Grammar, grammar_of};
use fact_text::{arguments, inputs, probes, push, reference_text, take_flag, usages_at_text};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::io::{BufWriter, Write as _};
use std::path::Path;
use sync_utils::child_process::windowless_command;

#[path = "shared/fact_text.rs"]
mod fact_text;

fn main() {
  let mut args = arguments();
  let parse_only = take_flag(&mut args, "--parse-only");
  let (probes, root, paths) = inputs(&mut args);
  // the declarations alone: no probe asks the navigation, which on a corpus of tens of thousands of files is hours of it
  let probes = if parse_only { 0 } else { probes };
  let project = root.unwrap_or_else(|| std::env::current_dir().unwrap());
  let project = if project.is_absolute() {
    project
  } else {
    std::env::current_dir().unwrap().join(project)
  };
  let mut files = Vec::new();
  for path in paths {
    let Some(path) = project_path(&project, &path) else {
      panic!("path is outside the accio project: {path}");
    };
    if let Some(file) = prepare_file(&project, path, probes) {
      files.push(file);
    }
  }
  for batch in files.chunks_mut(16) {
    compare_declarations(&project, batch);
    if !parse_only {
      process_batch(&project, batch);
    }
  }
  let mut stdout = BufWriter::new(std::io::stdout().lock());
  for file in files {
    let _ = stdout.write_all(render(&file.path, file.facts).as_bytes());
  }
  let _ = stdout.flush();
}

fn project_path(project: &Path, path: &str) -> Option<String> {
  let path = Path::new(path);
  let relative = if path.is_absolute() { path.strip_prefix(project).ok()? } else { path };
  Some(relative.to_string_lossy().trim_start_matches("./").replace('\\', "/"))
}

struct SummaryDeclaration {
  name: String,
  /// The 1-based line of the name, as accio's `line`
  line: u32,
  /// The UTF-16 column of the name
  column: u32,
  end_line: u32,
  kind: DeclKind,
  member: MemberKind,
}

struct FileFacts {
  path: String,
  grammar: Grammar,
  text: Option<String>,
  words: Vec<(Position, String)>,
  /// What the summary says of each declaration
  declarations: Vec<SummaryDeclaration>,
  facts: Vec<(&'static str, String)>,
}

fn prepare_file(project: &Path, path: String, probes_limit: usize) -> Option<FileFacts> {
  let language = language_of(&path)?;
  if !matches!(language, Language::Rust | Language::TypeScript) {
    return None;
  }
  let grammar = grammar_of(&path)?;
  let full = project.join(&path);
  let source = match std::fs::read(&full) {
    Ok(source) => source,
    Err(error) => {
      return Some(FileFacts {
        path,
        grammar,
        text: None,
        words: Vec::new(),
        declarations: Vec::new(),
        facts: vec![("read", format!("cannot read: {error}"))],
      });
    }
  };
  let summary = summarize(language, &path, &source);
  let mut facts = Vec::new();
  let mut declarations = Vec::new();
  for declaration in &summary.declarations {
    let name = &summary.names[usize::try_from(declaration.name).unwrap()];
    facts.push((
      "branch-deck-summary",
      format!(
        "{} {:?}/{:?} at {}:{}-{}",
        name, declaration.kind, declaration.member, declaration.name_line, declaration.name_column, declaration.end_line
      ),
    ));
    declarations.push(SummaryDeclaration {
      name: name.clone(),
      line: declaration.name_line,
      column: declaration.name_column,
      end_line: declaration.end_line,
      kind: declaration.kind,
      member: declaration.member,
    });
  }
  facts.push((
    "branch-deck-summary",
    format!("{} declarations truncated={}", summary.declarations.len(), summary.truncated),
  ));

  let Ok(text) = String::from_utf8(source) else {
    facts.push(("branch-deck-navigation", "not utf-8".to_string()));
    return Some(FileFacts {
      path,
      grammar,
      text: None,
      words: Vec::new(),
      declarations,
      facts,
    });
  };
  let words: Vec<(Position, String)> = probes(&text, probes_limit).into_iter().map(|(position, word)| (position, word.to_string())).collect();
  for (position, word) in &words {
    let reference = reference_at(grammar, &text, *position).map_or_else(|| "none".to_string(), |it| reference_text(&it));
    let usages = usages_at(grammar, &text, *position).map_or_else(|| "none".to_string(), |it| usages_at_text(&it));
    facts.push(("branch-deck-probe", format!("{}:{} {word} => {reference} | {usages}", position.line, position.column)));
  }
  Some(FileFacts {
    path,
    grammar,
    text: Some(text),
    words,
    declarations,
    facts,
  })
}

fn process_batch(project: &Path, files: &mut [FileFacts]) {
  let paths: Vec<String> = files.iter().map(|it| it.path.clone()).collect();
  let names: Vec<String> = files
    .iter()
    .flat_map(|it| it.words.iter().map(|(_, word)| word.clone()))
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect();
  let symbols = if names.is_empty() { Vec::new() } else { symbols(project, &paths, &names) };
  let file_indices: HashMap<String, usize> = files.iter().enumerate().map(|(index, file)| (file.path.clone(), index)).collect();

  let mut at: BTreeSet<(String, u32, u32)> = files
    .iter()
    .flat_map(|file| file.words.iter().map(|(position, _)| (file.path.clone(), position.line.saturating_add(1), position.column)))
    .collect();
  for symbol in symbols {
    let Some(&index) = file_indices.get(&symbol.file) else { continue };
    let file = &mut files[index];
    let column = symbol.column.map_or_else(|| "-".to_string(), |it| it.to_string());
    let end_line = symbol.end_line.map_or_else(|| "-".to_string(), |it| it.to_string());
    file.facts.push((
      "accio-symbol",
      format!("kind={} name={} at={}:{} end_line={end_line}", symbol.kind, symbol.name, symbol.line, column),
    ));
    // where a jump of the review lands against where one of the index does: the summary's own column, not a search.
    // A line may declare one name several times (the `line` of `line?: { side: number; line: number }`, a field of
    // each member of a union), and the index holds a symbol of each, so a column of the index is a difference only
    // when the summary writes no declaration of that name there.
    let column_diff = symbol.column.and_then(|column| {
      let columns: Vec<u32> = file
        .declarations
        .iter()
        .filter(|it| it.name == symbol.name && it.line == symbol.line)
        .map(|it| it.column)
        .collect();
      (!columns.is_empty() && !columns.contains(&column)).then_some((column, columns))
    });
    if let Some((column, columns)) = column_diff {
      let summary = columns.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
      file.facts.push((
        "declaration-column-diff",
        format!("name={} at line {} accio={column} summary={summary}", symbol.name, symbol.line),
      ));
    }
    if let (Some(column), Some(text)) = (symbol.column, file.text.as_deref()) {
      let position = Position {
        line: symbol.line.saturating_sub(1),
        column,
      };
      let reference = reference_at(file.grammar, text, position);
      let expected_end = column.saturating_add(u32::try_from(symbol.name.encode_utf16().count()).unwrap_or(u32::MAX));
      let position_matches = reference
        .as_ref()
        .is_some_and(|it| it.name == symbol.name && it.start.line == position.line && it.start.column == position.column && it.end_column == expected_end);
      if !position_matches {
        let local = reference.as_ref().map_or_else(|| "none".to_string(), reference_text);
        file.facts.push((
          "name-boundary-diff",
          format!("name={} accio={}:{}-{} local={local}", symbol.name, position.line, position.column, expected_end),
        ));
      }
      let reference = reference.map_or_else(|| "none".to_string(), |it| reference_text(&it));
      let usages = usages_at(file.grammar, text, position).map_or_else(|| "none".to_string(), |it| usages_at_text(&it));
      file.facts.push((
        "branch-deck-at-symbol",
        format!("at={}:{} name={} => {reference} | {usages}", position.line, position.column, symbol.name),
      ));
      at.insert((symbol.file, symbol.line, column));
    }
  }
  let at: Vec<_> = at.into_iter().collect();
  let at_indices: HashMap<String, (usize, u32, u32)> = at
    .iter()
    .map(|(path, line, column)| {
      let index = file_indices[path];
      (format!("{path}:{line}:{}", column.saturating_add(1)), (index, *line, *column))
    })
    .collect();
  for query in references(project, &paths, &at) {
    let Some(&(index, line, column)) = at_indices.get(&query.at) else { continue };
    let file = &mut files[index];
    let hits: Vec<_> = query.hits.into_iter().filter(|it| it.file == file.path).collect();
    file
      .facts
      .push(("accio-references", format!("at {}:{}:{} hits={}", file.path, line.saturating_sub(1), column, hits.len())));
    for hit in hits {
      let line = hit.line.saturating_sub(1);
      let column = hit.column.map_or_else(|| "-".to_string(), |it| it.saturating_sub(1).to_string());
      file.facts.push(("accio-reference", format!("{} at {line}:{column}", hit.kind)));
    }
  }
}

/// One declaration of `accio parse`: `path:line-end kind name[ in parent]`
struct ParsedDeclaration {
  line: u32,
  end_line: u32,
  kind: String,
  name: String,
}

/// The declarations accio's extractor reads of each file of the batch against those of its summary, by name and line
fn compare_declarations(project: &Path, files: &mut [FileFacts]) {
  let paths: Vec<String> = files.iter().filter(|it| it.text.is_some()).map(|it| it.path.clone()).collect();
  if paths.is_empty() {
    return;
  }
  let mut parsed: HashMap<String, Vec<ParsedDeclaration>> = HashMap::new();
  for line in accio_text(project, "parse", &paths).lines() {
    let Some((place, rest)) = line.split_once(' ') else { continue };
    let Some((path, range)) = place.rsplit_once(':') else { continue };
    let Some((start, end)) = range.split_once('-') else { continue };
    let (Ok(start), Ok(end)) = (start.parse(), end.parse()) else { continue };
    let Some((kind, named)) = rest.split_once(' ') else { continue };
    let name = named.split_once(" in ").map_or(named, |(name, _)| name);
    parsed.entry(path.to_string()).or_default().push(ParsedDeclaration {
      line: start,
      end_line: end,
      kind: kind.to_string(),
      name: name.to_string(),
    });
  }
  for file in files.iter_mut().filter(|it| it.text.is_some()) {
    let theirs = parsed.remove(&file.path).unwrap_or_default();
    let mut ours: Vec<&SummaryDeclaration> = file.declarations.iter().collect();
    for declaration in &theirs {
      file.facts.push((
        "accio-parse",
        format!("{} {} at {}-{}", declaration.kind, declaration.name, declaration.line, declaration.end_line),
      ));
      let Some(index) = ours.iter().position(|it| it.name == declaration.name && it.line == declaration.line) else {
        file.facts.push((
          "declaration-set-diff",
          format!("only accio: {} {} at {}-{}", declaration.kind, declaration.name, declaration.line, declaration.end_line),
        ));
        continue;
      };
      let summary = ours.swap_remove(index);
      file
        .facts
        .push(("kind-pair", format!("accio={} summary={:?}/{:?}", declaration.kind, summary.kind, summary.member)));
      if summary.end_line != declaration.end_line {
        file.facts.push((
          "end-line-diff",
          format!(
            "{} {} at {} accio={} summary={}",
            declaration.kind, declaration.name, declaration.line, declaration.end_line, summary.end_line
          ),
        ));
      }
    }
    // a local of the summary that the index has no symbol of is what only a reader of a file writes, no difference
    for summary in ours.into_iter().filter(|it| it.kind != DeclKind::Binding) {
      file.facts.push((
        "declaration-set-diff",
        format!(
          "only summary: {:?}/{:?} {} at {}-{}",
          summary.kind, summary.member, summary.name, summary.line, summary.end_line
        ),
      ));
    }
  }
}

fn symbols(project: &Path, paths: &[String], names: &[String]) -> Vec<SymbolHit> {
  let mut args = vec!["symbol".to_string()];
  args.extend(names.iter().cloned());
  for path in paths {
    args.extend(["--scope".to_string(), path.clone()]);
  }
  args.extend(["--format".to_string(), "json".to_string()]);
  let answers: Vec<SymbolsForName> = accio(project, &args);
  answers.into_iter().flat_map(|it| it.hits).collect()
}

fn references(project: &Path, paths: &[String], at: &[(String, u32, u32)]) -> Vec<ReferencesAt> {
  if at.is_empty() {
    return Vec::new();
  }
  let mut args = vec!["references".to_string()];
  for (path, line, column) in at {
    args.push("--at".to_string());
    args.push(format!("{path}:{line}:{}", column.saturating_add(1)));
  }
  for path in paths {
    args.extend(["--scope".to_string(), path.clone()]);
  }
  args.extend(["--format".to_string(), "json".to_string()]);
  accio(project, &args)
}

/// The text accio prints for `command` over `paths`
fn accio_text(project: &Path, command: &str, paths: &[String]) -> String {
  let output = windowless_command("accio")
    .arg("--project")
    .arg(project)
    .arg(command)
    .args(paths)
    .output()
    .unwrap_or_else(|error| panic!("cannot run accio: {error}"));
  assert!(output.status.success(), "accio {command} failed: {}", String::from_utf8_lossy(&output.stderr));
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn accio<T: for<'de> Deserialize<'de>>(project: &Path, args: &[String]) -> T {
  let mut command = windowless_command("accio");
  command.arg("--project").arg(project).args(args);
  let output = command.output().unwrap_or_else(|error| panic!("cannot run accio: {error}"));
  assert!(output.status.success(), "accio {:?} failed: {}", args, String::from_utf8_lossy(&output.stderr));
  serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("cannot read accio JSON for {:?}: {error}: {}", args, String::from_utf8_lossy(&output.stdout)))
}

#[derive(Deserialize)]
struct SymbolsForName {
  #[serde(default)]
  hits: Vec<SymbolHit>,
}

#[derive(Clone, Deserialize)]
struct SymbolHit {
  file: String,
  line: u32,
  column: Option<u32>,
  end_line: Option<u32>,
  kind: String,
  name: String,
}

#[derive(Deserialize)]
struct ReferencesAt {
  at: String,
  hits: Vec<ReferenceHit>,
}

#[derive(Deserialize)]
struct ReferenceHit {
  file: String,
  line: u32,
  column: Option<u32>,
  kind: String,
}

fn render(path: &str, mut facts: Vec<(&str, String)>) -> String {
  facts.sort_unstable();
  let mut out = String::new();
  for (family, fact) in facts {
    push(&mut out, path, family, &fact);
  }
  out
}
