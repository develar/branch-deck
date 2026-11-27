//! Show, the review after landing of ship/show/ask (docs/design/landing.md, "Show: a review after landing";
//! openspec/specs/branch/shown-reviews/spec.md): a pull request of the author from `{prefix}/review/<branch>/head` into
//! `{prefix}/review/<branch>/base`, the fork point of what landed, never master, so it can never land.
//!
//! - `is_shown`: a pull request is a shown review by its base, of anyone's prefix: a reviewer tells it so, and neither
//!   its Safe Push nor a merge after an approval is offered (`ActionBlocker::Shown`);
//! - `shown_title` and `listed_title`: its title on GitHub starts with `[post-merge review]` (`SHOWN_TITLE_PREFIX`), once,
//!   so that a reviewer tells it in any list of GitHub; the page shows the title without it, beside its badge;
//! - `review_state`: what it waits for, by the latest review of each reviewer it asked (the record of its body,
//!   `recorded_reviewers`): approved once every one of them approved and none asks for changes again; GitHub's
//!   `reviewDecision` decides nothing (it approves after the first approval, and counts a bot that requests changes),
//!   nor does a bot or someone not asked;
//! - `show_candidates`: whom it asks when the user names nobody: one available member of each owner group of the changed
//!   paths, from the reviewers of the group when it names any, never the author, never one the rotation passes by;
//! - `close_refusal`: a close of one not approved is refused, unless the user says so;
//! - `landed_head`: what landed the branch since its review opened, and the head the review opens on (one source in this
//!   order, each only if not older than the review, `ShownReview::created_at`): a merged Safe Push of the branch (also a
//!   safe-merge someone started from its pull request), the merged pull request of the branch into the target once the
//!   baseline has the branch whole (a safe-merge of GitHub), the baseline that has the branch whole by content. A draft
//!   opened ahead of time (a mark, `Intent::MarkShownReview`) so opens by whatever lands the branch, never by an older
//!   robot or pull request of the same name; its body names the merged pull request where it was reviewed before;
//! - `mark_refusals`: a mark of the review after landing ahead of time (`Intent::MarkShownReview`, a draft before the
//!   branch lands, its reviewers recorded) is refused as a Safe Push with it is (`mark_land`), without what refuses its
//!   robot, which starts later; approvals the code owners give first refuse it only without an open pull request, where
//!   they come before the landing;
//! - `pending_fixes` and `state_now`: a fix of the branch lands by a Safe Push of its own after the review opened; once
//!   its robot merged the review takes its commits on its head (`Intent::ExtendShownReview`), and the record of its body
//!   names its source revision (`recorded_landed`). While that robot runs, and until the review took it, the review
//!   waits for the fix (`ShownState::Extending`): neither the author's move nor approved;
//! - `closed_elsewhere`: a review closed outside Branch Deck (on GitHub, by anyone) leaves its branches, which its close
//!   would delete: an hour after its close, while no open shown review of the branch is listed, those the remote still
//!   has at the tips of its pull request are deleted (`Intent::DeleteClosedShownReview`); a branch gone or moved since
//!   stays (a newer review of the branch took the names);
//! - `fix_conflict_comment` and `fix_taken_comment`: the comment of a fix that doesn't apply on the commits of the
//!   review, of one whose commits took the files they conflict in from what landed them;
//! - `shown_news`: what the app does of them, each told once by the session: a draft whose branch landed since it opened
//!   opens (`landed_head`), a fix that landed extends, an approved one closes, one closed elsewhere loses its branches.

