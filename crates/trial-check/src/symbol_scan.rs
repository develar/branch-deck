//! The syntax summaries of the source files that the commits ahead of the baseline change (`code_symbols`), before and
//! after each commit, by blob id: from the session, else from the object cache (`blob_summary`), else from one
//! `cat-file --batch` of the blobs the cache misses, parsed in parallel and written back in one transaction. The same
//! process reads the `Cargo.toml` and `tsconfig*.json` files of the directories above the files (`ModuleRoots`, one
//! `cat-file --batch` of the trees at HEAD, or at the head a review plan reads), for the names of the crates and the path
//! aliases of TypeScript, each `tsconfig*.json` with its `extends` chain (`js_syntax::tsconfig`). The blobs of a caller
//! join that process (`read_symbol_input_with`).

use crate::diff_scan::CommitDiff;
use crate::module_roots::{FileAt, ModuleRoots};
use anyhow::Result;
use code_symbols::extract::{MAX_BYTES, summarize};
use code_symbols::language::{Language, language_of, summary_key};
use code_symbols::summary::FileSummary;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::MAX_BLOB_BYTES;
use git_ops::object_texts::read_blob_bytes;
use js_syntax::tsconfig::{self, Aliases};
use object_cache::cache::ObjectCache;
use object_cache::commit_diff::FileChange;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// What the scan read, for `brd commits --symbol-stats`
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
  /// The TypeScript path aliases of the `tsconfig*.json` files of each directory (`ts_aliases`), by name, those that
  /// give a specifier nothing left out
  pub aliases: HashMap<String, Vec<Aliases>>,
  pub stats: ScanStats,
}

/// The source blobs of `diffs`, each with its language: the old blob in the language of the old path, the new one in
/// the language of the new path
fn source_blobs<'a>(changes: impl IntoIterator<Item = &'a FileChange>) -> Vec<SourceBlob> {
  let mut seen: HashSet<&str> = HashSet::new();
  let mut blobs = Vec::new();
  for change in changes {
    let old_path = change.old_path.as_deref().unwrap_or(&change.path);
    for (blob, path) in [(&change.old_blob, old_path), (&change.new_blob, change.path.as_str())] {
      let (Some(blob), Some(language)) = (blob.as_deref(), language_of(path)) else { continue };
      if seen.insert(blob) {
        blobs.push(SourceBlob {
          id: blob.to_string(),
          language,
          path: path.to_string(),
        });
      }
    }
  }
  blobs
}

/// A blob to summarize: its id, its language, and the path it is of — which says the kind of a Bazel file, as the
/// same bytes summarize differently as a `BUILD` and as a `.bzl` (`summary_key`)
struct SourceBlob {
  id: String,
  language: Language,
  path: String,
}

/// The summaries of the source files `diffs` change, with the module structure above them at HEAD
pub fn read_symbol_input(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, diffs: &[CommitDiff]) -> Result<SymbolInput> {
  Ok(read_symbol_input_with(git, repository_path, "HEAD", cache, diffs, &[])?.0)
}

