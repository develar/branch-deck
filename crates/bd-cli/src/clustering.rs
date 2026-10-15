//! The pure core of `bd suggest` (docs/guidelines/agent-cli.md): the commits ahead of the baseline, described by their
//! prefix, issue keys, conventional scope, files and dependencies, clustered into branches. No git runs here;
//! `suggest.rs` reads the input, and checks the result through the oracle of `cluster_with_repair`.
//!
//! 1. Hard units (union-find): the dependency edges, fixup -> target, the same explicit prefix or issue group (an
//!    anchor, unless `fresh`), and the same issue key of the subject among the commits without a prefix. An edge
//!    between two anchors joins nothing: it is an `anchor_dependency` warning. With stacked branches a dependency
//!    joins nothing either: it is an edge between units, and only the edges of a cycle join.
//! 2. A soft score between two clusters, `S = max(R, P × Kp) × K + adj`, in integer units of 1/10 000 so that ties are
//!    exact. R links by issue keys and scopes, P by shared paths (IDF-weighted Jaccard), K lowers both when the issue
//!    projects or the scopes disagree, and adj adds a little for neighbours in the log.
//! 3. Agglomerative merges, the best pair first and on a tie the oldest; two anchors never merge, no merge goes over
//!    `max_commits`, no merge makes a cycle of dependencies, and the merging stops under `MERGE_THRESHOLD`.
//! 4. With stacked branches, `place_clusters` gives each cluster a parent, so that every cluster it depends on is an
//!    ancestor (`stack_clusters`).

use crate::areas::{DEFAULT_AREA_DEPTH, area_lines, area_of};
use git_ops::model::sanitize_branch_name;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
use std::hash::BuildHasher;
use sync_core::move_commits::validate_branch_name;
use sync_utils::issue_pattern::issue_keys_in;

/// Scores are integers in units of 1/10 000
const ONE: u32 = 10_000;
/// The score a soft merge needs, calibrated by the golden test of `clustering_test.rs`
pub const MERGE_THRESHOLD: u32 = 4_000;
/// A merge by paths less than this much over the threshold, or one that only adjacency lifted over it, makes its cluster
/// `weak`
const WEAK_MARGIN: u32 = 1_000;
/// No soft merge makes a cluster of more commits
pub const DEFAULT_MAX_COMMITS: usize = 40;
/// How many times `cluster_with_repair` clusters again after a conflict
pub const MAX_REPAIR_ROUNDS: usize = 5;

/// R, with stacked branches: a commit of one side depends on a commit of the other, and a side without an anchor has
/// no issue key, so the commit follows up on the other side; a commit with a key stacks on it instead
const DEPENDENCY: u32 = 9_500;
/// R: a body names an issue key of the other side
const MENTION: u32 = 9_000;
/// R: both sides name a third key
const CO_MENTION: u32 = 7_000;
/// R: the same conventional scope, when neither side has an issue key
const SCOPE_WITHOUT_KEYS: u32 = 8_000;
/// R: the same conventional scope, when a side has an issue key
const SCOPE_WITH_KEYS: u32 = 5_000;
/// K: the issue keys of the two sides are of different projects (`IJAI` and `IJPL`)
const OTHER_PROJECT: u32 = 3_000;
/// Kp: the issue keys differ, in the same project; on P only
const OTHER_KEY_PATHS: u32 = 7_000;
/// K: both sides have scopes, and none in common
const OTHER_SCOPE: u32 = 5_000;
/// adj: the two sides have neighbouring commits in the log
const ADJACENT: u32 = 500;

/// The weight of an area feature; a module, a package or a file weighs 1
const AREA_WEIGHT: f64 = 0.5;
/// The weight of a peripheral file: under `PERIPHERAL_SHARE` % of the commit's lines and outside its dominant area
const PERIPHERAL_WEIGHT: f64 = 0.2;
const PERIPHERAL_SHARE: u32 = 5;
/// The directories a module ends before; the package is the directory after them
const MODULE_MARKERS: [&str; 7] = ["src", "testSrc", "test", "tests", "resources", "gen", ANONYMOUS_MARKER];
/// What `anonymize` writes for a module marker of a file path; a hashed name is hexadecimal, so it never equals this
pub const ANONYMOUS_MARKER: &str = "@";

/// A name from a scope needs this share of the commits, in percent
const NAME_SCOPE_SHARE: usize = 60;
/// A name from a subject word needs this share of the subjects, in percent
const NAME_WORD_SHARE: usize = 50;
/// Words a name never takes
const STOPWORDS: &[&str] = &[
  "add", "after", "all", "also", "and", "any", "are", "before", "both", "but", "can", "does", "each", "every", "first", "fix", "for", "from", "gets", "has", "have", "into", "its",
  "keep", "longer", "make", "more", "new", "not", "now", "off", "one", "only", "other", "out", "over", "own", "per", "remove", "same", "should", "so", "still", "take", "test",
  "than", "that", "the", "their", "them", "then", "they", "this", "three", "through", "two", "update", "use", "when", "while", "with", "without",
];

