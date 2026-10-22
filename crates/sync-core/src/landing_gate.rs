//! What refuses the start of a Safe Push or a Dry Run before anything is pushed, one rule for `bd safe-push` and the
//! app (spec/cli/agent-cli.spec.md, spec/branch/patronus.spec.md). One trial check of what the robot lands, in one
//! `CheckSession`:
//! - a dangling reference a commit proves (`dangling_references`): the build of the tree it lands fails;
//! - a missing declaration (`missing_declarations`, `branchdeck.symbols on`): a commit of what lands uses a name that a
//!   commit outside it adds, which no merged Safe Push landed. git merges it and the robot's build fails, as
//!   the robots of 2026-10-17 and 2026-10-21 did. Names, not a compiler, but the edges by symbols held 141 of 141
//!   (docs/design/symbol-dependencies.md), so it refuses the start too, and the user can start anyway.
//!
//! The result is typed (`sync_types::landing`): the CLI and the app each name it in their own words, and each refusal
//! carries the fix the user can run (`LandingFix`).
//!
//! `landing_needs` asks the same of every branch before any start, in one trial check of all of them: what the Safe
//! Push of each branch would be refused for, by the same rule (`refuses`). The repository view computes it in the
//! background after a sync (`landing_needs_of`), the row says "Needs util" with the fix (spec/branch/next-step.spec.md),
//! and `bd status` lists it.

use crate::check::{CheckReport, CheckSession};
use crate::dangling_references::DanglingKind;
use crate::missing_declarations::{MissingDeclaration, missing_declarations};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::regroup::RegroupError;
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use sync_types::landing::{LandingFix, LandingRefusal, ReferenceKind, StackCommit};
use sync_types::patronus::LandingAllowances;
use sync_types::repository_view::RobotOf;
use tracing::instrument;

