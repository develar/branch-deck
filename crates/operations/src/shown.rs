//! The shown reviews of ship/show/ask after their start (docs/design/landing.md, "Show: a review after landing";
//! openspec/specs/branch/shown-reviews/spec.md). A Safe Push with a review after landing opens the draft before its
//! robot starts (`PlannedEffect::PushReview`, `CreateShownReview` of `land_execute`); then:
//!
//! - `Intent::MarkShownReview` (Request Review After Landing…, `brd request-review --after-landing` before the branch
//!   landed): the review after landing marked ahead of time, refused as a Safe Push with it is without what refuses its
//!   robot (`domain_policy::shown::mark_refusals`): its branches pushed at the tip of the branch and its fork point, its
//!   draft opened asking nobody, its body recording the reviewers its open asks; of a draft, only that record; of a
//!   branch that landed without one, the review opened now on what landed (`create_plan`);
//! - `Intent::OpenShownReview`, once something landed the branch since the draft opened (the news `ShownReviewLanded`,
//!   `brd request-review --after-landing`; `domain_policy::shown::landed_head`: a merged Safe Push of the branch, its
//!   merged pull request, the baseline by content): the branches of the review go to what landed and its fork point (one
//!   atomic push, leased on what the remote has; none when they are there), the draft is marked ready for review, and the
//!   reviewers the record of its body names are asked; its body says where the branch was reviewed before it landed (its
//!   merged pull request). A review ready already only asks again those it asked who have not approved and are asked no
//!   more. A reviewer the user adds is asked and recorded in its body;
//! - `Intent::ExtendShownReview`, once a fix of the branch landed after the review opened (the news
//!   `ShownReviewFixLanded`, `brd request-review`): the commits of the fixes are replayed onto its head and pushed
//!   leased on the head the remote has (`sync_core::review_head::extend_review`); a fix whose commits took the files
//!   they conflict in from what landed them is named in a comment with those files, one that doesn't apply in a
//!   comment instead (each once: a repeat finds it); once GitHub shows the new head (a mirror), its reviewers are asked again,
//!   and the record of its body names the fixes last, so a run that stopped between the steps is due again and adds
//!   nothing twice;
//! - `Intent::CloseShownReview`, once every reviewer it asked approved (the news `ShownReviewApproved`, `brd
//!   close-review`): the pull request is closed with a comment that names who approved it, and its branches are deleted,
//!   those the remote has, each leased on what it has. One not approved is refused unless the user says so;
//! - `Intent::DeleteClosedShownReview`, an hour after a review was closed elsewhere (on GitHub, the news
//!   `ShownReviewClosedElsewhere`; `brd close-review` of one closed there): its branches are deleted where the remote
//!   still has them at the tips of its pull request, leased on them (`sync_core::review_head::delete_review_at`); a
//!   branch gone or moved since, a newer review of the branch, stays.
//!
//! Each holds the fields of the pull request it writes and the branches of the review (`shown_request`); its effects
//! run in their order (`run_shown`), the first that fails skips the rest. The app runs them in the session of the
//! repository (`execute_shown`), `brd` over its one-shot reads with the same plans and the same run. A run of the app
//! that failed makes its news due again at the next answer of the listing or the robots (`Sessions::retell_shown`, a few
//! times per session).

use crate::engine::{check_admitted, outcome_of, run};
use crate::events::EventSink;
use crate::land_execute::{Accepted, refusal_code};
use crate::pull_request::{github_of, write_plan};
use crate::registry::{Answer, OperationRequest, Registry};
use crate::robot_start::github_failure;
use crate::witness::plan_changed;
use domain_policy::shown::{
  ContentLanding, LandedBy, LandedHead, close_comment, close_refusal, closed_remains, fix_conflict_comment, fix_taken_comment, landed_head, mark_land, mark_refusals,
  pending_fixes, review_state, reviewers_record, shown_review,
};
use domain_types::branch_names::{baseline_on_remote, review_branch_names, to_final_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{FactWait, LandFact, LandedFix, PlannedEffect};
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationResult, OperationStep, Plan, PullRequestField, Resource};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::{ClosedShownReview, CreatedPullRequest, PullRequest, PullRequestState, RequestedReviewers, Reviewer, ReviewerState, ShownReview};
use domain_types::reason::Reason;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_signing::commit_signing;
use git_ops::fix_replay::FixReplay;
use github::service::GitHubService;
use remote_facts::pull_request_create::{CreatePullRequestParams, GetNewPullRequestParams, MIRROR_WAIT, create_pull_request, new_pull_request, wait_for_branch_at};
use remote_facts::pull_request_writes::{close_pull_request, comment_once, mark_ready_for_review, record_shown_review, request_pull_request_reviewers};
use remote_facts::pull_requests::{GitHubWriteError, failure_message};
use remote_facts::service_sources::ServiceHost;
use repository_model::branch_table::BranchTable;
use repository_model::derive::{land_decision, with_land_sources};
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::refusal_words::start_refusal_text;
use repository_model::session::{SessionKey, Sessions};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use sync_core::review_head::{Fixes, RemoteReview, ReviewExtension, delete_review, delete_review_at, extend_review, push_review};
use sync_utils::span::in_current_span;
use tracing::instrument;

/// The body of the draft of a shown review: the text of the branch, what a shown review is, and the record of the
/// reviewers its open asks (`domain_policy::shown::reviewers_record`)
#[must_use]
pub fn shown_body(body: &str, reviewers: &[String], target: &str) -> String {
  let mut text = String::new();
  if !body.trim().is_empty() {
    text.push_str(body.trim_end());
    text.push_str("\n\n");
  }
  let _ = write!(
    text,
    "Shown after landing: these commits land in {target} with the Safe Push of the branch, and the review follows there. \
     This pull request goes into their fork point: it cannot be merged, and it closes once every reviewer approved."
  );
  text.push_str("\n\n");
  text.push_str(&reviewers_record(reviewers));
  text
}