/// What the clustering reads, oldest commit first; a commit is named by its position
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterInput {
  pub commits: Vec<CommitFeatures>,
  /// `[from, to]`: `from` (the newer) depends on `to`
  #[serde(default)]
  pub dependencies: Vec<[usize; 2]>,
  /// Stacked branches are on (`branchdeck.stacks`): a prefix path is an anchor by its leaf, and a dependency between
  /// two units stacks one cluster on the other instead of joining them
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub stacks: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitFeatures {
  /// The sanitized `(prefix)` of the subject: a path with stacked branches on
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prefix: Option<String>,
  /// The group now: the prefix (its leaf with stacked branches on), else the issue group of the key, else the issue
  /// key; `None` when unassigned
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub group: Option<String>,
  /// The issue key of the subject
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key: Option<String>,
  /// The issue group of `key` now (`refs/branch-deck/issue-groups`): the anchor of a commit without a prefix
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub issue_group: Option<String>,
  /// The conventional scope of the subject, lowercase
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub scope: Option<String>,
  /// The issue keys the body names, without `key`, sorted
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub mentions: Vec<String>,
  /// For a fixup, the position of its target
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fixup_of: Option<usize>,
  /// Without hub files, lock files and generated files
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<FileLines>,
  /// `subject_words` of the subject, for a name; empty in an anonymized input
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileLines {
  pub path: String,
  /// Added plus deleted lines; 0 for a binary file
  pub lines: u32,
}

#[derive(Debug, Clone)]
pub struct ClusterOptions {
  /// Ignore the prefixes: no anchors, and the issue keys of prefixed commits join too
  pub fresh: bool,
  pub max_commits: usize,
  /// Hard edges a repair added
  pub repairs: Vec<RepairEdge>,
}

impl Default for ClusterOptions {
  fn default() -> Self {
    Self {
      fresh: false,
      max_commits: DEFAULT_MAX_COMMITS,
      repairs: Vec::new(),
    }
  }
}

/// A commit that conflicted, joined to a commit it needs, in repair round `round` (from 1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairEdge {
  pub commit: usize,
  pub needs: usize,
  pub round: usize,
  /// The edge stacks the cluster of `commit` on the cluster of `needs`, as a dependency does with stacked branches,
  /// instead of joining their units
  pub stack: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clustering {
  /// Ordered by their oldest commit
  pub clusters: Vec<Cluster>,
  pub warnings: Vec<Warning>,
  /// The hard unit of each commit, an index into `unit_anchors`
  pub unit_of: Vec<usize>,
  /// The anchor of each hard unit, ordered by their oldest commit
  pub unit_anchors: Vec<Option<String>>,
  /// `[from, to]` between clusters, sorted: a commit of `from` depends on a commit of `to`, or a repair stacked them
  pub dependencies: Vec<[usize; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cluster {
  /// Oldest first
  pub commits: Vec<usize>,
  /// The prefix of its anchor unit
  pub anchor: Option<String>,
  /// The issue keys of its subjects, by their oldest commit
  pub keys: Vec<String>,
  /// The hard units it was merged from
  pub units: usize,
  /// The units that are one whole issue key without an anchor
  pub issue_units: usize,
  /// What its soft merges went by, in merge order
  pub links: Vec<Link>,
  /// A merge by paths near the threshold, or one adjacency lifted over it, made it
  pub weak: bool,
  /// The repair rounds whose edges joined its units
  pub repair_rounds: Vec<usize>,
}

/// What a soft merge went by
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
  /// A commit of one side depends on a commit of the other
  Dependency,
  /// A body names this issue key of the other side
  Mention(String),
  /// Both sides name this key
  CoMention(String),
  Scope(String),
  /// Shared paths, mostly in this area
  Paths(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
  /// A commit of the first anchor depends on (or is a fixup of) a commit of the second: the anchors stay apart
  AnchorDependency { anchors: [String; 2], fixup: bool },
}

/// A conflict the check of a clustering found: the commit, and the older commits it likely needs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
  pub commit: usize,
  pub needs: Vec<usize>,
}

/// The clustering the check accepted, or the last one tried
#[derive(Debug, Clone)]
pub struct Repaired {
  pub clustering: Clustering,
  /// The repair rounds run
  pub rounds: usize,
  /// The conflicts of the last check; empty when it found none
  pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClusterChange {
  /// Exactly the commits of one branch now, under its name
  Kept,
  /// Exactly the commits of one branch now, under another name
  Renamed,
  /// The whole commits of two or more branches now, or of one with unassigned commits
  Merged,
  /// A part of one branch now
  Split,
  /// Anything else: unassigned commits, or parts of several branches
  New,
}

/// Cluster `input`, then check the result with `check` and, while it finds conflicts, join each conflicting commit to
/// the commits it needs as hard edges and cluster again, at most `MAX_REPAIR_ROUNDS` times. An edge is added only
/// between two hard units, so also inside a cluster whose units build apart (an issue group), and never between two
/// anchors, so such a conflict stays. With stacked branches, a commit that needs a commit of another cluster first
/// stacks its cluster on that one; only when the same conflict comes again do their units join
pub fn cluster_with_repair<E>(input: &ClusterInput, options: &ClusterOptions, mut check: impl FnMut(&Clustering) -> Result<Vec<Conflict>, E>) -> Result<Repaired, E> {
  let mut options = options.clone();
  let mut rounds = 0;
  loop {
    let clustering = cluster(input, &options);
    let conflicts = check(&clustering)?;
    if conflicts.is_empty() || rounds == MAX_REPAIR_ROUNDS {
      return Ok(Repaired { clustering, rounds, conflicts });
    }
    let count = clustering.unit_of.len();
    let mut cluster_of = vec![usize::MAX; count];
    for (index, cluster) in clustering.clusters.iter().enumerate() {
      for &commit in &cluster.commits {
        cluster_of[commit] = index;
      }
    }
    let joinable = |a: usize, b: usize| {
      let (a, b) = (clustering.unit_of[a], clustering.unit_of[b]);
      a != b && !(clustering.unit_anchors[a].is_some() && clustering.unit_anchors[b].is_some())
    };
    let stacked_before = |commit: usize, needs: usize| options.repairs.iter().any(|repair| repair.stack && repair.commit == commit && repair.needs == needs);
    // one edge per pair of units: each joins or stacks two units, so the rounds end
    let mut pairs: BTreeSet<(usize, usize, bool)> = BTreeSet::new();
    let added: Vec<RepairEdge> = conflicts
      .iter()
      .flat_map(|conflict| conflict.needs.iter().map(|&needs| (conflict.commit, needs)))
      .filter(|&(commit, needs)| commit < count && needs < count)
      .filter_map(|(commit, needs)| {
        let stack = input.stacks && cluster_of[commit] != cluster_of[needs] && !stacked_before(commit, needs);
        (stack || joinable(commit, needs)).then_some((commit, needs, stack))
      })
      .filter(|&(commit, needs, stack)| {
        let (a, b) = (clustering.unit_of[commit], clustering.unit_of[needs]);
        pairs.insert((a.min(b), a.max(b), stack))
      })
      .map(|(commit, needs, stack)| RepairEdge {
        commit,
        needs,
        round: rounds + 1,
        stack,
      })
      .collect();
    if added.is_empty() {
      return Ok(Repaired { clustering, rounds, conflicts });
    }
    rounds += 1;
    options.repairs.extend(added);
  }
}

/// Cluster the commits of `input`: hard units, then soft merges
pub fn cluster(input: &ClusterInput, options: &ClusterOptions) -> Clustering {
  let mut features = FeatureTable::default();
  let commit_features: Vec<Vec<(u32, f64)>> = input.commits.iter().map(|commit| features.of_commit(&commit.files)).collect();
  let HardUnits {
    mut groups,
    warnings,
    dependencies: unit_edges,
    declared,
  } = hard_units(input, options, &commit_features);
  let mut unit_of = vec![0; input.commits.len()];
  for (unit, group) in groups.iter().enumerate() {
    for &commit in &group.commits {
      unit_of[commit] = unit;
    }
  }
  let unit_anchors = groups.iter().map(|group| group.anchor.clone()).collect();
  let scorer = Scorer::new(&groups, &features, options.max_commits);

  let count = groups.len();
  // the groups each group depends on: with stacked branches, the dependencies that joined no units; and the groups
  // each group must build on, also by the parents its prefixes declare
  let mut needs: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); count];
  for [from, to] in unit_edges {
    needs[from].insert(to);
  }
  let mut below = needs.clone();
  for [from, to] in declared {
    below[from].insert(to);
  }
  let linked = |needs: &[BTreeSet<usize>], a: usize, b: usize| needs[a].contains(&b) || needs[b].contains(&a);
  let mut alive = vec![true; count];
  // scores[i][j] for i < j, and the best score of each row with its column, the lowest on a tie
  let mut scores: Vec<Vec<Option<Scored>>> = (0..count)
    .map(|i| {
      (0..count)
        .map(|j| if i < j { scorer.score(&groups[i], &groups[j], linked(&needs, i, j)) } else { None })
        .collect()
    })
    .collect();
  let row_best = |scores: &[Vec<Option<Scored>>], alive: &[bool], row: usize| {
    let mut best: Option<(u32, usize)> = None;
    for column in (row + 1..count).filter(|&column| alive[column]) {
      if let Some(scored) = &scores[row][column]
        && best.is_none_or(|(score, _)| scored.score > score)
      {
        best = Some((scored.score, column));
      }
    }
    best
  };
  let mut best: Vec<Option<(u32, usize)>> = (0..count).map(|row| row_best(&scores, &alive, row)).collect();
  loop {
    // the best pair, on a tie the oldest: the lowest row of the highest score, at its lowest column
    let mut pick: Option<(u32, usize, usize)> = None;
    for row in (0..count).filter(|&row| alive[row]) {
      if let Some((score, column)) = best[row]
        && score >= MERGE_THRESHOLD
        && pick.is_none_or(|(picked, _, _)| score > picked)
      {
        pick = Some((score, row, column));
      }
    }
    let Some((_, i, j)) = pick else { break };
    // a merge with a path through a third group makes a cycle, and no stack builds a cycle
    if reaches_through(&below, i, j) || reaches_through(&below, j, i) {
      scores[i][j] = None;
      best[i] = row_best(&scores, &alive, i);
      continue;
    }
    let scored = scores[i][j].take().expect("the best pair has a score");
    let link = scorer.link(&groups[i], &groups[j], linked(&needs, i, j), &scored);
    let other = std::mem::take(&mut groups[j]);
    groups[i].absorb(other, link, scored.weak);
    absorb_edges(&mut needs, i, j);
    absorb_edges(&mut below, i, j);
    alive[j] = false;
    best[j] = None;
    for k in (0..count).filter(|&k| alive[k] && k != i) {
      let (low, high) = (i.min(k), i.max(k));
      scores[low][high] = scorer.score(&groups[low], &groups[high], linked(&needs, low, high));
    }
    best[i] = row_best(&scores, &alive, i);
    for k in (0..i).filter(|&k| alive[k]) {
      best[k] = match (best[k], &scores[k][i]) {
        // the best column changed or is gone
        (Some((_, column)), _) if column == i || column == j => row_best(&scores, &alive, k),
        (Some((score, column)), Some(scored)) if scored.score > score || (scored.score == score && i < column) => Some((scored.score, i)),
        (None, Some(scored)) => Some((scored.score, i)),
        (kept, _) => kept,
      };
    }
    for k in (i + 1..j).filter(|&k| alive[k]) {
      if best[k].is_some_and(|(_, column)| column == j) {
        best[k] = row_best(&scores, &alive, k);
      }
    }
  }

  let mut cluster_of = vec![usize::MAX; count];
  for (index, group) in (0..count).filter(|&group| alive[group]).enumerate() {
    cluster_of[group] = index;
  }
  let mut dependencies: Vec<[usize; 2]> = (0..count)
    .filter(|&group| alive[group])
    .flat_map(|group| needs[group].iter().map(move |&needed| (group, needed)))
    .map(|(group, needed)| [cluster_of[group], cluster_of[needed]])
    .collect();
  dependencies.sort_unstable();
  let clusters = groups
    .into_iter()
    .zip(alive)
    .filter(|(_, alive)| *alive)
    .map(|(group, _)| group.into_cluster(input))
    .collect();
  Clustering {
    clusters,
    warnings,
    unit_of,
    unit_anchors,
    dependencies,
  }
}

/// The edges of group `j` become edges of group `i`, which absorbed it
fn absorb_edges(edges: &mut [BTreeSet<usize>], i: usize, j: usize) {
  let absorbed = std::mem::take(&mut edges[j]);
  edges[i].extend(absorbed);
  for targets in edges.iter_mut() {
    if targets.remove(&j) {
      targets.insert(i);
    }
  }
  edges[i].remove(&i);
}

/// Whether `from` reaches `to` over the edges through at least one other group
fn reaches_through(needs: &[BTreeSet<usize>], from: usize, to: usize) -> bool {
  let mut seen: HashSet<usize> = HashSet::new();
  let mut stack: Vec<usize> = needs[from].iter().copied().filter(|&needed| needed != to).collect();
  while let Some(group) = stack.pop() {
    if group == to {
      return true;
    }
    if seen.insert(group) {
      stack.extend(needs[group].iter().copied());
    }
  }
  false
}

/// The interned path features, with the area each one comes from
#[derive(Default)]
struct FeatureTable {
  ids: HashMap<(FeatureKind, String), u32>,
  areas: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FeatureKind {
  Area,
  Module,
  Package,
  File,
}

impl FeatureTable {
  fn id(&mut self, kind: FeatureKind, value: &str, area: &str) -> u32 {
    let next = u32::try_from(self.areas.len()).unwrap_or(u32::MAX);
    *self.ids.entry((kind, value.to_string())).or_insert_with(|| {
      self.areas.push(area.to_string());
      next
    })
  }

  /// The features of a commit's files with their weights, sorted by id: per file its area, module, package and path
  fn of_commit(&mut self, files: &[FileLines]) -> Vec<(u32, f64)> {
    let total: u32 = files.iter().map(|file| file.lines).sum();
    let dominant = area_lines(files.iter().map(|file| (file.path.as_str(), file.lines)), DEFAULT_AREA_DEPTH)
      .first()
      .map(|&(area, _)| area);
    let mut weights: BTreeMap<u32, f64> = BTreeMap::new();
    for file in files {
      let area = area_of(&file.path, DEFAULT_AREA_DEPTH);
      let peripheral = u64::from(file.lines) * 100 < u64::from(total) * u64::from(PERIPHERAL_SHARE) && Some(area) != dominant;
      let factor = if peripheral { PERIPHERAL_WEIGHT } else { 1.0 };
      let (module, package) = module_and_package(&file.path);
      let mut add = |id: u32, weight: f64| {
        let entry = weights.entry(id).or_default();
        *entry = entry.max(weight);
      };
      add(self.id(FeatureKind::Area, area, area), AREA_WEIGHT * factor);
      add(self.id(FeatureKind::Module, module, area), factor);
      if !package.is_empty() {
        add(self.id(FeatureKind::Package, package, area), factor);
      }
      add(self.id(FeatureKind::File, &file.path, area), factor);
    }
    weights.into_iter().collect()
  }
}

/// The module of a path, the directories before the first module marker (`src`, `test`, …), and the package, the
/// directories after it; without a marker the module is the directory of the file and the package is empty
fn module_and_package(path: &str) -> (&str, &str) {
  let directory = path.rfind('/').map_or("", |end| &path[..end]);
  let mut start = 0;
  for segment in directory.split('/') {
    if MODULE_MARKERS.contains(&segment) {
      let module = if start == 0 { "." } else { &directory[..start - 1] };
      let package = directory.get(start + segment.len() + 1..).unwrap_or("");
      return (module, package);
    }
    start += segment.len() + 1;
  }
  (if directory.is_empty() { "." } else { directory }, "")
}

/// A unit, or a cluster of units
#[derive(Debug, Default)]
struct Group {
  /// Oldest first
  commits: Vec<usize>,
  anchor: Option<String>,
  keys: BTreeSet<String>,
  /// The keys the bodies name, without `keys`
  mentions: BTreeSet<String>,
  scopes: BTreeSet<String>,
  /// Sorted by feature id, the largest weight of each
  features: Vec<(u32, f64)>,
  units: usize,
  issue_units: usize,
  links: Vec<Link>,
  weak: bool,
  repair_rounds: BTreeSet<usize>,
}

impl Group {
  fn absorb(&mut self, other: Group, link: Link, weak: bool) {
    self.commits.extend(other.commits);
    self.commits.sort_unstable();
    if self.anchor.is_none() {
      self.anchor = other.anchor;
    }
    self.keys.extend(other.keys);
    self.mentions.extend(other.mentions);
    self.mentions.retain(|key| !self.keys.contains(key));
    self.scopes.extend(other.scopes);
    self.features = merge_features(&self.features, &other.features);
    self.units += other.units;
    self.issue_units += other.issue_units;
    self.links.extend(other.links);
    self.links.push(link);
    self.weak |= other.weak || weak;
    self.repair_rounds.extend(other.repair_rounds);
  }

  fn into_cluster(self, input: &ClusterInput) -> Cluster {
    let mut keys: Vec<String> = Vec::new();
    for &commit in &self.commits {
      if let Some(key) = &input.commits[commit].key
        && !keys.contains(key)
      {
        keys.push(key.clone());
      }
    }
    Cluster {
      commits: self.commits,
      anchor: self.anchor,
      keys,
      units: self.units,
      issue_units: self.issue_units,
      links: self.links,
      weak: self.weak,
      repair_rounds: self.repair_rounds.into_iter().collect(),
    }
  }
}

fn merge_features(a: &[(u32, f64)], b: &[(u32, f64)]) -> Vec<(u32, f64)> {
  let mut merged: BTreeMap<u32, f64> = a.iter().copied().collect();
  for &(id, weight) in b {
    let entry = merged.entry(id).or_default();
    *entry = entry.max(weight);
  }
  merged.into_iter().collect()
}

/// Union-find over commit positions whose root is the oldest member, with the anchor of each set
struct Sets {
  parents: Vec<usize>,
  anchors: Vec<Option<String>>,
}

impl Sets {
  fn root(&mut self, mut index: usize) -> usize {
    while self.parents[index] != index {
      self.parents[index] = self.parents[self.parents[index]];
      index = self.parents[index];
    }
    index
  }

  /// Join the sets of `a` and `b`; two different anchors stay apart and are returned
  fn union(&mut self, a: usize, b: usize) -> Result<(), [String; 2]> {
    let (a, b) = (self.root(a), self.root(b));
    if a == b {
      return Ok(());
    }
    if let (Some(first), Some(second)) = (&self.anchors[a], &self.anchors[b])
      && first != second
    {
      return Err([first.clone(), second.clone()]);
    }
    let (root, child) = (a.min(b), a.max(b));
    self.parents[child] = root;
    if self.anchors[root].is_none() {
      self.anchors[root] = self.anchors[child].take();
    }
    Ok(())
  }
}

/// The anchor of a commit: its prefix (the leaf of its path with stacked branches), else the issue group of its key;
/// none with `fresh`
fn anchor_of(commit: &CommitFeatures, fresh: bool, stacks: bool) -> Option<&str> {
  if fresh {
    None
  } else {
    commit.prefix.as_deref().map(|prefix| prefix_anchor(prefix, stacks)).or(commit.issue_group.as_deref())
  }
}

/// The branch a prefix names: with stacked branches the leaf of its path
fn prefix_anchor(prefix: &str, stacks: bool) -> &str {
  if stacks { leaf_of_path(prefix) } else { prefix }
}

/// The last level of a stack path
fn leaf_of_path(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}

/// The level before the last of a stack path: the parent it declares
fn parent_of_path(path: &str) -> Option<&str> {
  path.rsplit('/').nth(1)
}

/// What `hard_units` finds: the units, ordered by their oldest commit, the warnings for edges between anchors, and with
/// stacked branches the edges between the units, `[from, to]` by unit
struct HardUnits {
  groups: Vec<Group>,
  warnings: Vec<Warning>,
  /// The dependencies that joined no units
  dependencies: Vec<[usize; 2]>,
  /// A unit and the parent its prefix paths declare
  declared: Vec<[usize; 2]>,
}

fn hard_units(input: &ClusterInput, options: &ClusterOptions, commit_features: &[Vec<(u32, f64)>]) -> HardUnits {
  let count = input.commits.len();
  let anchor_of = |commit| anchor_of(commit, options.fresh, input.stacks);
  let mut sets = Sets {
    parents: (0..count).collect(),
    anchors: input.commits.iter().map(|commit| anchor_of(commit).map(str::to_string)).collect(),
  };
  // the same anchor, then the same issue key among the commits without one; a key that names an anchor is that
  // branch, so its commits join the anchor
  let anchors: HashSet<&str> = input.commits.iter().filter_map(anchor_of).collect();
  let mut first_with: HashMap<(bool, &str), usize> = HashMap::new();
  for (position, commit) in input.commits.iter().enumerate() {
    let label = match (anchor_of(commit), &commit.key) {
      (Some(anchor), _) => Some((true, anchor)),
      (None, Some(key)) => Some((anchors.contains(key.as_str()), key.as_str())),
      _ => None,
    };
    if let Some(label) = label {
      let first = *first_with.entry(label).or_insert(position);
      // a prefix joins one anchor, a key only commits without anchors: neither refuses
      let _ = sets.union(first, position);
    }
  }

  let mut warnings = Vec::new();
  let warn = |warnings: &mut Vec<Warning>, anchors: [String; 2], fixup: bool| {
    let warning = Warning::AnchorDependency { anchors, fixup };
    if !warnings.contains(&warning) {
      warnings.push(warning);
    }
  };
  // with stacked branches a dependency is an edge of the stack, not a join
  let mut stack_edges: Vec<[usize; 2]> = Vec::new();
  let edges = input
    .dependencies
    .iter()
    .map(|&[from, to]| (from, to, false))
    .chain(
      input
        .commits
        .iter()
        .enumerate()
        .filter_map(|(position, commit)| commit.fixup_of.map(|target| (position, target, true))),
    )
    .filter(|&(from, to, _)| from < count && to < count);
  for (from, to, fixup) in edges {
    if input.stacks && !fixup {
      stack_edges.push([from, to]);
    } else if let Err(anchors) = sets.union(from, to) {
      warn(&mut warnings, anchors, fixup);
    }
  }
  let mut joined_in: Vec<(usize, usize)> = Vec::new();
  for repair in options.repairs.iter().filter(|repair| repair.commit < count && repair.needs < count) {
    if repair.stack {
      stack_edges.push([repair.commit, repair.needs]);
    } else if sets.union(repair.commit, repair.needs).is_ok() {
      joined_in.push((repair.commit, repair.round));
    }
  }
  // no stack builds a cycle of units: its edges join, as without stacked branches
  for [from, to] in cyclic_edges(&mut sets, &stack_edges) {
    if let Err(anchors) = sets.union(from, to) {
      warn(&mut warnings, anchors, false);
    }
  }

  let mut by_root: BTreeMap<usize, Group> = BTreeMap::new();
  for (position, commit) in input.commits.iter().enumerate() {
    let root = sets.root(position);
    let group = by_root.entry(root).or_default();
    group.commits.push(position);
    group.keys.extend(commit.key.clone());
    group.mentions.extend(commit.mentions.iter().cloned());
    group.scopes.extend(commit.scope.clone());
    group.features = merge_features(&group.features, &commit_features[position]);
  }
  for (commit, round) in joined_in {
    let root = sets.root(commit);
    if let Some(group) = by_root.get_mut(&root) {
      group.repair_rounds.insert(round);
    }
  }
  let unit_of_root: HashMap<usize, usize> = by_root.keys().enumerate().map(|(unit, &root)| (root, unit)).collect();
  let mut unit_edges: Vec<[usize; 2]> = stack_edges
    .iter()
    .map(|&[from, to]| [unit_of_root[&sets.root(from)], unit_of_root[&sets.root(to)]])
    .filter(|[from, to]| from != to)
    .collect();
  unit_edges.sort_unstable();
  unit_edges.dedup();
  let unit_of_anchor: HashMap<String, usize> = by_root.keys().enumerate().filter_map(|(unit, &root)| Some((sets.anchors[root].clone()?, unit))).collect();
  let mut declared: Vec<[usize; 2]> = Vec::new();
  if input.stacks {
    for (position, prefix) in input
      .commits
      .iter()
      .enumerate()
      .filter_map(|(position, commit)| Some((position, commit.prefix.as_deref()?)))
    {
      let unit = unit_of_root[&sets.root(position)];
      if let Some(&parent) = parent_of_path(prefix).and_then(|parent| unit_of_anchor.get(parent))
        && parent != unit
      {
        declared.push([unit, parent]);
      }
    }
  }
  declared.sort_unstable();
  declared.dedup();
  let groups = by_root
    .into_iter()
    .map(|(root, mut group)| {
      group.anchor.clone_from(&sets.anchors[root]);
      group.mentions.retain(|key| !group.keys.contains(key));
      group.units = 1;
      let whole_issue = group.anchor.is_none() && group.keys.len() == 1 && group.commits.iter().all(|&commit| input.commits[commit].key.is_some());
      group.issue_units = usize::from(whole_issue);
      group
    })
    .collect();
  HardUnits {
    groups,
    warnings,
    dependencies: unit_edges,
    declared,
  }
}

/// The edges whose two ends are in one strongly connected component of the graph of their sets: a cycle that no
/// stack builds
fn cyclic_edges(sets: &mut Sets, edges: &[[usize; 2]]) -> Vec<[usize; 2]> {
  let roots: Vec<[usize; 2]> = edges.iter().map(|&[from, to]| [sets.root(from), sets.root(to)]).collect();
  let component = components(sets.parents.len(), &roots);
  edges
    .iter()
    .zip(&roots)
    .filter(|(_, [from, to])| from != to && component[*from] == component[*to])
    .map(|(&edge, _)| edge)
    .collect()
}

/// The strongly connected component of each node of a directed graph (Kosaraju, without recursion)
fn components(count: usize, edges: &[[usize; 2]]) -> Vec<usize> {
  let mut successors: Vec<Vec<usize>> = vec![Vec::new(); count];
  let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); count];
  for &[from, to] in edges {
    successors[from].push(to);
    predecessors[to].push(from);
  }
  let mut visited = vec![false; count];
  let mut finished: Vec<usize> = Vec::with_capacity(count);
  for start in 0..count {
    if visited[start] {
      continue;
    }
    visited[start] = true;
    // the node and the index of its next successor
    let mut stack = vec![(start, 0)];
    while let Some(&(node, next)) = stack.last() {
      if let Some(&successor) = successors[node].get(next) {
        if let Some(top) = stack.last_mut() {
          top.1 += 1;
        }
        if !visited[successor] {
          visited[successor] = true;
          stack.push((successor, 0));
        }
      } else {
        finished.push(node);
        stack.pop();
      }
    }
  }
  let mut component = vec![usize::MAX; count];
  let mut next_component = 0;
  for &start in finished.iter().rev() {
    if component[start] != usize::MAX {
      continue;
    }
    component[start] = next_component;
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
      for &predecessor in &predecessors[node] {
        if component[predecessor] == usize::MAX {
          component[predecessor] = next_component;
          stack.push(predecessor);
        }
      }
    }
    next_component += 1;
  }
  component
}

