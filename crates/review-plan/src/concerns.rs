//! The concerns of a review (spec/branch/review-concerns.spec.md): the tasks a range does, each a set of parts of its
//! units, joined only by evidence the plan read already, so that the tree of the review lists them and the story
//! follows them. No git process, no accio: the pending plan and the plan with the usages group alike.
//!
//! - **The nodes**: a unit; one per block for a file of no language the summaries parse (Markdown, JSON, TOML are one
//!   unit `other` as a whole); one per file for a repeated edit and for the deleted files of a directory, joined by the
//!   group. A mechanical file is no node: it stays in the Mechanical group, except a generated file whose header names a
//!   changed file, which follows the concern of that file.
//! - **Hard edges** (as ClusterChanges): a node that changes the form of a declaration, adds or removes one, and a node
//!   whose changed lines name it, in the same file or in a file that links to it (`links`); a change of a body alone
//!   forces no use to change, so it joins none; a declaration that more than `MAX_USES` nodes name joins none of them,
//!   a hub is no evidence. The files of a group.
//! - **Soft edges**: a rare token that the changed lines of two or more files all remove, or all add (`node_tokens`):
//!   a name of code, never a word of prose or a comment, of a source file only a name the change declares or a word of
//!   a string; in at most `MAX_TOKEN_NODES` nodes and a quarter of a larger range, no keyword, common word or common
//!   key of a config (`STOP_WORDS`), no part of a changed path. Two nodes join by the cosine of their rare tokens; a
//!   soft edge joins two concerns only when one is a single node or two tokens join them, so one shared word bridges
//!   nothing.
//! - **The rest**, off by default (it costs more purity than it adds recall): a node joined to nothing goes to the
//!   largest concern of its file, a test to the largest of its subject. A generated file goes to the largest concern of
//!   the files its header names by their path (a glob names none); what is left is the last concern, `Rest`.
//! - **The order** is that of the units, the story: each concern at its first part. **The identity**: `id` the
//!   fingerprint of its first part, `fingerprint` the set of the fingerprints of its parts.
//! - **The name**, by its evidence: the identifier that joined most of it, else its first changed declaration, else its
//!   one file (a source with its generated copies), else the directory its files share.
//! - **The main row** of a file in several concerns is in the one whose parts of it change the most lines (`elsewhere`
//!   of the others), so that the tree lists each file once and the others point to it.

use crate::files::{ChangedFile, MechanicalKind, subject_stem, test_subject};
use crate::links::FileLink;
use crate::units::{OtherReason, ReviewUnit, UnitKind, UnitSlice, digest};
use code_symbols::language::language_of;
use generated_files::generated_by::GeneratedBy;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ops::Range;

/// A declaration named by more nodes than this joins none of them
const MAX_USES: usize = 6;
/// An identifier in more nodes than this, or, in a range of `MIN_SHARE_NODES` nodes or more, in more than one of
/// `MAX_TOKEN_SHARE` of them, is not rare
const MAX_TOKEN_NODES: usize = 10;
const MAX_TOKEN_SHARE: usize = 4;
const MIN_SHARE_NODES: usize = 16;
/// The least cosine of the rare tokens of two nodes that joins them: a pair of large blocks of prose shares many words
/// by chance, a server dropped from two configs nearly all of its words
const MIN_ALIKE: f64 = 0.35;
/// The least share of the weight of the smaller of two nodes that the larger holds, with `MIN_CONTAINED_TOKENS` or more
/// tokens, that joins them as `MIN_ALIKE` does
const MIN_CONTAINED: f64 = 0.8;
const MIN_CONTAINED_TOKENS: usize = 4;
/// The shortest identifier, and the shortest name of a declaration, that may join nodes
const MIN_TOKEN: usize = 4;
const MIN_NAME: usize = 3;

/// Keywords of the languages, common words of prose and common keys of configs: no evidence of a concern
const STOP_WORDS: &[&str] = &[
  "about",
  "abstract",
  "also",
  "args",
  "bin",
  "build",
  "config",
  "dist",
  "index",
  "json",
  "main",
  "node",
  "src",
  "target",
  "toml",
  "yaml",
  "async",
  "await",
  "boolean",
  "break",
  "case",
  "catch",
  "class",
  "command",
  "const",
  "continue",
  "could",
  "crate",
  "data",
  "default",
  "description",
  "enabled",
  "enum",
  "else",
  "error",
  "export",
  "extends",
  "false",
  "file",
  "files",
  "final",
  "from",
  "function",
  "have",
  "headers",
  "http",
  "https",
  "impl",
  "import",
  "inner",
  "interface",
  "internal",
  "into",
  "just",
  "line",
  "lines",
  "local",
  "match",
  "more",
  "must",
  "name",
  "none",
  "null",
  "number",
  "only",
  "open",
  "other",
  "override",
  "package",
  "private",
  "protected",
  "public",
  "remote",
  "return",
  "self",
  "should",
  "some",
  "static",
  "stdio",
  "string",
  "struct",
  "super",
  "than",
  "that",
  "their",
  "them",
  "then",
  "there",
  "they",
  "this",
  "throw",
  "timeout",
  "trait",
  "true",
  "type",
  "types",
  "used",
  "uses",
  "using",
  "value",
  "values",
  "version",
  "void",
  "when",
  "where",
  "which",
  "while",
  "will",
  "with",
  "would",
  "your",
];