/// The reviewers `asked` of a review, then those of `added` it doesn't name yet (by login, in any case)
fn with_added(asked: &[String], added: &[String]) -> Vec<String> {
  let mut all = asked.to_vec();
  for login in added {
    if !all.iter().any(|it| it.eq_ignore_ascii_case(login)) {
      all.push(login.clone());
    }
  }
  all
}

/// The plan of the open of the shown review `review` of `branch` (module docs) over `landed`, what landed the branch since
/// it opened (`domain_policy::shown::landed_head`; `robots`, the robots it may name): a draft pushes its branches to the
/// head that names (none: keeps the head it has), is marked ready and asks its reviewers, its body naming the merged pull
/// request where the branch was reviewed before; one ready asks again those it asked who neither approved nor are asked
/// now. `added`, reviewers the user names, are asked too and recorded in its body; of a draft whose branch nothing landed
/// yet they are only recorded, its open asks them with the others. Of a review whose base the remote lacks (reopened
/// after its branches were deleted: GitHub restores only the head) the branches are pushed again first, the base to the
/// fork point of its head
///
/// # Errors
/// `no_shown_review` for a draft whose branch nothing landed yet and that is given nobody new, or one that records no
/// reviewer and is given none
pub fn open_plan<'a>(
  review: &ShownReview,
  branch: &str,
  landed: Option<&LandedHead>,
  robots: impl IntoIterator<Item = &'a PatronusRobot>,
  added: &[String],
) -> Result<Plan, ApiError> {
  let pull_request = &review.pull_request;
  let number = pull_request.number;
  let intent = Intent::OpenShownReview {
    branch: branch.to_owned(),
    reviewers: added.to_vec(),
  };
  let reviewers = with_added(&review.reviewers, added);
  if reviewers.is_empty() {
    return Err(ApiError::new(
      ErrorCode::NoShownReview,
      format!("The shown review #{number} of {branch} records no reviewer to ask: ask them on its page"),
    ));
  }
  let head = landed.and_then(|landed| landed.head.clone()).filter(|head| *head != pull_request.head_ref_oid);
  let mut effects = if pull_request.is_draft && landed.is_none() && reviewers.len() > review.reviewers.len() {
    // not landed yet: the record names whom its open asks
    Vec::new()
  } else if pull_request.is_draft {
    if landed.is_none() {
      return Err(ApiError::new(
        ErrorCode::NoShownReview,
        format!("Nothing landed {branch} since its shown review #{number} opened: it opens once a Safe Push or a merge of {branch} lands it"),
      ));
    }
    let push = head.map(|tip| PlannedEffect::PushReview { branch: branch.to_owned(), tip });
    push
      .into_iter()
      .chain([
        PlannedEffect::MarkReady { number },
        PlannedEffect::RequestReviewers {
          number,
          reviewers: reviewers.clone(),
        },
      ])
      .collect()
  } else {
    let again: Vec<String> = reviewers
      .iter()
      .filter(|login| {
        let named = added.iter().any(|added| added.eq_ignore_ascii_case(login));
        named
          || !pull_request
            .reviewers
            .iter()
            .any(|reviewer| reviewer.login.eq_ignore_ascii_case(login) && (reviewer.requested || reviewer.state == ReviewerState::Approved))
      })
      .cloned()
      .collect();
    if again.is_empty() {
      Vec::new()
    } else {
      vec![PlannedEffect::RequestReviewers { number, reviewers: again }]
    }
  };
  let pushes = effects.iter().any(|effect| matches!(effect, PlannedEffect::PushReview { .. }));
  if review.base_missing && !pushes && !pull_request.head_ref_oid.is_empty() {
    effects.insert(
      0,
      PlannedEffect::PushReview {
        branch: branch.to_owned(),
        tip: pull_request.head_ref_oid.clone(),
      },
    );
  }
  // at its open: where the branch was reviewed before it landed
  let reviewed_in = landed.filter(|_| pull_request.is_draft).and_then(|landed| landed.reviewed_in);
  if reviewers.len() > review.reviewers.len() || reviewed_in.is_some() {
    effects.push(PlannedEffect::RecordShown {
      number,
      reviewers,
      landed: review.landed.clone(),
      reviewed_in,
    });
  }
  let robot = landed.and_then(|landed| match &landed.by {
    LandedBy::Robot { robot } => Some(robot.as_str()),
    LandedBy::PullRequest { .. } | LandedBy::Content => None,
  });
  let robots: Vec<&PatronusRobot> = robots.into_iter().filter(|it| Some(it.id.as_str()) == robot).collect();
  Ok(write_plan(intent, effects, Vec::new(), Some(number), [pull_request], robots))
}

/// The plan of the extend of the open shown review `review` of `branch` by `fixes`, the merged Safe Pushes of the branch
/// it has not taken yet (`domain_policy::shown::pending_fixes`, oldest first), with `added`, reviewers the user names
/// (module docs): the fixes replayed onto its head (once GitHub shows it), its reviewers asked again, then the record of
/// its body names them. Until that record the fixes are due (`ShownState::Extending`), so a run that stopped between
/// the steps never lets an approval of the old head close it
///
/// # Errors
/// `no_shown_review` when no fix of the branch is due
pub fn extend_plan(review: &ShownReview, branch: &str, fixes: &[&PatronusRobot], added: &[String]) -> Result<Plan, ApiError> {
  let pull_request = &review.pull_request;
  let number = pull_request.number;
  let landed: Vec<LandedFix> = fixes
    .iter()
    .filter_map(|robot| {
      Some(LandedFix {
        robot: robot.id.clone(),
        source: robot.source_revision.clone()?,
        url: robot.url.clone(),
      })
    })
    .collect();
  if landed.is_empty() {
    return Err(ApiError::new(
      ErrorCode::NoShownReview,
      format!("No fix of {branch} landed after its shown review #{number} opened: a fix lands by a Safe Push of the branch"),
    ));
  }
  let reviewers = with_added(&review.reviewers, added);
  let mut recorded = review.landed.clone();
  recorded.extend(landed.iter().map(|fix| fix.source.clone()).filter(|source| !review.landed.contains(source)));
  let ask = (!reviewers.is_empty()).then(|| PlannedEffect::RequestReviewers {
    number,
    reviewers: reviewers.clone(),
  });
  let extend = PlannedEffect::ExtendReview {
    branch: branch.to_owned(),
    number,
    fixes: landed,
  };
  let record = PlannedEffect::RecordShown {
    number,
    reviewers,
    landed: recorded,
    reviewed_in: None,
  };
  let effects = std::iter::once(extend).chain(ask).chain([record]).collect();
  let intent = Intent::ExtendShownReview { branch: branch.to_owned() };
  Ok(write_plan(intent, effects, Vec::new(), Some(number), [pull_request], fixes.iter().copied()))
}

