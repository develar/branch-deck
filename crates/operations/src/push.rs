//! The push of the virtual branches, one core for the app and `bd` (docs/design/git-operations.md, "Push and land";
//! docs/design/rust-domain-authority.md, "Конкурентность"): the repository lock is held only while the witness of the
//! plan is read in full and its commits and leases are fixed (`PreparedPush`: `<commit>:refs/heads/<name>`, each leased
//! on the remote head the user saw), never across the network. After it, `send` pushes exactly those commits in one
//! push, and a remote branch that moved meanwhile without commits a branch lacks takes the push once more over its new
//! head, the same commits (D4, `PreparedPush::over_moved`).
//!
//! The push of a branch alone and of an unapplied branch is the intent `Push` (`prepare_push`, `execute_push`): its plan
//! (`push_planned`) retargets the pull request of the branch out of a virtual branch first (D3a), cancels the Safe Push
//! of the branch it restarts (`restart_running`), pushes while the branch has work for the remote, asks first for a
//! push that drops remote commits the branch lacks (`Confirmation::ForcePushDrops`, unless the intent forces it) and is
//! refused by what keeps the branch out of a push (parked, held or landed by a Safe Push, not built); its run is the
//! run of a start (`land_execute::run_planned`): admitted per ref, after the sync of the session that runs, the robots
//! asked now (none for an unapplied branch, C3), the witness, the lock only for the freeze, then the effects.
//!
//! The pushes of a stack are the same intent (`stack_planned`): Push Stack and the push button (`WithAncestors`), Push
//! with Children, Push Up to Here (`UpTo`), in one atomic push of what `domain_policy::push` covers, and the stack
//! branch of a whole stack alone (`whole_stack_planned`). `bd push` plans one intent per branch it names over its
//! one-shot session, the same plans, and sends them as one atomic push (`push_together`) through the same run.