#[derive(Debug, Clone, Copy)]
struct Scored {
  score: u32,
  /// P decided it, not R
  by_paths: bool,
  /// The shared feature that weighs most
  heaviest: Option<u32>,
  weak: bool,
}

/// The weights fixed by the units: the IDF of each feature, and the keys too many units name
struct Scorer<'f> {
  /// By feature id
  idf: Vec<f64>,
  ignored_mentions: HashSet<String>,
  features: &'f FeatureTable,
  max_commits: usize,
}

impl<'f> Scorer<'f> {
  fn new(units: &[Group], features: &'f FeatureTable, max_commits: usize) -> Self {
    let count = units.len();
    let mut document_frequency = vec![0usize; features.areas.len()];
    let mut mentioned_by: HashMap<&str, usize> = HashMap::new();
    for unit in units {
      for &(id, _) in &unit.features {
        document_frequency[id as usize] += 1;
      }
      for key in &unit.mentions {
        *mentioned_by.entry(key).or_default() += 1;
      }
    }
    // a feature that many units share tells nothing
    let common = (count / 4).max(4);
    let idf = document_frequency
      .iter()
      .map(|&frequency| {
        if frequency == 0 || frequency > common {
          0.0
        } else {
          (1.0 + ratio(count) / ratio(frequency)).ln()
        }
      })
      .collect();
    let too_many = (count / 3).max(10);
    let ignored_mentions = mentioned_by.into_iter().filter(|&(_, units)| units > too_many).map(|(key, _)| key.to_string()).collect();
    Self {
      idf,
      ignored_mentions,
      features,
      max_commits,
    }
  }

