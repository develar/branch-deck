//! The syntax summaries of the source files that the commits ahead of the baseline change (`code_symbols`), before and
//! after each commit, by blob id: from the session, else from the object cache (`blob_summary`), else from one
//! `cat-file --batch` of the blobs the cache misses, parsed in parallel and written back in one transaction. The same
//! process reads the `Cargo.toml` and `tsconfig*.json` files of the directories above the files (`ModuleRoots`, one
//! `cat-file --batch` of the trees at HEAD), for the names of the crates and the path aliases of TypeScript.

use crate::config_snapshot::ConfigSnapshot;
use crate::diff_scan::CommitDiff;
use crate::module_roots::ModuleRoots;
use anyhow::Result;
use code_symbols::extract::{MAX_BYTES, extractor_version, summarize};
use code_symbols::language::{Language, language_of};
use code_symbols::summary::FileSummary;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::object_texts::read_blob_bytes;
use object_cache::cache::ObjectCache;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// The git config key of the symbol mode (`off`, `report`, `on`)
pub const SYMBOLS_CONFIG_KEY: &str = "branchdeck.symbols";

/// How far the dependencies by symbols reach (`branchdeck.symbols`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum SymbolMode {
  /// No summaries are read
  Off,
  /// `bd commits` reports the dependencies by symbols, and the dangling-reference check subtracts by the trees; nothing
  /// decides by them
  Report,
  /// They also order and stack branches, and the conflict fix and the checks use them
  #[default]
  On,
}

impl SymbolMode {
  pub fn parse(value: &str) -> Option<Self> {
    match value.trim().to_ascii_lowercase().as_str() {
      "off" | "false" => Some(Self::Off),
      "report" => Some(Self::Report),
      "on" | "true" => Some(Self::On),
      _ => None,
    }
  }
}

/// `branchdeck.symbols` of the repository, `SymbolMode::default()` when unset: one `ConfigSnapshot`; an operation that
/// reads more of the config takes `ConfigSnapshot::symbol_mode` of its snapshot
pub fn read_symbol_mode(git: &GitCommandExecutor, repository_path: &str) -> Result<SymbolMode> {
  ConfigSnapshot::read(git, repository_path)?.symbol_mode()
}