use crate::engine::{run, run_planned_in};
use crate::events::EventSink;
use crate::land_execute::{Accepted, Planned, PushLease, PushedRef, refusal_of};
use crate::land_session::push_start;
use crate::registry::{Answer, OperationRequest, Registry, Stepper};
use crate::witness::Witness;
use anyhow::Result;
use domain_policy::pull_request::base_of;
use domain_policy::push::{PushPlan as PolicyPlan, PushScope as PlanScope, PushState, Verdict, action_push, push_plan, verdict};
use domain_policy::reasons::stack_not_built;
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name, to_stack_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Confirmation, PlannedEffect, Subject};
use domain_types::operation::{Fingerprint, Intent, OperationStep, Plan, PullRequestField, Push, PushScope, Resource};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestBase, PullRequestState};
use domain_types::reason::Reason;
use domain_types::repository_view::PatronusView;
use domain_types::session::{StackBranchAlong, SyncReason, SyncState, stack_readiness_key};
use domain_types::{PushAction, RemoteStatusUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use remote_facts::service_sources::ServiceHost;
use repository_model::derive::{Holds, LandSources, PushRules, alone_push_of, up_to_push_of, with_pull_request_sources, with_push_states};
use repository_model::facts::FactStore;
use repository_model::push_guard::guard_facts;
use repository_model::session::{SessionKey, Sessions};
use repository_model::table::TableState;
use std::collections::BTreeMap;
use std::sync::Arc;
use sync_core::push::{LeasedPush, PUSH_REMOTE, PreparedPush};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// A push sent (`send`): the refs as they went, what the remote answered, and what a push again went over
#[derive(Debug)]
pub struct Sent {
  /// The refs of the push that answered `answer`: re-leased on the new heads when it went over a moved remote
  pub prepared: PreparedPush,
  pub answer: LeasedPush,
  /// The push went again over a moved remote: what the first push found
  pub over: Option<Over>,
}

/// What the refused first push of `Sent` found, which the push again went over
#[derive(Debug, Clone)]
pub struct Over {
  /// The refs whose remote had moved, indexes into the refs, parents first
  pub moved: Vec<usize>,
  /// The remote status of every ref after the fetch of the refusal, in their order
  pub statuses: Vec<RemoteStatusUpdate>,
}

impl Sent {
  /// The remote status the ref at `index` went over, when its remote had moved
  #[must_use]
  pub fn moved_to(&self, index: usize) -> Option<&RemoteStatusUpdate> {
    self.over.as_ref().filter(|over| over.moved.contains(&index)).and_then(|over| over.statuses.get(index))
  }
}

/// The network part of every push, after the repository lock: the frozen commits of `prepared` in one push, its
/// progress to `on_progress`; refused because remote branches moved without commits their branches lack, once more over
/// what the remote has now (`over`), the same commits: nothing of the remote is lost, whatever lease the user named
/// (D4). A dry run never goes again: the remote answers it `WouldReject`, and it fetches nothing. Blocking
///
/// # Errors
/// What git failed with
#[instrument(skip_all, fields(branches = prepared.branches().len()))]
pub fn send(git: &GitCommandExecutor, prepared: PreparedPush, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<Sent> {
  let answer = prepared.push_leased(git, on_progress)?;
  let again = match &answer {
    LeasedPush::RemoteMoved { moved, remote_statuses } => prepared.over_moved(&answer).map(|again| {
      let found = Over {
        moved: moved.clone(),
        statuses: remote_statuses.clone(),
      };
      (again, found)
    }),
    _ => None,
  };
  let Some((again, found)) = again else {
    return Ok(Sent { prepared, answer, over: None });
  };
  debug!(moved = ?found.moved, "a remote branch moved without commits its branch lacks: the push goes again over it");
  let answer = again.push_leased(git, on_progress)?;
  Ok(Sent {
    prepared: again,
    answer,
    over: Some(found),
  })
}

/// The plan of the push `push` in the open session of `key` over the facts it holds now (`push_planned`), the holds of
/// its Patronus view; `None` for a session that is not open or has no table yet, a branch its table lacks, an unapplied
/// branch it doesn't list, a commit no Push Up to Here is offered of (the remote has it, a merged Safe Push landed it, a
/// sync rebuilt the branch since), and a whole stack in another scope than alone
pub async fn prepare_push(sessions: &Sessions, key: &SessionKey, push: &Push) -> Option<Planned> {
  let (prefix, push) = (key.prefix.clone(), push.clone());
  sessions
    .read(key, move |state, facts| push_planned(state, facts, &prefix, &push, None, ExpectRemote::OfIntent))
    .await
    .flatten()
}

/// The remote head a push leases on and is witnessed with
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectRemote {
  /// The one the user saw (`Push::expect_remote`)
  OfIntent,
  /// The one the session has now: the push waited for a sync or another push, which brought what the remote has, and
  /// goes over it while it drops nothing the user didn't accept
  OfSession,
}

/// The robots a push is checked on, asked now, and the Patronus view of the table of the last sync derived of them
pub struct CheckedHolds {
  pub view: PatronusView,
  pub robots: Vec<PatronusRobot>,
}

impl CheckedHolds {
  const fn holds(&self) -> Holds<'_> {
    Holds {
      view: &self.view,
      robots: self.robots.as_slice(),
    }
  }
}

/// The plan of the push `push` over the table of `state` and the facts of `facts`, with what its run needs: of a branch
/// alone (its row, its preview, its review window, the toast of a remote that moved), of an unapplied branch, of a stack
/// (`stack_planned`: Push Stack and the push button, Push with Children, Push Up to Here) or of the stack branch of a
/// whole stack (`whole_stack_planned`); `checked` the holds a run of it checks (the robots asked for it now), else those
/// of the Patronus view of the session. `None` as `prepare_push`
#[must_use]
pub fn push_planned(state: &TableState, facts: &FactStore, prefix: &str, push: &Push, checked: Option<&CheckedHolds>, expect: ExpectRemote) -> Option<Planned> {
  if let (Subject::Branch { name }, PushScope::Unapplied) = (&push.subject, &push.scope) {
    return unapplied_planned(state, name, push, expect);
  }
  with_pull_request_sources(state, facts, prefix, |sources| {
    let holds = checked.map_or(
      Holds {
        view: sources.view,
        robots: sources.robots,
      },
      CheckedHolds::holds,
    );
    match (&push.subject, &push.scope) {
      (Subject::Branch { name }, PushScope::Alone) => alone_planned(state, facts, sources, holds, name, push, expect),
      (Subject::Branch { name }, PushScope::WithAncestors | PushScope::WithChildren | PushScope::UpTo { .. }) => stack_planned(state, facts, sources, holds, name, push, expect),
      (Subject::WholeStack { root }, PushScope::Alone) => whole_stack_planned(state, sources, holds, root, push, expect),
      (_, PushScope::Unapplied) | (Subject::WholeStack { .. }, _) => None,
    }
  })
  .flatten()
}

/// A branch a push of a stack sends (`covered`)
struct CoveredStep {
  name: String,
  /// Its push drops remote commits it lacks (`Verdict::AsksFirst`): `drops` of them
  asks: bool,
  drops: u32,
}

/// What a push of a stack covers by its scope (`covered`)
struct Covered {
  /// The branches it sends, parents first
  sends: Vec<CoveredStep>,
  /// What keeps it from going
  blockers: Vec<Reason>,
  /// The stack branch of the whole stack of the subject goes along, after the branches (Push with Children of a root)
  stack_branch: bool,
  /// The copy of a commit of the subject it pushes instead of its tip (Push Up to Here of a commit before the last one
  /// the remote lacks)
  up_to: Option<String>,
}

/// What the push `push` of a stack from `name` covers, by `domain_policy::push` over `rules` (the holds of the push):
///
/// - `WithAncestors`, the push button and an action: the branches it is built on that have work, root first, above the
///   nearest one whose push drops remote commits (that one asks first in its own row, `action_push`), then the branch;
///   forced (Push Stack, whose description named every drop), every one of them up to the nearest held one
///   (`push_plan`). What keeps the branch out refuses it
/// - `WithChildren`: the branch while it has work, then the branches stacked on it that have work, each after its
///   parent, and the stack branch of the whole stack of a root while it goes along (`stack_branch_along`); one stacked on
///   it whose push drops remote commits only when forced. What keeps the branch out refuses it while nothing else goes
/// - `UpTo`: the branches it is built on that have work, whole, then the branch up to the commit (`up_to_push`), which
///   must be one the view offers it of; `None` for another
fn covered(rules: &PushRules<'_>, state: &TableState, facts: &FactStore, holds: Holds<'_>, name: &str, push: &Push) -> Option<Covered> {
  let step = |name: &str, verdict: Verdict| CoveredStep {
    name: name.to_string(),
    asks: verdict == Verdict::AsksFirst,
    drops: rules.drops(name),
  };
  let goes = |verdict: Verdict| matches!(verdict, Verdict::Goes | Verdict::AsksFirst);
  let state_of = |branch: &str| rules.state(branch);
  match &push.scope {
    PushScope::WithAncestors => {
      let plan = push_plan(&rules.repo, state_of, name, PlanScope::WithAncestors);
      let (&(own_name, own), before) = plan.steps.split_last()?;
      let ancestors = if push.force {
        PolicyPlan { steps: before.to_vec() }.order(true)
      } else {
        action_push(&rules.repo, state_of, name)?.ancestors
      };
      let mut sends: Vec<CoveredStep> = ancestors.iter().map(|ancestor| step(ancestor, plan.verdict(ancestor).unwrap_or(Verdict::Goes))).collect();
      if goes(own) {
        sends.push(step(own_name, own));
      }
      Some(Covered {
        sends,
        blockers: rules.refusal(own_name, own).into_iter().collect(),
        stack_branch: false,
        up_to: None,
      })
    }
    PushScope::WithChildren => {
      let plan = push_plan(&rules.repo, state_of, name, PlanScope::WithChildren);
      let (&(own_name, own), children) = plan.steps.split_first()?;
      let mut sends = Vec::new();
      if goes(own) {
        sends.push(step(own_name, own));
      }
      let sent = |verdict: Verdict| verdict == Verdict::Goes || (push.force && verdict == Verdict::AsksFirst);
      sends.extend(children.iter().filter(|(_, verdict)| sent(*verdict)).map(|(child, verdict)| step(child, *verdict)));
      let stack_branch = rules.stack_branch_along(own_name) == Some(StackBranchAlong::Along);
      let blockers = if sends.is_empty() && !stack_branch {
        rules.refusal(own_name, own).into_iter().collect()
      } else {
        Vec::new()
      };
      Some(Covered {
        sends,
        blockers,
        stack_branch,
        up_to: None,
      })
    }
    PushScope::UpTo { commit } => {
      let up_to = up_to_push_of(state, facts, holds, name)?;
      if !up_to.commits.contains(commit) {
        return None;
      }
      let last = state.branch(name)?.remote.status.as_ref()?.unpushed_commits.last()?;
      let mut sends: Vec<CoveredStep> = up_to.ancestors.iter().map(|ancestor| step(ancestor, Verdict::Goes)).collect();
      sends.push(step(name, Verdict::Goes));
      Some(Covered {
        sends,
        blockers: up_to.blocker.into_iter().collect(),
        stack_branch: false,
        up_to: (last != commit).then(|| commit.clone()),
      })
    }
    PushScope::Alone | PushScope::Unapplied => None,
  }
}

/// The push `push` of a stack from the branch `name` over `sources` and the holds of `holds`, in one atomic push of what
/// its scope covers (`covered`): first the retarget of each open pull request of a branch it sends that goes into a
/// virtual branch (D3a), then the push; each branch it sends whose push drops remote commits asks first
/// (`Confirmation::ForcePushDrops`) unless the intent forces it. Each ref leases on the remote head the session has, the
/// branch on the one the user saw (`ExpectRemote::OfIntent`) until a wait brought a newer one. The drops it asks of are
/// counted on the head the session has: a run of an accepted drop names that head (the page: the one its row shows at
/// the click of the Force Push), or the remote refuses the lease
fn stack_planned(state: &TableState, facts: &FactStore, sources: &LandSources<'_>, holds: Holds<'_>, name: &str, push: &Push, expect: ExpectRemote) -> Option<Planned> {
  let full = |branch: &str| to_final_branch_name(sources.prefix, branch).ok();
  let full_name = full(name)?;
  sources.table.branch(name)?;
  let covered = with_push_states(state, facts, holds, |rules| covered(rules, state, facts, holds, name, push))?;
  let goes = covered.blockers.is_empty();
  let sends: &[CoveredStep] = if goes { &covered.sends } else { &[] };
  let mut effects = Vec::new();
  let mut retargeted = Vec::new();
  for sent in sends {
    if let Some(pull_request) = virtual_pull_request(sources, &sent.name) {
      effects.push(PlannedEffect::Retarget {
        branch: sent.name.clone(),
        number: pull_request.number,
        from: pull_request.base_ref_name.clone(),
        to: sources.target.to_string(),
      });
      retargeted.push(pull_request);
    }
  }
  let (mut refs, mut leases) = (BTreeMap::new(), BTreeMap::new());
  let mut pushes = Vec::new();
  for sent in sends {
    let sent_full = full(&sent.name)?;
    let remote = if sent.name == name && expect == ExpectRemote::OfIntent {
      push.expect_remote.clone()
    } else {
      sources.remote.branches.get(&sent.name).and_then(|remote| remote.remote_tip.clone())
    };
    refs.insert(format!("refs/heads/{sent_full}"), sources.table.tips.get(&sent_full).cloned());
    leases.insert(format!("refs/remotes/{PUSH_REMOTE}/{sent_full}"), remote.clone());
    pushes.push(PushedRef {
      up_to: if sent.name == name { covered.up_to.clone() } else { None },
      ..PushedRef::branch(sources.table, sources.prefix, &sent.name, PushLease::Planned(remote))?
    });
  }
  let mut names: Vec<String> = sends.iter().map(|sent| sent.name.clone()).collect();
  if goes && covered.stack_branch {
    let stack = sources.table.stack_branches.iter().find(|stack| stack.root == name)?;
    refs.insert(format!("refs/heads/{}", stack.branch_name), stack.tip.clone());
    let remote = sources.remote.stacks.get(name).and_then(|remote| remote.remote_tip.clone());
    leases.insert(format!("refs/remotes/{PUSH_REMOTE}/{}", stack.branch_name), remote.clone());
    pushes.push(PushedRef::stack_branch(stack, PushLease::Planned(remote)));
    names.push(stack_readiness_key(name));
  }
  if !pushes.is_empty() {
    effects.push(PlannedEffect::Push {
      branches: names,
      force: sends.iter().any(|sent| sent.asks),
    });
  }
  let confirmations = if push.force {
    Vec::new()
  } else {
    sends
      .iter()
      .filter(|sent| sent.asks)
      .map(|sent| Confirmation::ForcePushDrops {
        branch: sent.name.clone(),
        commits: sent.drops,
      })
      .collect()
  };
  let witness = Witness::of(refs, retargeted, std::iter::empty()).leasing(leases);
  Some(push_plan_of(
    push,
    witness,
    effects,
    confirmations,
    covered.blockers,
    pushes,
    format!("refs/heads/{full_name}"),
  ))
}

/// The push of the stack branch of the whole stack of `root` alone (the push a refused start of the whole stack offers):
/// the build of the last sync while its remote lacks it, leased on the remote head of its status, asking first when it
/// drops remote commits unless forced; while it has work, what holds its push refuses it (`StackLanding::hold`: a member
/// parked, every branch of the stack landed, its own robot runs on it, the sync did not build it), and a stack that does
/// not fork has none. Its remote status is the session's (a push since the sync left it)
fn whole_stack_planned(state: &TableState, sources: &LandSources<'_>, holds: Holds<'_>, root: &str, push: &Push, expect: ExpectRemote) -> Option<Planned> {
  sources.table.branch(root)?;
  let key = stack_readiness_key(root);
  let Some(stack) = sources.table.stack_branches.iter().find(|stack| stack.root == root && !stack.members.is_empty()) else {
    let blockers = vec![Reason::NoStackBranch { root: root.to_string() }];
    return Some(push_plan_of(push, Witness::default(), Vec::new(), Vec::new(), blockers, Vec::new(), String::new()));
  };
  let status = state.stack(root).and_then(|stack| stack.remote.as_ref());
  let action = status.map_or(PushAction::Create, |status| status.action);
  let verdict = verdict(&PushState { action: Some(action), kept: None });
  let source_ref = format!("refs/heads/{}", stack.branch_name);
  if !matches!(verdict, Verdict::Goes | Verdict::AsksFirst) {
    return Some(push_plan_of(push, Witness::default(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), source_ref));
  }
  let hold = holds.view.stacks.get(root).and_then(|facts| facts.landing.hold.clone());
  if let Some(reason) = hold.or_else(|| stack_not_built(stack)) {
    return Some(push_plan_of(push, Witness::default(), Vec::new(), Vec::new(), vec![reason], Vec::new(), source_ref));
  }
  let asks = verdict == Verdict::AsksFirst;
  let remote = match expect {
    ExpectRemote::OfIntent => push.expect_remote.clone(),
    ExpectRemote::OfSession => sources.remote.stacks.get(root).and_then(|remote| remote.remote_tip.clone()),
  };
  let refs = BTreeMap::from([(source_ref.clone(), stack.tip.clone())]);
  let leases = BTreeMap::from([(format!("refs/remotes/{PUSH_REMOTE}/{}", stack.branch_name), remote.clone())]);
  let confirmations = if asks && !push.force {
    vec![Confirmation::ForcePushDrops {
      branch: key.clone(),
      commits: status.map_or(0, |status| status.commits_behind),
    }]
  } else {
    Vec::new()
  };
  let effects = vec![PlannedEffect::Push { branches: vec![key], force: asks }];
  let witness = Witness::of(refs, std::iter::empty(), std::iter::empty()).leasing(leases);
  Some(push_plan_of(
    push,
    witness,
    effects,
    confirmations,
    Vec::new(),
    vec![PushedRef::stack_branch(stack, PushLease::Planned(remote))],
    source_ref,
  ))
}

/// The open pull request of `branch` that goes into a virtual branch: a push retargets it into the target first, where
/// every pull request of a virtual branch goes (D3a)
fn virtual_pull_request<'a>(sources: &LandSources<'a>, branch: &str) -> Option<&'a PullRequest> {
  sources
    .pull_requests
    .pull_requests
    .get(branch)
    .filter(|pull_request| !sources.target.is_empty() && pull_request.state == PullRequestState::Open)
    .filter(|pull_request| matches!(base_of(pull_request, sources.target, sources.prefix), PullRequestBase::Virtual { .. }))
}