/// The plan of the shown review of `branch`, which landed at `source` without one (`brd request-review --after-landing`,
/// Request Review After Landing… of a branch that landed): its branches pushed to `source` and its fork point, its pull
/// request opened ready with `reviewers` asked
#[must_use]
pub fn create_plan(branch: &str, source: &str, reviewers: &[String]) -> Plan {
  let effects = vec![
    PlannedEffect::PushReview {
      branch: branch.to_owned(),
      tip: source.to_owned(),
    },
    PlannedEffect::CreateShownReview {
      branch: branch.to_owned(),
      reviewers: reviewers.to_vec(),
      draft: false,
    },
  ];
  let intent = Intent::MarkShownReview {
    branch: branch.to_owned(),
    reviewers: reviewers.to_vec(),
  };
  write_plan(intent, effects, Vec::new(), None, [], [])
}

/// What the mark of the review after landing of a branch ahead of time is planned on (`mark_plan`)
#[derive(Clone, Copy, Debug)]
pub struct MarkFacts<'a> {
  pub branch: &'a str,
  /// Whom its open asks, as the user picked them
  pub reviewers: &'a [String],
  /// Its review after landing, open or a draft
  pub review: Option<&'a ShownReview>,
  /// What landed it, of a branch without a review (`domain_policy::shown::landed_head` with no review): the review opens
  /// now on what landed
  pub landed: Option<&'a LandedHead>,
  /// The commit the branch is at; none until the sync built it
  pub tip: Option<&'a str>,
  /// The decision of the Safe Push with the review after landing of it (`domain_policy::shown::mark_land`); none of a
  /// branch the table lacks
  pub decision: Option<MarkDecision<'a>>,
}

/// What the decision of the Safe Push with the review after landing of a branch says of a mark of it (`mark_plan`)
#[derive(Clone, Copy, Debug)]
pub struct MarkDecision<'a> {
  pub blockers: &'a [Reason],
  pub waits: &'a [FactWait],
  /// The pull request of the branch is open: the approvals the code owners give first come there
  pub pull_request_open: bool,
}

/// The plan of the mark of the review after landing of `facts.branch` ahead of time (module docs): of a draft, its
/// reviewers recorded as picked (nothing when they are); of an open review, refused (`show_review_open`); of a branch
/// that landed, the review opened now on what landed (`create_plan`); else its branches pushed at the tip of the branch
/// and its fork point and its draft opened, refused as a Safe Push with it is (`domain_policy::shown::mark_refusals`).
/// Nobody to ask refuses it (`no_reviewer`), a branch that nothing tells what landed it and the table lacks
/// (`show_nothing_landed`)
#[must_use]
pub fn mark_plan(facts: &MarkFacts<'_>) -> Plan {
  let branch = facts.branch;
  let intent = Intent::MarkShownReview {
    branch: branch.to_owned(),
    reviewers: facts.reviewers.to_vec(),
  };
  let nobody = facts.reviewers.is_empty().then(|| Reason::NoReviewer { groups: Vec::new() });
  if let Some(review) = facts.review {
    let pull_request = &review.pull_request;
    let number = pull_request.number;
    if !pull_request.is_draft {
      return write_plan(intent, Vec::new(), vec![Reason::ShowReviewOpen { number }], Some(number), [pull_request], []);
    }
    let same = review.reviewers.len() == facts.reviewers.len() && review.reviewers.iter().all(|login| facts.reviewers.iter().any(|it| it.eq_ignore_ascii_case(login)));
    let effects = if same || nobody.is_some() {
      Vec::new()
    } else {
      vec![PlannedEffect::RecordShown {
        number,
        reviewers: facts.reviewers.to_vec(),
        landed: review.landed.clone(),
        reviewed_in: None,
      }]
    };
    return write_plan(intent, effects, nobody.into_iter().collect(), Some(number), [pull_request], []);
  }
  let nothing_landed = || write_plan(intent.clone(), Vec::new(), vec![Reason::ShowNothingLanded { branch: branch.to_owned() }], None, [], []);
  if let Some(landed) = facts.landed {
    let Some(head) = landed.head.as_deref() else {
      return nothing_landed();
    };
    let mut plan = create_plan(branch, head, facts.reviewers);
    if let Some(nobody) = nobody {
      plan.blockers.push(nobody);
    }
    return plan;
  }
  // a branch the table lacks that nothing tells what landed
  let Some(decision) = facts.decision else {
    return nothing_landed();
  };
  let (mut blockers, mut waits) = mark_refusals(decision.blockers, decision.waits, decision.pull_request_open);
  blockers.extend(nobody);
  let effects = if let Some(tip) = facts.tip {
    vec![
      PlannedEffect::PushReview {
        branch: branch.to_owned(),
        tip: tip.to_owned(),
      },
      PlannedEffect::CreateShownReview {
        branch: branch.to_owned(),
        reviewers: facts.reviewers.to_vec(),
        draft: true,
      },
    ]
  } else {
    // the sync did not build it yet
    waits.push(FactWait {
      fact: LandFact::Remote,
      failed: None,
    });
    Vec::new()
  };
  let mut plan = write_plan(intent, effects, blockers, None, [], []);
  plan.waits = waits;
  plan
}

