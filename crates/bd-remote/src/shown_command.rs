//! `brd close-review` and the shared part of the shown reviews of `brd request-review` (its open, the extend by the
//! fixes that landed; openspec/specs/branch/shown-reviews/spec.md): the reads of the shown reviews and of the robots
//! over the one-shot session of the command (`operations::needs` of the intent, as the app's session reads them; the
//! state of a review as the robots of its branch leave it), the plans of
//! `operations::shown`, their run (`operations::shown::run_shown`) recorded in the operations registry of the
//! repository as the app's run holds it (`shown_request`). A close of a review not every reviewer approved is refused
//! (`review_not_approved`) unless `--allow-unapproved`, given only once the user agreed; a close of a review closed on
//! GitHub deletes its branches where the remote still has them at the tips of its pull request
//! (`operations::shown::delete_closed_plan`).

use crate::request_review_command::require_listing;
use crate::safe_push_command::{UNKNOWN_GITHUB_REPOSITORY, compared_table};
use crate::session_facts::SessionFacts;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::operation_record::admit;
use bd_core::output::SchemaVersion;
use bd_core::services::{BdServices, github_service, patronus_service};
use bd_core::sync_command::{SyncOutput, sync};
use domain_policy::shown::{closed_remains, state_now};
use domain_types::branch_names::{baseline_on_remote, review_branch_names};
use domain_types::error::ErrorCode;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, Intent, OperationResult, OperationStats, Outcome, Plan};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::{ClosedShownReview, ShownReview, ShownState};
use domain_types::session::FactState;
use github::service::GitHubService;
use operations::needs::needs;
use operations::shown::{ShownRun, ShownServices, close_plan, delete_closed_plan, no_shown_review, run_shown, shown_refusal, shown_request};
use schemars::JsonSchema;
use serde::Serialize;
use std::sync::Arc;
use sync_core::github_repository::{GitHubRepository, resolve_github_repository_with};

/// What a command of a shown review read: the sync, the shown review of the branch (as the robots of its branch leave
/// it, `domain_policy::shown::state_now`), the robots, the services of its run
pub(crate) struct ShownRead {
  pub synced: SyncOutput,
  pub review: Option<ShownReview>,
  /// The shown reviews of the user closed in the last days, of a read that asked them (a close)
  pub closed: Vec<ClosedShownReview>,
  pub robots: Vec<PatronusRobot>,
  /// Patronus answered the robots: a draft of a branch the baseline landed whole by content opens on its head when
  /// none of them is of the branch (`domain_policy::shown::opens_by_content`)
  pub robots_known: bool,
  pub services: ShownServices,
  /// The branch of origin the robots push into ("master")
  pub target: String,
}

/// The reads of a command of the shown review of `branch` (`intent`): the sync, then what the intent needs of the
/// remote (`operations::needs`: the pull requests with the shown reviews and the robots, of the landing and the fixes)
pub(crate) fn read_shown(context: &RepoContext, environment: &Environment, branch: &str, intent: &Intent) -> Result<ShownRead, CliError> {
  review_branch_names(&context.prefix, branch).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
  let synced = sync(context)?;
  read_shown_of(context, environment, branch, intent, synced)
}