/// The plan of a push of a stack with its witness, fingerprinted
fn push_plan_of(
  push: &Push,
  witness: Witness,
  effects: Vec<PlannedEffect>,
  confirmations: Vec<Confirmation>,
  blockers: Vec<Reason>,
  pushes: Vec<PushedRef>,
  source_ref: String,
) -> Planned {
  let mut plan = Plan {
    intent: Intent::Push(push.clone()),
    fingerprint: Fingerprint(String::new()),
    lands: Vec::new(),
    effects,
    confirmations,
    blockers,
    waits: Vec::new(),
    alternatives: None,
    pull_request: None,
    approvals_skipped_by: None,
  };
  plan.fingerprint = witness.fingerprint(&plan);
  Planned {
    plan,
    witness,
    pushes,
    source_ref,
    merge_request: None,
    notes: Vec::new(),
    subject: push.subject.clone(),
    mode: PatronusPushMode::RebaseAutosquash,
  }
}

/// The plans of several pushes as the plan of one atomic push (`bd push auth docs`, `--all`, Push with Children beside
/// Push Stack): the retargets of all of them first, then their cancels, the one push of every ref they send, then their
/// restarts, each effect once. The branches go in the order of `order` (the branches of the table, parents first), the
/// stack branches after them in the order of their roots, a ref two plans send once; the witness, the blockers and the
/// confirmations are those of all of them, the intent, the subject and the mode those of the first, fingerprinted anew.
/// `None` without a plan
#[must_use]
pub fn push_together(plans: Vec<Planned>, order: &[&str]) -> Option<Planned> {
  let mut plans = plans.into_iter();
  let mut together = plans.next()?;
  let rest: Vec<Planned> = plans.collect();
  let (mut retargets, mut cancels, mut restarts) = (Vec::new(), Vec::new(), Vec::new());
  let mut force = false;
  for planned in std::iter::once(&together).chain(&rest) {
    for effect in &planned.plan.effects {
      let kept = match effect {
        PlannedEffect::Retarget { number, .. } => {
          let retargeted = retargets.iter().any(|it| matches!(it, PlannedEffect::Retarget { number: kept, .. } if kept == number));
          (!retargeted).then_some(&mut retargets)
        }
        PlannedEffect::CancelRobot { .. } => (!cancels.contains(effect)).then_some(&mut cancels),
        PlannedEffect::RestartRobot { .. } => (!restarts.contains(effect)).then_some(&mut restarts),
        PlannedEffect::Push { force: forced, .. } => {
          force |= forced;
          None
        }
        _ => None,
      };
      if let Some(kept) = kept {
        kept.push(effect.clone());
      }
    }
  }
  let mut pushes: Vec<PushedRef> = Vec::new();
  for pushed in std::iter::once(&together).chain(&rest).flat_map(|planned| &planned.pushes) {
    if !pushes.iter().any(|it| it.full_name == pushed.full_name) {
      pushes.push(pushed.clone());
    }
  }
  let place = |name: &str| order.iter().position(|it| *it == name).unwrap_or(usize::MAX);
  pushes.sort_by_key(|pushed| (pushed.whole_stack, place(&pushed.name)));
  let mut effects = retargets;
  effects.extend(cancels);
  if !pushes.is_empty() {
    let branches = pushes
      .iter()
      .map(|pushed| if pushed.whole_stack { stack_readiness_key(&pushed.name) } else { pushed.name.clone() })
      .collect();
    effects.push(PlannedEffect::Push { branches, force });
  }
  effects.extend(restarts);
  for planned in rest {
    let Planned { plan, witness, .. } = planned;
    together.witness.refs.extend(witness.refs);
    together.witness.leases.extend(witness.leases);
    together.witness.pull_requests.extend(witness.pull_requests);
    together.witness.robots.extend(witness.robots);
    together.plan.blockers.extend(plan.blockers);
    for confirmation in plan.confirmations {
      if !together.plan.confirmations.contains(&confirmation) {
        together.plan.confirmations.push(confirmation);
      }
    }
  }
  together.plan.effects = effects;
  together.pushes = pushes;
  together.plan.fingerprint = together.witness.fingerprint(&together.plan);
  Some(together)
}