use crate::readiness::is_bot;
use domain_types::branch_names::is_review_base;
use domain_types::code_owners::OwnerGroup;
use domain_types::landing::{FactWait, Land, LandFact, ProfileChoice, ShowRequest, Subject};
use domain_types::patronus::{LandingAllowances, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{
  ClosedShownReview, PullRequest, PullRequestState, ReviewerState, SHOWN_LANDED_MARKER, SHOWN_REVIEWERS_MARKER, SHOWN_TITLE_PREFIX, ShownReview, ShownState,
};
use domain_types::reason::Reason;
use domain_types::session::{LandedSection, NewsEvent};
use std::collections::BTreeMap;

/// The pull request whose base is `base_ref_name` is a shown review (of anyone's prefix): its change is in master
/// already, and a review of it gates nothing
#[must_use]
pub fn is_shown(base_ref_name: &str) -> bool {
  is_review_base(base_ref_name)
}

/// The title of a shown review on GitHub: `SHOWN_TITLE_PREFIX`, then `title`, the title of its branch; one that starts
/// with the prefix already (in any case) stays as it is
#[must_use]
pub fn shown_title(title: &str) -> String {
  if has_shown_prefix(title) {
    title.to_owned()
  } else {
    format!("{SHOWN_TITLE_PREFIX} {}", title.trim_start())
  }
}

/// `title` starts with `SHOWN_TITLE_PREFIX`, in any case
#[must_use]
pub fn has_shown_prefix(title: &str) -> bool {
  title
    .trim_start()
    .get(..SHOWN_TITLE_PREFIX.len())
    .is_some_and(|start| start.eq_ignore_ascii_case(SHOWN_TITLE_PREFIX))
}

/// The title of the pull request `title` into `base_ref_name` as the page shows it: of a shown review without
/// `SHOWN_TITLE_PREFIX`, which its badge says (every reader of GitHub takes it so, before the issue key of the title is
/// split off); of any other the title as it is
#[must_use]
pub fn listed_title<'a>(title: &'a str, base_ref_name: &str) -> &'a str {
  if is_shown(base_ref_name) && has_shown_prefix(title) {
    title.trim_start()[SHOWN_TITLE_PREFIX.len()..].trim_start()
  } else {
    title
  }
}

/// The record of the body of a shown review: an HTML comment that GitHub doesn't show and that mentions nobody
#[must_use]
pub fn reviewers_record(reviewers: &[String]) -> String {
  format!("{SHOWN_REVIEWERS_MARKER} {} -->", reviewers.join(" "))
}

/// The reviewers the record of `body` names (`reviewers_record`); none without a record
#[must_use]
pub fn recorded_reviewers(body: &str) -> Vec<String> {
  recorded(body, SHOWN_REVIEWERS_MARKER)
}

/// The record of the fixes a shown review took, by the source revisions of their Safe Pushes
#[must_use]
pub fn landed_record(landed: &[String]) -> String {
  format!("{SHOWN_LANDED_MARKER} {} -->", landed.join(" "))
}

/// The source revisions the record of the fixes of `body` names (`landed_record`); none without a record
#[must_use]
pub fn recorded_landed(body: &str) -> Vec<String> {
  recorded(body, SHOWN_LANDED_MARKER)
}

/// The words of the record line of `marker` in `body`
fn recorded(body: &str, marker: &str) -> Vec<String> {
  body
    .lines()
    .find_map(|line| line.trim().strip_prefix(marker))
    .and_then(|rest| rest.trim().strip_suffix("-->"))
    .map_or_default(|words| words.split_whitespace().map(str::to_owned).collect())
}

/// `body` with its records naming `reviewers` and the fixes `landed` (none: no record of the fixes), at its end in place
/// of those it had; the rest of its text as it is, with the line of the pull request `reviewed_in` where the branch was
/// reviewed before it landed (`reviewed_before_line`) before them unless it says it already
#[must_use]
pub fn with_record(body: &str, reviewers: &[String], landed: &[String], reviewed_in: Option<u32>) -> String {
  let kept: Vec<&str> = body
    .lines()
    .filter(|line| {
      let line = line.trim();
      !line.starts_with(SHOWN_REVIEWERS_MARKER) && !line.starts_with(SHOWN_LANDED_MARKER)
    })
    .collect();
  let mut text = kept.join("\n").trim_end().to_owned();
  if let Some(line) = reviewed_in.map(reviewed_before_line).filter(|line| !text.contains(line.as_str())) {
    if !text.is_empty() {
      text.push_str("\n\n");
    }
    text.push_str(&line);
  }
  if !text.is_empty() {
    text.push_str("\n\n");
  }
  text.push_str(&reviewers_record(reviewers));
  if !landed.is_empty() {
    text.push('\n');
    text.push_str(&landed_record(landed));
  }
  text
}