/// `read_shown` after the sync `synced` of its caller
pub(crate) fn read_shown_of(context: &RepoContext, environment: &Environment, branch: &str, intent: &Intent, synced: SyncOutput) -> Result<ShownRead, CliError> {
  review_branch_names(&context.prefix, branch).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
  let configured = context.config()?.github_repository();
  let repository = resolve_github_repository_with(&context.git, &context.repository, configured, context.origin_url()?.as_deref())?
    .ok_or_else(|| CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY))?;
  let space = context.space_repository()?;
  // an open and an extend go by the robots; a close reads them when it can (a fix that lands holds it back)
  let patronus = match (&space, intent) {
    (None, _) => None,
    (Some(_), Intent::CloseShownReview { .. }) => patronus_service(environment).ok().map(Arc::new),
    (Some(_), _) => Some(Arc::new(patronus_service(environment)?)),
  };
  let github = Arc::new(github_service(context, environment));
  let target = synced.report.target_branch.clone().unwrap_or_else(|| baseline_on_remote(&synced.baseline).to_owned());
  let table = compared_table(context, &synced);
  let services = BdServices {
    patronus,
    space,
    github_repository: Some(repository.clone()),
    target: Some(target.clone()),
    ..BdServices::new(context, Arc::clone(&github))
  };
  let mut facts = SessionFacts::new(context, synced.table.clone(), table, services.sources());
  facts.read(context, &needs(intent))?;
  let listed = facts.session.facts().pull_requests.view();
  require_listing(listed)?;
  let robots = facts.session.robots().to_vec();
  let robots_known = matches!(facts.session.facts().robots.view().head.state, FactState::Fresh | FactState::Stale { .. });
  let review = listed.shown_reviews.get(branch).cloned().map(|review| ShownReview {
    state: state_now(&review, branch, &robots),
    ..review
  });
  let closed = facts.session.facts().pull_requests.closed_shown().to_vec();
  let services = shown_services(context, &synced, github, repository);
  Ok(ShownRead {
    synced,
    review,
    closed,
    robots,
    robots_known,
    services,
    target,
  })
}

/// The services of the run of a shown review of the repository of `context`
pub(crate) fn shown_services(context: &RepoContext, synced: &SyncOutput, github: Arc<GitHubService>, repository: GitHubRepository) -> ShownServices {
  ShownServices {
    git: context.git.clone(),
    repository_path: context.repository.clone(),
    prefix: context.prefix.clone(),
    remote: "origin".to_owned(),
    baseline: synced.baseline.clone(),
    github,
    repository,
    text: None,
  }
}

/// Runs `plan` (of `intent`) recorded in the operations registry of the repository, as the app's run holds it: busy
/// while another process holds what it needs; its effects in order, the first that fails stops the rest and fails the
/// command
pub(crate) fn run_recorded(context: &RepoContext, services: &ShownServices, plan: &Plan) -> Result<ShownRun, CliError> {
  let entry = admit(context, &shown_request(&context.prefix, &plan.intent, Some(plan)), None).map_err(CliError::from)?;
  let runtime = context.runtime()?;
  let run = runtime.block_on(entry.tally().counting(run_shown(services, plan)));
  let stats: OperationStats = entry.stats();
  let outcome = Outcome {
    op: entry.id().clone(),
    effects: run.records.clone(),
    news_key: None,
    version_after: None,
    result: run
      .requested
      .clone()
      .map_or(OperationResult::None, |requested| OperationResult::Reviewers(Box::new(requested))),
    undo: None,
    stats: stats.clone(),
  };
  entry.finish(Ok(&outcome), stats);
  let failed = run.records.iter().find_map(|record| match &record.status {
    EffectStatus::Failed { error } => Some(error.clone()),
    _ => None,
  });
  match failed {
    Some(error) => Err(CliError::from(error)),
    None => Ok(run),
  }
}

#[derive(Debug, Default, clap::Args)]
pub struct CloseReviewArgs {
  /// The branch whose shown review closes
  pub branch: String,
  /// Close it although a reviewer it asked has not approved it (`review_not_approved`): only when the user says so
  #[arg(long)]
  pub allow_unapproved: bool,
  /// Print what it would close and delete, and close nothing
  #[arg(long)]
  pub dry_run: bool,
}

/// `brd close-review`
#[derive(Debug, Serialize, JsonSchema)]
pub struct CloseReviewOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  pub number: u32,
  pub url: String,
  /// `approved` (every reviewer it asked approved), `in_review`, `changes_requested`, `extending` (a fix of the branch
  /// lands), `draft`, or `closed` (closed on GitHub: only its branches are deleted)
  pub state: String,
  /// Who approved it, of those it asked
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub approved_by: Vec<String>,
  /// The comment of the close
  pub comment: String,
  /// It was closed and its branches deleted; false with `--dry-run`
  pub closed: bool,
  /// Of a review closed on GitHub: its branches the remote still has at the tips of its pull request, which it deleted
  /// (with `--dry-run`: would delete)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub deleted: Vec<String>,
}