/// `read_symbol_input` with the module structure at `rev` (the head a review plan reads, which may not be checked out)
/// above the source files, and the bytes of
/// the `extra` blobs of the caller in its one `cat-file --batch`, in order: `None` for a missing or binary one and one
/// larger than the texts of the diffs (`MAX_BLOB_BYTES`), so that the caller reads a blob the memory of the diffs
/// would hold as that memory holds it; a source blob or a manifest larger than a parsed file (`MAX_BYTES`) is none, as
/// before. With no summary to parse and no manifest to read, the batch reads
/// `extra` alone; with no `extra` either, it runs no process
#[instrument(skip_all, fields(commits = diffs.len(), extra = extra.len()))]
pub fn read_symbol_input_with(
  git: &GitCommandExecutor,
  repository_path: &str,
  rev: &str,
  cache: &ObjectCache,
  diffs: &[CommitDiff],
  extra: &[String],
) -> Result<(SymbolInput, Vec<Option<Vec<u8>>>)> {
  let started = Instant::now();
  let version = file_diffs::summaries_version();
  let wanted = source_blobs(diffs.iter().flat_map(|diff| &diff.files));
  let sources: Vec<&str> = diffs
    .iter()
    .flat_map(|diff| &diff.files)
    .filter(|change| language_of(&change.path).is_some())
    .map(|change| change.path.as_str())
    .collect();
  let roots = ModuleRoots::read(git, repository_path, rev, sources.iter().copied())?;

  let (mut summaries, missing) = cached_summaries(cache, version, wanted);
  let cached_count = summaries.len();

  // the misses, the manifests and the extra blobs in one process
  let manifests: Vec<(String, String)> = roots.manifests().map(|(dir, blob)| (dir.to_string(), blob.to_string())).collect();
  let tsconfigs: Vec<(String, String)> = roots.all_tsconfigs().map(|(path, blob)| (path, blob.to_string())).collect();
  let names: Vec<String> = missing
    .iter()
    .map(|blob| blob.id.clone())
    .chain(manifests.iter().map(|(_, blob)| blob.clone()))
    .chain(tsconfigs.iter().map(|(_, blob)| blob.clone()))
    .chain(extra.iter().cloned())
    .collect();
  // one batch under the larger limit of the extra blobs; a source or a manifest over the limit of a parsed file is none
  let parsed_size = |content: Option<Vec<u8>>| content.filter(|bytes| bytes.len() <= MAX_BYTES);
  let mut contents = read_blob_bytes(git, repository_path, &names, MAX_BLOB_BYTES)?.into_iter();
  let sources: Vec<Option<Vec<u8>>> = contents.by_ref().take(missing.len()).map(parsed_size).collect();
  let crates: HashMap<String, String> = manifests
    .iter()
    .zip(contents.by_ref().map(parsed_size))
    .map(|((dir, _), content)| {
      let name = content.as_deref().and_then(crate_name).unwrap_or_else(|| dir.rsplit('/').next().unwrap_or(dir).to_string());
      (dir.clone(), name.replace('-', "_"))
    })
    .collect();
  let configs: Vec<(String, Option<Vec<u8>>)> = tsconfigs.into_iter().map(|(path, _)| path).zip(contents.by_ref().map(parsed_size)).collect();
  let extra: Vec<Option<Vec<u8>>> = contents.collect();
  let aliases = ts_aliases(git, repository_path, rev, &roots, configs)?;
  let read_time = started.elapsed();

  let parse_started = Instant::now();
  let parsed_count = parse_and_store(cache, version, missing, sources, &mut summaries);
  let parse_time = parse_started.elapsed();

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
  let input = SymbolInput {
    summaries,
    roots,
    crates,
    aliases,
    stats,
  };
  Ok((input, extra))
}

/// The summaries of the source blobs of `changes`, before and after each, by blob id: from the object cache, else from
/// one `cat-file --batch` of the misses (none when the cache has them all), parsed in parallel and written back. A blob
/// that is missing, binary or larger than a parsed file (`MAX_BYTES`) has none
#[instrument(skip_all)]
pub fn read_change_summaries<'a>(
  git: &GitCommandExecutor,
  repository_path: &str,
  cache: &ObjectCache,
  changes: impl IntoIterator<Item = &'a FileChange>,
) -> Result<HashMap<String, Arc<FileSummary>>> {
  let version = file_diffs::summaries_version();
  let (mut summaries, missing) = cached_summaries(cache, version, source_blobs(changes));
  if missing.is_empty() {
    return Ok(summaries);
  }
  let names: Vec<String> = missing.iter().map(|blob| blob.id.clone()).collect();
  let sources = read_blob_bytes(git, repository_path, &names, MAX_BYTES)?;
  parse_and_store(cache, version, missing, sources, &mut summaries);
  Ok(summaries)
}

/// The summaries of `wanted` that the object cache has, by blob id, and the blobs it misses
fn cached_summaries(cache: &ObjectCache, version: u32, wanted: Vec<SourceBlob>) -> (HashMap<String, Arc<FileSummary>>, Vec<SourceBlob>) {
  let keys: Vec<(u8, &str)> = wanted.iter().map(|blob| (summary_key(blob.language, &blob.path), blob.id.as_str())).collect();
  let cached: Vec<Option<FileSummary>> = cache.get_blob_summaries(version, &keys);
  let mut summaries: HashMap<String, Arc<FileSummary>> = HashMap::with_capacity(wanted.len());
  let mut missing: Vec<SourceBlob> = Vec::new();
  for (blob, summary) in wanted.into_iter().zip(cached) {
    match summary {
      Some(summary) => {
        summaries.insert(blob.id, Arc::new(summary));
      }
      None => missing.push(blob),
    }
  }
  (summaries, missing)
}

