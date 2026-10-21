//! What refuses the start of a Safe Push or a Dry Run before anything is pushed, one rule for `bd safe-push` and the
//! app (spec/cli/agent-cli.spec.md, spec/branch/patronus.spec.md). One trial check of what the robot lands, in one
//! `CheckSession`:
//! - a dangling reference a commit proves (`dangling_references`): the build of the tree it lands fails;
//! - a missing declaration (`missing_declarations`, `branchdeck.symbols on`): a commit of what lands uses a name that a
//!   commit outside it adds, which no merged Safe Push landed. git merges it and the robot's build fails, as
//!   the robots of 2026-10-17 and 2026-10-21 did. Names, not a compiler, but the edges by symbols held 141 of 141
//!   (docs/design/symbol-dependencies.md), so it refuses the start too, and the user can start anyway.
//!
//! The result is typed: the CLI and the app each name it in their own words, and each refusal carries the fix the
//! user can run (`LandingFix`).

use crate::check::{CheckReport, CheckSession, StackCommit};
use crate::dangling_references::DanglingKind;
use crate::missing_declarations::{MissingDeclaration, missing_declarations};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::regroup::RegroupError;
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use sync_types::patronus::LandingAllowances;
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

/// Why a start is refused
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LandingRefusal {
  /// A commit of `branch` removes or changes a declaration, or adds one again, that the tree it lands still uses; the
  /// commit outside it that removes the use proves it
  DanglingReference {
    branch: String,
    reference: ReferenceKind,
    identifier: String,
    /// Where the use is
    file: String,
    line: u32,
    removed_by: StackCommit,
    fixed_by: StackCommit,
  },
  /// A commit of `branch` uses a name that a commit outside what lands adds
  MissingDeclaration {
    branch: String,
    identifier: String,
    /// Where the use is
    file: String,
    line: u32,
    used_by: StackCommit,
    declared_by: StackCommit,
    fix: LandingFix,
  },
}

/// What a branch does to the declaration of a dangling reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReferenceKind {
  Removed,
  Changed,
  Duplicate,
}

/// How the user gets the missing declaration into what lands
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LandingFix {
  /// Stack `branch`, the root of what lands, on `onto`, the branch that adds the name: the start lands it too
  Restack { branch: String, onto: String },
  /// Move the commit that adds the name into `into`, the branch that uses it: the commit is unassigned, stacked branches
  /// are off, or its branch is built on what lands, so a restack would close a cycle
  MoveCommit { commit: StackCommit, into: String },
}

/// The check of what the start of `subject` lands, in `session`; `landed` holds the commits (original ids) a merged Safe
/// Push landed, whose names the target has
#[instrument(skip(session, landed))]
pub fn check_landing_gate(session: &CheckSession, subject: LandingSubject<'_>, landed: &HashSet<String>) -> Result<LandingGate, RegroupError> {
  let snapshot = session.snapshot();
  let chains: Vec<(String, Vec<String>)> = snapshot.forest.as_ref().map_or_default(|forest| {
    forest
      .order
      .iter()
      .map(|branch| (branch.clone(), forest.chain(branch).into_iter().skip(1).map(str::to_string).collect()))
      .collect()
  });
  let (report, lands, root) = match subject {
    LandingSubject::Branch(name) => {
      let built: Vec<String> = chains
        .iter()
        .find(|(branch, _)| branch == name)
        .map_or_default(|(_, chain)| chain.iter().rev().cloned().collect());
      let mut lands: Vec<String> = built.into_iter().filter(|branch| chains.iter().any(|(other, _)| other == branch)).collect();
      lands.push(name.to_string());
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
    .filter(|missing| lands.contains(&missing.branch))
    .filter(|missing| missing.declared_in.as_ref().is_none_or(|declared_in| !lands.contains(declared_in)) && !landed.contains(&missing.declared_by))
    .collect();
  Ok(LandingGate {
    lands,
    report,
    missing,
    root,
    chains,
  })
}

impl LandingGate {
  /// Every refusal of the start, each with what the user can do; empty when nothing refuses it
  pub fn refusals(&self, session: &CheckSession) -> Vec<LandingRefusal> {
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
        let fixed_by = reference.fixed_by.as_deref()?;
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
          fixed_by: commit(fixed_by),
        })
      });
    let missing = self.missing.iter().map(|missing| LandingRefusal::MissingDeclaration {
      branch: missing.branch.clone(),
      identifier: missing.identifier.clone(),
      file: missing.file.clone(),
      line: missing.line,
      used_by: commit(&missing.used_by),
      declared_by: commit(&missing.declared_by),
      fix: self.fix_of(missing, &commit),
    });
    dangling.chain(missing).collect()
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

  /// A restack of the root onto the branch that adds the name, unless stacked branches are off, that branch is built on
  /// what lands (a cycle) or the commit is unassigned: then the move of the commit into the branch that uses it
  fn fix_of(&self, missing: &MissingDeclaration, commit: &dyn Fn(&str) -> StackCommit) -> LandingFix {
    let built_on_lands = |branch: &str| {
      self
        .chains
        .iter()
        .find(|(name, _)| name == branch)
        .is_some_and(|(_, chain)| chain.iter().any(|parent| self.lands.contains(parent)))
    };
    match missing.declared_in.as_deref() {
      Some(onto) if self.report.stacks && !built_on_lands(onto) => LandingFix::Restack {
        branch: self.root.clone(),
        onto: onto.to_string(),
      },
      _ => LandingFix::MoveCommit {
        commit: commit(&missing.declared_by),
        into: missing.branch.clone(),
      },
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

impl LandingRefusal {
  /// In a sentence: what breaks the build of what lands
  pub fn reason(&self) -> String {
    let named = |commit: &StackCommit| {
      if commit.branch.is_empty() {
        format!("\"{}\" (unassigned)", commit.subject)
      } else {
        format!("\"{}\" of {}", commit.subject, commit.branch)
      }
    };
    match self {
      Self::DanglingReference {
        identifier,
        file,
        removed_by,
        fixed_by,
        reference,
        ..
      } => {
        let change = match reference {
          ReferenceKind::Removed => "removes",
          ReferenceKind::Changed => "changes",
          ReferenceKind::Duplicate => "declares again",
        };
        format!("{} {change} {identifier}, which {file} still uses; {} changes that use", named(removed_by), named(fixed_by))
      }
      Self::MissingDeclaration {
        identifier,
        file,
        used_by,
        declared_by,
        ..
      } => format!("{} uses {identifier} in {file}, which {} adds", named(used_by), named(declared_by)),
    }
  }
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
