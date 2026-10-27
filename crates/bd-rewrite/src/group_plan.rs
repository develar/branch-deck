//! The plan of `bd group` and `bd check`: a `RegroupPlan` plus the `descriptions` of the branches, which bd writes
//! itself after the rewrite, under the same repository lock

use crate::describe::{description_branch, write_description};
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use domain_types::error::ErrorCode;
use domain_types::rewrite::RegroupPlan;
use git_ops::branch_description::Description;
use history_rewrite::regroup::{RegroupError, RegroupedCommit};
use std::collections::{BTreeMap, BTreeSet};

const DESCRIPTIONS: &str = "descriptions";

#[derive(Debug, Default)]
pub struct GroupPlan {
  pub regroup: RegroupPlan,
  /// Branch -> its pull request text, trimmed
  pub descriptions: BTreeMap<String, Description>,
}

/// Parse a plan: the fields of `RegroupPlan`, which refuses unknown ones, and `descriptions`
pub fn parse_group_plan(text: &str) -> Result<GroupPlan, CliError> {
  let invalid = |e: serde_json::Error| CliError::new(ErrorCode::InvalidPlan, format!("The plan is not valid JSON of the expected shape: {e}"));
  let mut value: serde_json::Value = serde_json::from_str(text).map_err(invalid)?;
  let descriptions = match value.as_object_mut().and_then(|plan| plan.remove(DESCRIPTIONS)) {
    Some(descriptions) => serde_json::from_value::<BTreeMap<String, Description>>(descriptions).map_err(invalid)?,
    None => BTreeMap::new(),
  };
  // `RegroupPlan` lists the fields it knows; `descriptions` is removed before it parses
  let regroup = serde_json::from_value(value).map_err(|e| {
    let mut error = invalid(e);
    if error.message.contains("unknown field") {
      error.message = format!("{}, or `{DESCRIPTIONS}`", error.message);
    }
    error
  })?;
  Ok(GroupPlan {
    regroup,
    descriptions: descriptions.into_iter().map(|(branch, description)| (branch, description.trimmed())).collect(),
  })
}

/// Refuse a description without a title of one line, or of a branch that no commit is in after the plan
pub fn check_descriptions(descriptions: &BTreeMap<String, Description>, commits: &[RegroupedCommit]) -> Result<(), RegroupError> {
  let branches: BTreeSet<&str> = commits.iter().filter_map(|commit| commit.group.as_deref()).collect();
  let without_title: Vec<&str> = descriptions
    .iter()
    .filter(|(_, description)| !description.has_valid_title())
    .map(|(branch, _)| branch.as_str())
    .collect();
  if !without_title.is_empty() {
    return Err(RegroupError::InvalidPlan {
      message: format!("A description needs a title of one line: {}", without_title.join(", ")),
      commits: Vec::new(),
    });
  }
  let unknown: Vec<&str> = descriptions.keys().map(String::as_str).filter(|branch| !branches.contains(branch)).collect();
  if !unknown.is_empty() {
    // a stacked branch is described by its leaf, as `bd describe` names it
    let leaves: Vec<String> = unknown
      .iter()
      .filter_map(|branch| branch.rsplit_once('/').map(|(_, leaf)| leaf))
      .filter(|leaf| branches.contains(leaf))
      .map(|leaf| format!("`{leaf}`"))
      .collect();
    let hint = if leaves.is_empty() {
      String::new()
    } else {
      format!("; a stacked branch is named by its leaf: {}", leaves.join(", "))
    };
    return Err(RegroupError::InvalidPlan {
      message: format!("Descriptions of branches that have no commits after the plan: {}{hint}", unknown.join(", ")),
      commits: Vec::new(),
    });
  }
  Ok(())
}

/// A description that the plan replaces or adds: its branch, the full name of its virtual branch, the new text and the
/// current one
pub struct DescriptionChange<'a> {
  pub branch: &'a str,
  pub name: String,
  pub description: &'a Description,
  pub previous: Option<Description>,
}

/// The descriptions that differ from `descriptions`, with the current texts of the command's config
/// (`RepoContext::config`, read again after a wait for the repository lock)
pub fn changed_descriptions<'a>(context: &RepoContext, descriptions: &'a BTreeMap<String, Description>) -> Result<Vec<DescriptionChange<'a>>, CliError> {
  if descriptions.is_empty() {
    return Ok(Vec::new());
  }
  let mut current = context.config()?.descriptions().clone();
  let mut changed = Vec::new();
  for (branch, description) in descriptions {
    let (reference, name) = description_branch(context, branch)?;
    let previous = current.remove(&reference);
    if previous.as_ref() != Some(description) {
      changed.push(DescriptionChange {
        branch,
        name,
        description,
        previous,
      });
    }
  }
  Ok(changed)
}

/// Write the changed descriptions in order: one `git config` process per branch, git has no batch writer. Returns how
/// many were written, and the error that stopped the rest
pub fn write_descriptions(context: &RepoContext, changed: &[DescriptionChange]) -> (usize, Option<CliError>) {
  for (written, change) in changed.iter().enumerate() {
    if let Err(error) = write_description(context, &change.name, change.description) {
      return (written, Some(error));
    }
  }
  (changed.len(), None)
}