/// The name of `state` in the output
pub(crate) const fn state_name(state: &ShownState) -> &'static str {
  match state {
    ShownState::Draft => "draft",
    ShownState::InReview { .. } => "in_review",
    ShownState::ChangesRequested { .. } => "changes_requested",
    ShownState::Approved { .. } => "approved",
    ShownState::Extending { .. } => "extending",
  }
}

/// `brd close-review <branch>`; of a review closed on GitHub, the delete of its branches (`close_closed`)
///
/// # Errors
/// No open shown review of the branch, nor a closed one whose branches the remote still has (`no_shown_review`), one not
/// every reviewer approved (`review_not_approved`), a failure of GitHub or of the push of the delete
pub fn close_review(context: &RepoContext, args: &CloseReviewArgs, environment: &Environment) -> Result<CloseReviewOutput, CliError> {
  let intent = Intent::CloseShownReview {
    branch: args.branch.clone(),
    allow_unapproved: args.allow_unapproved,
  };
  let read = read_shown(context, environment, &args.branch, &intent)?;
  let Some(review) = read.review.as_ref() else {
    return close_closed(context, args, &read);
  };
  let plan = close_plan(review, &args.branch, args.allow_unapproved, &read.target);
  let comment = plan
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::ClosePullRequest { comment, .. } => Some(comment.clone()),
      _ => None,
    })
    .unwrap_or_default();
  let mut output = CloseReviewOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    number: review.pull_request.number,
    url: review.pull_request.url.clone(),
    state: state_name(&review.state).to_owned(),
    approved_by: match &review.state {
      ShownState::Approved { by } => by.clone(),
      _ => Vec::new(),
    },
    comment,
    closed: false,
    deleted: Vec::new(),
  };
  // --allow-unapproved is the user's word, never a next step: its message names it
  if let Some(refusal) = shown_refusal(&plan, &args.branch) {
    return Err(CliError::from(refusal));
  }
  if args.dry_run {
    return Ok(output);
  }
  run_recorded(context, &read.services, &plan)?;
  output.closed = true;
  Ok(output)
}

/// `brd close-review` of a branch without an open shown review: the newest of its reviews closed on GitHub whose branches
/// the remote still has at the tips of its pull request loses them (`operations::shown::delete_closed_plan`)
fn close_closed(context: &RepoContext, args: &CloseReviewArgs, read: &ShownRead) -> Result<CloseReviewOutput, CliError> {
  let closed = read
    .closed
    .iter()
    .filter(|closed| closed.branch == args.branch && closed_remains(closed).is_some())
    .max_by_key(|closed| closed.closed_at)
    .ok_or_else(|| CliError::from(no_shown_review(&args.branch)))?;
  let plan = delete_closed_plan(closed, None).map_err(CliError::from)?;
  let (head, base) = review_branch_names(&context.prefix, &args.branch).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
  let deleted = plan
    .effects
    .iter()
    .flat_map(|effect| match effect {
      PlannedEffect::DeleteReviewAt { head: at_head, base: at_base, .. } => [at_head.as_ref().map(|_| head.clone()), at_base.as_ref().map(|_| base.clone())],
      _ => [None, None],
    })
    .flatten()
    .collect();
  let mut output = CloseReviewOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    number: closed.number,
    url: closed.url.clone(),
    state: "closed".to_owned(),
    approved_by: Vec::new(),
    comment: String::new(),
    closed: false,
    deleted,
  };
  if args.dry_run {
    return Ok(output);
  }
  run_recorded(context, &read.services, &plan)?;
  output.closed = true;
  Ok(output)
}