/// The push of the branch `name` alone over `sources` (`derive::alone_push_of`): the retarget of its open pull request
/// that goes into a virtual branch (D3a), the cancel of its Safe Push while it runs and the push restarts it, the push
/// while the branch has work for the remote, then the restart (`restart_running`). A push that drops remote commits the
/// branch lacks asks first (`Confirmation::ForcePushDrops`) unless the intent forces it; a branch that stays local, one a
/// running Safe Push holds (or a merged one landed whole) and one the last sync did not build refuse it
fn alone_planned(state: &TableState, facts: &FactStore, sources: &LandSources<'_>, holds: Holds<'_>, name: &str, push: &Push, expect: ExpectRemote) -> Option<Planned> {
  let full_name = to_final_branch_name(sources.prefix, name).ok()?;
  sources.table.branch(name)?;
  // the Safe Push of the branch the push restarts on its commit, canceled first while it runs
  let robot = if push.restart_running {
    let id = holds.view.branches.get(name).and_then(|facts| facts.robot_id.as_deref());
    id.and_then(|id| holds.robots.iter().find(|robot| robot.id == id))
  } else {
    None
  };
  // one canceling already (the cancel of a restart whose push the remote refused) is canceled once
  let running = robot.filter(|robot| robot.status == PatronusRobotStatus::Running);
  let alone = alone_push_of(state, facts, holds, name, running.map(|robot| robot.id.as_str()))?;
  let goes = matches!(alone.verdict, Verdict::Goes | Verdict::AsksFirst);
  let mut effects = Vec::new();
  // its pull request goes into the target first, where every pull request of a virtual branch goes (D3a)
  let retarget = virtual_pull_request(sources, name).filter(|_| goes);
  if let Some(pull_request) = retarget {
    effects.push(PlannedEffect::Retarget {
      branch: name.to_string(),
      number: pull_request.number,
      from: pull_request.base_ref_name.clone(),
      to: sources.target.to_string(),
    });
  }
  if let Some(robot) = running {
    effects.push(PlannedEffect::CancelRobot { robot_id: robot.id.clone() });
  }
  let mut pushed = Vec::new();
  if goes {
    effects.push(PlannedEffect::Push {
      branches: vec![name.to_string()],
      force: alone.verdict == Verdict::AsksFirst,
    });
    pushed.push(PushedRef::branch(sources.table, sources.prefix, name, PushLease::Frozen)?);
  }
  if let Some(robot) = robot {
    effects.push(PlannedEffect::RestartRobot { robot_id: robot.id.clone() });
  }
  let tip = sources.table.tips.get(&full_name).cloned();
  let expect_remote = match expect {
    ExpectRemote::OfIntent => push.expect_remote.clone(),
    ExpectRemote::OfSession => sources.remote.branches.get(name).and_then(|remote| remote.remote_tip.clone()),
  };
  let plan = PushPlan {
    push,
    expect_remote,
    effects,
    blockers: alone.blocker.into_iter().collect(),
    drops: alone.drops,
    pushed,
    tip,
    retargeted: retarget,
    robot,
  };
  Some(plan.planned(&full_name, name))
}