  /// The score of `a` and `b`, of which one depends on the other when `linked`
  fn score(&self, a: &Group, b: &Group, linked: bool) -> Option<Scored> {
    if (a.anchor.is_some() && b.anchor.is_some()) || a.commits.len() + b.commits.len() > self.max_commits {
      return None;
    }
    let (relation, _) = self.relation(a, b, linked);
    let (paths, heaviest) = self.paths(a, b);

    let both_keys = !a.keys.is_empty() && !b.keys.is_empty();
    let projects = |group: &Group| group.keys.iter().map(|key| project_of(key).to_string()).collect::<BTreeSet<String>>();
    let projects_meet = both_keys && !projects(a).is_disjoint(&projects(b));
    let mut factor = ONE;
    if both_keys && !projects_meet {
      factor = scale(factor, OTHER_PROJECT);
    }
    if !a.scopes.is_empty() && !b.scopes.is_empty() && a.scopes.is_disjoint(&b.scopes) {
      factor = scale(factor, OTHER_SCOPE);
    }
    let paths = if both_keys && projects_meet && a.keys.is_disjoint(&b.keys) {
      scale(paths, OTHER_KEY_PATHS)
    } else {
      paths
    };

    let base = relation.max(paths);
    if base == 0 {
      return None;
    }
    let adjacent = if is_adjacent(a, b) { ADJACENT } else { 0 };
    let without_adjacency = scale(base, factor);
    let score = without_adjacency + adjacent;
    let by_paths = paths > relation;
    let weak = without_adjacency < MERGE_THRESHOLD || (by_paths && score < MERGE_THRESHOLD + WEAK_MARGIN);
    Some(Scored { score, by_paths, heaviest, weak })
  }