/// The plan of the close of the shown review `review` of `branch`: its close with the comment that names who approved it
/// (in `target`), then the delete of its branches; refused (a blocker) while a reviewer it asked has not approved, unless
/// `allow_unapproved`
#[must_use]
pub fn close_plan(review: &ShownReview, branch: &str, allow_unapproved: bool, target: &str) -> Plan {
  let number = review.pull_request.number;
  let effects = vec![
    PlannedEffect::ClosePullRequest {
      branch: branch.to_owned(),
      number,
      comment: close_comment(review, target),
    },
    PlannedEffect::DeleteReview { branch: branch.to_owned() },
  ];
  let intent = Intent::CloseShownReview {
    branch: branch.to_owned(),
    allow_unapproved,
  };
  write_plan(
    intent,
    effects,
    close_refusal(review, allow_unapproved).into_iter().collect(),
    Some(number),
    [&review.pull_request],
    [],
  )
}

/// The plan of the delete of the branches of the shown review `closed` of `branch`, closed elsewhere: those the remote
/// still has at the tips of its pull request (`domain_policy::shown::closed_remains`), leased on them; nothing when none
/// is left
///
/// # Errors
/// `no_shown_review` while an open shown review of the branch is listed (`open`): its branches are that review's
pub fn delete_closed_plan(closed: &ClosedShownReview, open: Option<&ShownReview>) -> Result<Plan, ApiError> {
  let (branch, number) = (closed.branch.as_str(), closed.number);
  if let Some(open) = open {
    return Err(ApiError::new(
      ErrorCode::NoShownReview,
      format!(
        "{branch} has the open shown review #{}: the branches of #{number}, closed, are its branches now",
        open.pull_request.number
      ),
    ));
  }
  let effects = closed_remains(closed)
    .map(|(head, base)| PlannedEffect::DeleteReviewAt {
      branch: branch.to_owned(),
      head,
      base,
    })
    .into_iter()
    .collect();
  let intent = Intent::DeleteClosedShownReview {
    branch: branch.to_owned(),
    number,
  };
  Ok(write_plan(intent, effects, Vec::new(), None, [], []))
}

/// What an open or a close of the shown review of a branch under `prefix` asks the registry for: the reviewers (an open)
/// or the state (a close) of its pull request, and its branches, which it pushes or deletes
#[must_use]
pub fn shown_request(prefix: &str, intent: &Intent, plan: Option<&Plan>) -> OperationRequest {
  let (branch, field) = match intent {
    Intent::OpenShownReview { branch, .. } | Intent::ExtendShownReview { branch } | Intent::MarkShownReview { branch, .. } => (branch.as_str(), PullRequestField::Reviewers),
    Intent::CloseShownReview { branch, .. } | Intent::DeleteClosedShownReview { branch, .. } => (branch.as_str(), PullRequestField::State),
    _ => ("", PullRequestField::State),
  };
  let mut resources: Vec<Resource> =
    review_branch_names(prefix, branch).map_or_else(|_| Vec::new(), |(head, base)| vec![Resource::Ref { name: head }, Resource::Ref { name: base }]);
  resources.extend(plan.and_then(|plan| plan.pull_request).map(|number| Resource::PullRequest { number, field }));
  OperationRequest {
    intent: intent.clone(),
    resources,
    step: OperationStep::Writing,
    moves: Vec::new(),
  }
}

/// What the effects of a shown review run with
#[derive(Clone)]
pub struct ShownServices {
  pub git: GitCommandExecutor,
  pub repository_path: String,
  pub prefix: String,
  /// The remote its branches go to ("origin")
  pub remote: String,
  /// The baseline its fork point is on ("origin/master")
  pub baseline: String,
  pub github: Arc<GitHubService>,
  pub repository: GitHubRepository,
  /// The title and the text of the branch, of a shown review it opens (`CreateShownReview`); `None` where it opens none
  pub text: Option<ShownText>,
}

/// The title and the text of the branch a shown review opens with, its body adding what a shown review is and the record
/// of its reviewers (`shown_body`), and whether it is assigned to the user
#[derive(Clone, Debug, Default)]
pub struct ShownText {
  pub title: String,
  pub body: String,
  pub assign_self: bool,
  /// The branch of origin the robots push into ("master"), which the body names
  pub target: String,
}

/// What a run of the effects of a shown review did
#[derive(Debug, Default)]
pub struct ShownRun {
  pub records: Vec<EffectRecord>,
  /// Whom its review request asked
  pub requested: Option<RequestedReviewers>,
  /// The pull request it opened (or found open), `CreateShownReview`
  pub created: Option<CreatedPullRequest>,
  /// Where its extend left the head of the review, `ExtendReview`
  pub extended: Option<ReviewExtension>,
  /// The fixes that don't apply on the head of the review, which a comment names (`ExtendReview`)
  pub not_applied: Vec<LandedFix>,
  /// The fixes whose commits took the files they conflict in from what landed them, which a comment names with those
  /// files (`ExtendReview`)
  pub taken: Vec<TakenFix>,
}

/// A fix of a shown review whose commits took the files they conflict in from what landed them on the target
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakenFix {
  pub fix: LandedFix,
  /// The files it took, each once, in the order of its commits
  pub files: Vec<String>,
}

/// What the extend of a shown review into `target` says of its fixes (module docs), `replayed` what became of each of
/// `fixes`, in their order
#[derive(Debug, Default)]
struct FixOutcomes {
  not_applied: Vec<LandedFix>,
  taken: Vec<TakenFix>,
  /// One per fix that doesn't apply or took files, in the order of the fixes
  comments: Vec<String>,
}

