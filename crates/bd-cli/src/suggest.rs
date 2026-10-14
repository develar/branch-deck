//! `bd suggest`: a grouping of the commits ahead of the baseline into branches, as a plan for `bd group`, already
//! checked. It reads the commits once (`CheckSession`), clusters them (`clustering.rs`) and checks the plan with a trial
//! sync; a conflict joins the commits it needs and clusters again. It writes git objects and the cache only
//! (docs/guidelines/agent-cli.md)

use crate::areas::{DEFAULT_AREA_DEPTH, DirLines, top_areas};
use crate::check_command::{CheckedBranch, changed_files, checked_branches, missing_commits};
use crate::clustering::{
  ClusterChange, ClusterInput, ClusterOptions, Clustering, CommitFeatures, Conflict, DEFAULT_MAX_COMMITS, FileLines, MAX_REPAIR_ROUNDS, Warning, anonymize, cluster_change,
  cluster_with_repair, is_issue_only, name_clusters, subject_words, why,
};
use crate::commits::{CommitsHeader, LinkedDiffs, fixup_targets, read_linked_diffs};
use crate::context::RepoContext;
use crate::dependencies::compute_dependencies;
use crate::error::CliError;
use crate::generated::is_hub_file;
use crate::output::SchemaVersion;
use crate::rewrite::regroup_error;
use clap::builder::RangedU64ValueParser;
use git_ops::commit_list::Commit;
use git_ops::model::{BranchKind, sanitize_branch_name, strip_branch_namespace};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::RandomState;
use sync_core::check::{CheckOutcome, CheckReport, CheckSession};
use sync_core::commit_grouper::parse_subject;
use sync_core::regroup::{RegroupPlan, Snapshot};
use sync_core::stack::stack_path;
use sync_utils::issue_pattern::{find_issue_number, issue_keys_in, parse_semantic_prefix};

#[derive(Debug, clap::Args)]
pub struct SuggestArgs {
  /// Ignore the `(prefix)` of the subjects: cluster every commit as if it had none
  #[arg(long)]
  pub fresh: bool,
  /// No merge of two clusters by score makes a branch of more commits
  #[arg(long, value_name = "N", default_value_t = DEFAULT_MAX_COMMITS, value_parser = RangedU64ValueParser::<usize>::new().range(1..))]
  pub max_commits: usize,
  /// Put a cluster of whole issue branches into the plan too, which gives its commits a prefix
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

/// The plan of `bd group` (`RegroupPlan`)
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct SuggestPlan {
  pub branches: BTreeMap<String, Vec<String>>,
  pub unassign: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SuggestedCluster {
  /// The branch name
  pub name: String,
  pub commit_count: usize,
  pub change: ClusterChange,
  /// The branches its commits are in now, by their oldest commit
  pub from: Vec<String>,
  /// Why the commits are together, one line to quote
  pub why: String,
  /// A merge by paths near the threshold, or one that log adjacency decided, made it: review it first
  pub weak: bool,
  /// The issue keys of its subjects
  pub issue_keys: Vec<String>,
  /// The directories with the most lines, as in `bd commits --brief`
  pub areas: Vec<DirLines>,
  /// `issue_group`: whole issue branches, left out of the plan so that their subjects stay (`--rewrite-issues` puts
  /// them in)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
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
  /// `anchor_dependency`: commits of two prefixes depend on each other, and prefixes stay apart; `unresolved_conflict`:
  /// a branch still conflicts after the repair
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
}

impl Reading {
  fn read(context: &RepoContext) -> Result<Self, CliError> {
    let session = CheckSession::open(&context.git, &context.repository, &context.prefix, &context.cache()).map_err(regroup_error)?;
    let snapshot = session.snapshot();
    let ids: Vec<String> = snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
    let diffs = read_linked_diffs(context, &ids, false, false)?;
    let input = cluster_input(snapshot, &diffs);
    Ok(Self { session, diffs, input })
  }
}

/// The names, the plan (positions) and the check of the clustering the repair loop checked last
struct Checked {
  names: Vec<String>,
  branches: BTreeMap<String, Vec<usize>>,
  unassign: Vec<usize>,
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
  let Reading { session, diffs, input } = Reading::read(context)?;
  let snapshot = session.snapshot();
  let commit_ids: Vec<&str> = snapshot.commits.iter().map(|commit| commit.id.as_str()).collect();
  let position_of: HashMap<&str, usize> = commit_ids.iter().enumerate().map(|(position, id)| (*id, position)).collect();
  // every changed file, generated ones too: a conflict may be in a lock file
  let files_of = changed_files(&diffs.diffs);
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
    let names = name_clusters(&input, clustering, &taken);
    let (branches, unassign) = plan_of(&input, clustering, &names, args.rewrite_issues);
    let plan = RegroupPlan {
      branches: branches
        .iter()
        .map(|(name, commits)| (name.clone(), commits.iter().map(|&commit| commit_ids[commit].to_string()).collect()))
        .collect(),
      unassign: unassign.iter().map(|&commit| commit_ids[commit].to_string()).collect(),
    };
    let report = session.check(Some(&plan)).map_err(regroup_error)?;
    let conflicts = conflicts_of(&report, &files_of, &position_of);
    last = Some(Checked {
      names,
      branches,
      unassign,
      report,
    });
    Ok::<_, CliError>(conflicts)
  })?;
  let Checked {
    names,
    branches,
    unassign,
    report,
  } = last.expect("the check ran");

