//! The reviewed tip of a local branch (spec/branch/review-plan.spec.md): `refs/branch-deck/reviewed/<sha256 of the
//! branch's full name>/{base,tip}`, the base and the tip the branch had when the user last marked it reviewed. The refs
//! keep the reviewed commits from `git gc`, so the interdiff of a later review still reads them after a rewrite. One
//! `for-each-ref` lists them with the local branches (`read_reviewed_refs`); a mark is one `update-ref --stdin`
//! (`ReviewedRefs::mark`) with compare-and-swap on every ref as that listing read it, which also deletes the pairs of
//! the branches that are gone. `mark_reviewed_core` (crates/review-plan) is their one writer.

use crate::ref_transaction::RefTransaction;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// The namespace of the reviewed refs, a pair per branch
pub const REVIEWED_REFS: &str = "refs/branch-deck/reviewed/";

/// The reflog message of a mark: `branch-deck: mark-reviewed`
pub const MARK_REVIEWED: &str = "mark-reviewed";

const BRANCHES: &str = "refs/heads/";
const BASE: &str = "base";
const TIP: &str = "tip";

/// The key of the pair of the branch `full_branch_name` (`user/virtual/auth`, without `refs/heads/`): a hash of the
/// name, so the refs of a branch `a` and a branch `a/b` never clash
pub fn reviewed_key(full_branch_name: &str) -> String {
  Sha256::digest(full_branch_name.as_bytes()).iter().fold(String::with_capacity(64), |mut key, byte| {
    let _ = write!(key, "{byte:02x}");
    key
  })
}

/// The ref of the `base` or the `tip` of the pair `key`
fn pair_ref(key: &str, part: &str) -> String {
  format!("{REVIEWED_REFS}{key}/{part}")
}

/// The base and the tip of a branch as the user reviewed it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewedPair {
  /// The commit the reviewed commits are built on
  pub base: String,
  /// The last reviewed commit
  pub tip: String,
}

/// The refs of one pair as listed; a pair a crash or a hand cut short may lack one of them
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ListedPair {
  base: Option<String>,
  tip: Option<String>,
}

/// The reviewed refs and the local branches, as one `for-each-ref` read them
#[derive(Debug, Clone, Default)]
pub struct ReviewedRefs {
  /// Key -> the refs of its pair
  pairs: HashMap<String, ListedPair>,
  /// Full branch name (without `refs/heads/`) -> tip
  pub branches: HashMap<String, String>,
}

/// The reviewed refs with the local branches, from one `for-each-ref`
pub fn read_reviewed_refs(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<ReviewedRefs> {
  let lines = git_executor.execute_command_lines(&["for-each-ref", "--format=%(refname)%00%(objectname)", REVIEWED_REFS, BRANCHES], repo_path)?;
  let mut refs = ReviewedRefs::default();
  for (reference, id) in lines.iter().filter_map(|line| line.split_once('\0')) {
    if let Some(branch) = reference.strip_prefix(BRANCHES) {
      refs.branches.insert(branch.to_string(), id.to_string());
    } else if let Some((key, part)) = reference.strip_prefix(REVIEWED_REFS).and_then(|rest| rest.split_once('/')) {
      let pair = refs.pairs.entry(key.to_string()).or_default();
      match part {
        BASE => pair.base = Some(id.to_string()),
        TIP => pair.tip = Some(id.to_string()),
        _ => {}
      }
    }
  }
  Ok(refs)
}

/// The reviewed pair of the branch `full_branch_name` alone, `None` when it was never marked (or a ref of its pair is
/// gone): one `for-each-ref` of its own pair, without the branches
pub fn read_reviewed_pair(git_executor: &GitCommandExecutor, repo_path: &str, full_branch_name: &str) -> Result<Option<ReviewedPair>> {
  let key = reviewed_key(full_branch_name);
  let namespace = format!("{REVIEWED_REFS}{key}/");
  let lines = git_executor.execute_command_lines(&["for-each-ref", "--format=%(refname)%00%(objectname)", &namespace], repo_path)?;
  let mut pair = ListedPair::default();
  for (reference, id) in lines.iter().filter_map(|line| line.split_once('\0')) {
    match reference.strip_prefix(namespace.as_str()) {
      Some(BASE) => pair.base = Some(id.to_string()),
      Some(TIP) => pair.tip = Some(id.to_string()),
      _ => {}
    }
  }
  Ok(complete(&pair))
}

fn complete(pair: &ListedPair) -> Option<ReviewedPair> {
  Some(ReviewedPair {
    base: pair.base.clone()?,
    tip: pair.tip.clone()?,
  })
}

/// What a mark writes
#[derive(Debug, Default)]
pub struct ReviewedMark {
  /// Never empty: a ref the mark leaves as it is gets a `verify`, so a mark over a listing another process outdated
  /// fails also when it would move nothing
  pub transaction: RefTransaction,
  /// Whether the transaction moves or deletes a ref
  pub changed: bool,
  /// The pair the branch had before, `None` when it was never marked
  pub previous: Option<ReviewedPair>,
  /// The pairs deleted because their branch is gone
  pub pruned: usize,
}

impl ReviewedRefs {
  /// The reviewed pair of the branch `full_branch_name`, `None` when it was never marked
  pub fn reviewed(&self, full_branch_name: &str) -> Option<ReviewedPair> {
    self.pairs.get(&reviewed_key(full_branch_name)).and_then(complete)
  }

  /// The ref transaction that records `mark` as the reviewed pair of `full_branch_name` and deletes the pairs of the
  /// branches that are gone, each move with compare-and-swap on the value this listing read: a mark or a prune of
  /// another process meanwhile fails the whole transaction. Commit it with the operation `MARK_REVIEWED`
  pub fn mark(&self, full_branch_name: &str, mark: &ReviewedPair) -> ReviewedMark {
    let key = reviewed_key(full_branch_name);
    let listed = self.pairs.get(&key).cloned().unwrap_or_default();
    let mut transaction = RefTransaction::new();
    let mut changed = false;
    for (part, old, new) in [(BASE, &listed.base, &mark.base), (TIP, &listed.tip, &mark.tip)] {
      let reference = pair_ref(&key, part);
      match old {
        Some(old) if old == new => {
          transaction.verify(&reference, old);
        }
        Some(old) => {
          transaction.update(&reference, new, Some(old));
          changed = true;
        }
        None => {
          transaction.create(&reference, new);
          changed = true;
        }
      }
    }
    let live: HashSet<String> = self.branches.keys().map(|branch| reviewed_key(branch)).collect();
    let mut pruned = 0;
    for (gone, pair) in self.pairs.iter().filter(|(other, _)| **other != key && !live.contains(*other)) {
      for (part, old) in [(BASE, &pair.base), (TIP, &pair.tip)] {
        if let Some(old) = old {
          transaction.delete(&pair_ref(gone, part), Some(old));
        }
      }
      pruned += 1;
    }
    ReviewedMark {
      transaction,
      changed: changed || pruned > 0,
      previous: complete(&listed),
      pruned,
    }
  }
}