/// What a start lands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandingSubject<'a> {
  /// The branch and the branches it is built on: one chain, one robot
  Branch(&'a str),
  /// Every branch of the stack of this root, which may fork: the robot of the whole stack
  WholeStack(&'a str),
}

/// The check of a start: what lands, its trial report and the names it misses
#[derive(Debug, Clone)]
pub struct LandingGate {
  /// Root first; for a whole stack every member
  pub lands: Vec<String>,
  /// The trial check of what lands; for a whole stack of every branch
  pub report: CheckReport,
  /// The uses, by commits of what lands, of names a commit outside it adds that no merged Safe Push landed
  pub missing: Vec<MissingDeclaration>,
  /// The branch a restack puts on the branch that adds a missing name: the root of the chain or of the whole stack
  root: String,
  /// Each branch of the snapshot and the branches it is declared on, nearest first
  chains: Vec<(String, Vec<String>)>,
}

/// The check of what the start of `subject` lands, in `session`; `landed` holds the commits (original ids) a merged Safe
/// Push landed, whose names the target has
#[instrument(skip(session, landed))]
pub fn check_landing_gate(session: &CheckSession, subject: LandingSubject<'_>, landed: &HashSet<String>) -> Result<LandingGate, RegroupError> {
  let chains = chains_of(session);
  let (report, lands, root) = match subject {
    LandingSubject::Branch(name) => {
      let lands = lands_of(&chains, name);
      let report = session.check_landing(name, &lands)?;
      let root = lands.first().cloned().unwrap_or_default();
      (report, lands, root)
    }
    LandingSubject::WholeStack(root) => {
      let lands: Vec<String> = chains
        .iter()
        .filter(|(branch, chain)| branch == root || chain.iter().any(|parent| parent == root))
        .map(|(branch, _)| branch.clone())
        .collect();
      let report = session.check_with_dangling_of(None, &lands)?;
      (report, lands, root.to_string())
    }
  };
  let missing = missing_declarations(session, &report)?
    .into_iter()
    .filter(|missing| refuses(missing, &lands, landed))
    .collect();
  Ok(LandingGate {
    lands,
    report,
    missing,
    root,
    chains,
  })
}

/// What the Safe Push of each branch needs that a branch outside it adds, the refusals `check_landing_gate` would give
/// its start, from one trial check of every branch and one search of the missing declarations in `session`; `landed` as
/// there. Only the branches with a refusal are listed. The repository view computes it in the background after a sync
/// (`landing_needs_of`), `bd status` with its sync
#[instrument(skip(session, landed))]
pub fn landing_needs(session: &CheckSession, landed: &HashSet<String>) -> Result<BTreeMap<String, Vec<LandingRefusal>>, RegroupError> {
  let chains = chains_of(session);
  let report = session.check(None)?;
  let missing = missing_declarations(session, &report)?;
  let mut needs = BTreeMap::new();
  for check in &report.branches {
    let lands = lands_of(&chains, &check.name);
    let landing = Landing {
      lands: &lands,
      report: &report,
      missing: missing.iter().filter(|missing| refuses(missing, &lands, landed)).collect(),
      root: lands.first().map_or_default(String::as_str),
      chains: &chains,
    };
    let refusals = landing.refusals(session);
    if !refusals.is_empty() {
      needs.insert(check.name.clone(), refusals);
    }
  }
  Ok(needs)
}

/// `landing_needs` in a session of the current grouping: the background computation of the repository view
#[instrument(skip(git_executor, cache, landed))]
pub fn landing_needs_of(
  git_executor: &GitCommandExecutor,
  cache: &ObjectCache,
  repository_path: &str,
  branch_prefix: &str,
  landed: &HashSet<String>,
) -> Result<BTreeMap<String, Vec<LandingRefusal>>, RegroupError> {
  let session = CheckSession::open(git_executor, repository_path, branch_prefix, cache)?;
  landing_needs(&session, landed)
}

/// Names in each missing declaration of `refusals` the running Safe Push that lands the branch adding the name
/// (`landing_of`, by branch): a start beside it fails all the same, its robot tests that branch without this one
pub fn name_landing_robots(refusals: &mut [LandingRefusal], landing_of: impl Fn(&str) -> Option<RobotOf>) {
  for refusal in refusals {
    if let LandingRefusal::MissingDeclaration { declared_by, landing_in, .. } = refusal {
      *landing_in = if declared_by.branch.is_empty() { None } else { landing_of(&declared_by.branch) };
    }
  }
}

/// The one rule of a missing declaration that refuses a start: a commit of what lands (`lands`) uses a name that a commit
/// outside it adds, which no merged Safe Push landed (`landed`)
fn refuses(missing: &MissingDeclaration, lands: &[String], landed: &HashSet<String>) -> bool {
  lands.contains(&missing.branch) && missing.declared_in.as_ref().is_none_or(|declared_in| !lands.contains(declared_in)) && !landed.contains(&missing.declared_by)
}

/// Each branch of the snapshot and the branches it is declared on, nearest first
fn chains_of(session: &CheckSession) -> Vec<(String, Vec<String>)> {
  session.snapshot().forest.as_ref().map_or_default(|forest| {
    forest
      .order
      .iter()
      .map(|branch| (branch.clone(), forest.chain(branch).into_iter().skip(1).map(str::to_string).collect()))
      .collect()
  })
}

/// What the Safe Push of `name` lands: the branches it is built on, root first, then the branch
fn lands_of(chains: &[(String, Vec<String>)], name: &str) -> Vec<String> {
  let built: Vec<String> = chains
    .iter()
    .find(|(branch, _)| branch == name)
    .map_or_default(|(_, chain)| chain.iter().rev().cloned().collect());
  let mut lands: Vec<String> = built.into_iter().filter(|branch| chains.iter().any(|(other, _)| other == branch)).collect();
  lands.push(name.to_string());
  lands
}

impl LandingGate {
  /// Every refusal of the start, each with what the user can do; empty when nothing refuses it
  pub fn refusals(&self, session: &CheckSession) -> Vec<LandingRefusal> {
    Landing {
      lands: &self.lands,
      report: &self.report,
      missing: self.missing.iter().collect(),
      root: &self.root,
      chains: &self.chains,
    }
    .refusals(session)
  }

  /// The refusals that hold the start back after the user's overrides
  pub fn blocking(&self, session: &CheckSession, allow: LandingAllowances) -> Vec<LandingRefusal> {
    self
      .refusals(session)
      .into_iter()
      .filter(|refusal| match refusal {
        LandingRefusal::DanglingReference { .. } => !allow.allow_dangling,
        LandingRefusal::MissingDeclaration { .. } => !allow.allow_missing,
      })
      .collect()
  }
}

/// What one start lands, borrowed from a gate or from the check of every branch (`landing_needs`)
struct Landing<'a> {
  lands: &'a [String],
  report: &'a CheckReport,
  /// The missing declarations that refuse it (`refuses`)
  missing: Vec<&'a MissingDeclaration>,
  /// The branch a restack puts on the branch the start needs: the root of the chain or of the whole stack
  root: &'a str,
  chains: &'a [(String, Vec<String>)],
}

