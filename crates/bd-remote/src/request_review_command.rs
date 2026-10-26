//! `bd request-review`: ask GitHub users to review the open pull request of a virtual branch, as the app's Request
//! Review… does (`spec/branch/pull-requests.spec.md#request-review`). Without `--reviewer` it asks the code owners
//! Patronus suggests for the groups whose approval is missing, one per group nobody is asked for yet, from the review
//! readiness `bd safe-push` checks (the pre-check with the pull request, asked again, through the reads of the app's
//! session, `ServiceFactSources`): the ones the app's popover
//! preselects, by the one rule of both (`domain_policy::review::candidates`). Users only: a team is not asked.
//! No repository lock and no sync: the readiness is that of the virtual branch as the last sync built it; the only
//! write is the request on GitHub (`POST pulls/<n>/requested_reviewers`).

use crate::safe_push_command::{Review, UNKNOWN_GITHUB_REPOSITORY, github_error, readiness_names};
use bd_core::branch_view::read_grouping;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use bd_core::review_summary::PersonSummary;
use bd_core::services::{BdServices, github_service, patronus_service, session_key};
use domain_policy::pull_request::base_of;
use domain_policy::review::{Candidates, candidates};
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ErrorCode;
use domain_types::patronus::{Person, ReviewReadiness, ReviewReadinessBranch, UnavailableReadinessReason};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, PullRequestState};
use git_ops::revs::resolve_commit;
use github::service::GitHubService;
use patronus::service::PatronusService;
use remote_facts::pull_request_writes::request_pull_request_reviewers;
use remote_facts::pull_requests::{GitHubWriteError, SIGNED_OUT_MESSAGE};
use schemars::JsonSchema;
use serde::Serialize;
use std::sync::Arc;
use sync_core::github_repository::{GitHubRepository, resolve_github_repository_with};
use sync_core::patronus::{SpaceRepository, target_branch_of};

#[derive(Debug, Default, clap::Args)]
pub struct RequestReviewArgs {
  /// The branch whose pull request is reviewed
  pub branch: String,
  /// Ask this GitHub login (repeatable) instead of the suggested code owners; Patronus is not asked then
  #[arg(long = "reviewer", value_name = "LOGIN")]
  pub reviewers: Vec<String>,
  /// Print whom it would ask and ask nobody
  #[arg(long)]
  pub check: bool,
}

/// `bd request-review`
#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestReviewOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  pub pull_request: ReviewedPullRequest,
  /// The code-owner review before the request, as `bd safe-push` names it (`missing`, `approved`, `not_required`,
  /// `unknown`); `null` with `--reviewer`, which doesn't ask Patronus
  pub readiness: Option<String>,
  /// Whom it asks: the `--reviewer` logins, else one suggested code owner of each missing group nobody is asked for
  /// yet; with `--check`, whom it would ask
  pub reviewers: Vec<PersonSummary>,
  /// Whom GitHub asked to review; empty with `--check`
  pub requested: Vec<PersonSummary>,
  /// Whom GitHub refused (the author, a user without access to the repository), with its reason
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub refused: Vec<RefusedSummary>,
  /// The reviewers the pull request asks after the request: those asked before and those asked now
  pub waiting_for: Vec<PersonSummary>,
  /// The missing groups nobody is asked for whose suggested code owners Patronus names no GitHub login of: pass
  /// `--reviewer` for them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub groups_without_reviewer: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewedPullRequest {
  pub number: u32,
  pub url: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RefusedSummary {
  pub login: String,
  pub full_name: Option<String>,
  /// What GitHub said
  pub reason: String,
}