/// The push of the unapplied branch `name` (the branch it was) under its own name, so the remote keeps a copy: nothing
/// refuses it, no robot is read for it (C3), and its pull request stays as it was; it asks first only when it drops
/// remote commits and the intent doesn't force it
fn unapplied_planned(state: &TableState, name: &str, push: &Push, expect: ExpectRemote) -> Option<Planned> {
  let branch = state.sections().unapplied.iter().find(|branch| branch.branch == name)?;
  // no remote status: the remote has no such branch, the push creates it
  let action = branch.remote.as_ref().map_or(PushAction::Create, |status| status.action);
  let verdict = verdict(&PushState { action: Some(action), kept: None });
  let goes = matches!(verdict, Verdict::Goes | Verdict::AsksFirst);
  let pushed = goes
    .then(|| PushedRef {
      name: name.to_string(),
      full_name: branch.name.clone(),
      parent_ref: None,
      whole_stack: false,
      unapplied: true,
      up_to: None,
      lease: PushLease::Frozen,
      // the branch it was: no sync counts whose its commits are
      my_email: None,
    })
    .into_iter()
    .collect();
  let expect_remote = match expect {
    ExpectRemote::OfIntent => push.expect_remote.clone(),
    ExpectRemote::OfSession => branch.remote.as_ref().and_then(|status| status.remote_tip.clone()),
  };
  let plan = PushPlan {
    push,
    expect_remote,
    effects: goes
      .then(|| PlannedEffect::Push {
        branches: vec![name.to_string()],
        force: verdict == Verdict::AsksFirst,
      })
      .into_iter()
      .collect(),
    blockers: Vec::new(),
    drops: if verdict == Verdict::AsksFirst {
      branch.remote.as_ref().map_or(0, |status| status.commits_behind)
    } else {
      0
    },
    pushed,
    tip: Some(branch.tip.clone()),
    retargeted: None,
    robot: None,
  };
  Some(plan.planned(&branch.name, name))
}