impl Landing<'_> {
  /// The refusals of the start: the proven dangling references of what lands, then the missing declarations
  fn refusals(&self, session: &CheckSession) -> Vec<LandingRefusal> {
    let commit = |id: &str| {
      session.stack_commit_of(id).unwrap_or_else(|| StackCommit {
        id: id.to_string(),
        subject: String::new(),
        branch: String::new(),
      })
    };
    let dangling = self
      .lands
      .iter()
      .filter_map(|branch| self.report.dangling.get(branch).map(|references| (branch, references)))
      .flat_map(|(branch, references)| references.iter().map(move |reference| (branch, reference)))
      .filter(|(_, reference)| reference.is_confirmed())
      .filter_map(|(branch, reference)| {
        let fixed_by = commit(reference.fixed_by.as_deref()?);
        Some(LandingRefusal::DanglingReference {
          branch: branch.clone(),
          reference: match reference.kind {
            DanglingKind::Removed => ReferenceKind::Removed,
            DanglingKind::Changed => ReferenceKind::Changed,
            DanglingKind::Duplicate => ReferenceKind::Duplicate,
          },
          identifier: reference.identifier.clone(),
          file: reference.file.clone(),
          line: reference.line,
          removed_by: commit(&reference.removed_by),
          fix: self.fix_of(&fixed_by, branch),
          fixed_by,
        })
      });
    let missing = self.missing.iter().map(|missing| {
      let declared_by = commit(&missing.declared_by);
      LandingRefusal::MissingDeclaration {
        branch: missing.branch.clone(),
        identifier: missing.identifier.clone(),
        file: missing.file.clone(),
        line: missing.line,
        used_by: commit(&missing.used_by),
        fix: self.fix_of(&declared_by, &missing.branch),
        declared_by,
        landing_in: None,
      }
    });
    dangling.chain(missing).collect()
  }

  /// A restack of the root onto the branch of `needed`, the commit the start needs, unless stacked branches are off, that
  /// branch is built on what lands (a cycle) or the commit is unassigned: then the move of the commit into `into`, the
  /// branch that needs it
  fn fix_of(&self, needed: &StackCommit, into: &str) -> LandingFix {
    let built_on_lands = |branch: &str| {
      self
        .chains
        .iter()
        .find(|(name, _)| name == branch)
        .is_some_and(|(_, chain)| chain.iter().any(|parent| self.lands.contains(parent)))
    };
    let onto = needed.branch.as_str();
    if !onto.is_empty() && self.report.stacks && !built_on_lands(onto) {
      LandingFix::Restack {
        branch: self.root.to_string(),
        onto: onto.to_string(),
      }
    } else {
      LandingFix::MoveCommit {
        commit: needed.clone(),
        into: into.to_string(),
      }
    }
  }
}

/// What the app asks before a start (`check_landing_gate`), and the start itself (`start_patronus_robot`)
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingGateParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// The branch that starts; with `stack` the root of the whole stack
  pub branch_name: String,
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: bool,
  /// The commits (original ids) merged Safe Pushes landed (`landedBy` of the repository view)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub landed_commits: Vec<String>,
  /// What the user starts anyway
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub allow: LandingAllowances,
}

/// The refusals that hold a start back after the user's overrides; empty: it starts
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingCheck {
  pub refusals: Vec<LandingRefusal>,
}

/// The start of `subject` is refused: the first refusal names why
#[derive(Debug, Clone, thiserror::Error)]
#[error("{}", refused_message(.subject, .refusals))]
pub struct LandingRefused {
  pub subject: String,
  pub refusals: Vec<LandingRefusal>,
}

/// The check of the start `params` asks, in a session of the current grouping
#[instrument(skip(git_executor, cache))]
pub fn landing_check(git_executor: &GitCommandExecutor, cache: &ObjectCache, params: &LandingGateParams) -> anyhow::Result<LandingCheck> {
  let session = CheckSession::open(git_executor, &params.repository_path, &params.branch_prefix, cache)?;
  let subject = if params.stack {
    LandingSubject::WholeStack(&params.branch_name)
  } else {
    LandingSubject::Branch(&params.branch_name)
  };
  let landed: HashSet<String> = params.landed_commits.iter().cloned().collect();
  let gate = check_landing_gate(&session, subject, &landed)?;
  Ok(LandingCheck {
    refusals: gate.blocking(&session, params.allow),
  })
}

/// `landing_check`, refused as `LandingRefused` when a refusal holds the start back: the start of the app runs it before
/// it asks Patronus
pub fn ensure_landing_starts(git_executor: &GitCommandExecutor, cache: &ObjectCache, params: &LandingGateParams) -> anyhow::Result<()> {
  let check = landing_check(git_executor, cache, params)?;
  if check.refusals.is_empty() {
    return Ok(());
  }
  let subject = if params.stack {
    format!("the whole {} stack", params.branch_name)
  } else {
    params.branch_name.clone()
  };
  Err(
    LandingRefused {
      subject,
      refusals: check.refusals,
    }
    .into(),
  )
}

/// "auth would not build: … (2 more)"
fn refused_message(subject: &str, refusals: &[LandingRefusal]) -> String {
  let Some(first) = refusals.first() else { return format!("{subject} builds") };
  let more = match refusals.len() - 1 {
    0 => String::new(),
    rest => format!(" ({rest} more)"),
  };
  format!("{subject} would not build: {}{more}", first.reason())
}

#[cfg(test)]
#[path = "landing_gate_test.rs"]
pub(crate) mod landing_gate_test;