/// Parses `missing` from their `sources` into `summaries`, and writes those that are not truncated to the object cache
/// in one transaction; the count of the parsed blobs
fn parse_and_store(cache: &ObjectCache, version: u32, missing: Vec<SourceBlob>, sources: Vec<Option<Vec<u8>>>, summaries: &mut HashMap<String, Arc<FileSummary>>) -> usize {
  let parsed = parse_all(&missing, sources);
  let fresh: Vec<((u8, &str), &FileSummary)> = missing
    .iter()
    .zip(&parsed)
    .filter_map(|(blob, summary)| {
      summary
        .as_ref()
        .filter(|summary| !summary.truncated)
        .map(|summary| ((summary_key(blob.language, &blob.path), blob.id.as_str()), summary))
    })
    .collect();
  cache.put_blob_summaries(version, &fresh);
  let parsed_count = parsed.iter().flatten().count();
  for (blob, summary) in missing.into_iter().zip(parsed) {
    if let Some(summary) = summary {
      summaries.insert(blob.id, Arc::new(summary));
    }
  }
  parsed_count
}

/// The summaries of `blobs` from their `sources` (`None` for a blob that is missing, binary or too large), parsed by
/// one thread per core, each with its own parsers
fn parse_all(blobs: &[SourceBlob], sources: Vec<Option<Vec<u8>>>) -> Vec<Option<FileSummary>> {
  let jobs: Vec<(&SourceBlob, Option<Vec<u8>>)> = blobs.iter().zip(sources).collect();
  let results: Vec<Mutex<Option<FileSummary>>> = jobs.iter().map(|_| Mutex::new(None)).collect();
  let next = AtomicUsize::new(0);
  let threads = std::thread::available_parallelism().map_or(1, usize::from).min(jobs.len().max(1));
  std::thread::scope(|scope| {
    for _ in 0..threads {
      scope.spawn(in_current_span(|| {
        loop {
          let index = next.fetch_add(1, Ordering::Relaxed);
          let Some((blob, source)) = jobs.get(index) else { break };
          let Some(source) = source else { continue };
          let summary = summarize(blob.language, &blob.path, source);
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

/// The path aliases of each configuration of `configs` (its path, and its bytes or none), with the `extends` chain tsc
/// applies (`js_syntax::tsconfig::aliases`), by its directory: the configurations of a directory by name, those that
/// give a specifier nothing left out. `configs` are the `tsconfig*.json` files of the directories above the files, read
/// in the batch of the summaries, so a chain of those costs nothing; a name tried that a directory `roots` read lacks (a
/// name without `.json`) costs nothing either. Any other configuration a chain names is read in one `cat-file --batch`
/// of what a pass lacks, a pass a level of the chains, wherever it lies: by its blob when a directory `roots` read lists
/// it (`./base.json` beside the files), else at `rev` by its path. The reads stop where `tsconfig::aliases` stops
/// following a chain (`tsconfig::MAX_EXTENDS`): it has the whole answer after at most `MAX_EXTENDS - 1` passes, so the
/// stop at `MAX_EXTENDS` is a guard that no answer reaches
fn ts_aliases(git: &GitCommandExecutor, repository_path: &str, rev: &str, roots: &ModuleRoots, configs: Vec<(String, Option<Vec<u8>>)>) -> Result<HashMap<String, Vec<Aliases>>> {
  let mut paths: Vec<String> = configs.iter().filter(|(_, text)| text.is_some()).map(|(path, _)| path.clone()).collect();
  paths.sort_unstable();
  let mut texts: HashMap<String, Option<Vec<u8>>> = configs.into_iter().collect();
  let mut pass = 0;
  loop {
    // path -> the name `cat-file` reads it by, of each configuration a chain names that no read has given yet
    let mut wanted: BTreeMap<String, String> = BTreeMap::new();
    let mut read = |path: &str| {
      if let Some(text) = texts.get(path) {
        return text.clone();
      }
      match roots.file_at(path) {
        FileAt::Blob(blob) => {
          wanted.insert(path.to_string(), blob.to_string());
        }
        FileAt::Unknown => {
          wanted.insert(path.to_string(), format!("{rev}:{path}"));
        }
        FileAt::Absent => {}
      }
      None
    };
    let mut aliases: HashMap<String, Vec<Aliases>> = HashMap::new();
    for path in &paths {
      let Some(config) = tsconfig::aliases(path, &mut read).filter(|config| !config.is_empty()) else {
        continue;
      };
      let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
      aliases.entry(dir.to_string()).or_default().push(config);
    }
    if wanted.is_empty() || pass == tsconfig::MAX_EXTENDS {
      return Ok(aliases);
    }
    pass += 1;
    let names: Vec<String> = wanted.values().cloned().collect();
    let contents = read_blob_bytes(git, repository_path, &names, MAX_BYTES)?;
    texts.extend(wanted.into_keys().zip(contents));
  }
}