  /// What the merge of `a` and `b` went by
  fn link(&self, a: &Group, b: &Group, linked: bool, scored: &Scored) -> Link {
    match self.relation(a, b, linked) {
      (_, Some(link)) if !scored.by_paths => link,
      _ => Link::Paths(scored.heaviest.map_or_else(String::new, |id| self.features.areas[id as usize].clone())),
    }
  }

  /// R and its link: the strongest of a dependency of a follow-up, a mention of the other side's key, the same scope
  /// and a third key both name
  fn relation(&self, a: &Group, b: &Group, linked: bool) -> (u32, Option<Link>) {
    let mut best: (u32, Option<Link>) = (0, None);
    let mut consider = |score: u32, link: Link| {
      if score > best.0 {
        best = (score, Some(link));
      }
    };
    let follows_up = |group: &Group| group.anchor.is_none() && group.keys.is_empty();
    if linked && (follows_up(a) || follows_up(b)) {
      consider(DEPENDENCY, Link::Dependency);
    }
    // a key that too many units name is the key of a series, not of the other side
    if let Some(key) = a
      .mentions
      .intersection(&b.keys)
      .chain(b.mentions.intersection(&a.keys))
      .filter(|key| !self.ignored_mentions.contains(*key))
      .min()
    {
      consider(MENTION, Link::Mention(key.clone()));
    }
    if let Some(scope) = a.scopes.intersection(&b.scopes).next() {
      let score = if a.keys.is_empty() && b.keys.is_empty() { SCOPE_WITHOUT_KEYS } else { SCOPE_WITH_KEYS };
      consider(score, Link::Scope(scope.clone()));
    }
    if let Some(key) = a.mentions.intersection(&b.mentions).find(|key| !self.ignored_mentions.contains(*key)) {
      consider(CO_MENTION, Link::CoMention(key.clone()));
    }
    best
  }

  /// P, the IDF-weighted Jaccard similarity of the features, and the shared feature that weighs most
  fn paths(&self, a: &Group, b: &Group) -> (u32, Option<u32>) {
    let (mut shared, mut all) = (0.0, 0.0);
    let mut heaviest: Option<(f64, u32)> = None;
    let (mut next_a, mut next_b) = (0, 0);
    loop {
      // the next feature of either side, with its weight on each side
      let (id, weight_a, weight_b) = match (a.features.get(next_a).copied(), b.features.get(next_b).copied()) {
        (Some((id_a, weight_a)), Some((id_b, weight_b))) if id_a == id_b => {
          next_a += 1;
          next_b += 1;
          (id_a, weight_a, weight_b)
        }
        (Some((id_a, weight_a)), Some((id_b, _))) if id_a < id_b => {
          next_a += 1;
          (id_a, weight_a, 0.0)
        }
        (Some((id_a, weight_a)), None) => {
          next_a += 1;
          (id_a, weight_a, 0.0)
        }
        (_, Some((id_b, weight_b))) => {
          next_b += 1;
          (id_b, 0.0, weight_b)
        }
        (None, None) => break,
      };
      let idf = self.idf[id as usize];
      let common = weight_a.min(weight_b) * idf;
      shared += common;
      all += weight_a.max(weight_b) * idf;
      if common > 0.0 && heaviest.is_none_or(|(weight, _)| common > weight) {
        heaviest = Some((common, id));
      }
    }
    let similarity = if all > 0.0 { shared / all } else { 0.0 };
    (to_units(similarity), heaviest.map(|(_, id)| id))
  }
}