/// What the plan of a push is made of, before its witness
struct PushPlan<'a> {
  push: &'a Push,
  /// The remote head it leases on (`ExpectRemote`)
  expect_remote: Option<String>,
  effects: Vec<PlannedEffect>,
  blockers: Vec<Reason>,
  /// The remote commits the push drops while it asks first
  drops: u32,
  pushed: Vec<PushedRef>,
  /// The tip of the branch as the table has it
  tip: Option<String>,
  retargeted: Option<&'a PullRequest>,
  /// The robot it restarts
  robot: Option<&'a PatronusRobot>,
}

impl PushPlan<'_> {
  /// The plan of the branch `name` (`full_name` on the remote) with its witness: the tip of the branch and the remote
  /// head it leases on (`expect_remote`) while it pushes (none of them else: a plan that moves and leases no ref takes
  /// no lock), the pull request it retargets and the robot it restarts
  fn planned(self, full_name: &str, name: &str) -> Planned {
    let force = self.effects.iter().any(|effect| matches!(effect, PlannedEffect::Push { force: true, .. }));
    let confirmations = if force && !self.push.force {
      vec![Confirmation::ForcePushDrops {
        branch: name.to_string(),
        commits: self.drops,
      }]
    } else {
      Vec::new()
    };
    let (refs, leases) = if self.pushed.is_empty() {
      (BTreeMap::new(), BTreeMap::new())
    } else {
      (
        BTreeMap::from([(format!("refs/heads/{full_name}"), self.tip)]),
        BTreeMap::from([(format!("refs/remotes/{PUSH_REMOTE}/{full_name}"), self.expect_remote.clone())]),
      )
    };
    let pushes = self
      .pushed
      .into_iter()
      .map(|pushed| PushedRef {
        lease: PushLease::Planned(self.expect_remote.clone()),
        ..pushed
      })
      .collect();
    let witness = Witness::of(refs, self.retargeted, self.robot).leasing(leases);
    let mut plan = Plan {
      intent: Intent::Push(self.push.clone()),
      fingerprint: Fingerprint(String::new()),
      lands: Vec::new(),
      effects: self.effects,
      confirmations,
      blockers: self.blockers,
      waits: Vec::new(),
      alternatives: None,
      pull_request: None,
      approvals_skipped_by: None,
    };
    plan.fingerprint = witness.fingerprint(&plan);
    Planned {
      plan,
      witness,
      pushes,
      source_ref: format!("refs/heads/{full_name}"),
      merge_request: None,
      notes: Vec::new(),
      subject: self.push.subject.clone(),
      mode: self.robot.map_or(PatronusPushMode::RebaseAutosquash, |robot| robot.push_mode),
    }
  }
}