pub fn request_review(context: &RepoContext, args: &RequestReviewArgs, environment: &Environment) -> Result<RequestReviewOutput, CliError> {
  let name = args.branch.as_str();
  // the name must be one a virtual branch can have
  to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  // the push URL of origin is read once, for this and the Space repository of the review
  let configured = context.config()?.github_repository();
  let repository = resolve_github_repository_with(&context.git, &context.repository, configured, context.origin_url()?)?
    .ok_or_else(|| CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY))?;
  let github = Arc::new(github_service(context, environment));
  let runtime = context.runtime()?;

  // the pull requests and the review as the app's session reads them (`ServiceFactSources`)
  let reads = Reads {
    context,
    github: &github,
    repository: &repository,
  };
  let listed = runtime.block_on(reads.services(None, None, None).sources().pull_requests(&session_key(context)));
  if !matches!(listed.availability, PullRequestAvailability::Available { .. }) {
    return Err(availability_error(&listed.availability));
  }
  let pull_request = listed
    .pull_requests
    .get(name)
    .filter(|pull_request| pull_request.state == PullRequestState::Open)
    .cloned()
    .ok_or_else(|| {
      CliError::new(
        ErrorCode::NoPullRequest,
        format!("{name} has no open pull request: `bd safe-push {name} --create-pull-request` pushes the branch and opens it, with `--reviewer`"),
      )
    })?;
  if pull_request.is_draft {
    return Err(CliError::new(
      ErrorCode::NoPullRequest,
      format!(
        "Pull request #{} is a draft: mark it ready for review on GitHub first ({})",
        pull_request.number, pull_request.url
      ),
    ));
  }

  let (readiness, review, reviewers, groups_without_reviewer) = if args.reviewers.is_empty() {
    let review = code_owner_review(&reads, environment, name, &pull_request)?;
    // the candidates of the app's Request Review…: it asks the ones the popover preselects
    let Candidates { candidates, without_reviewer } = candidates(Some(&review.readiness), &pull_request);
    let reviewers: Vec<Person> = candidates
      .into_iter()
      .filter(|candidate| candidate.preselected)
      .map(|candidate| Person {
        login: Some(candidate.login),
        full_name: candidate.full_name,
      })
      .collect();
    let groups_without_reviewer = without_reviewer;
    if reviewers.is_empty() && !groups_without_reviewer.is_empty() {
      return Err(CliError::new(
        ErrorCode::NoReviewer,
        format!(
          "Patronus names no code owner with a GitHub login to ask for {}: pass --reviewer <login> (`bd safe-push {name} --check` lists the groups and their reviewers)",
          groups_without_reviewer.join(", ")
        ),
      ));
    }
    (Some(readiness_names(&review.readiness).0.to_string()), Some(review), reviewers, groups_without_reviewer)
  } else {
    let mut logins: Vec<&str> = Vec::new();
    for login in args.reviewers.iter().map(|login| login.trim()).filter(|login| !login.is_empty()) {
      if !logins.iter().any(|it| it.eq_ignore_ascii_case(login)) {
        logins.push(login);
      }
    }
    if logins.is_empty() {
      return Err(CliError::new(ErrorCode::InvalidArguments, "--reviewer needs a GitHub login"));
    }
    let reviewers = logins.into_iter().map(|login| Person {
      login: Some(login.to_string()),
      full_name: None,
    });
    (None, None, reviewers.collect(), Vec::new())
  };
  let full_name_of = |login: &str| review.as_ref().and_then(|review| review_full_name(review, login));
  let summary = |login: &str| PersonSummary {
    login: Some(login.to_string()),
    full_name: full_name_of(login),
  };

  let mut output = RequestReviewOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    pull_request: ReviewedPullRequest {
      number: pull_request.number,
      url: pull_request.url.clone(),
    },
    readiness,
    reviewers: reviewers.iter().filter_map(|person| person.login.as_deref()).map(summary).collect(),
    requested: Vec::new(),
    refused: Vec::new(),
    waiting_for: Vec::new(),
    groups_without_reviewer,
  };
  let mut waiting: Vec<String> = pull_request
    .reviewers
    .iter()
    .filter(|reviewer| reviewer.requested)
    .map(|reviewer| reviewer.login.clone())
    .collect();
  let logins: Vec<String> = reviewers.into_iter().filter_map(|person| person.login).collect();
  if !args.check && !logins.is_empty() {
    let result = runtime
      .block_on(request_pull_request_reviewers(&github, &context.repository, &repository, pull_request.number, &logins))
      .map_err(|error| match error {
        GitHubWriteError::GitHub(failure) => github_error(&failure),
        GitHubWriteError::Refused(message) => CliError::new(ErrorCode::InvalidArguments, message),
      })?;
    output.refused = result
      .refused
      .iter()
      .map(|refused| RefusedSummary {
        login: refused.login.clone(),
        full_name: full_name_of(&refused.login),
        reason: refused.reason.clone(),
      })
      .collect();
    if result.requested.is_empty() {
      let reasons: Vec<String> = result.refused.iter().map(|refused| format!("{}: {}", refused.login, refused.reason)).collect();
      return Err(CliError::new(
        ErrorCode::ReviewRefused,
        format!("GitHub asked nobody to review pull request #{}: {}", pull_request.number, reasons.join("; ")),
      ));
    }
    for login in &result.requested {
      if !waiting.iter().any(|it| it.eq_ignore_ascii_case(login)) {
        waiting.push(login.clone());
      }
    }
    output.requested = result.requested.iter().map(|login| summary(login)).collect();
  }
  output.waiting_for = waiting.iter().map(|login| summary(login)).collect();
  Ok(output)
}

/// What `bd request-review` reads the remote with: the GitHub service and repository of the branches
struct Reads<'a> {
  context: &'a RepoContext,
  github: &'a Arc<GitHubService>,
  repository: &'a GitHubRepository,
}

impl Reads<'_> {
  /// The services of the reads, with the Patronus service, the Space repository and the target of the review
  fn services(&self, patronus: Option<Arc<PatronusService>>, space: Option<SpaceRepository>, target: Option<String>) -> BdServices {
    BdServices {
      patronus,
      space,
      github_repository: Some(self.repository.clone()),
      target,
      ..BdServices::new(self.context, Arc::clone(self.github))
    }
  }
}

