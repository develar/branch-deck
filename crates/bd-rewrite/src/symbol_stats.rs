//! `bd commits --symbol-stats` (hidden): what the dependencies by symbols find on the commits ahead of the baseline,
//! next to the line dependencies, for the gate that decides whether they order branches (docs/design/
//! symbol-dependencies.md, "Measurements"). It writes the object cache only.

use crate::commits::read_linked_diffs;
use bd_core::branch_view::read_grouping;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;
use trial_check::dependencies::compute_dependencies;
use trial_check::symbol_dependencies::{SymbolEdge, compute_symbol_dependencies};
use trial_check::symbol_scan::read_symbol_input;

#[derive(Debug, Serialize)]
pub struct SymbolStatsOutput {
  pub head: String,
  pub commits: usize,
  pub extractor_version: u32,
  /// By language
  pub languages: BTreeMap<String, LanguageRow>,
  /// Summaries from the object cache, and parsed now
  pub cached: usize,
  pub parsed: usize,
  /// The reads (trees, cache, blobs), the parse and the graph, in milliseconds
  pub read_ms: u128,
  pub parse_ms: u128,
  pub graph_ms: u128,
  /// Edges by lines, by symbols, by both, by symbols only
  pub line_edges: usize,
  pub symbol_edges: usize,
  pub both: usize,
  pub symbols_only: usize,
  /// The links of the symbol edges by their evidence
  pub links_by_evidence: BTreeMap<String, usize>,
  /// The symbols-only edges by the evidence of their first link
  pub symbols_only_by_evidence: BTreeMap<String, usize>,
  /// The entities the commits change, and the pairs of commits that share one without a line edge
  pub entities: usize,
  pub entity_pairs: usize,
  /// A deterministic sample of the symbols-only edges, spread evenly over each evidence
  pub sample: Vec<SampleEdge>,
}

#[derive(Debug, Default, Serialize)]
pub struct LanguageRow {
  pub blobs: usize,
  pub mib: f64,
  pub error_bytes: u64,
  pub degraded: usize,
  pub truncated: usize,
}

#[derive(Debug, Serialize)]
pub struct SampleEdge {
  pub from: String,
  pub from_subject: String,
  pub to: String,
  pub to_subject: String,
  pub name: String,
  pub evidence: &'static str,
  pub used_in: String,
  pub line: u32,
  pub declared_in: String,
}

pub fn symbol_stats(context: &RepoContext, sample_size: usize) -> Result<SymbolStatsOutput, CliError> {
  let grouping = read_grouping(context, None)?;
  let head = &grouping.head;
  let ids = &grouping.commit_ids;
  let diffs = read_linked_diffs(context, ids, false, false)?;
  let linked = diffs.linked();
  let input = read_symbol_input(&context.git, &context.repository, &context.cache(), linked)?;
  let started = Instant::now();
  let graph = compute_symbol_dependencies(linked, &input);
  let graph_ms = started.elapsed().as_millis();

  let lines: BTreeSet<(usize, usize)> = compute_dependencies(linked).iter().map(|edge| (edge.from, edge.to)).collect();
  let symbols: BTreeSet<(usize, usize)> = graph.edges.iter().map(|edge| (edge.from, edge.to)).collect();
  let only: Vec<&SymbolEdge> = graph.edges.iter().filter(|edge| !lines.contains(&(edge.from, edge.to))).collect();
  let mut links_by_evidence: BTreeMap<String, usize> = BTreeMap::new();
  for link in graph.edges.iter().flat_map(|edge| &edge.links) {
    *links_by_evidence.entry(link.evidence.name().to_string()).or_default() += 1;
  }
  let mut by_evidence: BTreeMap<&'static str, Vec<&SymbolEdge>> = BTreeMap::new();
  for edge in &only {
    if let Some(link) = edge.links.first() {
      by_evidence.entry(link.evidence.name()).or_default().push(edge);
    }
  }

  // the subjects, for a sample a human labels
  let subjects: HashMap<&str, &str> = grouping
    .branches
    .iter()
    .flat_map(|branch| &branch.commits)
    .chain(&grouping.unassigned)
    .map(|commit| (commit.id.as_str(), commit.subject.as_str()))
    .collect();
  let short = context.short_ids(ids.iter().map(String::as_str));
  let mut sample = Vec::new();
  for edges in by_evidence.values() {
    let step = edges.len().div_ceil(sample_size.max(1)).max(1);
    for edge in edges.iter().step_by(step).take(sample_size) {
      let Some(link) = edge.links.first() else { continue };
      sample.push(SampleEdge {
        from: short.get(&ids[edge.from]).to_string(),
        from_subject: subjects.get(ids[edge.from].as_str()).copied().unwrap_or_default().to_string(),
        to: short.get(&ids[edge.to]).to_string(),
        to_subject: subjects.get(ids[edge.to].as_str()).copied().unwrap_or_default().to_string(),
        name: link.name.clone(),
        evidence: link.evidence.name(),
        used_in: link.used_in.clone(),
        line: link.line,
        declared_in: link.declared_in.clone(),
      });
    }
  }

  // two commits that change the same entity without a line edge between them
  let mut owners: HashMap<&str, Vec<usize>> = HashMap::new();
  for (position, entities) in graph.entities.iter().enumerate() {
    for entity in entities {
      owners.entry(entity.as_str()).or_default().push(position);
    }
  }
  let mut entity_pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
  for positions in owners.values().filter(|positions| positions.len() <= 8) {
    for (index, &a) in positions.iter().enumerate() {
      for &b in &positions[index + 1..] {
        if !lines.contains(&(b, a)) {
          entity_pairs.insert((b, a));
        }
      }
    }
  }

  let stats = &input.stats;
  Ok(SymbolStatsOutput {
    head: short.get(head).to_string(),
    commits: ids.len(),
    extractor_version: code_symbols::extract::extractor_version(),
    languages: stats
      .blobs
      .iter()
      .map(|(language, row)| {
        let row = LanguageRow {
          blobs: row.blobs,
          #[expect(clippy::cast_precision_loss, reason = "a size in MiB for a human")]
          mib: (row.bytes as f64 / f64::from(1 << 20) * 100.0).round() / 100.0,
          error_bytes: row.error_bytes,
          degraded: row.degraded,
          truncated: row.truncated,
        };
        (format!("{language:?}"), row)
      })
      .collect(),
    cached: stats.cached,
    parsed: stats.parsed,
    read_ms: stats.read_time.as_millis(),
    parse_ms: stats.parse_time.as_millis(),
    graph_ms,
    line_edges: lines.len(),
    symbol_edges: symbols.len(),
    both: symbols.intersection(&lines).count(),
    symbols_only: only.len(),
    links_by_evidence,
    symbols_only_by_evidence: by_evidence.iter().map(|(evidence, edges)| ((*evidence).to_string(), edges.len())).collect(),
    entities: owners.len(),
    entity_pairs: entity_pairs.len(),
    sample,
  })
}