/// What the shown review `pull_request` waits for, by the latest review of each of `reviewers`, those it asked (module
/// docs): a draft; changes requested by one of them that nobody asked again; approved by all of them; else in review,
/// waiting for those who have not approved yet (or were asked again after their approval). A bot and the author are no
/// reviewer of it
#[must_use]
pub fn review_state(pull_request: &PullRequest, reviewers: &[String]) -> ShownState {
  if pull_request.is_draft {
    return ShownState::Draft { landing: false };
  }
  let author = pull_request.author.as_deref().unwrap_or_default();
  let (mut waiting, mut changes, mut approved) = (Vec::new(), Vec::new(), Vec::new());
  for login in reviewers.iter().filter(|login| !is_bot(login) && !login.eq_ignore_ascii_case(author)) {
    let review = pull_request.reviewers.iter().find(|reviewer| reviewer.login.eq_ignore_ascii_case(login));
    match review {
      Some(review) if review.requested => waiting.push(login.clone()),
      Some(review) if review.state == ReviewerState::ChangesRequested => changes.push(login.clone()),
      Some(review) if review.state == ReviewerState::Approved => approved.push(login.clone()),
      _ => waiting.push(login.clone()),
    }
  }
  if !changes.is_empty() {
    ShownState::ChangesRequested { by: changes }
  } else if waiting.is_empty() && !approved.is_empty() {
    ShownState::Approved { by: approved }
  } else {
    ShownState::InReview { waiting }
  }
}

/// The shown review of `pull_request`, whose body records `body`'s reviewers, with what it waits for
#[must_use]
pub fn shown_review(pull_request: PullRequest, body: &str) -> ShownReview {
  let reviewers = recorded_reviewers(body);
  let state = review_state(&pull_request, &reviewers);
  ShownReview {
    pull_request,
    reviewers,
    landed: recorded_landed(body),
    state,
    base_missing: false,
    created_at: 0,
    prefix_missing: false,
  }
}

/// Whom a review after landing asks when the user names nobody (`show_candidates`)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShowCandidates {
  /// One GitHub login per owner group, each once, in the order of the groups
  pub reviewers: Vec<String>,
  /// The groups none of whose members or reviewers can be asked: none available with a GitHub login but the author
  pub without_reviewer: Vec<String>,
}

/// One reviewer of each of `groups`, the owner groups of the changed paths, for a review after landing by `author` (a
/// GitHub login): from the reviewers of the group when it names any, else from its members; available, not passed by the
/// rotation, with a GitHub login, not the author. One chosen for a group covers every other group that has them too
#[must_use]
pub fn show_candidates(groups: &[OwnerGroup], author: &str) -> ShowCandidates {
  let mut candidates = ShowCandidates::default();
  for group in groups {
    let pool = if group.reviewers.is_empty() { &group.members } else { &group.reviewers };
    let eligible: Vec<&str> = pool
      .iter()
      .filter(|member| member.available && !member.rotated)
      .filter_map(|member| member.github_login.as_deref())
      .filter(|login| !login.is_empty() && !login.eq_ignore_ascii_case(author))
      .collect();
    if eligible.iter().any(|login| candidates.reviewers.iter().any(|chosen| chosen.eq_ignore_ascii_case(login))) {
      continue;
    }
    match eligible.first() {
      Some(login) => candidates.reviewers.push((*login).to_owned()),
      None => candidates.without_reviewer.push(group.name.clone()),
    }
  }
  candidates
}

/// Why a close of `review` is refused: a reviewer it asked has not approved it, unless `allow_unapproved` (the user's
/// word); `None` when it may close
#[must_use]
pub fn close_refusal(review: &ShownReview, allow_unapproved: bool) -> Option<Reason> {
  if allow_unapproved {
    return None;
  }
  let waiting = match &review.state {
    ShownState::Approved { .. } => return None,
    ShownState::Draft { .. } | ShownState::Extending { .. } => review.reviewers.clone(),
    ShownState::InReview { waiting } => waiting.clone(),
    ShownState::ChangesRequested { by } => by.clone(),
  };
  Some(Reason::ReviewNotApproved {
    number: review.pull_request.number,
    waiting,
  })
}