/// Whether the two groups have neighbouring commits in the log
fn is_adjacent(a: &Group, b: &Group) -> bool {
  a.commits
    .iter()
    .any(|&commit| b.commits.binary_search(&(commit + 1)).is_ok() || (commit > 0 && b.commits.binary_search(&(commit - 1)).is_ok()))
}

fn project_of(key: &str) -> &str {
  key.split_once('-').map_or(key, |(project, _)| project)
}

fn scale(value: u32, factor: u32) -> u32 {
  u32::try_from(u64::from(value) * u64::from(factor) / u64::from(ONE)).unwrap_or(u32::MAX)
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a similarity in 0 ..= 1")]
fn to_units(similarity: f64) -> u32 {
  (similarity.clamp(0.0, 1.0) * f64::from(ONE)).round() as u32
}

fn ratio(count: usize) -> f64 {
  f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}

/// The lowercase words of a subject (without its prefix, issue key and conventional prefix) a name may take: letters
/// only, at least three, no stopwords, with a plain `-ing`, `-ed` or `-s` cut off; each once
pub fn subject_words(text: &str) -> Vec<String> {
  let mut words: Vec<String> = Vec::new();
  for word in text.split(|c: char| !c.is_ascii_alphabetic()) {
    let word = word.to_ascii_lowercase();
    if word.len() < 3 || STOPWORDS.contains(&word.as_str()) {
      continue;
    }
    let stem = stem(&word);
    if stem.len() >= 3 && !STOPWORDS.contains(&stem) && !words.iter().any(|known| known == stem) {
      words.push(stem.to_string());
    }
  }
  words
}

fn stem(word: &str) -> &str {
  if let Some(stem) = word.strip_suffix("ing").filter(|stem| stem.len() >= 3) {
    stem
  } else if let Some(stem) = word.strip_suffix("ed").filter(|stem| stem.len() >= 3) {
    stem
  } else if let Some(stem) = word.strip_suffix('s').filter(|stem| stem.len() >= 3 && !stem.ends_with('s')) {
    stem
  } else {
    word
  }
}

/// The branch name of each cluster, in the order of `clustering.clusters`: its anchor; the branch its commits come from,
/// when it holds the most commits of that branch (with --fresh); its single issue key; a scope of at least 60 % of its
/// commits; `<area leaf>-<word>` for a word of at least half its subjects that is rare elsewhere; its most frequent key;
/// or `cluster-N`. A name another cluster took, a group name now or an existing
/// virtual branch (`taken`) that the cluster's commits don't come from, or a case twin of one, gets `-2`, `-3`, …;
/// the larger clusters choose first
pub fn name_clusters(input: &ClusterInput, clustering: &Clustering, taken: &HashSet<String>) -> Vec<String> {
  let mut reserved: HashSet<String> = taken.iter().map(|name| name.to_lowercase()).collect();
  reserved.extend(input.commits.iter().filter_map(|commit| commit.group.as_ref()).map(|group| group.to_lowercase()));
  let mut subjects_with: HashMap<&str, usize> = HashMap::new();
  for commit in &input.commits {
    for word in &commit.words {
      *subjects_with.entry(word).or_default() += 1;
    }
  }

  let clusters = &clustering.clusters;
  // group -> (the cluster with the most of its commits, how many); the oldest cluster on a tie
  let mut holders: HashMap<&str, (usize, usize)> = HashMap::new();
  for (index, cluster) in clusters.iter().enumerate() {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for group in cluster.commits.iter().filter_map(|&commit| input.commits[commit].group.as_deref()) {
      *counts.entry(group).or_default() += 1;
    }
    for (group, count) in counts {
      let holder = holders.entry(group).or_insert((index, count));
      if count > holder.1 {
        *holder = (index, count);
      }
    }
  }
  let mut order: Vec<usize> = (0..clusters.len()).collect();
  order.sort_by_key(|&index| (Reverse(clusters[index].commits.len()), index));
  let mut names = vec![String::new(); clusters.len()];
  let mut used: HashSet<String> = HashSet::new();
  for index in order {
    let cluster = &clusters[index];
    let from: HashSet<String> = cluster_groups(input, cluster).iter().map(|group| group.to_lowercase()).collect();
    let inherited = match cluster_groups(input, cluster).as_slice() {
      [group] if holders.get(group).is_some_and(|&(holder, _)| holder == index) => Some(inherited_name(input, cluster, group)),
      _ => None,
    };
    let base = name_candidates(input, cluster, inherited, &subjects_with)
      .into_iter()
      .find_map(|candidate| valid_name(&candidate))
      .unwrap_or_else(|| format!("cluster-{}", index + 1));
    let free = |name: &str| {
      let lower = name.to_lowercase();
      !used.contains(&lower) && (!reserved.contains(&lower) || from.contains(&lower))
    };
    let mut name = base.clone();
    let mut suffix = 1;
    while !free(&name) {
      suffix += 1;
      name = format!("{base}-{suffix}");
    }
    used.insert(name.to_lowercase());
    names[index] = name;
  }
  names
}

/// The name of a group the cluster takes over: the prefix its commits have (the leaf of the path with stacked
/// branches on, which `place_clusters` stacks), else the group, an issue key
fn inherited_name(input: &ClusterInput, cluster: &Cluster, group: &str) -> String {
  cluster
    .commits
    .iter()
    .map(|&commit| &input.commits[commit])
    .find(|commit| commit.group.as_deref() == Some(group))
    .and_then(|commit| commit.prefix.as_deref())
    .map_or_else(|| group.to_string(), |prefix| prefix_anchor(prefix, input.stacks).to_string())
}

/// The name of a candidate, or `None` when it can't be a branch name
fn valid_name(candidate: &str) -> Option<String> {
  let name = sanitize_branch_name(candidate);
  validate_branch_name(&name).is_ok().then_some(name)
}

fn name_candidates(input: &ClusterInput, cluster: &Cluster, inherited: Option<String>, subjects_with: &HashMap<&str, usize>) -> Vec<String> {
  let commits: Vec<&CommitFeatures> = cluster.commits.iter().map(|&commit| &input.commits[commit]).collect();
  let mut candidates = Vec::new();
  candidates.extend(cluster.anchor.clone());
  candidates.extend(inherited);
  if let [key] = cluster.keys.as_slice() {
    candidates.push(key.clone());
  }
  // a name from the data, not a stack path
  let flat = |name: &str| name.replace('/', "-");
  if let Some((scope, count)) = most_frequent(commits.iter().filter_map(|commit| commit.scope.as_deref()))
    && count * 100 >= commits.len() * NAME_SCOPE_SHARE
  {
    candidates.push(flat(scope));
  }
  if let Some(word) = distinctive_word(&commits, subjects_with, input.commits.len()) {
    let area = top_area(&commits);
    let leaf = area.rsplit('/').next().filter(|leaf| *leaf != ".");
    candidates.push(leaf.map_or_else(|| word.clone(), |leaf| format!("{}-{word}", flat(leaf))));
  }
  if let Some((key, _)) = most_frequent(commits.iter().filter_map(|commit| commit.key.as_deref())) {
    candidates.push(key.to_string());
  }
  candidates
}

/// The most frequent value and its count; on a tie the first seen
fn most_frequent<'a>(values: impl Iterator<Item = &'a str>) -> Option<(&'a str, usize)> {
  let mut counts: Vec<(&str, usize)> = Vec::new();
  for value in values {
    match counts.iter_mut().find(|(known, _)| *known == value) {
      Some((_, count)) => *count += 1,
      None => counts.push((value, 1)),
    }
  }
  counts.into_iter().fold(None, |best, (value, count)| match best {
    Some((_, best_count)) if best_count >= count => best,
    _ => Some((value, count)),
  })
}

