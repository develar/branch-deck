//! `bd suggest`: a grouping of the commits ahead of the baseline into branches, as a plan for `bd group`, already
//! checked. It reads the commits once (`CheckSession`), clusters them (`clustering.rs`), with stacked branches stacks
//! each cluster on the clusters it needs, and checks the plan with a trial sync; a conflict stacks on or joins the
//! commits it needs and clusters again. It writes git objects and the cache only (docs/guidelines/agent-cli.md)

use crate::check_command::{CheckedBranch, checked_branches, kind_verb, possible_dangling_references};
use crate::commits::{CommitsHeader, LinkedDiffs, fixup_targets, read_linked_diffs};
use bd_clustering::areas::{DEFAULT_AREA_DEPTH, DirLines, top_areas};
use bd_clustering::clustering::{
  Cluster, ClusterChange, ClusterInput, ClusterOptions, Clustering, CommitFeatures, Conflict, DEFAULT_MAX_COMMITS, FileLines, MAX_REPAIR_ROUNDS, Placement, Restack, Warning,
  anonymize, cluster_change, cluster_with_repair, is_issue_only, leaf_of_path, name_clusters, place_clusters, stacked_without_need, subject_words, why,
};
use bd_clustering::generated::is_hub_file;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::error::regroup_error;
use bd_core::output::SchemaVersion;
use bd_core::suggest_plan::SuggestPlan;
use clap::builder::RangedU64ValueParser;
use git_ops::branch_names::{BranchKind, sanitize_branch_name, strip_branch_namespace};
use git_ops::commit_list::Commit;
use history_rewrite::commit_grouper::parse_subject;
use history_rewrite::regroup::RegroupPlan;
use history_rewrite::snapshot::Snapshot;
use history_rewrite::stack::{STACKS_CONFIG_KEY, stack_path};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::RandomState;
use sync_core::check::{BranchCheck, CheckOutcome, CheckReport, CheckSession};
use sync_core::dependencies::compute_dependencies;
use sync_core::missing_commits::{changed_files, missing_commits};
use sync_core::symbol_dependencies::{SymbolGraph, compute_symbol_dependencies};
use sync_core::symbol_scan::{SymbolMode, read_symbol_input};
use sync_utils::issue_pattern::{find_issue_number, issue_keys_in, parse_semantic_prefix};

#[derive(Debug, clap::Args)]
pub struct SuggestArgs {
  /// Ignore the `(prefix)` of the subjects and the issue groups: cluster every commit as if it had neither
  #[arg(long)]
  pub fresh: bool,
  /// No merge of two clusters by score makes a branch of more commits
  #[arg(long, value_name = "N", default_value_t = DEFAULT_MAX_COMMITS, value_parser = RangedU64ValueParser::<usize>::new().range(1..))]
  pub max_commits: usize,
  /// Give the commits of a cluster of whole issue branches a prefix, instead of an issue group
  #[arg(long)]
  pub rewrite_issues: bool,
  /// Print the input of the clustering, anonymized, instead of a suggestion: test data
  #[arg(long, hide = true)]
  pub dump_input: bool,
}