fn fix_outcomes(fixes: &[LandedFix], replayed: &[FixReplay], target: &str) -> FixOutcomes {
  let mut outcomes = FixOutcomes::default();
  for (fix, replay) in fixes.iter().zip(replayed) {
    match replay {
      FixReplay::NotApplied { .. } => {
        outcomes.comments.push(fix_conflict_comment(&fix.source, &fix.url));
        outcomes.not_applied.push(fix.clone());
      }
      FixReplay::Applied { taken: commits } if !commits.is_empty() => {
        let mut files: Vec<String> = Vec::new();
        for path in commits.iter().flat_map(|commit| &commit.paths) {
          if !files.contains(path) {
            files.push(path.clone());
          }
        }
        outcomes.comments.push(fix_taken_comment(&fix.source, &files, &fix.url, target));
        outcomes.taken.push(TakenFix { fix: fix.clone(), files });
      }
      FixReplay::Applied { .. } => {}
    }
  }
  outcomes
}

/// The effects of `plan` in their order (module docs); the first that fails skips the rest
pub async fn run_shown(services: &ShownServices, plan: &Plan) -> ShownRun {
  let mut run = ShownRun::default();
  let mut failed = false;
  for effect in &plan.effects {
    let status = if failed {
      EffectStatus::Skipped { reason: None }
    } else {
      // boxed: one future of every kind of effect would overflow the evaluation of `Send` of its callers
      match Box::pin(shown_effect(services, effect, &mut run)).await {
        Ok(()) => EffectStatus::Done,
        Err(error) => {
          failed = true;
          EffectStatus::Failed { error }
        }
      }
    };
    run.records.push(EffectRecord::new(effect.clone(), status));
  }
  run
}

