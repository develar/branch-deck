//! The names a built branch uses that a commit outside its tree adds, by the edges by symbols (`symbol_dependencies`,
//! `branchdeck.symbols on`): git merges the branch, its build would fail. `bd check` reports them; names, not a
//! compiler, so they don't fail the check.

use crate::check::{CheckOutcome, CheckReport, CheckSession};
use crate::symbol_dependencies::Evidence;
use history_rewrite::regroup::RegroupError;
use std::collections::{HashMap, HashSet};

/// A name a commit of `branch` uses that a commit outside the branch and the branches it is built on adds
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDeclaration {
  pub branch: String,
  pub identifier: String,
  /// Where the branch uses it
  pub file: String,
  pub line: u32,
  /// The commit of the branch that uses it
  pub used_by: String,
  /// The commit outside that adds it
  pub declared_by: String,
  /// The branch of `declared_by`; `None` for an unassigned commit
  pub declared_in: Option<String>,
  pub evidence: Evidence,
}

/// The names the built branches of `report`, a check in `session`, use and a commit outside their trees adds, the first
/// link of each edge; empty unless `branchdeck.symbols` is `on`
pub fn missing_declarations(session: &CheckSession, report: &CheckReport) -> Result<Vec<MissingDeclaration>, RegroupError> {
  let Some(graph) = session.symbol_graph()? else { return Ok(Vec::new()) };
  let ids: Vec<&str> = session.snapshot().commits.iter().map(|commit| commit.id.as_str()).collect();
  let group_of: HashMap<&str, Option<&str>> = report.regroup.commits.iter().map(|commit| (commit.id.as_str(), commit.group.as_deref())).collect();
  let mut found = Vec::new();
  for branch in report.branches.iter().filter(|branch| matches!(branch.outcome, CheckOutcome::Ok { .. })) {
    let own: HashSet<&str> = branch.commits.iter().map(String::as_str).collect();
    let mut tree: HashSet<&str> = own.clone();
    let mut parent = branch.built_on.as_deref();
    for _ in 0..report.branches.len() {
      let Some(check) = parent.and_then(|name| report.branches.iter().find(|check| check.name == name)) else {
        break;
      };
      tree.extend(check.commits.iter().map(String::as_str));
      parent = check.built_on.as_deref();
    }
    for edge in &graph.edges {
      let (Some(from), Some(to)) = (ids.get(edge.from), ids.get(edge.to)) else { continue };
      if !own.contains(from) || tree.contains(to) {
        continue;
      }
      let Some(link) = edge.links.first() else { continue };
      found.push(MissingDeclaration {
        branch: branch.name.clone(),
        identifier: link.name.clone(),
        file: link.used_in.clone(),
        line: link.line,
        used_by: (*from).to_string(),
        declared_by: (*to).to_string(),
        declared_in: group_of.get(to).copied().flatten().map(str::to_string),
        evidence: link.evidence,
      });
    }
  }
  Ok(found)
}