/// `bd suggest`
#[derive(Debug, Serialize, JsonSchema)]
pub struct SuggestOutput {
  #[serde(flatten)]
  pub header: CommitsHeader,
  /// The plan for `bd check` and `bd group`: only the commits whose branch changes; a fixup that follows its target is
  /// left out
  pub plan: SuggestPlan,
  /// Ordered by their oldest commit
  pub clusters: Vec<SuggestedCluster>,
  /// The subjects the plan changes
  pub rewritten: usize,
  /// The trial sync of the plan
  pub check: SuggestCheck,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub warnings: Vec<SuggestWarning>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SuggestedCluster {
  /// The branch name
  pub name: String,
  pub commit_count: usize,
  pub change: ClusterChange,
  /// The branches its commits are in now, by their oldest commit
  pub current_branches: Vec<String>,
  /// Its commits that the plan leaves as they are, oldest first
  pub kept: Vec<String>,
  /// Its commits whose branch stays but whose subject the plan gives another stacked path to the same leaf, oldest
  /// first (`(gone/a)` to `a`, a restack): the commit gets a new id. With `moved`, what the plan changes of a commit
  /// another session made
  pub new_path: Vec<String>,
  /// Its commits that change branch, oldest first: the commits of `plan.branches` and `unassign` that go to it, a fixup
  /// that follows its target (left out of the plan), and the commits an issue group takes in or lets go
  /// (`issue_groups`, `ungroup_issues`)
  pub moved: Vec<MovedCommit>,
  /// Why the commits are together, one line to quote
  pub why: String,
  /// A merge by paths near the threshold, or one that log adjacency decided, made it: review it first
  pub weak: bool,
  /// The issue keys of its subjects
  pub issue_keys: Vec<String>,
  /// The directories with the most lines, as in `bd commits --brief`
  pub areas: Vec<DirLines>,
  /// `issue_group`: whole issue branches that an issue group puts together, so that their subjects stay
  /// (`--rewrite-issues` gives them a prefix instead)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// With stacked branches: the branch it builds on, so that every branch it needs is an ancestor
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// The parent changes: another one, a new one, or none any more
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub restacked: bool,
  /// The stack parents stack it on `parent`, but it needs no commit of `parent` or of its ancestors by its lines. The
  /// plan keeps the parent: `bd unstack <name>` makes it a root, `bd safe-push --check` tells whether it builds alone
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub stacked_without_need: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MovedCommit {
  pub commit: String,
  /// The branch the commit leaves; `null` for an unassigned commit
  pub leaves: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SuggestCheck {
  /// Every branch builds
  pub ok: bool,
  /// The times a conflict joined the commits it needs and the clustering ran again
  pub repair_rounds: usize,
  /// As in `bd check`
  pub branches: Vec<CheckedBranch>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SuggestWarning {
  /// `anchor_dependency`: a commit of one prefix depends on a commit of another, and prefixes stay apart (with stacked
  /// branches only in a cycle, which no stack builds); `unresolved_conflict`: a branch still conflicts (or has a
  /// dangling reference) after the repair; `possible_dangling_reference`, `possible_changed_declaration`,
  /// `possible_duplicate_declaration`: a use of a name a branch removes or changes, or a second declaration of a name it
  /// adds, that no commit here changes; `symbol_dependency` (`branchdeck.symbols on`): a branch uses a name another branch adds and the plan
  /// doesn't build it on that branch
  pub kind: String,
  pub branches: Vec<String>,
  pub message: String,
}

/// `bd suggest --dump-input`
#[derive(Debug, Serialize)]
pub struct SuggestInputDump {
  pub schema: SchemaVersion,
  pub input: ClusterInput,
}

/// What a suggestion reads: the session holds the commits, the diffs their files
struct Reading {
  session: CheckSession,
  diffs: LinkedDiffs,
  input: ClusterInput,
  /// The dependencies by symbols of the linked changes, with `branchdeck.symbols on`
  symbols: Option<SymbolGraph>,
}

impl Reading {
  fn read(context: &RepoContext) -> Result<Self, CliError> {
    let session = CheckSession::open(&context.git, &context.repository, &context.prefix, &context.cache())
      .map_err(regroup_error)?
      .verifying_mapping_notes(!context.use_cache);
    let snapshot = session.snapshot();
    let ids: Vec<String> = snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
    let diffs = read_linked_diffs(context, &ids, false, false)?;
    let (session, symbols) = if session.symbol_mode() == SymbolMode::On {
      let input = read_symbol_input(&context.git, &context.repository, &context.cache(), diffs.linked())?;
      let symbols = compute_symbol_dependencies(diffs.linked(), &input);
      // the checks of the repair read the same summaries
      (session.with_symbol_input(input), Some(symbols))
    } else {
      (session, None)
    };
    let input = cluster_input(session.snapshot(), &diffs, symbols.as_ref());
    Ok(Self { session, diffs, input, symbols })
  }
}

/// The placements, the plan (positions) and the check of the clustering the repair loop checked last
struct Checked {
  placements: Vec<Placement>,
  positions: PlanPositions,
  report: CheckReport,
}

/// `bd suggest --dump-input`: the input with every name hashed by `SipHash` with a random key of the operating system,
/// which is neither kept nor printed
pub fn suggest_input_dump(context: &RepoContext) -> Result<SuggestInputDump, CliError> {
  let reading = Reading::read(context)?;
  Ok(SuggestInputDump {
    schema: SchemaVersion,
    input: anonymize(&reading.input, &RandomState::new()),
  })
}

pub fn suggest(context: &RepoContext, args: &SuggestArgs) -> Result<SuggestOutput, CliError> {
  let Reading { session, diffs, input, symbols } = Reading::read(context)?;
  let snapshot = session.snapshot();
  let commit_ids: Vec<&str> = snapshot.commits.iter().map(|commit| commit.id.as_str()).collect();
  let position_of: HashMap<&str, usize> = commit_ids.iter().enumerate().map(|(position, id)| (*id, position)).collect();
  // every changed file, generated ones too: a conflict may be in a lock file
  let files_of = changed_files(&diffs.diffs);
  // what each commit needs by its lines, lock and generated files included, for the same reason
  let mut needs_by_lines: Vec<Vec<usize>> = vec![Vec::new(); commit_ids.len()];
  for dependency in compute_dependencies(&diffs.diffs) {
    needs_by_lines[dependency.from].push(dependency.to);
  }
  let taken: HashSet<String> = session
    .tips()
    .keys()
    .filter_map(|name| strip_branch_namespace(name, &context.prefix, BranchKind::Virtual))
    .map(str::to_string)
    .collect();

  let options = ClusterOptions {
    fresh: args.fresh,
    max_commits: args.max_commits,
    repairs: Vec::new(),
  };
  let mut last: Option<Checked> = None;
  let repaired = cluster_with_repair(&input, &options, |clustering| {
    let leaves = name_clusters(&input, clustering, &taken);
    let placements = place_clusters(&input, clustering, &leaves, args.fresh);
    let positions = plan_of(&input, clustering, &placements, args.rewrite_issues);
    let plan = RegroupPlan {
      branches: positions
        .branches
        .iter()
        .map(|(name, commits)| (name.clone(), commits.iter().map(|&commit| commit_ids[commit].to_string()).collect()))
        .collect(),
      unassign: positions.unassign.iter().map(|&commit| commit_ids[commit].to_string()).collect(),
      issue_groups: positions.issue_groups.clone(),
      ungroup_issues: positions.ungroup_issues.clone(),
      unstack: positions.unstack.clone(),
    };
    let report = session.check(Some(&plan)).map_err(regroup_error)?;
    let conflicts = conflicts_of(&report, &files_of, &position_of, &needs_by_lines);
    last = Some(Checked { placements, positions, report });
    Ok::<_, CliError>(conflicts)
  })?;
  let Checked { placements, positions, report } = last.expect("the check ran");

  let short = context.short_ids(commit_ids.iter().copied().chain([snapshot.head.as_str()]));
  let short_of = |commit: usize| short.get(commit_ids[commit]).to_string();
  let linked = diffs.linked();
  // the commits whose subject the plan rewrites
  let in_plan: HashSet<usize> = positions.branches.values().flatten().chain(&positions.unassign).copied().collect();
  let without_need = stacked_without_need(&input, &repaired.clustering, &placements);
  let clusters = repaired
    .clustering
    .clusters
    .iter()
    .zip(&placements)
    .zip(without_need)
    .map(|((cluster, placement), stacked_without_need)| {
      let (change, from) = cluster_change(&input, cluster, &placement.path);
      let via = (!args.rewrite_issues && is_issue_only(&input, cluster)).then(|| "issue_group".to_string());
      let (kept, moved) = kept_and_moved(&input, cluster, &placement.path);
      let (new_path, kept): (Vec<usize>, Vec<usize>) = kept.into_iter().partition(|commit| in_plan.contains(commit));
      SuggestedCluster {
        name: placement.path.clone(),
        commit_count: cluster.commits.len(),
        change,
        current_branches: from.into_iter().map(str::to_string).collect(),
        kept: kept.into_iter().map(short_of).collect(),
        new_path: new_path.into_iter().map(short_of).collect(),
        moved: moved
          .into_iter()
          .map(|(commit, leaves)| MovedCommit {
            commit: short_of(commit),
            leaves: leaves.map(str::to_string),
          })
          .collect(),
        why: stacked_why(why(&input, cluster), placement),
        weak: cluster.weak,
        issue_keys: cluster.keys.clone(),
        areas: top_areas(cluster.commits.iter().flat_map(|&commit| &linked[commit].files), DEFAULT_AREA_DEPTH),
        via,
        parent: placement.parent.clone(),
        restacked: placement.restack.is_some(),
        stacked_without_need,
      }
    })
    .collect();
  // the fixes are `bd check`'s: each costs a trial build of every branch
  let checked = checked_branches(&report, &files_of, &short, &mut |_, _| None);
  let mut warnings = warnings_of(&repaired.clustering, input.stacks);
  for (branch, result) in report.branches.iter().zip(&checked).filter(|(_, checked)| checked.result != "ok") {
    let conflict = match &branch.outcome {
      CheckOutcome::Conflict { commit, .. } => position_of
        .get(commit.as_str())
        .and_then(|&position| repaired.conflicts.iter().find(|conflict| conflict.commit == position)),
      _ => None,
    };
    let mutual = report
      .dangling
      .get(&branch.name)
      .into_iter()
      .flatten()
      .find(|reference| reference.is_confirmed() && reference.mutual);
    let reason = match conflict {
      _ if let Some(reference) = mutual => {
        let (fixed_by, removed_by) = (short.get(reference.fixed_by.as_deref().unwrap_or_default()), short.get(&reference.removed_by));
        let only_the_file = files_of
          .get(reference.fixed_by.as_deref().unwrap_or_default())
          .is_some_and(|files| files.iter().all(|path| *path == reference.file));
        let how = if only_the_file {
          format!("move {fixed_by}, which changes only {}, into {}", reference.file, branch.name)
        } else {
          format!("move that change of {} into {removed_by}; `bd check` proves the move by a trial build", reference.file)
        };
        format!(
          "{fixed_by} in {} changes the use of {} with what the branch adds, so no stack builds both: {how}",
          reference.file, reference.identifier
        )
      }
      Some(conflict) if conflict.needs.is_empty() => "no older commit ahead of the baseline changes the conflicting files".to_string(),
      Some(_) if repaired.rounds == MAX_REPAIR_ROUNDS => format!("the {MAX_REPAIR_ROUNDS} repair rounds ran out"),
      Some(_) if input.stacks => "it still conflicts on the branch of the commits it needs, and two prefixes never join: move the commits".to_string(),
      Some(_) => format!("the commits it needs have another prefix: turn on stacked branches (`git config {STACKS_CONFIG_KEY} true`) to stack the branch on theirs, or move them"),
      None => format!("the repair joins only conflicting commits, and the branch is {}", result.result),
    };
    warnings.push(SuggestWarning {
      kind: "unresolved_conflict".to_string(),
      branches: vec![branch.name.clone()],
      message: format!("{} is {} after {} repair rounds: {reason}", branch.name, result.result, repaired.rounds),
    });
  }

  if let Some(graph) = &symbols {
    warnings.extend(symbol_dependency_warnings(graph, &input, &repaired.clustering, &placements));
  }
  warnings.extend(possible_dangling_references(&report, &short).into_iter().map(|warning| SuggestWarning {
    message: format!(
      "{}:{} {} {}, which {} of {} {}{}; no commit here changes it, so it may not compile",
      warning.file,
      warning.line,
      if warning.kind == "possible_duplicate_declaration" { "declares" } else { "uses" },
      warning.identifier,
      warning.removed_by,
      warning.branch,
      kind_verb(&warning.kind),
      if warning.on_baseline { ", and the baseline got the use after the base" } else { "" }
    ),
    kind: warning.kind,
    branches: vec![warning.branch],
  }));

  Ok(SuggestOutput {
    header: CommitsHeader {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      head: short.get(&snapshot.head).to_string(),
      baseline: snapshot.baseline.clone(),
    },
    plan: SuggestPlan {
      branches: positions
        .branches
        .into_iter()
        .map(|(name, commits)| (name, commits.into_iter().map(short_of).collect()))
        .collect(),
      unassign: positions.unassign.into_iter().map(short_of).collect(),
      issue_groups: positions.issue_groups,
      ungroup_issues: positions.ungroup_issues,
      unstack: positions.unstack,
    },
    clusters,
    rewritten: report.regroup.changed_count(),
    check: SuggestCheck {
      ok: checked.iter().all(|branch| branch.result == "ok"),
      repair_rounds: repaired.rounds,
      branches: checked,
    },
    warnings,
  })
}

/// A cluster that uses a name another cluster adds, by symbols only, where the plan doesn't build it on that cluster:
/// git builds them apart and a build would fail. Once per pair of clusters, with the first name
fn symbol_dependency_warnings(graph: &SymbolGraph, input: &ClusterInput, clustering: &Clustering, placements: &[Placement]) -> Vec<SuggestWarning> {
  let mut cluster_of: HashMap<usize, usize> = HashMap::new();
  for (index, cluster) in clustering.clusters.iter().enumerate() {
    cluster_of.extend(cluster.commits.iter().map(|&commit| (commit, index)));
  }
  let path_of: HashMap<&str, usize> = placements.iter().enumerate().map(|(index, placement)| (leaf_of_path(&placement.path), index)).collect();
  // the cluster and the clusters it builds on
  let lineage = |cluster: usize| {
    let mut line = vec![cluster];
    let mut current = placements[cluster].parent.as_deref();
    while let Some(parent) = current
      .and_then(|parent| path_of.get(leaf_of_path(parent)).copied())
      .filter(|parent| !line.contains(parent))
    {
      line.push(parent);
      current = placements[parent].parent.as_deref();
    }
    line
  };
  let symbols_only: HashSet<[usize; 2]> = input.symbol_dependencies.iter().copied().collect();
  let mut seen: HashSet<(usize, usize)> = HashSet::new();
  let mut warnings = Vec::new();
  for edge in graph.edges.iter().filter(|edge| symbols_only.contains(&[edge.from, edge.to])) {
    let (Some(&user), Some(&owner)) = (cluster_of.get(&edge.from), cluster_of.get(&edge.to)) else {
      continue;
    };
    if user == owner || lineage(user).contains(&owner) || !seen.insert((user, owner)) {
      continue;
    }
    let Some(link) = edge.links.first() else { continue };
    let (user_name, owner_name) = (leaf_of_path(&placements[user].path), leaf_of_path(&placements[owner].path));
    let how = if input.stacks {
      format!("stack {user_name} on {owner_name}, or Safe Push {owner_name} first")
    } else {
      format!("Safe Push {owner_name} first, or turn on stacked branches (`git config {STACKS_CONFIG_KEY} true`) to stack {user_name} on it")
    };
    warnings.push(SuggestWarning {
      kind: "symbol_dependency".to_string(),
      branches: vec![user_name.to_string(), owner_name.to_string()],
      message: format!(
        "{user_name} uses {}, which {owner_name} adds ({}:{}): git builds them apart, and the build of {user_name} would fail: {how}",
        link.name, link.used_in, link.line
      ),
    });
  }
  warnings
}

/// The features of every commit of the snapshot, oldest first; with `symbols` the edges by symbols only and the entities
fn cluster_input(snapshot: &Snapshot, diffs: &LinkedDiffs, symbols: Option<&SymbolGraph>) -> ClusterInput {
  let position_of: HashMap<&str, usize> = snapshot.commits.iter().enumerate().map(|(position, commit)| (commit.id.as_str(), position)).collect();
  let ordered: Vec<(&Commit, Option<&str>)> = snapshot
    .commits
    .iter()
    .map(|commit| (commit, snapshot.groups.get(&commit.id).and_then(Option::as_deref)))
    .collect();
  let fixups = fixup_targets(&snapshot.head, &ordered, &snapshot.grouping);
  let issue_groups = &snapshot.grouping.issue_groups.entries;
  let linked = diffs.linked();
  let commits = ordered
    .iter()
    .enumerate()
    .map(|(position, &(commit, group))| {
      let parsed = parse_subject(&commit.subject);
      let prefix = parsed.prefix.and_then(|prefix| {
        let path = if snapshot.grouping.stacks {
          stack_path(prefix).ok().map(|levels| levels.join("/"))
        } else {
          Some(sanitize_branch_name(prefix.trim()))
        };
        path.filter(|path| !path.is_empty())
      });
      let semantic = parse_semantic_prefix(parsed.rest);
      let key = find_issue_number(parsed.rest).map(str::to_string);
      let body = commit.message.split_once('\n').map_or("", |(_, body)| body);
      let mut mentions: Vec<String> = issue_keys_in(body).filter(|mention| key.as_deref() != Some(*mention)).map(str::to_string).collect();
      mentions.sort();
      mentions.dedup();
      CommitFeatures {
        issue_group: key.as_ref().and_then(|key| issue_groups.get(key)).cloned(),
        prefix,
        group: group.map(str::to_string),
        key,
        scope: semantic.and_then(|semantic| semantic.scope).map(str::to_ascii_lowercase),
        mentions,
        fixup_of: fixups
          .get(&commit.id)
          .and_then(|(_, target)| target.as_deref())
          .and_then(|target| position_of.get(target).copied()),
        files: linked.get(position).map_or_else(Vec::new, |diff| {
          diff
            .files
            .iter()
            .filter(|change| !is_hub_file(&change.path))
            .map(|change| FileLines {
              path: change.path.clone(),
              lines: if change.binary { 0 } else { change.additions + change.deletions },
            })
            .collect()
        }),
        words: name_words(semantic.map_or(parsed.rest, |semantic| semantic.rest)),
      }
    })
    .collect();
  let dependencies: Vec<[usize; 2]> = compute_dependencies(linked).into_iter().map(|edge| [edge.from, edge.to]).collect();
  let by_lines: HashSet<[usize; 2]> = dependencies.iter().copied().collect();
  let symbol_dependencies = symbols
    .into_iter()
    .flat_map(|graph| &graph.edges)
    .map(|edge| [edge.from, edge.to])
    .filter(|edge| !by_lines.contains(edge))
    .collect();
  ClusterInput {
    commits,
    dependencies,
    symbol_dependencies,
    stacks: snapshot.grouping.stacks,
    stack_parents: if snapshot.grouping.stacks {
      snapshot.grouping.stack_parents.entries.clone()
    } else {
      BTreeMap::new()
    },
  }
}

/// `why` with the new parent of a restacked cluster
fn stacked_why(why: String, placement: &Placement) -> String {
  let parent = placement.parent.as_deref().unwrap_or_default();
  match placement.restack {
    None => why,
    Some(Restack::Needs) => format!("{why}; stacked on {parent}, whose commits it needs"),
    Some(Restack::ForAbove) => format!("{why}; stacked on {parent}, because a branch above needs both"),
    Some(Restack::ParentNeedsIt) => format!("{why}; unstacked, because its parent needs it"),
    Some(Restack::ParentWithoutCommits) => format!("{why}; unstacked, because its parent has no commits"),
    Some(Restack::ParentMerged) => format!("{why}; unstacked, because the commits of its parent went into another branch"),
  }
}

/// The words of a subject after its conventional prefix, without its issue keys: `fix(vm): ABC-1 x` gives `x`
pub(crate) fn name_words(text: &str) -> Vec<String> {
  // cut by position: the same key text may also sit inside a word (`ABC-1` in `XABC-1`)
  let mut without_keys = String::with_capacity(text.len());
  let mut copied = 0;
  for key in issue_keys_in(text) {
    let start = key.as_ptr().addr() - text.as_ptr().addr();
    without_keys.push_str(&text[copied..start]);
    without_keys.push(' ');
    copied = start + key.len();
  }
  without_keys.push_str(&text[copied..]);
  subject_words(&without_keys)
}

/// What a plan of a clustering changes, by position
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct PlanPositions {
  /// Branch name -> the commits that move there
  pub branches: BTreeMap<String, Vec<usize>>,
  /// The commits that lose their prefix to fall back to their own issue branch
  pub unassign: Vec<usize>,
  /// Issue group name -> the issue keys it gets
  pub issue_groups: BTreeMap<String, Vec<String>>,
  /// The keys whose issue group goes
  pub ungroup_issues: Vec<String>,
  /// The branches whose stack parent goes: they become roots
  pub unstack: Vec<String>,
}

/// The plan of a clustering: per branch path the commits that move there, the commits that lose their prefix to fall
/// back to their own issue branch, the changes of the issue groups, and the stack parents that go. A commit already
/// grouped by that path stays out, and so does a fixup that follows its target. A cluster of whole issue branches
/// becomes an issue group unless `rewrite_issues`, stacked by the stack parents when its path names a parent; a commit
/// of an issue group that goes to the branch of its own key loses the group
pub(crate) fn plan_of(input: &ClusterInput, clustering: &Clustering, placements: &[Placement], rewrite_issues: bool) -> PlanPositions {
  let mut plan = PlanPositions::default();
  let mut ungroup: BTreeSet<&str> = BTreeSet::new();
  // commit -> where the plan puts it: `Some(name)` or `None` for unassign
  let mut destination: HashMap<usize, Option<&str>> = HashMap::new();
  for (cluster, placement) in clustering.clusters.iter().zip(placements) {
    let name = &placement.path;
    let leaf = leaf_of_path(name);
    if placement.restack.is_some() {
      if placement.is_stacked() {
        // the path writes the stack parent, also when no commit moves
        plan.branches.entry(name.clone()).or_default();
      } else if input.stack_parents.contains_key(leaf) {
        plan.unstack.push(leaf.to_string());
      }
    }
    if !rewrite_issues && is_issue_only(input, cluster) {
      for key in &cluster.keys {
        let now = cluster
          .commits
          .iter()
          .map(|&position| &input.commits[position])
          .find(|commit| commit.key.as_ref() == Some(key))
          .and_then(|commit| commit.issue_group.as_deref());
        // a key that names the cluster needs no issue group
        if now.unwrap_or(key) != leaf {
          plan.issue_groups.entry(leaf.to_string()).or_default().push(key.clone());
        }
      }
      continue;
    }
    for &position in &cluster.commits {
      let commit = &input.commits[position];
      let own_key = commit.key.as_deref() == Some(leaf);
      match &commit.prefix {
        // the bare leaf is in the branch whatever its parent: the stack parents stack it
        Some(prefix) if prefix == name || prefix == leaf => {}
        // without a prefix, the commit is in its issue group, else in the branch of its key
        None if commit.issue_group.as_deref().or(commit.key.as_deref()) == Some(leaf) => {}
        // the branch of its own key: the commit loses its issue group
        None if own_key => ungroup.extend(commit.key.as_deref()),
        Some(_) if own_key => {
          destination.insert(position, None);
          // `unassign` puts it into the issue group of its key, if it has one
          if commit.issue_group.is_some() {
            ungroup.extend(commit.key.as_deref());
          }
        }
        _ => {
          destination.insert(position, Some(name.as_str()));
        }
      }
    }
  }
  let mut moved: Vec<(usize, Option<&str>)> = destination.iter().map(|(&position, &target)| (position, target)).collect();
  moved.sort_unstable();
  for (position, target) in moved {
    let follows = input.commits[position].fixup_of.is_some_and(|fixup_target| destination.get(&fixup_target) == Some(&target));
    if follows {
      continue;
    }
    match target {
      Some(name) => plan.branches.entry(name.to_string()).or_default().push(position),
      None => plan.unassign.push(position),
    }
  }
  plan.ungroup_issues = ungroup.into_iter().map(str::to_string).collect();
  plan
}

/// The commits of a cluster whose branch stays, and those that change branch with the branch they leave (`None`:
/// unassigned), by position. The branch is the group of the commit now against the leaf of `path`, so this agrees
/// with `plan_of`: a commit the plan moves or unassigns changes branch, and so does a fixup that follows its target
/// and a commit an issue group takes in or lets go. The one difference is a subject that the plan gives another path to the same
/// leaf (a restack): the commit stays in its branch, and `suggest` lists it in `new_path`
pub(crate) fn kept_and_moved<'a>(input: &'a ClusterInput, cluster: &Cluster, path: &str) -> (Vec<usize>, Vec<(usize, Option<&'a str>)>) {
  let leaf = leaf_of_path(path);
  let (kept, moved): (Vec<usize>, Vec<usize>) = cluster.commits.iter().partition(|&&commit| input.commits[commit].group.as_deref() == Some(leaf));
  (kept, moved.into_iter().map(|commit| (commit, input.commits[commit].group.as_deref())).collect())
}

/// The conflicts of a report, as positions: each conflicting commit with the commits it needs by its lines that its
/// branch and the branches it is built on lack (and what those need in turn); when the lines name none, the commits it
/// likely needs by the conflicting files, else the newest older commit outside its branch that changes one
fn conflicts_of(report: &CheckReport, files_of: &HashMap<String, HashSet<String>>, position_of: &HashMap<&str, usize>, needs_by_lines: &[Vec<usize>]) -> Vec<Conflict> {
  let mut conflicts = Vec::new();
  for branch in &report.branches {
    let CheckOutcome::Conflict { commit, files } = &branch.outcome else { continue };
    let Some(&position) = position_of.get(commit.as_str()) else { continue };
    let present = present_commits(report, branch, position_of);
    let mut needs = needed_by_lines(needs_by_lines, position, &present);
    if needs.is_empty() {
      needs.extend(
        missing_commits(report, branch, commit, files, files_of)
          .iter()
          .filter_map(|id| position_of.get(id.as_str()).copied()),
      );
    }
    if needs.is_empty() {
      needs.extend(
        report
          .regroup
          .commits
          .iter()
          .map(|regrouped| regrouped.id.as_str())
          .take_while(|id| *id != commit)
          .filter(|id| !branch.commits.iter().any(|own| own == id))
          .filter(|id| files_of.get(*id).is_some_and(|changed| files.iter().any(|file| changed.contains(file))))
          .filter_map(|id| position_of.get(id).copied())
          .last(),
      );
    }
    conflicts.push(Conflict { commit: position, needs });
  }
  // a dangling reference is a conflict that git doesn't see: the removing commit needs the commit that changed the
  // use. Not when the change needs the branch too (`mutual`): no stack builds both, the change has to move
  for reference in report.dangling.values().flatten().filter(|reference| reference.is_confirmed() && !reference.mutual) {
    let (Some(&commit), Some(&needs)) = (
      position_of.get(reference.removed_by.as_str()),
      reference.fixed_by.as_deref().and_then(|fixed_by| position_of.get(fixed_by)),
    ) else {
      continue;
    };
    conflicts.push(Conflict { commit, needs: vec![needs] });
  }
  conflicts
}

/// The positions of the commits of `branch` and of the branches it is built on
fn present_commits(report: &CheckReport, branch: &BranchCheck, position_of: &HashMap<&str, usize>) -> HashSet<usize> {
  let mut present = HashSet::new();
  let mut seen: HashSet<&str> = HashSet::new();
  let mut current = Some(branch);
  while let Some(check) = current.filter(|check| seen.insert(check.name.as_str())) {
    present.extend(check.commits.iter().filter_map(|id| position_of.get(id.as_str()).copied()));
    current = check
      .built_on
      .as_deref()
      .and_then(|parent| report.branches.iter().find(|candidate| candidate.name == parent));
  }
  present
}

/// The commits `start` needs by its lines that `present` lacks, and what those need in turn, oldest first
fn needed_by_lines(needs_by_lines: &[Vec<usize>], start: usize, present: &HashSet<usize>) -> Vec<usize> {
  let mut needed: BTreeSet<usize> = BTreeSet::new();
  let mut pending = vec![start];
  while let Some(position) = pending.pop() {
    for &to in needs_by_lines.get(position).into_iter().flatten() {
      if to != start && !present.contains(&to) && needed.insert(to) {
        pending.push(to);
      }
    }
  }
  needed.into_iter().collect()
}

fn warnings_of(clustering: &Clustering, stacks: bool) -> Vec<SuggestWarning> {
  clustering
    .warnings
    .iter()
    .map(|warning| match warning {
      Warning::AnchorDependency { anchors, fixup } => {
        let [first, second] = anchors;
        let message = if *fixup {
          format!("A commit of {first} is a fixup of a commit of {second}, but commits with different prefixes stay in their branches: move one")
        } else if stacks {
          format!("A commit of {first} depends on a commit of {second}, and {second} depends on {first}: no stack builds both, and prefixes stay apart: move one")
        } else {
          format!(
            "A commit of {first} depends on a commit of {second}, but commits with different prefixes stay in their branches: turn on stacked branches (`git config {STACKS_CONFIG_KEY} true`) to stack them, or move one"
          )
        };
        SuggestWarning {
          kind: "anchor_dependency".to_string(),
          branches: anchors.to_vec(),
          message,
        }
      }
    })
    .collect()
}