/// What the push `push` asks the registry for: the refs its plan pushes now (of `planned`, its plan as the session has
/// it now), in its order, so the view tells which branches it sends; the ref of its subject while it pushes none (a sync
/// it waits for may give it work); the robot it cancels and restarts and the base of each pull request it retargets
fn push_request(prefix: &str, push: &Push, planned: Option<&Planned>) -> OperationRequest {
  let pushes = planned.map_or(&[][..], |planned| planned.pushes.as_slice());
  let mut resources: Vec<Resource> = pushes.iter().map(|pushed| Resource::Ref { name: pushed.full_name.clone() }).collect();
  if resources.is_empty() {
    let name = push.subject.name();
    let full_name = match (&push.subject, &push.scope) {
      (Subject::WholeStack { .. }, _) => to_stack_branch_name(prefix, name),
      (_, PushScope::Unapplied) => qualified_branch_name(prefix, BranchKind::Unapplied, name),
      _ => to_final_branch_name(prefix, name),
    };
    resources.extend(full_name.ok().map(|name| Resource::Ref { name }));
  }
  for effect in planned.map_or(&[][..], |planned| planned.plan.effects.as_slice()) {
    match effect {
      PlannedEffect::Retarget { number, .. } => resources.push(Resource::PullRequest {
        number: *number,
        field: PullRequestField::Base,
      }),
      PlannedEffect::CancelRobot { robot_id } | PlannedEffect::RestartRobot { robot_id } => {
        let robot = Resource::Robot { id: robot_id.clone() };
        if !resources.contains(&robot) {
          resources.push(robot);
        }
      }
      _ => {}
    }
  }
  OperationRequest {
    intent: Intent::Push(push.clone()),
    resources,
    step: OperationStep::Checking,
    moves: Vec::new(),
  }
}

