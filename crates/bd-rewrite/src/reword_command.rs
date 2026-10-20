//! `bd reword`: new messages of commits ahead of the baseline through `history_rewrite::reword`, with the contract of
//! `bd group` (`rewrite.rs`): the repository lock over the rewrite and, with --sync, the sync, `--expect-head`, the
//! undo token, `--dry-run`

use crate::rewrite::{RewriteArgs, RewriteOutput, rewrite};
use bd_core::context::RepoContext;
use bd_core::error::{CliError, ErrorCode};
use bd_core::text_input::read_text;
use history_rewrite::reword::{CommitReword, RewordText, reword_core};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
#[command(group(clap::ArgGroup::new("text").args(["message", "message_file", "subject"]).multiple(false)))]
pub struct RewordArgs {
  /// The commit ahead of the baseline, by the id `bd status` printed (abbreviated or full)
  #[arg(required_unless_present = "plan", requires = "text", conflicts_with = "plan")]
  pub commit: Option<String>,
  /// The whole new message, the `(prefix)` of its subject included
  #[arg(long)]
  pub message: Option<String>,
  /// The whole new message from a file, `-` for stdin
  #[arg(long, value_name = "FILE")]
  pub message_file: Option<PathBuf>,
  /// The new subject line, the `(prefix)` included (without it the commit leaves its branch); the body stays
  #[arg(long)]
  pub subject: Option<String>,
  /// Several commits in one rewrite: `{"rewrites": [{"commit": "<id>", "message": "…"} | {"commit": "<id>", "subject":
  /// "…"}]}`, `-` for stdin
  #[arg(long, value_name = "FILE", conflicts_with = "text")]
  pub plan: Option<PathBuf>,
  #[command(flatten)]
  pub rewrite: RewriteArgs,
}

/// One entry of the plan of `bd reword`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlannedReword {
  commit: String,
  #[serde(default)]
  message: Option<String>,
  #[serde(default)]
  subject: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RewordPlan {
  rewrites: Vec<PlannedReword>,
}

/// The rewords of a plan: each entry a commit id and exactly one of `message` and `subject`
pub fn parse_reword_plan(text: &str) -> Result<Vec<CommitReword>, CliError> {
  let plan: RewordPlan = serde_json::from_str(text).map_err(|e| CliError::new(ErrorCode::InvalidPlan, format!("The plan is not valid JSON of the expected shape: {e}")))?;
  if plan.rewrites.is_empty() {
    return Err(CliError::new(ErrorCode::InvalidPlan, "The plan rewords no commit: `rewrites` is empty"));
  }
  plan
    .rewrites
    .into_iter()
    .map(|entry| {
      let text = match (entry.message, entry.subject) {
        (Some(message), None) => RewordText::Message(message),
        (None, Some(subject)) => RewordText::Subject(subject),
        _ => {
          return Err(CliError {
            commits: vec![entry.commit.clone()],
            ..CliError::new(
              ErrorCode::InvalidPlan,
              format!("The rewrite of {} needs exactly one of `message` and `subject`", entry.commit),
            )
          });
        }
      };
      Ok(CommitReword { commit: entry.commit, text })
    })
    .collect()
}

pub fn reword(context: &RepoContext, args: &RewordArgs) -> Result<RewriteOutput, CliError> {
  let rewords = match (&args.plan, &args.commit) {
    (Some(plan), _) => parse_reword_plan(&read_text(plan, "the plan")?)?,
    (None, Some(commit)) => {
      let text = match (&args.message, &args.message_file, &args.subject) {
        (Some(message), _, _) => RewordText::Message(message.clone()),
        (_, Some(path), _) => RewordText::Message(read_text(path, "the message")?),
        (_, _, Some(subject)) => RewordText::Subject(subject.clone()),
        // clap requires one of them with a commit
        (None, None, None) => return Err(CliError::new(ErrorCode::InvalidArguments, "Give --message, --message-file or --subject")),
      };
      vec![CommitReword { commit: commit.clone(), text }]
    }
    // clap requires a commit without a plan
    (None, None) => return Err(CliError::new(ErrorCode::InvalidArguments, "Give a commit or --plan")),
  };
  let not_ids: Vec<String> = rewords
    .iter()
    .filter(|reword| reword.commit.is_empty() || !reword.commit.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .map(|reword| reword.commit.clone())
    .collect();
  if !not_ids.is_empty() {
    return Err(CliError {
      commits: not_ids.clone(),
      ..CliError::new(
        ErrorCode::InvalidArguments,
        format!("Not commit ids: {} (give the ids `bd status` printed, not a revision such as HEAD~1)", not_ids.join(", ")),
      )
    });
  }
  rewrite(context, "reword", &args.rewrite, &BTreeMap::new(), |options| {
    reword_core(&context.git, &context.repository, &rewords, options)
  })
}
