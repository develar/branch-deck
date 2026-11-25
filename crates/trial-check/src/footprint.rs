//! What the changes of each branch touch where it lands (docs/design/git-operations.md, "Landing footprint";
//! openspec/specs/branch/landing-plan/spec.md): the required checks of Patronus its paths start (`commit_rules::checks`)
//! and the code owners of its paths (`git_ops::ownership`), as the tree of the baseline has them. The landing needs
//! compute it in their `CheckSession` (`landing_gate::landing_needs`), so the price of landing two branches in one robot
//! (`domain_policy::landing_plan`) reads no git of its own.
//!
//! The paths of a branch are those its commits change (`CheckSession::commit_diffs`, read once per session, from the
//! object cache). The `OWNERSHIP` files of their directories and `.patronus/config.yaml` come from one `cat-file
//! --batch` of the baseline, remembered in this process by the baseline commit and the paths: a sync that moves neither
//! reads nothing again; the checks are parsed once per blob of the config. It fails open: what can't be read is unknown
//! with the reason (`LandingFootprints::checks_unknown`, `owners_unknown`), never an error of the needs.

use crate::check::CheckSession;
use commit_rules::PATRONUS_FILE;
use commit_rules::checks::{PatronusChecks, parse_patronus_checks};
use domain_types::landing::{BranchFootprint, LandingFootprints};
use git_ops::ownership::{OwnershipRules, read_ownership_with_files};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{instrument, warn};

/// What the baseline says of the paths: the checks of the config (or why they are unknown) and the ownership rules
#[derive(Clone)]
struct Rules {
  checks: Checks,
  ownership: Result<Arc<OwnershipRules>, String>,
}

/// The checks of a config, or why they are unknown
type Checks = Result<Arc<PatronusChecks>, String>;
/// What the rules of the baseline were read for: the repository, the baseline commit, a hash of the paths
type RulesKey = (String, String, u64);

/// The rules read last, by what they were read for
static READ: LazyLock<Mutex<Option<(RulesKey, Rules)>>> = LazyLock::new(|| Mutex::new(None));
/// The checks parsed last, by the blob id of the config: a baseline that moved without a change of it parses nothing
static PARSED: LazyLock<Mutex<Option<(String, Checks)>>> = LazyLock::new(|| Mutex::new(None));

/// The footprint of every branch of `session` with commits
#[instrument(skip(session))]
pub fn landing_footprints(session: &CheckSession) -> LandingFootprints {
  let paths = match paths_by_branch(session) {
    Ok(paths) => paths,
    Err(reason) => {
      warn!(%reason, "cannot read the paths of the branches");
      return LandingFootprints {
        checks_unknown: Some(reason.clone()),
        owners_unknown: Some(reason),
        branches: BTreeMap::new(),
      };
    }
  };
  let all: Vec<String> = paths.values().flatten().cloned().collect::<BTreeSet<_>>().into_iter().collect();
  let rules = rules_of(session, &all);
  let branches = paths
    .into_iter()
    .map(|(branch, paths)| {
      let checks = rules
        .checks
        .as_ref()
        .ok()
        .map_or_default(|checks| checks.started_by(&paths).filter(|check| !check.optional).map(|check| check.name.clone()).collect());
      let owners = rules.ownership.as_ref().ok().map_or_default(|ownership| {
        paths
          .iter()
          .filter_map(|path| ownership.owner_of(path).map(|owner| owner.owner))
          .collect::<BTreeSet<_>>()
          .into_iter()
          .collect()
      });
      (branch, BranchFootprint { checks, owners })
    })
    .collect();
  LandingFootprints {
    checks_unknown: rules.checks.err(),
    owners_unknown: rules.ownership.err(),
    branches,
  }
}

/// Branch -> the paths its commits change (a rename both), sorted; an unassigned commit is of no branch
fn paths_by_branch(session: &CheckSession) -> Result<BTreeMap<String, Vec<String>>, String> {
  let diffs = session.commit_diffs().map_err(|error| error.to_string())?;
  let groups = &session.snapshot().groups;
  let mut paths: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
  for diff in diffs {
    let Some(branch) = groups.get(&diff.id).and_then(Option::as_deref) else {
      continue;
    };
    let of = paths.entry(branch.to_string()).or_default();
    for file in &diff.files {
      of.insert(file.path.clone());
      of.extend(file.old_path.iter().cloned());
    }
  }
  Ok(paths.into_iter().map(|(branch, paths)| (branch, paths.into_iter().collect())).collect())
}

/// The rules of the baseline of `session` for `paths`: remembered, else from one `cat-file --batch`
fn rules_of(session: &CheckSession, paths: &[String]) -> Rules {
  let mut hasher = DefaultHasher::new();
  paths.hash(&mut hasher);
  let key = (session.repository_path().to_string(), session.baseline_commit().to_string(), hasher.finish());
  // never a guard across git: looked up, then read, then remembered
  if let Some((_, rules)) = READ.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().filter(|(of, _)| *of == key) {
    return rules.clone();
  }
  let rules = match read_ownership_with_files(session.git_executor(), session.repository_path(), session.baseline_commit(), paths, &[PATRONUS_FILE]) {
    Ok((ownership, mut files)) => Rules {
      checks: match files.pop().flatten() {
        Some((blob, text)) => parsed(&blob, &text),
        None => Err(format!("the baseline has no {PATRONUS_FILE}")),
      },
      ownership: Ok(Arc::new(ownership)),
    },
    Err(error) => {
      let reason = format!("Cannot read the baseline: {error:#}");
      warn!(%reason, "cannot read the ownership and the checks of the baseline");
      Rules {
        checks: Err(reason.clone()),
        ownership: Err(reason),
      }
    }
  };
  *READ.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((key, rules.clone()));
  rules
}

/// The checks of the config `text` of the blob `blob`, parsed once per blob
fn parsed(blob: &str, text: &str) -> Checks {
  if let Some((_, checks)) = PARSED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().filter(|(of, _)| of == blob) {
    return checks.clone();
  }
  let checks = parse_patronus_checks(text).map(Arc::new);
  *PARSED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((blob.to_string(), checks.clone()));
  checks
}

#[cfg(test)]
#[path = "footprint_test.rs"]
mod footprint_test;