/// The review readiness of the virtual branch at its local tip, for the start of its pull request when it merges into
/// the target, else for a start through the API (whose answer still names the code owners): the pre-check asked again,
/// never a kept answer, through the reads of the app's session (`ServiceFactSources::readiness`); a pre-check that
/// failed is an error, a Patronus that doesn't let the user in `not_logged_in`
fn code_owner_review(reads: &Reads<'_>, environment: &Environment, name: &str, pull_request: &PullRequest) -> Result<Review, CliError> {
  let context = reads.context;
  let service = Arc::new(patronus_service(environment)?);
  let space = context.space_repository()?.ok_or_else(|| {
    CliError::new(
      ErrorCode::NotOnSpace,
      format!("The repository is not on Space: Patronus can't name the code owners; pass --reviewer <login> to ask someone for {name}"),
    )
  })?;
  let grouping = read_grouping(context, Some(name))?;
  let target = target_branch_of(grouping.upstream.as_ref()).ok_or_else(|| {
    CliError::new(
      ErrorCode::NotOnSpace,
      format!(
        "The baseline {} is not a branch of origin: Patronus names the code owners of a branch of origin",
        grouping.baseline_branch
      ),
    )
  })?;
  let full_name = to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  let tip = resolve_commit(&context.git, &context.repository, &format!("refs/heads/{full_name}"))?
    .ok_or_else(|| CliError::new(ErrorCode::UnknownBranch, format!("{name} has no virtual branch yet: `bd sync` builds it")))?;
  // the robot starts from the pull request only when it goes into the target
  let into_target = (base_of(pull_request, &target, &context.prefix) == PullRequestBase::Target).then_some(pull_request);
  let ask = ReviewReadinessBranch::starting_from(name.to_string(), None, tip, into_target);
  let sources = reads.services(Some(service), Some(space), Some(target)).sources();
  let key = session_key(context);
  let read = sources.readiness(&key, grouping.baseline_branch, vec![ask], true);
  let answer = context
    .runtime()?
    .block_on(read)
    .map_err(|error| CliError::new(error.code, error.message))?
    .pop()
    .ok_or_else(|| CliError::new(ErrorCode::PatronusError, "Patronus gave no answer for the review"))?;
  match (answer.requirement, answer.readiness) {
    (Some(requirement), readiness) => Ok(Review { requirement, readiness }),
    (
      None,
      ReviewReadiness::Unavailable {
        reason: UnavailableReadinessReason::NotLoggedIn,
        message,
      },
    ) => Err(CliError::new(ErrorCode::NotLoggedIn, message)),
    (None, ReviewReadiness::Unavailable { message, .. }) => Err(CliError::new(ErrorCode::PatronusError, message)),
    (None, _) => Err(CliError::new(ErrorCode::PatronusError, "Patronus gave no answer for the review")),
  }
}

/// The full name Patronus knows for a GitHub login
fn review_full_name(review: &Review, login: &str) -> Option<String> {
  review
    .requirement
    .groups
    .iter()
    .flat_map(|group| &group.reviewers)
    .find(|member| member.github_login.as_deref().is_some_and(|it| it.eq_ignore_ascii_case(login)))
    .map(|member| member.full_name.clone())
    .filter(|name| !name.is_empty())
}

/// Why the pull requests of the branches can't be read: `not_logged_in` without a token GitHub accepts
pub(crate) fn availability_error(availability: &PullRequestAvailability) -> CliError {
  match availability {
    PullRequestAvailability::SignedOut => CliError::new(ErrorCode::NotLoggedIn, SIGNED_OUT_MESSAGE),
    PullRequestAvailability::NotAuthenticated { source } => CliError::new(
      ErrorCode::NotLoggedIn,
      format!(
        "GitHub rejected the token from {}: tell the user to sign in to GitHub again (in the app, or gh auth login), or to set a valid GH_TOKEN",
        source.label()
      ),
    ),
    PullRequestAvailability::NotGitHub => CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY),
    PullRequestAvailability::RateLimited { .. } => CliError::new(ErrorCode::NetworkError, "GitHub refused the request for its rate limit: try again later"),
    PullRequestAvailability::SsoRequired { url } => CliError::new(
      ErrorCode::NotLoggedIn,
      match url {
        Some(url) => format!("The organization requires SAML SSO: authorize the token at {url}"),
        None => "The organization requires SAML SSO: tell the user to authorize the token for the organization on github.com, or to sign in to GitHub again".to_string(),
      },
    ),
    PullRequestAvailability::NoAccess { message, .. } | PullRequestAvailability::Error { message } => CliError::new(ErrorCode::NetworkError, message.clone()),
    PullRequestAvailability::Available { .. } => CliError::new(ErrorCode::NetworkError, "GitHub listed the pull requests"),
  }
}
