//! `bd request-review`: ask GitHub users to review the open pull request of a virtual branch, as the app's Request
//! Review… does (`spec/branch/pull-requests.spec.md#request-review`). Without `--reviewer` it asks the code owners
//! Patronus suggests for the groups whose approval is missing, one per group nobody is asked for yet, from the review
//! readiness `bd safe-push` checks (the pre-check with the pull request, asked again). Users only: a team is not asked.
//! No repository lock and no sync: the readiness is that of the virtual branch as the last sync built it; the only
//! write is the request on GitHub (`POST pulls/<n>/requested_reviewers`).

use crate::context::{Environment, RepoContext};
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use crate::safe_push_command::{PersonSummary, Review, UNKNOWN_GITHUB_REPOSITORY, github_error, github_service, patronus_service, readiness_names, review_readiness};
use crate::status::read_grouping;
use git_ops::branch_names::to_final_branch_name;
use git_ops::revs::resolve_commit;
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::patronus::target_branch_of;
use sync_core::pull_request_writes::request_pull_request_reviewers;
use sync_core::pull_requests::{GitHubRepository, GitHubWriteError, SIGNED_OUT_MESSAGE, list_pull_requests, resolve_github_repository_with};
use sync_core::review_readiness::ReadinessRepository;
use sync_types::patronus::{Person, ReviewReadiness, ReviewReadinessBranch, ReviewStart};
use sync_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestState};
use tokio::runtime::Runtime;

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
  let github = github_service(context, environment);
  let runtime = context.runtime()?;

  let listed = runtime.block_on(list_pull_requests(&github, &context.repository, &repository, &context.prefix));
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
    let review = code_owner_review(context, environment, runtime, &repository, name, &pull_request)?;
    let (reviewers, groups_without_reviewer) = suggested_reviewers(&review.readiness, &pull_request);
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

/// The review readiness of the virtual branch at its local tip, for the start of its pull request when it merges into
/// the target, else for a start through the API (whose answer still names the code owners)
fn code_owner_review(
  context: &RepoContext,
  environment: &Environment,
  runtime: &Runtime,
  repository: &GitHubRepository,
  name: &str,
  pull_request: &PullRequest,
) -> Result<Review, CliError> {
  let service = patronus_service(environment)?;
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
  let readiness_repository = ReadinessRepository {
    space: &space,
    github: Some(repository),
    target: &target,
    baseline: &grouping.baseline_branch,
    branch_prefix: &context.prefix,
  };
  let start = if pull_request.base_ref_name.is_empty() || pull_request.base_ref_name == target {
    ReviewStart::PullRequest { number: pull_request.number }
  } else {
    ReviewStart::Rest
  };
  let branch = ReviewReadinessBranch {
    branch_name: name.to_string(),
    tip,
    start,
    reviewers: pull_request.reviewers.clone(),
    author: pull_request.author.clone(),
  };
  review_readiness(context, &service, runtime, &readiness_repository, &branch)
}

/// One suggested code owner with a GitHub login of each missing group nobody is asked for, each once (a person chosen
/// for another group covers this one too), as the app's Request Review… preselects them; never the author, a member
/// asked already or one whose approval counts. And the missing groups nobody is asked for that have no such owner.
fn suggested_reviewers(readiness: &ReviewReadiness, pull_request: &PullRequest) -> (Vec<Person>, Vec<String>) {
  let ReviewReadiness::Missing { groups, .. } = readiness else {
    return (Vec::new(), Vec::new());
  };
  let is = |login: &str, other: Option<&str>| other.is_some_and(|other| other.eq_ignore_ascii_case(login));
  let asked = |login: &str| {
    pull_request
      .reviewers
      .iter()
      .any(|reviewer| reviewer.requested && reviewer.login.eq_ignore_ascii_case(login))
  };
  let mut chosen: Vec<Person> = Vec::new();
  let mut without = Vec::new();
  for group in groups.iter().filter(|group| group.requested.is_empty()) {
    let candidate = group.suggested.iter().find(|person| {
      person
        .login
        .as_deref()
        .is_some_and(|login| !is(login, pull_request.author.as_deref()) && !asked(login) && !group.approved_by.iter().any(|approver| is(login, approver.login.as_deref())))
    });
    match candidate {
      Some(person) if chosen.iter().any(|it| is(person.login.as_deref().unwrap_or_default(), it.login.as_deref())) => {}
      Some(person) => chosen.push(person.clone()),
      None => without.push(group.name.clone()),
    }
  }
  (chosen, without)
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
    PullRequestAvailability::NotAuthenticated { source } => CliError::new(ErrorCode::NotLoggedIn, format!("GitHub rejected the token from {}", source.label())),
    PullRequestAvailability::NotGitHub => CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY),
    PullRequestAvailability::RateLimited { .. } => CliError::new(ErrorCode::NetworkError, "GitHub refused the request for its rate limit: try again later"),
    PullRequestAvailability::SsoRequired { url } => CliError::new(
      ErrorCode::NotLoggedIn,
      match url {
        Some(url) => format!("The organization requires SAML SSO: authorize the token at {url}"),
        None => "The organization requires SAML SSO: authorize the token for it".to_string(),
      },
    ),
    PullRequestAvailability::NoAccess { message, .. } | PullRequestAvailability::Error { message } => CliError::new(ErrorCode::NetworkError, message.clone()),
    PullRequestAvailability::Available { .. } => CliError::new(ErrorCode::NetworkError, "GitHub listed the pull requests"),
  }
}