/// The word of at least half the subjects with the highest TF-IDF over all subjects; on a tie the first in order
fn distinctive_word(commits: &[&CommitFeatures], subjects_with: &HashMap<&str, usize>, total: usize) -> Option<String> {
  let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
  for commit in commits {
    for word in &commit.words {
      *counts.entry(word).or_default() += 1;
    }
  }
  counts
    .into_iter()
    .filter(|&(_, count)| count * 100 >= commits.len() * NAME_WORD_SHARE)
    .map(|(word, count)| {
      let everywhere = subjects_with.get(word).copied().unwrap_or(1);
      let score = ratio(count) / ratio(commits.len()) * (ratio(total) / ratio(everywhere)).ln();
      (word, score)
    })
    .filter(|&(_, score)| score > 0.0)
    .fold(None, |best: Option<(&str, f64)>, (word, score)| match best {
      Some((_, best_score)) if best_score >= score => best,
      _ => Some((word, score)),
    })
    .map(|(word, _)| word.to_string())
}

/// The area with the most lines of the commits; `.` without lines
fn top_area(commits: &[&CommitFeatures]) -> String {
  let files = commits.iter().flat_map(|commit| &commit.files).map(|file| (file.path.as_str(), file.lines));
  area_lines(files, DEFAULT_AREA_DEPTH).first().map_or(".", |&(area, _)| area).to_string()
}

/// The groups now of the cluster's commits, by their oldest commit
pub fn cluster_groups<'a>(input: &'a ClusterInput, cluster: &Cluster) -> Vec<&'a str> {
  let mut groups: Vec<&str> = Vec::new();
  for &commit in &cluster.commits {
    if let Some(group) = input.commits[commit].group.as_deref()
      && !groups.contains(&group)
    {
      groups.push(group);
    }
  }
  groups
}

/// How the cluster named `name` compares with the groups now, and those groups
pub fn cluster_change<'a>(input: &'a ClusterInput, cluster: &Cluster, name: &str) -> (ClusterChange, Vec<&'a str>) {
  let groups = cluster_groups(input, cluster);
  let members: HashSet<usize> = cluster.commits.iter().copied().collect();
  let whole = |group: &str| {
    input
      .commits
      .iter()
      .enumerate()
      .filter(|(_, commit)| commit.group.as_deref() == Some(group))
      .all(|(position, _)| members.contains(&position))
  };
  let unassigned = cluster.commits.iter().any(|&commit| input.commits[commit].group.is_none());
  let all_whole = groups.iter().all(|group| whole(group));
  // a group of a stack path is its leaf
  let same_name = groups.first().is_some_and(|group| name.rsplit('/').next() == Some(*group));
  let change = match groups.len() {
    1 if all_whole && !unassigned && same_name => ClusterChange::Kept,
    1 if all_whole && !unassigned => ClusterChange::Renamed,
    1 if !unassigned => ClusterChange::Split,
    1.. if all_whole => ClusterChange::Merged,
    _ => ClusterChange::New,
  };
  (change, groups)
}

/// Where a cluster builds
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
  /// The branch of the plan: `parent/leaf`, or the leaf of a root
  pub path: String,
  /// The branch it builds on; `None` for a root
  pub parent: Option<String>,
  /// Why the parent changes; `None` when it stays
  pub restack: Option<Restack>,
}

/// Why the parent of a cluster changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restack {
  /// It needs commits of its new parent
  Needs,
  /// A branch above it needs both it and its new parent, so their stacks became one
  ForAbove,
  /// Its declared parent depends on it
  ParentNeedsIt,
  /// Its declared parent has no commits, so the sync would block it
  ParentWithoutCommits,
  /// The commits of its declared parent went into a cluster of another name
  ParentMerged,
}

impl Placement {
  /// A branch without a parent
  pub fn root(leaf: &str) -> Self {
    Self {
      path: leaf.to_string(),
      parent: None,
      restack: None,
    }
  }

  /// The plan names a stacked branch by its path; an issue group can't
  pub fn is_stacked(&self) -> bool {
    self.path.contains('/')
  }
}

/// Where each cluster named `leaves` builds. Without stacked branches each one is a root under its leaf. With them,
/// the parent its prefixes declare stays when it holds every cluster it depends on as an ancestor (`stack_clusters`);
/// a declared parent that is no cluster goes, as the sync would block on it, and `fresh` declares no parent
pub fn place_clusters(input: &ClusterInput, clustering: &Clustering, leaves: &[String], fresh: bool) -> Vec<Placement> {
  if !input.stacks {
    return leaves.iter().map(|leaf| Placement::root(leaf)).collect();
  }
  // the parent that the prefix path of the leaf declares in its oldest commit; not the leaf itself
  let declared_names: Vec<Option<&str>> = clustering
    .clusters
    .iter()
    .zip(leaves)
    .map(|(cluster, leaf)| {
      let path = cluster
        .commits
        .iter()
        .filter_map(|&commit| input.commits[commit].prefix.as_deref())
        .find(|prefix| leaf_of_path(prefix) == leaf);
      path.and_then(parent_of_path).filter(|parent| !fresh && parent != leaf)
    })
    .collect();
  let index_of: HashMap<&str, usize> = leaves.iter().enumerate().map(|(index, leaf)| (leaf.as_str(), index)).collect();
  let declared: Vec<Option<usize>> = declared_names.iter().map(|name| name.and_then(|name| index_of.get(name).copied())).collect();
  let parents = stack_clusters(&clustering.dependencies, &declared);
  leaves
    .iter()
    .enumerate()
    .map(|(index, leaf)| {
      let parent = parents[index].map(|parent| leaves[parent].clone());
      let restack = match parents[index] {
        _ if parent.as_deref() == declared_names[index] => None,
        Some(parent) if clustering.dependencies.binary_search(&[index, parent]).is_ok() => Some(Restack::Needs),
        Some(_) => Some(Restack::ForAbove),
        None if declared[index].is_some() => Some(Restack::ParentNeedsIt),
        None if input.commits.iter().any(|commit| commit.group.as_deref() == declared_names[index]) => Some(Restack::ParentMerged),
        None => Some(Restack::ParentWithoutCommits),
      };
      // a path of more levels would declare more parents, which the placement may have moved
      let path = parent.as_ref().map_or_else(|| leaf.clone(), |parent| format!("{parent}/{leaf}"));
      Placement { path, parent, restack }
    })
    .collect()
}

/// The parent of each cluster, so that every cluster it depends on (`[from, to]`) is an ancestor. In the order of the
/// dependencies and the declared parents (`stack_order`), each cluster takes its declared parent when that one covers
/// them, else the deepest one it depends on that covers the others; when they lie on two stacks, those stacks become
/// one chain in that order. A chain only adds ancestors, so the clusters placed before keep theirs. A dependency
/// that a cycle turned around is left out
pub fn stack_clusters(dependencies: &[[usize; 2]], declared: &[Option<usize>]) -> Vec<Option<usize>> {
  let count = declared.len();
  let mut needs: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); count];
  for &[from, to] in dependencies.iter().filter(|&&[from, to]| from != to && from < count && to < count) {
    needs[from].insert(to);
  }
  let order = stack_order(&needs, declared);
  let mut rank = vec![0; count];
  for (position, &cluster) in order.iter().enumerate() {
    rank[cluster] = position;
  }
  let mut parents: Vec<Option<usize>> = vec![None; count];
  for &cluster in &order {
    let needed: Vec<usize> = needs[cluster].iter().copied().filter(|&needed| rank[needed] < rank[cluster]).collect();
    let preferred = declared[cluster].filter(|&parent| parent < count && rank[parent] < rank[cluster]);
    let covers = |parents: &[Option<usize>], candidate: usize| {
      let line = lineage(parents, candidate);
      needed.iter().all(|needed| line.contains(needed))
    };
    let parent = preferred
      .filter(|&parent| covers(&parents, parent))
      .or_else(|| needed.iter().copied().find(|&candidate| covers(&parents, candidate)));
    let parent = if parent.is_some() {
      parent
    } else {
      // the stacks of what it needs, and of its declared parent, become one chain
      let members: BTreeSet<usize> = needed.iter().chain(&preferred).flat_map(|&member| lineage(&parents, member)).collect();
      let mut chain: Vec<usize> = members.into_iter().collect();
      chain.sort_unstable_by_key(|&member| rank[member]);
      for pair in chain.windows(2) {
        parents[pair[1]] = Some(pair[0]);
      }
      chain.last().copied()
    };
    parents[cluster] = parent;
  }
  parents
}