/// One effect of a shown review
async fn shown_effect(services: &ShownServices, effect: &PlannedEffect, run: &mut ShownRun) -> Result<(), ApiError> {
  let (github, path, repository) = (&services.github, services.repository_path.as_str(), &services.repository);
  let github_error = |what: String| {
    move |error: GitHubWriteError| {
      let failure = github_failure(error);
      ApiError::new(failure.code, format!("{what}: {}", failure.message))
    }
  };
  match effect {
    PlannedEffect::PushReview { branch, tip } => {
      let (git, path, remote, prefix, branch, tip) = (
        services.git.clone(),
        path.to_owned(),
        services.remote.clone(),
        services.prefix.clone(),
        branch.clone(),
        tip.clone(),
      );
      let target_ref = format!("refs/remotes/{}", services.baseline);
      blocking(move || push_review(&git, &path, &remote, &prefix, &branch, &tip, &target_ref).map(|_| ())).await
    }
    PlannedEffect::MarkReady { number } => mark_ready_for_review(github, path, repository, *number)
      .await
      .map(|_| ())
      .map_err(github_error(format!("Cannot mark pull request #{number} ready for review"))),
    PlannedEffect::RequestReviewers { number, reviewers } => {
      let requested = request_pull_request_reviewers(github, path, repository, *number, reviewers)
        .await
        .map_err(github_error(format!("Cannot ask the reviewers of pull request #{number}")))?;
      let nobody = requested.requested.is_empty();
      let refused: Vec<String> = requested.refused.iter().map(|refused| format!("{}: {}", refused.login, refused.reason)).collect();
      run.requested = Some(requested);
      if nobody {
        return Err(ApiError::new(
          ErrorCode::ReviewRefused,
          format!("GitHub asked nobody to review pull request #{number}: {}", refused.join("; ")),
        ));
      }
      Ok(())
    }
    PlannedEffect::ClosePullRequest { number, comment, .. } => close_pull_request(github, path, repository, *number, comment)
      .await
      .map_err(github_error(format!("Cannot close pull request #{number}"))),
    PlannedEffect::CreateShownReview { branch, reviewers, draft } => {
      let text = services
        .text
        .clone()
        .ok_or_else(|| ApiError::internal("A shown review opens with the text of its branch"))?;
      let (head, base) = review_branch_names(&services.prefix, branch).map_err(|error| ApiError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
      let params = CreatePullRequestParams {
        repository_path: path.to_owned(),
        branch_name: branch.clone(),
        head,
        base,
        title: text.title,
        body: shown_body(&text.body, reviewers, &text.target),
        reviewers: if *draft { Vec::new() } else { reviewers.clone() },
        assign_self: text.assign_self,
        wait_for_mirror: true,
        draft: *draft,
      };
      let created = create_pull_request(github, repository, &params)
        .await
        .map_err(github_error(format!("Cannot open the shown review of {branch}")))?;
      run.created = Some(created);
      Ok(())
    }
    PlannedEffect::ExtendReview { branch, number, fixes } => {
      let (git, path_owned, remote, prefix, branch_owned) = (services.git.clone(), path.to_owned(), services.remote.clone(), services.prefix.clone(), branch.clone());
      let sources: Vec<String> = fixes.iter().map(|fix| fix.source.clone()).collect();
      let target_ref = format!("refs/remotes/{}", services.baseline);
      let extension = tokio::task::spawn_blocking(in_current_span(move || {
        let signing = commit_signing(&git, &path_owned)?;
        let fixes = Fixes {
          sources: &sources,
          target_ref: &target_ref,
          signing,
        };
        extend_review(&git, &path_owned, &remote, &prefix, &branch_owned, fixes)
      }))
      .await
      .map_err(|error| ApiError::internal(format!("The extend of the shown review failed: {error}")))?
      .map_err(|error| ApiError::new(ErrorCode::PushRefused, format!("{error:#}")))?;
      let outcomes = fix_outcomes(fixes, &extension.fixes, baseline_on_remote(&services.baseline));
      // the reviewers are asked again once GitHub shows them the fix: a mirror copies the push a moment later
      if extension.head != extension.old_head {
        let (head, _) = review_branch_names(&services.prefix, branch).map_err(|error| ApiError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
        let shown = wait_for_branch_at(github, path, repository, &head, &extension.head, MIRROR_WAIT)
          .await
          .map_err(|failure| ApiError::new(ErrorCode::NetworkError, failure_message(&failure)))?;
        if !shown {
          return Err(ApiError::new(
            ErrorCode::NetworkError,
            format!("GitHub doesn't show {head} at {} yet: a mirror copies a push a moment later; try again", extension.head),
          ));
        }
      }
      run.extended = Some(extension);
      for comment in &outcomes.comments {
        // the whole comment is its mark: a repeat of the same fix says the same
        comment_once(github, path, repository, *number, comment, comment)
          .await
          .map_err(github_error(format!("Cannot comment on pull request #{number}")))?;
      }
      run.not_applied = outcomes.not_applied;
      run.taken = outcomes.taken;
      Ok(())
    }
    PlannedEffect::RecordShown {
      number,
      reviewers,
      landed,
      reviewed_in,
    } => record_shown_review(github, path, repository, *number, reviewers, landed, *reviewed_in)
      .await
      .map(|_| ())
      .map_err(github_error(format!("Cannot record the reviewers and the fixes of pull request #{number}"))),
    PlannedEffect::DeleteReview { branch } => {
      let (git, path, remote, prefix, branch) = (services.git.clone(), path.to_owned(), services.remote.clone(), services.prefix.clone(), branch.clone());
      blocking(move || delete_review(&git, &path, &remote, &prefix, &branch).map(|_| ())).await
    }
    PlannedEffect::DeleteReviewAt { branch, head, base } => {
      let (git, path, remote, prefix, branch) = (services.git.clone(), path.to_owned(), services.remote.clone(), services.prefix.clone(), branch.clone());
      let at = RemoteReview {
        head: head.clone(),
        base: base.clone(),
      };
      blocking(move || delete_review_at(&git, &path, &remote, &prefix, &branch, &at).map(|_| ())).await
    }
    other => Err(ApiError::internal(format!("A shown review makes no {other:?}"))),
  }
}

async fn blocking(work: impl FnOnce() -> anyhow::Result<()> + Send + 'static) -> Result<(), ApiError> {
  tokio::task::spawn_blocking(in_current_span(work))
    .await
    .map_err(|error| ApiError::internal(format!("The step of the shown review failed: {error}")))?
    .map_err(|error| ApiError::new(ErrorCode::PushRefused, format!("{error:#}")))
}

/// The refusal of a plan of a shown review by its first blocker, in the words of the engine; of one that waits for a fact
/// nobody answered yet, a changed plan
#[must_use]
pub fn shown_refusal(plan: &Plan, branch: &str) -> Option<ApiError> {
  let Some(blocker) = plan.blockers.first() else {
    return (!plan.waits.is_empty()).then(|| plan_changed(&plan.intent, Some("it waits for facts nobody answered yet")));
  };
  match blocker {
    reason @ (Reason::ShowLandsStack { .. }
    | Reason::ShowNeedsApprovals { .. }
    | Reason::ShowNeedsPatronus
    | Reason::NoReviewer { .. }
    | Reason::ShowReviewOpen { .. }
    | Reason::ShowNothingLanded { .. }) => Some(ApiError::new(
      refusal_code(reason),
      format!("Nothing was marked: {}", start_refusal_text(reason, branch, &[])),
    )),
    Reason::ReviewNotApproved { number, waiting } if waiting.is_empty() => Some(ApiError::new(
      ErrorCode::ReviewNotApproved,
      format!("Nothing was closed: the shown review #{number} of {branch} asked nobody yet; --allow-unapproved closes it anyway, only when the user says so"),
    )),
    Reason::ReviewNotApproved { number, waiting } => Some(ApiError::new(
      ErrorCode::ReviewNotApproved,
      format!(
        "Nothing was closed: {} has not approved the shown review #{number} of {branch}; --allow-unapproved closes it anyway, only when the user says so",
        waiting.join(", ")
      ),
    )),
    other => Some(ApiError::new(ErrorCode::PlanChanged, format!("Nothing was done: {other:?}"))),
  }
}

/// What tells what landed a branch of a review after landing (`landed_of`): the table of the last sync, the branches
/// the baseline landed whole by content with when (while the robots are known), the pull requests of the branches, the
/// target they go into, the robots
#[derive(Clone, Copy)]
pub struct ShownLandings<'a> {
  pub table: Option<&'a BranchTable>,
  pub landed: &'a BTreeMap<String, u32>,
  pub pull_requests: &'a BTreeMap<String, PullRequest>,
  pub target: &'a str,
  pub robots: &'a [&'a PatronusRobot],
}

impl ShownLandings<'_> {
  /// The tip of `branch`: its virtual branch as the table has it, else its remote branch
  fn tip(&self, branch: &str) -> Option<&str> {
    let table = self.table?;
    table
      .tip_of(branch)
      .or_else(|| table.remote.branches.get(branch).and_then(|remote| remote.remote_tip.as_deref()))
  }

  /// What landed `branch` since its review `review` opened (`domain_policy::shown::landed_head`); with no review, of a
  /// branch that landed: one the table has only once the baseline has it whole or a merged Safe Push took it as it is
  /// (never a robot of an earlier branch of the name), one it lacks by whatever names it
  #[must_use]
  pub fn landed_of(&self, review: Option<&ShownReview>, branch: &str) -> Option<LandedHead> {
    let tip = self.tip(branch);
    let content = self.landed.get(branch).map(|at| ContentLanding { landed_at: *at, tip });
    let in_table = self.table.is_some_and(|table| table.tip_of(branch).is_some());
    if review.is_none() && in_table {
      let took = self.robots.iter().any(|robot| {
        robot.branch_name.as_deref() == Some(branch) && robot.source_revision.as_deref() == tip && landed_head(None, branch, [*robot], None, self.target, None).is_some()
      });
      if content.is_none() && !took {
        return None;
      }
    }
    landed_head(review, branch, self.robots.iter().copied(), self.pull_requests.get(branch), self.target, content)
  }
}