  let short = context.short_ids(commit_ids.iter().copied().chain([snapshot.head.as_str()]));
  let short_of = |commit: usize| short.get(commit_ids[commit]).to_string();
  let linked = diffs.linked();
  let clusters = repaired
    .clustering
    .clusters
    .iter()
    .zip(&names)
    .map(|(cluster, name)| {
      let (change, from) = cluster_change(&input, cluster, name);
      let via = (!args.rewrite_issues && is_issue_only(&input, cluster)).then(|| "issue_group".to_string());
      SuggestedCluster {
        name: name.clone(),
        commit_count: cluster.commits.len(),
        change,
        from: from.into_iter().map(str::to_string).collect(),
        why: why(&input, cluster),
        weak: cluster.weak,
        issue_keys: cluster.keys.clone(),
        areas: top_areas(cluster.commits.iter().flat_map(|&commit| &linked[commit].files), DEFAULT_AREA_DEPTH),
        via,
      }
    })
    .collect();
  let checked = checked_branches(&report, &files_of, &short);
  let mut warnings = warnings_of(&repaired.clustering);
  for (branch, result) in report.branches.iter().zip(&checked).filter(|(_, checked)| checked.result != "ok") {
    let conflict = match &branch.outcome {
      CheckOutcome::Conflict { commit, .. } => position_of
        .get(commit.as_str())
        .and_then(|&position| repaired.conflicts.iter().find(|conflict| conflict.commit == position)),
      _ => None,
    };
    let reason = match conflict {
      Some(conflict) if conflict.needs.is_empty() => "no older commit ahead of the baseline changes the conflicting files".to_string(),
      Some(_) if repaired.rounds == MAX_REPAIR_ROUNDS => format!("the {MAX_REPAIR_ROUNDS} repair rounds ran out"),
      Some(_) => "the commits it needs have another prefix: stack the branch on theirs, or move them".to_string(),
      None => format!("the repair joins only conflicting commits, and the branch is {}", result.result),
    };
    warnings.push(SuggestWarning {
      kind: "unresolved_conflict".to_string(),
      branches: vec![branch.name.clone()],
      message: format!("{} is {} after {} repair rounds: {reason}", branch.name, result.result, repaired.rounds),
    });
  }