/// The edges a plan joins its nodes by, all on; `bd review-plan --concern-edges` picks them for a measurement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcernEdges {
  pub uses: bool,
  pub tokens: bool,
  pub rest: bool,
}

impl Default for ConcernEdges {
  fn default() -> Self {
    Self {
      uses: true,
      tokens: true,
      rest: false,
    }
  }
}

/// A task of the range: parts of its units
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Concern {
  /// Names it in this plan, and in the next one while its first part stays: the fingerprint of that part
  pub id: String,
  pub name: ConcernName,
  /// In the order of the story; a generated file after the parts of its source
  pub parts: Vec<ConcernPart>,
  /// The files of its parts, each once, in the order of its parts
  pub paths: Vec<String>,
  /// Its files whose main row, in the tree of the review, is another concern's: of the concerns of a file, the one whose
  /// parts of it change the most lines holds its row, the first of equal ones; each other one shows it as "also in"
  pub elsewhere: Vec<String>,
  /// Why its parts are together, the evidence of the most joins first
  pub evidence: Vec<ConcernEvidence>,
  /// The set of the fingerprints of its parts
  pub fingerprint: String,
}

/// What a concern holds of one unit in one file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConcernPart {
  /// `ReviewUnit::id`
  pub unit: String,
  pub path: String,
  /// The blocks of the unit's slice of `path` it holds, by their index; `None` for the whole slice
  pub blocks: Option<Vec<u32>>,
}

/// What a concern is called, by the evidence that made it; the page words it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum ConcernName {
  /// The rare identifier that joined most of it
  Token { token: String },
  /// A rare identifier its changed lines remove and one they add in its place, in the same changes
  Replacement { removed: String, added: String },
  /// Its first changed declaration
  Declaration { name: String },
  /// The one file of its own parts, beside the generated files that follow it
  File { path: String },
  /// The directory its files share, `""` for the root
  Directory { directory: String },
  /// What no evidence joined
  Rest,
}

/// One kind of evidence of a concern and how many joins it made
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConcernEvidence {
  pub kind: EvidenceKind,
  /// The name of a declaration of `uses`, the identifier of `token`
  pub detail: Option<String>,
  pub joins: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum EvidenceKind {
  /// A changed declaration and a change that names it
  Uses,
  /// The files of one group: a repeated edit, the deleted files of a directory
  Group,
  /// A rare identifier the changed lines of its files share
  Token,
  /// The largest concern of its file, of the subject of a test, of a file a generated file's header names
  Rest,
}

/// A node of the graph: a unit's slice, or one block of it
struct Node<'a> {
  unit: usize,
  path: &'a str,
  /// The block of a file of no language, `None` for the whole slice
  block: Option<u32>,
  /// Its changed lines on each side
  removed: Vec<&'a str>,
  added: Vec<&'a str>,
}

impl Node<'_> {
  const fn weight(&self) -> usize {
    self.removed.len() + self.added.len()
  }
}

/// The components of the nodes, by size
struct Components {
  parent: Vec<usize>,
  size: Vec<usize>,
}

impl Components {
  fn new(count: usize) -> Self {
    Self {
      parent: (0..count).collect(),
      size: vec![1; count],
    }
  }

  fn root(&mut self, mut node: usize) -> usize {
    while self.parent[node] != node {
      self.parent[node] = self.parent[self.parent[node]];
      node = self.parent[node];
    }
    node
  }

  /// The root of `node`, without shortening the path to it
  fn root_of(&self, mut node: usize) -> usize {
    while self.parent[node] != node {
      node = self.parent[node];
    }
    node
  }

  /// Joins the components of `a` and `b`; whether they were apart
  fn join(&mut self, a: usize, b: usize) -> bool {
    let (a, b) = (self.root(a), self.root(b));
    if a == b {
      return false;
    }
    let (big, small) = if self.size[a] >= self.size[b] { (a, b) } else { (b, a) };
    self.parent[small] = big;
    self.size[big] += self.size[small];
    true
  }
}