/// What the scan read, for `bd commits --symbol-stats`
#[derive(Debug, Clone, Default)]
pub struct ScanStats {
  /// By language: (blobs, bytes, degraded, truncated)
  pub blobs: BTreeMap<Language, LanguageStats>,
  pub cached: usize,
  pub parsed: usize,
  /// Wall time of the reads and of the parse
  pub read_time: Duration,
  pub parse_time: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LanguageStats {
  pub blobs: usize,
  pub bytes: u64,
  pub error_bytes: u64,
  pub degraded: usize,
  pub truncated: usize,
}

/// The summaries and the module structure the dependencies by symbols are computed from
#[derive(Debug, Default)]
pub struct SymbolInput {
  /// Blob id -> its summary
  pub summaries: HashMap<String, Arc<FileSummary>>,
  pub roots: ModuleRoots,
  /// Crate directory -> crate name, as Rust code names it (`-` as `_`)
  pub crates: HashMap<String, String>,
  /// TypeScript path aliases by the directory of their `tsconfig*.json`
  pub aliases: HashMap<String, TsAliases>,
  pub stats: ScanStats,
}

/// The path aliases of one `tsconfig*.json` (`compilerOptions.paths`, relative to `baseUrl`): pattern -> targets, both
/// relative to the repository root, with at most one `*` each
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TsAliases {
  pub paths: Vec<(String, Vec<String>)>,
}

/// The source blobs of `diffs`, each with its language: the old blob in the language of the old path, the new one in
/// the language of the new path
fn source_blobs(diffs: &[CommitDiff]) -> Vec<(String, Language)> {
  let mut seen: HashSet<&str> = HashSet::new();
  let mut blobs = Vec::new();
  for change in diffs.iter().flat_map(|diff| &diff.files) {
    let old_path = change.old_path.as_deref().unwrap_or(&change.path);
    for (blob, path) in [(&change.old_blob, old_path), (&change.new_blob, change.path.as_str())] {
      let (Some(blob), Some(language)) = (blob.as_deref(), language_of(path)) else { continue };
      if seen.insert(blob) {
        blobs.push((blob.to_string(), language));
      }
    }
  }
  blobs
}

/// The summaries of the source files `diffs` change, with the module structure above them
#[instrument(skip_all, fields(commits = diffs.len()))]
pub fn read_symbol_input(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, diffs: &[CommitDiff]) -> Result<SymbolInput> {
  let started = Instant::now();
  let version = extractor_version();
  let wanted = source_blobs(diffs);
  let paths = diffs
    .iter()
    .flat_map(|diff| &diff.files)
    .filter(|change| language_of(&change.path).is_some())
    .map(|change| change.path.as_str());
  let roots = ModuleRoots::read(git, repository_path, paths)?;

  let keys: Vec<(u8, &str)> = wanted.iter().map(|(blob, language)| (language.id(), blob.as_str())).collect();
  let cached: Vec<Option<FileSummary>> = cache.get_blob_summaries(version, &keys);
  let mut summaries: HashMap<String, Arc<FileSummary>> = HashMap::with_capacity(wanted.len());
  let mut missing: Vec<(String, Language)> = Vec::new();
  for ((blob, language), summary) in wanted.into_iter().zip(cached) {
    match summary {
      Some(summary) => {
        summaries.insert(blob, Arc::new(summary));
      }
      None => missing.push((blob, language)),
    }
  }
  let cached_count = summaries.len();

  // the misses and the manifests in one process
  let manifests: Vec<(String, String)> = roots.manifests().map(|(dir, blob)| (dir.to_string(), blob.to_string())).collect();
  let tsconfigs: Vec<(String, String)> = roots.all_tsconfigs().map(|(path, blob)| (path, blob.to_string())).collect();
  let names: Vec<String> = missing
    .iter()
    .map(|(blob, _)| blob.clone())
    .chain(manifests.iter().map(|(_, blob)| blob.clone()))
    .chain(tsconfigs.iter().map(|(_, blob)| blob.clone()))
    .collect();
  let mut contents = read_blob_bytes(git, repository_path, &names, MAX_BYTES)?.into_iter();
  let sources: Vec<Option<Vec<u8>>> = contents.by_ref().take(missing.len()).collect();
  let crates: HashMap<String, String> = manifests
    .iter()
    .zip(contents.by_ref())
    .map(|((dir, _), content)| {
      let name = content.as_deref().and_then(crate_name).unwrap_or_else(|| dir.rsplit('/').next().unwrap_or(dir).to_string());
      (dir.clone(), name.replace('-', "_"))
    })
    .collect();
  let aliases: HashMap<String, TsAliases> = tsconfigs
    .iter()
    .zip(contents)
    .filter_map(|((path, _), content)| {
      let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
      Some((dir.to_string(), ts_aliases(dir, &String::from_utf8_lossy(content.as_deref()?))))
    })
    .filter(|(_, aliases)| !aliases.paths.is_empty())
    .collect();
  let read_time = started.elapsed();

  let parse_started = Instant::now();
  let parsed = parse_all(&missing, sources);
  let parse_time = parse_started.elapsed();
  let fresh: Vec<((u8, &str), &FileSummary)> = missing
    .iter()
    .zip(&parsed)
    .filter_map(|((blob, language), summary)| {
      summary
        .as_ref()
        .filter(|summary| !summary.truncated)
        .map(|summary| ((language.id(), blob.as_str()), summary))
    })
    .collect();
  cache.put_blob_summaries(version, &fresh);
  let parsed_count = parsed.iter().flatten().count();
  for ((blob, _), summary) in missing.into_iter().zip(parsed) {
    if let Some(summary) = summary {
      summaries.insert(blob, Arc::new(summary));
    }
  }

  let mut stats = ScanStats {
    cached: cached_count,
    parsed: parsed_count,
    read_time,
    parse_time,
    ..ScanStats::default()
  };
  for summary in summaries.values() {
    let language = stats.blobs.entry(summary.language).or_default();
    language.blobs += 1;
    language.bytes += u64::from(summary.total_bytes);
    language.error_bytes += u64::from(summary.error_bytes);
    language.degraded += usize::from(summary.is_degraded());
    language.truncated += usize::from(summary.truncated);
  }
  debug!(cached = cached_count, parsed = parsed_count, ?read_time, ?parse_time, "symbol summaries");
  Ok(SymbolInput {
    summaries,
    roots,
    crates,
    aliases,
    stats,
  })
}

/// The summaries of `blobs` from their `sources` (`None` for a blob that is missing, binary or too large), parsed by
/// one thread per core, each with its own parsers
fn parse_all(blobs: &[(String, Language)], sources: Vec<Option<Vec<u8>>>) -> Vec<Option<FileSummary>> {
  let jobs: Vec<(Language, Option<Vec<u8>>)> = blobs.iter().map(|(_, language)| *language).zip(sources).collect();
  let results: Vec<Mutex<Option<FileSummary>>> = jobs.iter().map(|_| Mutex::new(None)).collect();
  let next = AtomicUsize::new(0);
  let threads = std::thread::available_parallelism().map_or(1, usize::from).min(jobs.len().max(1));
  std::thread::scope(|scope| {
    for _ in 0..threads {
      scope.spawn(in_current_span(|| {
        loop {
          let index = next.fetch_add(1, Ordering::Relaxed);
          let Some((language, source)) = jobs.get(index) else { break };
          let Some(source) = source else { continue };
          let summary = summarize(*language, source);
          if let Ok(mut slot) = results[index].lock() {
            *slot = Some(summary);
          }
        }
      }));
    }
  });
  results.into_iter().map(|slot| slot.into_inner().ok().flatten()).collect()
}

/// The `name` of the `[package]` table of a `Cargo.toml`
pub(crate) fn crate_name(manifest: &[u8]) -> Option<String> {
  let text = String::from_utf8_lossy(manifest);
  let mut in_package = false;
  for line in text.lines().map(str::trim) {
    if line.starts_with('[') {
      in_package = line == "[package]";
      continue;
    }
    if !in_package {
      continue;
    }
    let Some((key, value)) = line.split_once('=') else { continue };
    if key.trim() == "name" {
      let value = value.trim();
      return value.strip_prefix('"').and_then(|value| value.split('"').next()).map(str::to_string);
    }
  }
  None
}

/// The aliases of a `tsconfig*.json` in `dir`: `compilerOptions.paths` relative to `baseUrl` (default the directory),
/// as paths from the repository root. Comments and trailing commas, which `tsc` accepts, are removed first
pub(crate) fn ts_aliases(dir: &str, text: &str) -> TsAliases {
  let Ok(config) = serde_json::from_str::<serde_json::Value>(&without_comments(text)) else {
    return TsAliases::default();
  };
  let options = &config["compilerOptions"];
  let base = normalize(&join(dir, options["baseUrl"].as_str().unwrap_or(".")));
  let Some(paths) = options["paths"].as_object() else { return TsAliases::default() };
  TsAliases {
    paths: paths
      .iter()
      .map(|(pattern, targets)| {
        let targets = targets
          .as_array()
          .into_iter()
          .flatten()
          .filter_map(|target| Some(normalize(&join(&base, target.as_str()?))))
          .collect();
        (pattern.clone(), targets)
      })
      .collect(),
  }
}

/// JSON with comments (`//`, `/* */`) and trailing commas made JSON, in one pass that knows the strings
fn without_comments(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut chars = text.chars().peekable();
  let mut in_string = false;
  while let Some(c) = chars.next() {
    if in_string {
      out.push(c);
      if c == '\\' {
        out.extend(chars.next());
      } else if c == '"' {
        in_string = false;
      }
      continue;
    }
    match (c, chars.peek()) {
      ('/', Some('/')) => while chars.next_if(|&next| next != '\n').is_some() {},
      ('/', Some('*')) => {
        chars.next();
        let mut previous = ' ';
        for next in chars.by_ref() {
          if previous == '*' && next == '/' {
            break;
          }
          previous = next;
        }
      }
      ('}' | ']', _) => {
        // a comma before the closing bracket, with only whitespace (and the comments left out) between
        let content = out.trim_end().len();
        if out[..content].ends_with(',') {
          out.remove(content - 1);
        }
        out.push(c);
      }
      _ => {
        in_string = c == '"';
        out.push(c);
      }
    }
  }
  out
}

/// `dir/path`, or `path` from the root
pub(crate) fn join(dir: &str, path: &str) -> String {
  if dir.is_empty() || path.starts_with('/') {
    path.trim_start_matches('/').to_string()
  } else {
    format!("{dir}/{path}")
  }
}

/// `path` without `.` segments and with each `..` taking the segment before it away; `None`-like `..` above the root
/// are dropped
pub(crate) fn normalize(path: &str) -> String {
  let mut segments: Vec<&str> = Vec::new();
  for segment in path.split('/') {
    match segment {
      "" | "." => {}
      ".." => {
        segments.pop();
      }
      segment => segments.push(segment),
    }
  }
  segments.join("/")
}