/// The push has no plan in the session: "Nothing was pushed: the last sync has no auth"; of a commit no Push Up to Here
/// is offered of, a plan that changed (the remote has it now, or a sync rebuilt the branch and gave it another copy)
fn no_push_plan(push: &Push) -> ApiError {
  match &push.scope {
    PushScope::Unapplied => ApiError::new(
      ErrorCode::UnknownBranch,
      format!("Nothing was pushed: no unapplied branch {} is listed", push.subject.name()),
    ),
    PushScope::UpTo { commit } => ApiError::new(
      ErrorCode::PlanChanged,
      format!(
        "Nothing was pushed: {} is no commit of {} the remote lacks; sync, then push again",
        git_ops::revs::short_id(commit),
        push.subject.name()
      ),
    ),
    PushScope::Alone | PushScope::WithAncestors | PushScope::WithChildren => {
      ApiError::new(ErrorCode::UnknownBranch, format!("Nothing was pushed: the last sync has no {}", push.subject.words()))
    }
  }
}

/// Runs the push `push` in the session of `key` on the plan its caller accepted: admitted over the operations of the
/// session per ref (the same push joins it, another push of the ref waits for it), after the sync of the session that
/// runs, if one does; then planned again and run as a start runs (`land_execute::run_planned`: the witness, the lock only
/// for the full witness and the freeze, the retarget, the cancel, the push of the frozen commit with its lease, once more
/// over a remote that moved without commits the branch lacks, then the restart) over the services of `services`
/// (`push_start`: neither Patronus nor GitHub needed), its steps in the view and its progress and outcome to `events`.
/// After a wait (the sync, the operations it queued behind) the push is decided again on what they left, over the
/// remote head the session has now (`ExpectRemote::OfSession`), and goes while the user accepted what it asks: the click
/// asked for this push, and a remote that brought commits the branch lacks asks first again
///
/// # Errors
/// What refuses it before its first effect, a branch the session has no plan of, a session that closed
#[instrument(skip_all, fields(subject = %push.subject.name()))]
pub async fn execute_push(registry: &Registry, key: &SessionKey, push: Push, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let planned = prepare_push(sessions, key, &push).await;
  let request = push_request(&key.prefix, &push, planned.as_ref());
  run(registry, key, request, events, async |guard| {
    let began = std::time::Instant::now();
    let synced = wait_for_running_sync(sessions, key, &guard.stepper()).await;
    let waited = synced || guard.queued();
    let checked = checked_holds(services, sessions, key, &push).await.map(Arc::new);
    let (prefix, intent, holds) = (key.prefix.clone(), push.clone(), checked.clone());
    let expect = if waited { ExpectRemote::OfSession } else { ExpectRemote::OfIntent };
    let planned = sessions
      .read(key, move |state, facts| push_planned(state, facts, &prefix, &intent, holds.as_deref(), expect))
      .await
      .flatten()
      .ok_or_else(|| no_push_plan(&push))?;
    // what refuses it, in the words of the robots it was checked on
    if let Some(refusal) = refusal_of(&planned, checked.as_ref().map_or(&[][..], |checked| checked.robots.as_slice())) {
      return Err(refusal);
    }
    run_planned_in(guard, push_start(services, sessions, key).await?, planned, &accepted, waited, began).await
  })
  .await
}

/// The holds the run of `push` checks: the user's robots asked now, one full list (`PatronusService::robots_for_checks`,
/// which a robot started meanwhile in the IDE or by `bd` is in), and the Patronus view of the table of the last sync of
/// the session derived of them (`push_guard::guard_facts`); none for an unapplied branch, which nothing holds (C3), and
/// off Space or without a token, where no robot runs, nor before the first sync
async fn checked_holds(services: &dyn ServiceHost, sessions: &Sessions, key: &SessionKey, push: &Push) -> Option<CheckedHolds> {
  if push.scope == PushScope::Unapplied {
    return None;
  }
  let space = services.space_repository(key).await.ok().flatten()?;
  let patronus = services.patronus(key).ok()?;
  let robots = patronus.robots_for_checks(&space.name, &key.prefix).await;
  let table = sessions.table(key).await?;
  let (git, path) = (services.git(), key.worktree.clone());
  let facts = tokio::task::spawn_blocking(in_current_span(move || guard_facts(&git, &path, &table, robots))).await.ok()?;
  Some(CheckedHolds {
    view: facts.view,
    robots: facts.robots,
  })
}

/// Waits for the sync of the session of `key` that runs, if one does (a rewrite's, a focus'), with the step
/// `WaitingForSync`: it brings the tips into the table the push is checked on; a sync asked after it is not waited for.
/// Whether it waited
async fn wait_for_running_sync(sessions: &Sessions, key: &SessionKey, stepper: &Stepper) -> bool {
  if sessions.apply_with(key, |state| matches!(state.sync(), SyncState::Running { .. })).await != Some(true) {
    return false;
  }
  stepper.step(OperationStep::WaitingForSync);
  if let Err(error) = sessions.sync(key, SyncReason::Open).wait().await {
    debug!(error = %error.message, "the push checks the table of the last successful sync");
  }
  stepper.step(OperationStep::Checking);
  true
}

#[cfg(test)]
#[path = "push_test.rs"]
mod push_test;