/// The concerns of `units` (in the order of the story), from the files the plan reads (`files`), why each is generated
/// (`generated`, by place), the hunk text by blob (`texts`) and the links between the changed files (`links`)
pub(crate) fn review_concerns(
  units: &[ReviewUnit],
  files: &[ChangedFile<'_>],
  generated: &[Option<GeneratedBy>],
  texts: &HashMap<String, String>,
  links: &[FileLink],
  declared: &HashSet<String>,
  edges: ConcernEdges,
) -> Vec<Concern> {
  let file_of: HashMap<&str, usize> = files.iter().enumerate().map(|(index, file)| (file.path, index)).collect();
  let (nodes, groups, generated_parts) = graph_nodes(units, files, &file_of, texts);
  if nodes.is_empty() {
    return Vec::new();
  }
  let mut components = Components::new(nodes.len());
  // (a node of the join, the evidence)
  let mut joins: Vec<(usize, EvidenceKind, Option<String>)> = Vec::new();
  for group in &groups {
    for pair in group.windows(2) {
      if components.join(pair[0], pair[1]) {
        joins.push((pair[0], EvidenceKind::Group, None));
      }
    }
  }
  if edges.uses {
    join_uses(units, &nodes, links, &mut components, &mut joins);
  }
  let rare = if edges.tokens {
    join_tokens(&nodes, files, declared, &mut components, &mut joins)
  } else {
    Vec::new()
  };
  // the rest: a node alone goes to the largest concern of its file, a test to that of its subject
  if edges.rest {
    let roots: Vec<usize> = (0..nodes.len()).map(|node| components.root(node)).collect();
    let joined = |node: usize| components.size[roots[node]] > 1;
    let mut moves: Vec<(usize, usize)> = Vec::new();
    for node in (0..nodes.len()).filter(|&node| !joined(node)) {
      let path = nodes[node].path;
      let target = largest(
        &nodes,
        &roots,
        (0..nodes.len()).filter(|&other| other != node && nodes[other].path == path && joined(other)),
      )
      .or_else(|| {
        let subject = test_subject(path)?;
        largest(
          &nodes,
          &roots,
          (0..nodes.len()).filter(|&other| test_subject(nodes[other].path).is_none() && subject_stem(nodes[other].path) == subject),
        )
      });
      if let Some(target) = target {
        moves.push((node, target));
      }
    }
    for (node, target) in moves {
      if components.join(node, target) {
        joins.push((node, EvidenceKind::Rest, None));
      }
    }
  }
  // a generated file follows the largest concern of the files its header names
  let roots: Vec<usize> = (0..nodes.len()).map(|node| components.root(node)).collect();
  let mut generated_of: BTreeMap<usize, Vec<(usize, &str)>> = BTreeMap::new();
  for (unit, path) in generated_parts {
    let Some(GeneratedBy::Header { line }) = file_of.get(path).and_then(|&place| generated.get(place)).cloned().flatten() else {
      continue;
    };
    if let Some(root) = largest(&nodes, &roots, (0..nodes.len()).filter(|&node| names_path(&line, nodes[node].path))) {
      generated_of.entry(root).or_default().push((unit, path));
    }
  }
  assemble(units, &nodes, &mut components, &joins, &rare, &generated_of)
}

/// The root of the component of `candidates` (nodes, by their `roots`) with the most changed lines among them; of two
/// alike the first
fn largest(nodes: &[Node<'_>], roots: &[usize], candidates: impl Iterator<Item = usize>) -> Option<usize> {
  let mut weights: BTreeMap<usize, usize> = BTreeMap::new();
  for node in candidates {
    *weights.entry(roots[node]).or_default() += nodes[node].weight();
  }
  weights.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(root, _)| root)
}

/// The nodes of the graph, the nodes of each group, and the generated slices (unit, path)
type GraphNodes<'a> = (Vec<Node<'a>>, Vec<Vec<usize>>, Vec<(usize, &'a str)>);

/// The nodes of the graph (`GraphNodes`)
fn graph_nodes<'a>(units: &'a [ReviewUnit], files: &[ChangedFile<'a>], file_of: &HashMap<&str, usize>, texts: &'a HashMap<String, String>) -> GraphNodes<'a> {
  let lines = |blob: Option<&str>| -> Vec<&'a str> { blob.and_then(|blob| texts.get(blob)).map_or_else(Vec::new, |text| text.split('\n').collect()) };
  let take = |side: &[&'a str], range: Range<u32>| -> Vec<&'a str> {
    let end = (range.end as usize).min(side.len());
    side.get((range.start as usize).min(end)..end).unwrap_or_default().to_vec()
  };
  let mut nodes: Vec<Node<'a>> = Vec::new();
  let mut groups: Vec<Vec<usize>> = Vec::new();
  let mut generated: Vec<(usize, &'a str)> = Vec::new();
  for (index, unit) in units.iter().enumerate() {
    match unit.kind {
      UnitKind::Mechanical { mechanical } => {
        if mechanical == MechanicalKind::Generated {
          generated.extend(unit.slices.iter().map(|slice| (index, slice.path.as_str())));
        }
        continue;
      }
      UnitKind::RepeatedEdit { .. } | UnitKind::DeletedFiles { .. } => groups.push(Vec::new()),
      _ => {}
    }
    let per_block = unit.kind == UnitKind::Other { reason: OtherReason::NoLanguage };
    for slice in &unit.slices {
      let Some(&place) = file_of.get(slice.path.as_str()) else { continue };
      let file = &files[place];
      let old = lines(file.before.or_else(|| file.changes.last().and_then(|change| change.old_blob.as_deref())));
      let new = lines(file.after);
      let node = |block: Option<u32>, blocks: &mut dyn Iterator<Item = &crate::units::UnitBlock>| {
        let mut node = Node {
          unit: index,
          path: slice.path.as_str(),
          block,
          removed: Vec::new(),
          added: Vec::new(),
        };
        for block in blocks {
          node.removed.extend(take(&old, block.old_start..block.old_end));
          node.added.extend(take(&new, block.new_start..block.new_end));
        }
        node
      };
      if slice.blocks.is_empty() {
        // a file taken whole without blocks: added, deleted, or without text
        nodes.push(Node {
          unit: index,
          path: slice.path.as_str(),
          block: None,
          removed: if file.after.is_none() { old.clone() } else { Vec::new() },
          added: if file.before.is_none() { new.clone() } else { Vec::new() },
        });
      } else if per_block {
        for block in &slice.blocks {
          nodes.push(node(Some(block.index), &mut std::iter::once(block)));
        }
      } else {
        nodes.push(node(None, &mut slice.blocks.iter()));
      }
      if let Some(group) = groups
        .last_mut()
        .filter(|_| matches!(unit.kind, UnitKind::RepeatedEdit { .. } | UnitKind::DeletedFiles { .. }))
      {
        group.push(nodes.len() - 1);
      }
    }
  }
  (nodes, groups, generated)
}

/// Joins each node that changes a declaration to the nodes whose changed lines name it, in its file or in a file that
/// links to it by that name; a name of more than `MAX_USES` such nodes joins none
fn join_uses(units: &[ReviewUnit], nodes: &[Node<'_>], links: &[FileLink], components: &mut Components, joins: &mut Vec<(usize, EvidenceKind, Option<String>)>) {
  let linked: HashSet<(&str, &str, &str)> = links
    .iter()
    .flat_map(|link| link.symbols.iter().map(move |symbol| (link.from.as_str(), link.to.as_str(), symbol.name.as_str())))
    .collect();
  for (declaration, node) in nodes.iter().enumerate() {
    let unit = &units[node.unit];
    if !matches!(unit.kind, UnitKind::Shape { .. } | UnitKind::Added | UnitKind::Removed) {
      continue;
    }
    let Some(name) = unit.name.as_deref().filter(|name| name.len() >= MIN_NAME) else {
      continue;
    };
    let uses: Vec<usize> = nodes
      .iter()
      .enumerate()
      .filter(|(other, use_node)| {
        *other != declaration
          && use_node.unit != node.unit
          && (use_node.path == node.path || linked.contains(&(use_node.path, node.path, name)))
          && use_node.removed.iter().chain(&use_node.added).any(|line| names_word(line, name))
      })
      .map(|(other, _)| other)
      .collect();
    if uses.len() > MAX_USES {
      continue;
    }
    for other in uses {
      if components.join(declaration, other) {
        joins.push((declaration, EvidenceKind::Uses, Some(name.to_string())));
      }
    }
  }
}

/// Joins the nodes of two or more files whose changed lines all remove, or all add, a rare identifier
/// (`node_tokens`), the rarest first; a token joins two concerns of more than one node only when another token joins
/// them too. The rare tokens with their nodes, the rarest first
fn join_tokens(
  nodes: &[Node<'_>],
  files: &[ChangedFile<'_>],
  declared: &HashSet<String>,
  components: &mut Components,
  joins: &mut Vec<(usize, EvidenceKind, Option<String>)>,
) -> Vec<RareToken> {
  let path_parts: HashSet<String> = files
    .iter()
    .flat_map(|file| file.path.split(['/', '.']))
    .flat_map(|part| std::iter::once(part).chain(part.split(['-', '_'])))
    .filter(|part| !part.is_empty())
    .map(|part| part.to_lowercase().replace('-', "_"))
    .collect();
  // (removed?, token) -> its nodes
  let mut found: BTreeMap<(bool, String), BTreeSet<usize>> = BTreeMap::new();
  // a token a node both removes and adds is no change of it: the words a rewritten paragraph keeps
  let mut weights: Vec<BTreeSet<(bool, String)>> = Vec::with_capacity(nodes.len());
  for (index, node) in nodes.iter().enumerate() {
    let (removed, added) = (node_tokens(node.path, &node.removed, declared), node_tokens(node.path, &node.added, declared));
    let changed: BTreeSet<(bool, String)> = removed
      .difference(&added)
      .map(|token| (true, token.clone()))
      .chain(added.difference(&removed).map(|token| (false, token.clone())))
      .filter(|(_, token)| !path_parts.contains(&token.replace('-', "_")))
      .collect();
    for key in &changed {
      found.entry(key.clone()).or_default().insert(index);
    }
    weights.push(changed);
  }
  let mut rare: Vec<RareToken> = found
    .iter()
    .filter(|(_, with)| (2..=MAX_TOKEN_NODES).contains(&with.len()) && (nodes.len() < MIN_SHARE_NODES || with.len() * MAX_TOKEN_SHARE <= nodes.len()))
    .filter(|(_, with)| with.iter().map(|&node| nodes[node].path).collect::<HashSet<_>>().len() >= 2)
    .map(|((removed, token), with)| RareToken {
      token: token.clone(),
      removed: *removed,
      nodes: with.iter().copied().collect(),
    })
    .collect();
  rare.sort_by(|a, b| a.nodes.len().cmp(&b.nodes.len()).then(a.token.cmp(&b.token)));
  // the weight of a token: the rarer, the more (`ln((n + 1) / df)` over the nodes); of a node, of all its changed tokens
  let as_f64 = |count: usize| f64::from(u32::try_from(count).unwrap_or(u32::MAX));
  let count = as_f64(nodes.len());
  let idf = |nodes_with: usize| ((count + 1.0) / as_f64(nodes_with)).ln();
  let weight_of: HashMap<(bool, &str), f64> = found.iter().map(|((removed, token), with)| ((*removed, token.as_str()), idf(with.len()))).collect();
  let total: Vec<f64> = weights
    .iter()
    .map(|changed| changed.iter().map(|(removed, token)| weight_of[&(*removed, token.as_str())]).sum())
    .collect();
  // the pairs of nodes of two files that share rare tokens, by their cosine over the weights: the most alike first
  let mut shared: BTreeMap<(usize, usize), (f64, Vec<&str>)> = BTreeMap::new();
  for RareToken { token, removed, nodes: with } in &rare {
    let weight = weight_of[&(*removed, token.as_str())];
    for (place, &a) in with.iter().enumerate() {
      for &b in &with[place + 1..] {
        if nodes[a].path != nodes[b].path {
          let entry = shared.entry((a, b)).or_default();
          entry.0 += weight;
          entry.1.push(token);
        }
      }
    }
  }
  let mut pairs: Vec<((usize, usize), f64, Vec<&str>)> = shared
    .into_iter()
    .map(|((a, b), (weight, tokens))| {
      // a small change all of whose rare tokens a large one holds is alike too: a server dropped from a short config and
      // from one that lists its tools
      let cosine = weight / (total[a] * total[b]).sqrt();
      let contained = if tokens.len() >= MIN_CONTAINED_TOKENS { weight / total[a].min(total[b]) } else { 0.0 };
      ((a, b), cosine.max(contained * MIN_ALIKE / MIN_CONTAINED), tokens)
    })
    .filter(|(_, alike, _)| *alike >= MIN_ALIKE)
    .collect();
  pairs.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
  for ((a, b), _, tokens) in pairs {
    let (root_a, root_b) = (components.root(a), components.root(b));
    if root_a == root_b {
      continue;
    }
    // two concerns of more than one node each join only by two tokens their nodes share
    let alone = components.size[root_a] == 1 || components.size[root_b] == 1;
    let between = || -> usize {
      rare
        .iter()
        .filter(|rare| {
          let (mut in_a, mut in_b) = (false, false);
          for &node in &rare.nodes {
            let root = components.root_of(node);
            in_a |= root == root_a;
            in_b |= root == root_b;
          }
          in_a && in_b
        })
        .count()
    };
    if (alone || between() >= 2) && components.join(a, b) {
      joins.push((a, EvidenceKind::Token, Some(tokens[0].to_string())));
    }
  }
  rare
}

/// The identifiers of the changed `lines` of `path`, lowercase, each once: a run of letters, digits, `_` and `-` that
/// starts with a letter, and its parts between `_` and `-`, each of `MIN_TOKEN` characters or more and no word of
/// `STOP_WORDS`. Of prose (Markdown, text) only the code it quotes (`` `a` ``, a fence, an indented line); of a source
/// file no comment line, and only a name a changed file declares (`declared`, lowercase) or a word of a string literal:
/// a word of prose and a name of a library (`list`, `as_bytes`) are no evidence of a task, a name of the change or a
/// config is
fn node_tokens(path: &str, lines: &[&str], declared: &HashSet<String>) -> BTreeSet<String> {
  let extension = path.rsplit_once('.').map_or_else(String::new, |(_, extension)| extension.to_ascii_lowercase());
  let prose = matches!(extension.as_str(), "md" | "markdown" | "txt" | "rst" | "adoc");
  let source = language_of(path).is_some();
  let mut tokens = BTreeSet::new();
  for line in lines {
    let trimmed = line.trim_start();
    if source && ["//", "/*", "*", "#", "--"].iter().any(|comment| trimmed.starts_with(comment)) {
      continue;
    }
    let code: Vec<&str> = if prose && !(line.starts_with("    ") || trimmed.starts_with("```")) {
      line.split('`').skip(1).step_by(2).collect()
    } else {
      vec![line]
    };
    // the words of the string literals of a source line, which name no library
    let quoted: HashSet<&str> = if source {
      line
        .split('"')
        .skip(1)
        .step_by(2)
        .flat_map(|text| text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')))
        .collect()
    } else {
      HashSet::new()
    };
    for text in code {
      for run in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
        if source && !quoted.contains(run) && !declared.contains(&run.to_lowercase()) {
          continue;
        }
        let run = run.trim_matches(['_', '-']);
        if !run.starts_with(|c: char| c.is_ascii_alphabetic()) {
          continue;
        }
        let whole = run.to_lowercase();
        let parts: Vec<String> = whole.split(['_', '-']).filter(|part| !part.is_empty()).map(str::to_string).collect();
        for token in std::iter::once(whole).chain(if parts.len() > 1 { parts } else { Vec::new() }) {
          if token.len() >= MIN_TOKEN && token.starts_with(|c: char| c.is_ascii_alphabetic()) && !STOP_WORDS.contains(&token.as_str()) {
            tokens.insert(token);
          }
        }
      }
    }
  }
  tokens
}

/// The place of the first changed line of `node` that holds `token`, removed lines first; past them when none does
fn first_line_of(node: &Node<'_>, token: &str) -> usize {
  let lines: Vec<&str> = node.removed.iter().chain(&node.added).copied().collect();
  lines.iter().position(|line| line.to_lowercase().contains(token)).unwrap_or(lines.len())
}

/// A rare identifier of the changed lines of `nodes`, all removed or all added there
struct RareToken {
  token: String,
  removed: bool,
  nodes: Vec<usize>,
}

/// The name of a concern a token joined (its nodes `inside`): of the tokens with most of their nodes inside, those of
/// the most nodes inside, a simple word first, the one their changed lines name first (the heading of a block), the longer; a removed one and
/// an added one of the same nodes are a replacement
fn token_name(nodes: &[Node<'_>], rare: &[RareToken], inside: &HashSet<usize>) -> Option<ConcernName> {
  let best = |removed: bool| {
    let within = |rare: &RareToken| rare.nodes.iter().filter(|node| inside.contains(node)).count();
    rare
      .iter()
      .filter(|rare| rare.removed == removed && within(rare) * 2 > rare.nodes.len() && within(rare) >= 2)
      .max_by(|a, b| {
        let simple = |token: &str| !token.contains(['-', '_']);
        let first = |rare: &RareToken| -> usize {
          rare
            .nodes
            .iter()
            .filter(|node| inside.contains(node))
            .map(|&node| first_line_of(&nodes[node], &rare.token))
            .sum()
        };
        within(a)
          .cmp(&within(b))
          .then(simple(&a.token).cmp(&simple(&b.token)))
          .then(first(b).cmp(&first(a)))
          .then(a.token.len().cmp(&b.token.len()))
          .then(b.token.cmp(&a.token))
      })
  };
  match (best(true), best(false)) {
    (Some(removed), Some(added))
      if removed
        .nodes
        .iter()
        .filter(|node| inside.contains(node))
        .eq(added.nodes.iter().filter(|node| inside.contains(node))) =>
    {
      Some(ConcernName::Replacement {
        removed: removed.token.clone(),
        added: added.token.clone(),
      })
    }
    (Some(one), Some(other)) => Some(ConcernName::Token {
      token: if one.nodes.len() >= other.nodes.len() { one } else { other }.token.clone(),
    }),
    (Some(one), None) | (None, Some(one)) => Some(ConcernName::Token { token: one.token.clone() }),
    (None, None) => None,
  }
}

/// Whether `line` holds `name` as a whole word
fn names_word(line: &str, name: &str) -> bool {
  let is_word = |c: char| c.is_alphanumeric() || c == '_';
  line.match_indices(name).any(|(start, _)| {
    let before = line[..start].chars().next_back();
    let after = line[start + name.len()..].chars().next();
    !before.is_some_and(is_word) && !after.is_some_and(is_word)
  })
}

/// Whether the header `line` of a generated file names the changed file `path` by its whole path; a glob (`edit
/// docs/*`) names no source of its own
fn names_path(line: &str, path: &str) -> bool {
  line
    .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '`' | '"' | '\''))
    .map(|word| word.trim_end_matches(['.', ':']))
    .any(|word| word == path)
}

/// The concerns of the components of more than one node, in the order of their first unit, and the rest last
fn assemble(
  units: &[ReviewUnit],
  nodes: &[Node<'_>],
  components: &mut Components,
  joins: &[(usize, EvidenceKind, Option<String>)],
  rare: &[RareToken],
  generated_of: &BTreeMap<usize, Vec<(usize, &str)>>,
) -> Vec<Concern> {
  let mut members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
  for node in 0..nodes.len() {
    let root = components.root(node);
    members.entry(root).or_default().push(node);
  }
  let mut evidence: HashMap<usize, BTreeMap<(EvidenceKind, Option<String>), u32>> = HashMap::new();
  for (node, kind, detail) in joins {
    let root = components.root(*node);
    *evidence.entry(root).or_default().entry((*kind, detail.clone())).or_default() += 1;
  }
  // (its first unit, the concern, its nodes)
  let mut concerns: Vec<(usize, Concern, Vec<usize>)> = Vec::new();
  let mut rest: Vec<usize> = Vec::new();
  for (root, mut of) in members {
    let generated = generated_of.get(&root);
    if of.len() == 1 && generated.is_none() {
      rest.extend(of);
      continue;
    }
    of.sort_by_key(|&node| (nodes[node].unit, node));
    let mut evidence: Vec<ConcernEvidence> = evidence
      .remove(&root)
      .unwrap_or_default()
      .into_iter()
      .map(|((kind, detail), joins)| ConcernEvidence { kind, detail, joins })
      .collect();
    evidence.sort_by(|a, b| b.joins.cmp(&a.joins).then(a.kind.cmp(&b.kind)).then(a.detail.cmp(&b.detail)));
    let mut parts = parts_of(units, nodes, &of);
    parts.extend(generated.into_iter().flatten().map(|&(unit, path)| ConcernPart {
      unit: units[unit].id.clone(),
      path: path.to_string(),
      blocks: None,
    }));
    // the identifier of the most nodes of a concern that a token joined, all of them in it
    let joined_by_token = evidence.iter().any(|evidence| evidence.kind == EvidenceKind::Token);
    let inside: HashSet<usize> = of.iter().copied().collect();
    let name = joined_by_token
      .then(|| token_name(nodes, rare, &inside))
      .flatten()
      .or_else(|| {
        of.iter()
          .map(|&node| &units[nodes[node].unit])
          .find(|unit| matches!(unit.kind, UnitKind::Shape { .. } | UnitKind::Added | UnitKind::Removed | UnitKind::Behaviour))
          .and_then(|unit| unit.name.clone())
          .map(|name| ConcernName::Declaration { name })
      })
      .unwrap_or_else(|| {
        // a source with its generated copies is named by its file, more than one file by the directory they share
        let mut own = of.iter().map(|&node| nodes[node].path);
        let path = own.next().unwrap_or_default();
        if own.all(|other| other == path) {
          ConcernName::File { path: path.to_string() }
        } else {
          ConcernName::Directory {
            directory: shared_directory(parts.iter().map(|part| part.path.as_str())),
          }
        }
      });
    let first = nodes[of[0]].unit;
    concerns.push((first, concern(units, nodes, name, parts, evidence, &of), of));
  }
  concerns.sort_by_key(|(first, ..)| *first);
  if !rest.is_empty() {
    rest.sort_by_key(|&node| (nodes[node].unit, node));
    let parts = parts_of(units, nodes, &rest);
    concerns.push((usize::MAX, concern(units, nodes, ConcernName::Rest, parts, Vec::new(), &rest), rest));
  }
  mark_elsewhere(nodes, &mut concerns);
  concerns.into_iter().map(|(_, concern, _)| concern).collect()
}

/// The `elsewhere` of each concern (with its nodes): a file of several concerns has its main row in the one whose nodes
/// of it change the most lines, the first of equal ones
fn mark_elsewhere(nodes: &[Node<'_>], concerns: &mut [(usize, Concern, Vec<usize>)]) {
  // the path -> (the concern of its main row, its changed lines there)
  let mut main: HashMap<String, (usize, usize)> = HashMap::new();
  for (index, (_, concern, of)) in concerns.iter().enumerate() {
    // a generated file is no node: it changes no line of the graph, and only one concern holds it
    let mut weights: BTreeMap<&str, usize> = concern.paths.iter().map(|path| (path.as_str(), 0)).collect();
    for &node in of {
      *weights.entry(nodes[node].path).or_default() += nodes[node].weight();
    }
    for (path, weight) in weights {
      let entry = main.entry(path.to_string()).or_insert((index, weight));
      if weight > entry.1 {
        *entry = (index, weight);
      }
    }
  }
  for (index, (_, concern, _)) in concerns.iter_mut().enumerate() {
    concern.elsewhere = concern.paths.iter().filter(|path| main[path.as_str()].0 != index).cloned().collect();
  }
}

fn concern(units: &[ReviewUnit], nodes: &[Node<'_>], name: ConcernName, parts: Vec<ConcernPart>, evidence: Vec<ConcernEvidence>, of: &[usize]) -> Concern {
  let fingerprint_of = |node: usize| {
    let unit = &units[nodes[node].unit];
    let block = nodes[node].block.map_or_else(String::new, |block| block.to_string());
    digest([unit.fingerprint.as_str(), nodes[node].path, block.as_str()])
  };
  let mut fingerprints: Vec<String> = of.iter().map(|&node| fingerprint_of(node)).collect();
  let id = fingerprints.first().cloned().unwrap_or_default();
  fingerprints.sort_unstable();
  let mut seen: HashSet<&str> = HashSet::new();
  let paths = parts.iter().map(|part| part.path.as_str()).filter(|path| seen.insert(path)).map(str::to_string).collect();
  Concern {
    id,
    name,
    paths,
    elsewhere: Vec::new(),
    evidence,
    fingerprint: digest(fingerprints.iter().map(String::as_str)),
    parts,
  }
}

/// The parts of the nodes `of` (in the order of the story): one per unit and file, the blocks of a file of no language
/// that the nodes hold
fn parts_of(units: &[ReviewUnit], nodes: &[Node<'_>], of: &[usize]) -> Vec<ConcernPart> {
  let mut parts: Vec<ConcernPart> = Vec::new();
  for &node in of {
    let Node { unit, path, block, .. } = nodes[node];
    let id = &units[unit].id;
    match parts.iter_mut().find(|part| part.unit == *id && part.path == path) {
      Some(part) => {
        if let (Some(blocks), Some(block)) = (part.blocks.as_mut(), block) {
          blocks.push(block);
        }
      }
      None => parts.push(ConcernPart {
        unit: id.clone(),
        path: path.to_string(),
        blocks: block.map(|block| vec![block]),
      }),
    }
  }
  // a part that holds every block of its slice holds it whole
  for part in &mut parts {
    let slice: Option<&UnitSlice> = units
      .iter()
      .find(|unit| unit.id == part.unit)
      .and_then(|unit| unit.slices.iter().find(|slice| slice.path == part.path));
    if let (Some(blocks), Some(slice)) = (&part.blocks, slice)
      && blocks.len() == slice.blocks.len()
    {
      part.blocks = None;
    }
  }
  parts
}

/// The deepest directory of all of `paths`, `""` for the root
fn shared_directory<'a>(paths: impl Iterator<Item = &'a str>) -> String {
  let mut shared: Option<Vec<&str>> = None;
  for path in paths {
    let directories: Vec<&str> = path.rsplit_once('/').map_or_else(Vec::new, |(directory, _)| directory.split('/').collect());
    shared = Some(match shared {
      None => directories,
      Some(shared) => shared.iter().zip(&directories).take_while(|(a, b)| a == b).map(|(a, _)| *a).collect(),
    });
  }
  shared.unwrap_or_default().join("/")
}