/// The plan of `intent` (a mark, an open, an extend or a close of a shown review, a delete of the branches of a closed
/// one) in the open session of `key`, over its listing of the shown reviews, its robots and its table; `None` for a
/// session that is not open
///
/// # Errors
/// `no_shown_review`: the session lists no open shown review of the branch, or (an open) nothing landed it
pub async fn prepare_shown(sessions: &Sessions, key: &SessionKey, intent: &Intent) -> Result<Option<Plan>, ApiError> {
  let intent = intent.clone();
  let prefix = key.prefix.clone();
  let read = sessions
    .read(key, move |state, facts| {
      let target = baseline_on_remote(&state.header().baseline_branch).to_owned();
      let listed = facts.pull_requests.view();
      let shown = &listed.shown_reviews;
      let robots: Vec<&PatronusRobot> = facts.robots.view().robots.values().collect();
      let landings = ShownLandings {
        table: facts.table.value().map(AsRef::as_ref),
        landed: facts.shown_landed(),
        pull_requests: &listed.pull_requests,
        target: &target,
        robots: &robots,
      };
      match &intent {
        Intent::MarkShownReview { branch, reviewers } => {
          let review = shown.get(branch);
          let decision = with_land_sources(state, facts, &prefix, |sources| land_decision(sources, &mark_land(branch, reviewers))).flatten();
          let open = listed.pull_requests.get(branch).is_some_and(|pull_request| pull_request.state == PullRequestState::Open);
          let landed = review.is_none().then(|| landings.landed_of(None, branch)).flatten();
          let facts = MarkFacts {
            branch,
            reviewers,
            review,
            landed: landed.as_ref(),
            tip: landings.table.and_then(|table| table.tip_of(branch)),
            decision: decision.as_ref().map(|(decision, _)| MarkDecision {
              blockers: &decision.blockers,
              waits: &decision.waits,
              pull_request_open: open,
            }),
          };
          Ok(Some(mark_plan(&facts)))
        }
        Intent::OpenShownReview { branch, reviewers } => {
          let review = shown.get(branch).ok_or_else(|| no_shown_review(branch))?;
          let landed = landings.landed_of(Some(review), branch);
          open_plan(review, branch, landed.as_ref(), robots.iter().copied(), reviewers).map(Some)
        }
        Intent::ExtendShownReview { branch } => {
          let review = shown.get(branch).ok_or_else(|| no_shown_review(branch))?;
          extend_plan(review, branch, &pending_fixes(review, branch, facts.robots.view().robots.values()), &[]).map(Some)
        }
        Intent::CloseShownReview { branch, allow_unapproved } => {
          let review = shown.get(branch).ok_or_else(|| no_shown_review(branch))?;
          Ok(Some(close_plan(review, branch, *allow_unapproved, &target)))
        }
        Intent::DeleteClosedShownReview { branch, number } => {
          let closed = closed_shown_of(facts.pull_requests.closed_shown(), branch, *number)?;
          delete_closed_plan(closed, shown.get(branch)).map(Some)
        }
        _ => Ok(None),
      }
    })
    .await;
  read.unwrap_or(Ok(None))
}

/// The closed shown review `number` of `branch` among `closed`, the last listing of them
///
/// # Errors
/// `no_shown_review` when the listing names none
pub fn closed_shown_of<'a>(closed: &'a [ClosedShownReview], branch: &str, number: u32) -> Result<&'a ClosedShownReview, ApiError> {
  closed
    .iter()
    .find(|review| review.branch == branch && review.number == number)
    .ok_or_else(|| ApiError::new(ErrorCode::NoShownReview, format!("{branch} has no shown review #{number} closed in the last days")))
}

/// "auth has no open shown review"
#[must_use]
pub fn no_shown_review(branch: &str) -> ApiError {
  ApiError::new(ErrorCode::NoShownReview, format!("{branch} has no open shown review"))
}