/// The comment of the close of an approved shown review: "Reviewed after landing: approved by alice, bob. The change is in
/// master."; of one closed on the user's word, without the approvals: "Closed without the approval of carol. The change is
/// in master."; of a draft (Cancel Review After Landing… of a mark), whose change may not be in master: "Canceled: this
/// review after landing won't open."
#[must_use]
pub fn close_comment(review: &ShownReview, target: &str) -> String {
  match &review.state {
    ShownState::Draft { .. } => "Canceled: this review after landing won't open.".to_owned(),
    ShownState::Approved { by } => format!("Reviewed after landing: approved by {}. The change is in {target}.", by.join(", ")),
    ShownState::InReview { waiting } if !waiting.is_empty() => format!("Closed without the approval of {}. The change is in {target}.", waiting.join(", ")),
    ShownState::ChangesRequested { by } => format!("Closed without the approval of {}. The change is in {target}.", by.join(", ")),
    ShownState::Extending { .. } => format!("Closed before the review of the fix that lands. The change is in {target}."),
    ShownState::InReview { .. } => format!("Closed without a review. The change is in {target}."),
  }
}

/// A Safe Push of `branch` that merged, with the commit it took: not a Dry Run
fn merged_of(robot: &PatronusRobot, branch: &str) -> bool {
  robot.status == PatronusRobotStatus::Success && robot.push_mode != PatronusPushMode::DryRun && robot.branch_name.as_deref() == Some(branch) && robot.source_revision.is_some()
}

/// The merged Safe Push that landed `branch`: the newest successful robot of it that pushed, with the commit it took
#[must_use]
pub fn landing_robot<'a>(robots: impl IntoIterator<Item = &'a PatronusRobot>, branch: &str) -> Option<&'a PatronusRobot> {
  robots.into_iter().filter(|robot| merged_of(robot, branch)).max_by_key(|robot| robot.start_time)
}

/// The Safe Push of `branch` that runs now (not a Dry Run, not canceling): of a review open already, a fix of it
#[must_use]
pub fn running_fix<'a>(robots: impl IntoIterator<Item = &'a PatronusRobot>, branch: &str) -> Option<&'a PatronusRobot> {
  robots
    .into_iter()
    .filter(|robot| robot.status == PatronusRobotStatus::Running && robot.push_mode != PatronusPushMode::DryRun && robot.branch_name.as_deref() == Some(branch))
    .max_by_key(|robot| robot.start_time)
}

/// The merged Safe Pushes of `branch` not older than the open shown review `review` whose commits it has not taken yet,
/// oldest first: the ones after the newest it shows (its head is the commit that robot took, or the record of its body names it), all of
/// them when it shows none of those listed (the robot that landed the branch first is older than the listing, or it
/// opened on the start tip, `open_plan` without a robot); none of a draft
#[must_use]
pub fn pending_fixes<'a>(review: &ShownReview, branch: &str, robots: impl IntoIterator<Item = &'a PatronusRobot>) -> Vec<&'a PatronusRobot> {
  if review.pull_request.is_draft {
    return Vec::new();
  }
  // a robot older than the review (of an earlier branch of the name, the one a review created after it shows) is no fix
  let mut merged: Vec<&PatronusRobot> = robots
    .into_iter()
    .filter(|robot| merged_of(robot, branch) && robot.start_time >= review.created_at)
    .collect();
  merged.sort_by_key(|robot| robot.start_time);
  let head = review.pull_request.head_ref_oid.as_str();
  let shows = |robot: &&PatronusRobot| {
    robot
      .source_revision
      .as_deref()
      .is_some_and(|source| source == head || review.landed.iter().any(|landed| landed == source))
  };
  let after = merged.iter().rposition(shows).map_or(0, |index| index + 1);
  merged.split_off(after)
}

/// The Safe Push `robot` of the branch of the open shown review `review` lands a fix of it: the review opened (not a
/// draft, whose own robot lands the branch) and neither shows the commit it took nor records it as taken
#[must_use]
pub fn extends(review: &ShownReview, robot: &PatronusRobot) -> bool {
  let head = review.pull_request.head_ref_oid.as_str();
  !review.pull_request.is_draft
    && robot.push_mode != PatronusPushMode::DryRun
    && robot
      .source_revision
      .as_deref()
      .is_none_or(|source| source != head && !review.landed.iter().any(|landed| landed == source))
}

