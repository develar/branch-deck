//! `bd describe`: the pull request text of a virtual branch, kept in git's own `branch.<name>.description` as
//! `git branch --edit-description` writes it (the title, a blank line, the body)

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use crate::status::read_grouping;
use crate::text_input::read_text;
use git_ops::branch_description::{Description, read_descriptions};
use git_ops::model::to_final_branch_name;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct DescribeArgs {
  /// The branch (group name)
  pub branch: String,
  /// Set the title
  #[arg(long, conflicts_with = "clear")]
  pub title: Option<String>,
  /// Set the body
  #[arg(long, conflicts_with_all = ["body_file", "clear"])]
  pub body: Option<String>,
  /// Set the body from a file, `-` for stdin
  #[arg(long, value_name = "FILE", conflicts_with = "clear")]
  pub body_file: Option<PathBuf>,
  /// Remove the description
  #[arg(long)]
  pub clear: bool,
}

/// `bd describe`
#[derive(Debug, Serialize, JsonSchema)]
pub struct DescribeOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  /// The virtual branch, `refs/heads/{prefix}/virtual/{name}`
  #[serde(rename = "ref")]
  pub reference: String,
  /// `null` when the branch has none
  pub description: Option<Description>,
  /// Whether this call wrote or removed it
  pub changed: bool,
}

/// The virtual branch ref of `branch` and its `branch.<name>.description` key
pub fn description_key(context: &RepoContext, branch: &str) -> Result<(String, String), CliError> {
  let name = to_final_branch_name(&context.prefix, branch).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  Ok((format!("refs/heads/{name}"), format!("branch.{name}.description")))
}

/// Set the description under `key`: one `git config` process, git has no batch writer
pub fn write_description(context: &RepoContext, key: &str, description: &Description) -> Result<(), CliError> {
  context.git.execute_command(&["config", key, &description.to_text()], &context.repository)?;
  Ok(())
}

pub fn describe(context: &RepoContext, args: &DescribeArgs) -> Result<DescribeOutput, CliError> {
  // only a current branch can be described
  read_grouping(context, Some(&args.branch))?;
  let (reference, key) = description_key(context, &args.branch)?;
  let current = read_descriptions(&context.git, &context.repository)?.remove(&reference);

  let body = match &args.body_file {
    Some(path) => Some(read_text(path, "the body")?),
    None => args.body.clone(),
  };

  let (description, changed) = if args.clear {
    if current.is_some() {
      let (output, exit_code) = context.git.execute_command_with_status(&["config", "--unset", &key], &context.repository)?;
      // 5: the key was not set
      if exit_code != 0 && exit_code != 5 {
        return Err(CliError::new(ErrorCode::GitError, format!("Cannot remove the description: {output}")));
      }
    }
    (None, current.is_some())
  } else if args.title.is_some() || body.is_some() {
    // the parts not given are kept
    let mut description = current.clone().unwrap_or_default();
    if let Some(title) = &args.title {
      title.trim().clone_into(&mut description.title);
    }
    if let Some(body) = &body {
      body.trim().clone_into(&mut description.body);
    }
    if !description.has_valid_title() {
      return Err(CliError::new(ErrorCode::InvalidArguments, "The description needs a title of one line: pass --title"));
    }
    let changed = current.as_ref() != Some(&description);
    if changed {
      write_description(context, &key, &description)?;
    }
    (Some(description), changed)
  } else {
    (current, false)
  };

  Ok(DescribeOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    reference,
    description,
    changed,
  })
}