/// Runs the open, the extend, the close or the delete of the branches of a shown review (`intent`) in the session of `key` on the plan its caller
/// accepted: admitted (`shown_request`), planned again, refused by its first blocker, then its effects (`run_shown`) with
/// the GitHub service and repository `services` resolves; what it changed of the shown review goes into the session at
/// once. One that failed is told again at the next answer of the listing or the robots (`Sessions::retell_shown`)
///
/// # Errors
/// No open shown review of the branch, no merged Safe Push of it (an open), no fix due (an extend), one not approved (a
/// close), a changed plan
#[instrument(skip_all)]
pub async fn execute_shown(registry: &Registry, key: &SessionKey, intent: Intent, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let first = prepare_shown(sessions, key, &intent).await?;
  let number = match &intent {
    // its pull request is closed: the plan writes none
    Intent::DeleteClosedShownReview { number, .. } => Some(*number),
    _ => first.as_ref().and_then(|plan| plan.pull_request),
  };
  let request = shown_request(&key.prefix, &intent, first.as_ref());
  let answer = run(registry, key, request, events, async |guard| {
    let branch = intent.subject().unwrap_or_default();
    let plan = prepare_shown(sessions, key, &intent).await?.ok_or_else(|| no_shown_review(&branch))?;
    if let Some(refusal) = shown_refusal(&plan, &branch) {
      return Err(refusal);
    }
    check_admitted(guard, &accepted, &plan)?;
    let (github, repository) = github_of(services, key).await?;
    let baseline = sessions.read(key, |state, _| state.header().baseline_branch.clone()).await.unwrap_or_default();
    let creates = plan.effects.iter().any(|effect| matches!(effect, PlannedEffect::CreateShownReview { .. }));
    let text = if creates {
      Some(shown_text(&services.git(), &key.worktree, &key.prefix, &branch, baseline_on_remote(&baseline)).await)
    } else {
      None
    };
    let shown_services = ShownServices {
      git: services.git(),
      repository_path: key.worktree.clone(),
      prefix: key.prefix.clone(),
      remote: "origin".to_owned(),
      baseline,
      github,
      repository,
      text: text.clone(),
    };
    let done = run_shown(&shown_services, &plan).await;
    let all_done = done.records.iter().all(|record| record.status == EffectStatus::Done);
    let write = match &intent {
      // marked, or opened on what landed: the review it created, until the listing tells it
      Intent::MarkShownReview { .. } if all_done && done.created.is_some() => done.created.as_ref().map(|created| PullRequestWrite::Shown {
        branch: branch.clone(),
        review: Some(Box::new(created_review(created, &plan, text.as_ref()))),
      }),
      // closed: no longer an open shown review
      Intent::CloseShownReview { .. } if all_done => Some(PullRequestWrite::Shown {
        branch: branch.clone(),
        review: None,
      }),
      // opened or extended: as the run left it, until the listing tells it
      Intent::OpenShownReview { .. } | Intent::ExtendShownReview { .. } | Intent::MarkShownReview { .. } if all_done => sessions
        .read(key, {
          let branch = branch.clone();
          move |_, facts| facts.pull_requests.view().shown_reviews.get(&branch).cloned()
        })
        .await
        .flatten()
        .map(|review| PullRequestWrite::Shown {
          branch: branch.clone(),
          review: Some(Box::new(after_run(review, &plan, &done))),
        }),
      _ => None,
    };
    let version = match write {
      Some(write) => sessions.write_pull_request(key, write).await,
      None => None,
    };
    let result = done.requested.map_or(OperationResult::None, |requested| OperationResult::Reviewers(Box::new(requested)));
    Ok(outcome_of(guard.id().clone(), done.records, None, version, result))
  })
  .await;
  let failed = answer
    .as_ref()
    .map_or(true, |outcome| outcome.effects.iter().any(|record| matches!(record.status, EffectStatus::Failed { .. })));
  if let Some(number) = number.filter(|_| failed) {
    sessions.retell_shown(key, number).await;
  }
  answer
}

/// The text of a review after landing `branch` opens with (`CreateShownReview`): the description of its virtual branch
/// (`brd describe`), else its commits against `target` (`remote_facts::pull_request_create::new_pull_request`), as a
/// Safe Push with one opens it; of a branch that left the table, its name. Assigned to the user
pub async fn shown_text(git: &GitCommandExecutor, worktree: &str, prefix: &str, branch: &str, target: &str) -> ShownText {
  let read = if let Ok(head) = to_final_branch_name(prefix, branch) {
    let params = GetNewPullRequestParams {
      repository_path: worktree.to_owned(),
      head,
      base: target.to_owned(),
      stack_ancestors: Vec::new(),
    };
    new_pull_request(git, None, "origin", &params).await.ok()
  } else {
    None
  };
  ShownText {
    title: read.as_ref().map_or_else(|| branch.to_owned(), |text| text.title.clone()),
    body: read.map_or_default(|text| text.body),
    assign_self: true,
    target: target.to_owned(),
  }
}

/// The review after landing the run of `plan` created (`CreateShownReview`), as the listing will name it: its body as the
/// run wrote it of `text`, when GitHub opened it
fn created_review(created: &CreatedPullRequest, plan: &Plan, text: Option<&ShownText>) -> ShownReview {
  let reviewers = plan
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::CreateShownReview { reviewers, .. } => Some(reviewers.as_slice()),
      _ => None,
    })
    .unwrap_or_default();
  let body = text.map_or_else(|| reviewers_record(reviewers), |text| shown_body(&text.body, reviewers, &text.target));
  ShownReview {
    created_at: created.created_at,
    ..shown_review(created.pull_request.clone(), &body)
  }
}

/// `review` as the run `done` of `plan` left it: its head where the push or the extend took it, ready once marked, the
/// reviewers it asked waiting, the records of its body as written (`domain_policy::shown::review_state` of them; the
/// session settles it with the robots)
#[must_use]
pub fn after_run(mut review: ShownReview, plan: &Plan, done: &ShownRun) -> ShownReview {
  for (effect, record) in plan.effects.iter().zip(&done.records) {
    if record.status != EffectStatus::Done {
      break;
    }
    match effect {
      PlannedEffect::PushReview { tip, .. } => {
        tip.clone_into(&mut review.pull_request.head_ref_oid);
        review.base_missing = false;
      }
      PlannedEffect::MarkReady { .. } => review.pull_request.is_draft = false,
      PlannedEffect::RequestReviewers { reviewers, .. } => {
        for login in reviewers {
          let reviewers = &mut review.pull_request.reviewers;
          match reviewers.iter_mut().find(|reviewer| reviewer.login.eq_ignore_ascii_case(login)) {
            Some(reviewer) => reviewer.requested = true,
            None => reviewers.push(Reviewer {
              login: login.clone(),
              state: ReviewerState::Requested,
              requested: true,
            }),
          }
        }
      }
      PlannedEffect::ExtendReview { .. } => {
        if let Some(extension) = &done.extended {
          extension.head.clone_into(&mut review.pull_request.head_ref_oid);
        }
      }
      PlannedEffect::RecordShown { reviewers, landed, .. } => {
        review.reviewers.clone_from(reviewers);
        review.landed.clone_from(landed);
      }
      _ => {}
    }
  }
  review.state = review_state(&review.pull_request, &review.reviewers);
  review
}

#[cfg(test)]
#[path = "shown_test.rs"]
mod shown_test;