/// What `review` of `branch` waits for now, with the robots of the branch (module docs): a draft stays one, landing while
/// a Safe Push of the branch runs; a fix that lands (its robot runs, or merged and not taken yet) is waited for; else
/// what its reviews say (`review_state`)
#[must_use]
pub fn state_now<'a>(review: &ShownReview, branch: &str, robots: impl IntoIterator<Item = &'a PatronusRobot> + Clone) -> ShownState {
  if review.pull_request.is_draft {
    return ShownState::Draft {
      landing: running_fix(robots, branch).is_some(),
    };
  }
  if let Some(fix) = pending_fixes(review, branch, robots.clone()).last().copied().or_else(|| running_fix(robots, branch)) {
    return ShownState::Extending { robot: fix.id.clone() };
  }
  review_state(&review.pull_request, &review.reviewers)
}

/// The branches the fetched baseline landed whole by content (`landing`, then those an update dropped and the user has
/// not dismissed), each with when Branch Deck found it there (Unix seconds): a draft of one of them opens by it while no
/// robot of it is listed (`landed_head`)
#[must_use]
pub fn landed_by_content(section: &LandedSection) -> BTreeMap<String, u32> {
  let mut landed: BTreeMap<String, u32> = BTreeMap::new();
  for branch in section
    .landing
    .iter()
    .chain(&section.landed)
    .filter(|branch| branch.remaining == 0 && !branch.commits.is_empty())
  {
    if let Some(name) = &branch.name {
      let at = landed.entry(name.clone()).or_insert(branch.landed_at);
      *at = (*at).max(branch.landed_at);
    }
  }
  landed
}

/// What landed the branch of a shown review (`landed_head`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LandedBy {
  /// The merged Safe Push `robot` of the branch: its own, or a safe-merge someone started from its pull request
  Robot { robot: String },
  /// The merged pull request `number` of the branch into the target, the baseline having the branch whole: a safe-merge
  /// of GitHub whose robot is not listed
  PullRequest { number: u32 },
  /// The fetched baseline has the branch whole by content, while no robot of it is listed
  Content,
}

/// What landed the branch of a shown review since it opened, and where its head goes at its open (`landed_head`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandedHead {
  pub by: LandedBy,
  /// The commit the head of the review goes to: the one the robot took, the head of the merged pull request, the tip of
  /// the branch that landed by content; `None`: the head it has
  pub head: Option<String>,
  /// The merged pull request of the branch into the target, where it was reviewed before it landed: its body says so at
  /// the open ("Reviewed before landing in #7")
  pub reviewed_in: Option<u32>,
}

/// A landing of the branch by content, as `landed_head` reads it
#[derive(Clone, Copy, Debug)]
pub struct ContentLanding<'a> {
  /// When Branch Deck found the branch whole in the baseline (`landed_by_content`)
  pub landed_at: u32,
  /// The tip of the branch: its virtual branch, else its remote branch; `None` where neither is known
  pub tip: Option<&'a str>,
}

/// What landed `branch` since its shown review `review` opened (`ShownReview::created_at`; with no review, any time: a
/// review created after the landing), in this order (module docs): a merged Safe Push of the branch that finished since,
/// its head the commit the robot took; the merged pull request `own` of the branch into `target` closed since, while
/// the baseline has the branch whole (`content`), its head the head of the pull request; the baseline that has the
/// branch whole by content since, no robot of the branch running, its head the tip of the branch. `None` while nothing
/// landed it since
#[must_use]
pub fn landed_head<'a>(
  review: Option<&ShownReview>,
  branch: &str,
  robots: impl IntoIterator<Item = &'a PatronusRobot> + Clone,
  own: Option<&PullRequest>,
  target: &str,
  content: Option<ContentLanding<'_>>,
) -> Option<LandedHead> {
  let since = review.map_or(0, |review| review.created_at);
  let merged =
    own.filter(|pull_request| pull_request.state == PullRequestState::Merged && pull_request.base_ref_name == target && pull_request.closed_at.is_some_and(|at| at >= since));
  let reviewed_in = merged.map(|pull_request| pull_request.number);
  let robot = robots
    .clone()
    .into_iter()
    .filter(|robot| merged_of(robot, branch) && robot.finish_time.unwrap_or(robot.start_time) >= since)
    .max_by_key(|robot| robot.start_time);
  if let Some(robot) = robot {
    return Some(LandedHead {
      by: LandedBy::Robot { robot: robot.id.clone() },
      head: robot.source_revision.clone(),
      reviewed_in,
    });
  }
  if let (Some(pull_request), Some(_)) = (merged, content) {
    return Some(LandedHead {
      by: LandedBy::PullRequest { number: pull_request.number },
      head: Some(pull_request.head_ref_oid.clone()).filter(|head| !head.is_empty()),
      reviewed_in,
    });
  }
  let content = content.filter(|content| content.landed_at >= since && running_fix(robots, branch).is_none())?;
  Some(LandedHead {
    by: LandedBy::Content,
    head: content.tip.map(str::to_owned),
    reviewed_in,
  })
}