  Ok(SuggestOutput {
    header: CommitsHeader {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      head: short.get(&snapshot.head).to_string(),
      baseline: snapshot.baseline.clone(),
    },
    plan: SuggestPlan {
      branches: branches.into_iter().map(|(name, commits)| (name, commits.into_iter().map(short_of).collect())).collect(),
      unassign: unassign.into_iter().map(short_of).collect(),
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

/// The features of every commit of the snapshot, oldest first
fn cluster_input(snapshot: &Snapshot, diffs: &LinkedDiffs) -> ClusterInput {
  let position_of: HashMap<&str, usize> = snapshot.commits.iter().enumerate().map(|(position, commit)| (commit.id.as_str(), position)).collect();
  let ordered: Vec<(&Commit, Option<&str>)> = snapshot
    .commits
    .iter()
    .map(|commit| (commit, snapshot.groups.get(&commit.id).and_then(Option::as_deref)))
    .collect();
  let fixups = fixup_targets(&snapshot.head, &ordered, snapshot.stacks);
  let linked = diffs.linked();
  let commits = ordered
    .iter()
    .enumerate()
    .map(|(position, &(commit, group))| {
      let parsed = parse_subject(&commit.subject);
      let prefix = parsed.prefix.and_then(|prefix| {
        let path = if snapshot.stacks {
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
  let dependencies = compute_dependencies(linked).into_iter().map(|edge| [edge.from, edge.to]).collect();
  ClusterInput { commits, dependencies }
}

/// The words of a subject after its conventional prefix, without its issue keys: `fix(vm): ABC-1 x` gives `x`
fn name_words(text: &str) -> Vec<String> {
  let without_keys = issue_keys_in(text).fold(text.to_string(), |text, key| text.replacen(key, " ", 1));
  subject_words(&without_keys)
}

/// The plan of a clustering: per branch name the commits that move there, and the commits that lose their prefix to
/// fall back to their own issue branch. A commit already grouped by that name stays out, and so does a fixup that
/// follows its target; a cluster of whole issue branches stays out unless `rewrite_issues`
pub(crate) fn plan_of(input: &ClusterInput, clustering: &Clustering, names: &[String], rewrite_issues: bool) -> (BTreeMap<String, Vec<usize>>, Vec<usize>) {
  let mut branches: BTreeMap<String, Vec<usize>> = BTreeMap::new();
  let mut unassign: Vec<usize> = Vec::new();
  // commit -> where the plan puts it: `Some(name)` or `None` for unassign
  let mut destination: HashMap<usize, Option<&str>> = HashMap::new();
  for (cluster, name) in clustering.clusters.iter().zip(names) {
    if !rewrite_issues && is_issue_only(input, cluster) {
      continue;
    }
    for &position in &cluster.commits {
      let commit = &input.commits[position];
      if commit.prefix.as_deref().or(commit.key.as_deref()) == Some(name.as_str()) {
        continue;
      }
      if commit.prefix.is_some() && commit.key.as_deref() == Some(name.as_str()) {
        destination.insert(position, None);
      } else {
        destination.insert(position, Some(name.as_str()));
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
      Some(name) => branches.entry(name.to_string()).or_default().push(position),
      None => unassign.push(position),
    }
  }
  (branches, unassign)
}

/// The conflicts of a report, as positions: each conflicting commit with the commits it likely needs, else the newest
/// older commit outside its branch that changes a conflicting file
fn conflicts_of(report: &CheckReport, files_of: &HashMap<String, HashSet<String>>, position_of: &HashMap<&str, usize>) -> Vec<Conflict> {
  let mut conflicts = Vec::new();
  for branch in &report.branches {
    let CheckOutcome::Conflict { commit, files } = &branch.outcome else { continue };
    let Some(&position) = position_of.get(commit.as_str()) else { continue };
    let mut needs: Vec<usize> = missing_commits(report, branch, commit, files, files_of)
      .iter()
      .filter_map(|id| position_of.get(id.as_str()).copied())
      .collect();
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
  conflicts
}

fn warnings_of(clustering: &Clustering) -> Vec<SuggestWarning> {
  clustering
    .warnings
    .iter()
    .map(|warning| match warning {
      Warning::AnchorDependency { anchors, fixup } => SuggestWarning {
        kind: "anchor_dependency".to_string(),
        branches: anchors.to_vec(),
        message: format!(
          "A commit of {} {} a commit of {}, but commits with different prefixes stay in their branches: stack them, or move one",
          anchors[0],
          if *fixup { "is a fixup of" } else { "depends on" },
          anchors[1]
        ),
      },
    })
    .collect()
}
