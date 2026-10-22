//! `bd describe`: the pull request text of a virtual branch, kept in git's own `branch.<name>.description` as
//! `git branch --edit-description` writes it (the title, a blank line, the body)

use bd_core::branch_view::read_grouping;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use bd_core::text_input::read_text;
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ErrorCode;
use git_ops::branch_description::{Description, clear_description};
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

/// The virtual branch of `branch`: its ref and its full name, whose `branch.<name>.description` holds the text
pub fn description_branch(context: &RepoContext, branch: &str) -> Result<(String, String), CliError> {
  let name = to_final_branch_name(&context.prefix, branch).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  Ok((format!("refs/heads/{name}"), name))
}

/// Set the description of the virtual branch `name` (its full name): one `git config` process, git has no batch writer.
/// The command's config is read again after it (`RepoContext::forget_config`)
pub fn write_description(context: &RepoContext, name: &str, description: &Description) -> Result<(), CliError> {
  let written = git_ops::branch_description::write_description(&context.git, &context.repository, name, description);
  context.forget_config();
  Ok(written?)
}

pub fn describe(context: &RepoContext, args: &DescribeArgs) -> Result<DescribeOutput, CliError> {
  // only a current branch can be described
  read_grouping(context, Some(&args.branch))?;
  let (reference, name) = description_branch(context, &args.branch)?;
  let current = context.config()?.descriptions().get(&reference).cloned();

  let body = match &args.body_file {
    Some(path) => Some(read_text(path, "the body")?),
    None => args.body.clone(),
  };

  let (description, changed) = if args.clear {
    if current.is_some() {
      let cleared = clear_description(&context.git, &context.repository, &name);
      context.forget_config();
      cleared?;
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
      write_description(context, &name, &description)?;
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