/// The Safe Push with a review after landing a mark of it ahead of time is decided as (`mark_refusals`): of the branch,
/// its reviewers `reviewers`
#[must_use]
pub fn mark_land(branch: &str, reviewers: &[String]) -> Land {
  Land {
    subject: Subject::branch(branch),
    mode: PatronusPushMode::RebaseAutosquash,
    profile: ProfileChoice::Default,
    without_approvals: false,
    create_pull_request: false,
    allow: LandingAllowances::default(),
    anyway: None,
    request_review: Some(ShowRequest { reviewers: reviewers.to_vec() }),
  }
}

/// What refuses a mark of the review after landing of a branch ahead of time (`Intent::MarkShownReview`), of the
/// blockers and the waits of the decision of the Safe Push with that review (`mark_land`): what refuses a review after
/// landing of it (more than the branch to land, a repository Patronus doesn't land, its review after landing open
/// already; nobody to ask is the mark's own, of the reviewers it is given in the end) and approvals the code owners must
/// give before it lands, unless its pull request is open (`pull_request_open`: they come there, before the landing the
/// mark follows); nothing of the robot, which starts later; and the facts it waits for to tell them (the listing, the
/// review of the branch)
#[must_use]
pub fn mark_refusals(blockers: &[Reason], waits: &[FactWait], pull_request_open: bool) -> (Vec<Reason>, Vec<FactWait>) {
  let blockers = blockers
    .iter()
    .filter(|reason| match reason {
      Reason::ShowLandsStack { .. } | Reason::ShowNeedsPatronus | Reason::ShowReviewOpen { .. } => true,
      Reason::ShowNeedsApprovals { .. } => !pull_request_open,
      _ => false,
    })
    .cloned()
    .collect();
  let waits = waits
    .iter()
    .filter(|wait| matches!(wait.fact, LandFact::PullRequests | LandFact::Readiness))
    .cloned()
    .collect();
  (blockers, waits)
}

/// The line of the body of a review after landing whose branch was reviewed before it landed, in its merged pull request
/// `number`: "Reviewed before landing in #7."
#[must_use]
pub fn reviewed_before_line(number: u32) -> String {
  format!("Reviewed before landing in #{number}.")
}

/// A close elsewhere waits this long (Unix seconds) before its branches go: a review closed by mistake is reopened
/// first, and Branch Deck's own close deletes them itself
pub const CLOSED_ELSEWHERE_DELAY: u32 = 3600;

/// The tips of the branches of the closed shown review `closed` the remote still has at the tips of its pull request,
/// head and base, which its delete takes (`None`: gone or moved, that branch stays); `None` when neither is left
#[must_use]
pub fn closed_remains(closed: &ClosedShownReview) -> Option<(Option<String>, Option<String>)> {
  let at = |tip: &Option<String>, oid: &str| tip.as_deref().filter(|tip| !oid.is_empty() && *tip == oid).map(str::to_owned);
  let (head, base) = (at(&closed.head_tip, &closed.head_ref_oid), at(&closed.base_tip, &closed.base_ref_oid));
  (head.is_some() || base.is_some()).then_some((head, base))
}

/// The closed shown reviews of `closed` whose branches go at `now` (Unix seconds; module docs): closed at least
/// `CLOSED_ELSEWHERE_DELAY` ago, a branch of it still at the tip of its pull request (`closed_remains`), and no open shown
/// review of the branch listed in `open`; of several of one branch (their branches have the same names) the newest
/// close, in the order of the branches
#[must_use]
pub fn closed_elsewhere<'a>(closed: &'a [ClosedShownReview], open: &BTreeMap<String, ShownReview>, now: u32) -> Vec<&'a ClosedShownReview> {
  let mut newest: BTreeMap<&str, &ClosedShownReview> = BTreeMap::new();
  let due = closed
    .iter()
    .filter(|review| now >= review.closed_at.saturating_add(CLOSED_ELSEWHERE_DELAY))
    .filter(|review| !open.contains_key(&review.branch) && closed_remains(review).is_some());
  for review in due {
    let kept = newest.entry(&review.branch).or_insert(review);
    if (review.closed_at, review.number) > (kept.closed_at, kept.number) {
      *kept = review;
    }
  }
  newest.into_values().collect()
}