/// The cluster and its ancestors
fn lineage(parents: &[Option<usize>], cluster: usize) -> Vec<usize> {
  let mut line = vec![cluster];
  let mut current = cluster;
  while let Some(parent) = parents[current] {
    line.push(parent);
    current = parent;
  }
  line
}

/// The clusters in an order where each comes after the clusters it depends on and after its declared parent, on a
/// tie the oldest first. In a cycle the declared parents give way first, then the oldest cluster of the rest goes first
fn stack_order(needs: &[BTreeSet<usize>], declared: &[Option<usize>]) -> Vec<usize> {
  let count = needs.len();
  // per cluster the clusters that wait for it, each with whether it waits only as a declared child
  let mut waiting: Vec<Vec<(usize, bool)>> = vec![Vec::new(); count];
  let mut pending = vec![0usize; count];
  let mut declared_waits = vec![false; count];
  for (cluster, needed) in needs.iter().enumerate() {
    for &needed in needed {
      waiting[needed].push((cluster, false));
      pending[cluster] += 1;
    }
    if let Some(parent) = declared[cluster].filter(|&parent| parent < count && parent != cluster && !needed.contains(&parent)) {
      waiting[parent].push((cluster, true));
      pending[cluster] += 1;
      declared_waits[cluster] = true;
    }
  }
  let mut ready: BinaryHeap<Reverse<usize>> = (0..count).filter(|&cluster| pending[cluster] == 0).map(Reverse).collect();
  let mut placed = vec![false; count];
  let mut order = Vec::with_capacity(count);
  while order.len() < count {
    let next = if let Some(Reverse(cluster)) = ready.pop() {
      cluster
    } else {
      let giving_way: Vec<usize> = (0..count)
        .filter(|&cluster| !placed[cluster] && declared_waits[cluster] && declared[cluster].is_some_and(|parent| !placed[parent]))
        .collect();
      if !giving_way.is_empty() {
        for cluster in giving_way {
          declared_waits[cluster] = false;
          pending[cluster] -= 1;
          if pending[cluster] == 0 {
            ready.push(Reverse(cluster));
          }
        }
        continue;
      }
      (0..count).find(|&cluster| !placed[cluster]).expect("a cluster is left")
    };
    if placed[next] {
      continue;
    }
    placed[next] = true;
    order.push(next);
    for &(waiter, as_declared) in &waiting[next] {
      if placed[waiter] || (as_declared && !declared_waits[waiter]) {
        continue;
      }
      pending[waiter] -= 1;
      if pending[waiter] == 0 {
        ready.push(Reverse(waiter));
      }
    }
  }
  order
}

/// Whether the cluster is the commits of two or more issue keys, all without a prefix: the issue groups make it one
/// branch without a rewrite. A unit holds every commit without a prefix of its key, so the issue branches are whole
pub fn is_issue_only(input: &ClusterInput, cluster: &Cluster) -> bool {
  cluster.keys.len() >= 2
    && cluster.commits.iter().all(|&commit| {
      let commit = &input.commits[commit];
      commit.prefix.is_none() && commit.key.is_some()
    })
}

/// One line on why the commits are together: the anchor, the units, what the merges went by, the repairs and the top
/// area, as in `9 issue units; IJAI-3016 named in 4 bodies; area plugins/air/shared`
pub fn why(input: &ClusterInput, cluster: &Cluster) -> String {
  let commits: Vec<&CommitFeatures> = cluster.commits.iter().map(|&commit| &input.commits[commit]).collect();
  let mut parts: Vec<String> = Vec::new();
  if let Some(anchor) = &cluster.anchor {
    let prefixed = commits
      .iter()
      .any(|commit| commit.prefix.as_deref().map(|prefix| prefix_anchor(prefix, input.stacks)) == Some(anchor.as_str()));
    parts.push(if prefixed { format!("prefix ({anchor})") } else { format!("issue group {anchor}") });
  }
  let issue = if cluster.issue_units == cluster.units { "issue " } else { "" };
  parts.push(format!("{} {issue}unit{}", cluster.units, if cluster.units == 1 { "" } else { "s" }));
  for link in &cluster.links {
    let part = match link {
      Link::Dependency => "a dependency".to_string(),
      Link::Mention(key) | Link::CoMention(key) => {
        let bodies = commits.iter().filter(|commit| commit.mentions.contains(key)).count();
        format!("{key} named in {bodies} bod{}", if bodies == 1 { "y" } else { "ies" })
      }
      Link::Scope(scope) => {
        let subjects = commits.iter().filter(|commit| commit.scope.as_ref() == Some(scope)).count();
        format!("scope {scope} in {subjects} subject{}", if subjects == 1 { "" } else { "s" })
      }
      Link::Paths(area) => format!("shared paths in {area}"),
    };
    if !parts.contains(&part) {
      parts.push(part);
    }
  }
  for round in &cluster.repair_rounds {
    parts.push(format!("joined by repair round {round} after a conflict"));
  }
  parts.push(format!("area {}", top_area(&commits)));
  parts.join("; ")
}

/// `input` with every name hashed by `keys`, a keyed hash whose key the caller never keeps (`RandomState::new()`):
/// each path segment, prefix level, group and scope as 16 hexadecimal digits, and each issue key with its project hashed
/// apart from the key (so keys of one project still share a project). The module markers of file paths become
/// `ANONYMOUS_MARKER`, and the words go. Positions, lines and edges stay, so the clustering is the same
pub fn anonymize(input: &ClusterInput, keys: &impl BuildHasher) -> ClusterInput {
  let name = |text: &str| -> String {
    if is_issue_key(text) {
      hashed_key(keys, text)
    } else {
      format!("{:016x}", keys.hash_one((b's', text)))
    }
  };
  let names = |text: &str| text.split('/').map(name).collect::<Vec<_>>().join("/");
  let file = |text: &str| {
    let segments = text.split('/').map(|segment| {
      if MODULE_MARKERS.contains(&segment) {
        ANONYMOUS_MARKER.to_string()
      } else {
        name(segment)
      }
    });
    segments.collect::<Vec<_>>().join("/")
  };
  ClusterInput {
    commits: input
      .commits
      .iter()
      .map(|commit| CommitFeatures {
        prefix: commit.prefix.as_deref().map(names),
        group: commit.group.as_deref().map(names),
        key: commit.key.as_deref().map(|key| hashed_key(keys, key)),
        issue_group: commit.issue_group.as_deref().map(names),
        scope: commit.scope.as_deref().map(name),
        mentions: {
          let mut mentions: Vec<String> = commit.mentions.iter().map(|key| hashed_key(keys, key)).collect();
          mentions.sort();
          mentions
        },
        fixup_of: commit.fixup_of,
        files: commit
          .files
          .iter()
          .map(|changed| FileLines {
            path: file(&changed.path),
            lines: changed.lines,
          })
          .collect(),
        words: Vec::new(),
      })
      .collect(),
    dependencies: input.dependencies.clone(),
    stacks: input.stacks,
  }
}

fn is_issue_key(text: &str) -> bool {
  issue_keys_in(text).next() == Some(text)
}

/// `PROJECT-123` as 14 uppercase letters for the hash of the project and the decimal hash of the whole key, 64 bits each
fn hashed_key(keys: &impl BuildHasher, key: &str) -> String {
  let mut project = keys.hash_one((b'p', project_of(key)));
  let letters: String = (0..14)
    .map(|_| {
      let letter = char::from(b'A' + u8::try_from(project % 26).unwrap_or(0));
      project /= 26;
      letter
    })
    .collect();
  format!("{letters}-{}", keys.hash_one((b'k', key)))
}