/// What the app does of the shown reviews `shown` (by branch) over `robots`, the user's robots, `pull_requests`, the
/// pull requests of the branches (into `target`), and `landed`, the branches the baseline landed whole by content while
/// the robots are known (`landed_by_content`): a draft whose branch something landed since it opened (`landed_head`)
/// opens (`ShownReviewLanded`); one a fix
/// landed after it opened extends (`ShownReviewFixLanded`, the newest such robot); one every reviewer it asked approved
/// and no fix lands for closes (`ShownReviewApproved`); one of `closed` closed elsewhere loses its branches at `now`
/// (`ShownReviewClosedElsewhere`, `closed_elsewhere`). The session tells each once
#[must_use]
pub fn shown_news<'a>(
  shown: &BTreeMap<String, ShownReview>,
  robots: &(impl IntoIterator<Item = &'a PatronusRobot> + Clone),
  pull_requests: (&BTreeMap<String, PullRequest>, &str),
  landed: &BTreeMap<String, u32>,
  closed: &[ClosedShownReview],
  now: u32,
) -> Vec<NewsEvent> {
  let (own, target) = pull_requests;
  let open = shown.iter().filter_map(|(branch, review)| {
    let (branch, number) = (branch.clone(), review.pull_request.number);
    match state_now(review, &branch, robots.clone()) {
      ShownState::Draft { .. } => {
        let content = landed.get(&branch).map(|at| ContentLanding { landed_at: *at, tip: None });
        landed_head(Some(review), &branch, robots.clone(), own.get(&branch), target, content).map(|_| NewsEvent::ShownReviewLanded { branch, number })
      }
      ShownState::Extending { .. } => {
        let fix = pending_fixes(review, &branch, robots.clone()).last().map(|robot| robot.id.clone())?;
        Some(NewsEvent::ShownReviewFixLanded { branch, number, robot_id: fix })
      }
      ShownState::Approved { .. } => Some(NewsEvent::ShownReviewApproved { branch, number }),
      ShownState::InReview { .. } | ShownState::ChangesRequested { .. } => None,
    }
  });
  let closed = closed_elsewhere(closed, shown, now).into_iter().map(|review| NewsEvent::ShownReviewClosedElsewhere {
    branch: review.branch.clone(),
    number: review.number,
  });
  open.chain(closed).collect()
}

/// The comment of a fix that doesn't apply on the commits of a shown review: "Landed a follow-up as 1a2b3c4d5e; it does
/// not apply on this review's commits: <the robot page>"
#[must_use]
pub fn fix_conflict_comment(source: &str, url: &str) -> String {
  format!("Landed a follow-up as {}; it does not apply on this review's commits: {url}", short_id(source))
}

/// The comment of a fix whose commits conflicted on those of a shown review and took the files git cannot merge, `files`,
/// from the commits that landed them on `target`: "Landed a follow-up as 1a2b3c4d5e; `auth.rs` also shows what master
/// changed in it after this review: <the robot page>"; up to four files named, from five on the first three and the count
#[must_use]
pub fn fix_taken_comment(source: &str, files: &[String], url: &str, target: &str) -> String {
  let quoted: Vec<String> = files.iter().map(|file| format!("`{file}`")).collect();
  let named = match quoted.as_slice() {
    [] => String::new(),
    [only] => only.clone(),
    all @ [_, _, _, _, _, ..] => format!("{} and {} more", all[..NAMED_FILES].join(", "), all.len() - NAMED_FILES),
    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
  };
  let (shows, them) = if files.len() == 1 { ("shows", "it") } else { ("show", "them") };
  format!(
    "Landed a follow-up as {}; {named} also {shows} what {target} changed in {them} after this review: {url}",
    short_id(source)
  )
}

/// The files a comment of a fix names before the count of the rest, from five on
const NAMED_FILES: usize = 3;

/// The ten first characters of a commit id
fn short_id(id: &str) -> &str {
  id.get(..10).unwrap_or(id)
}

#[cfg(test)]
#[path = "shown_test.rs"]
mod shown_test;
