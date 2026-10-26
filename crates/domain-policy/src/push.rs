//! What a push of a branch does to its remote branch, by its remote status (`RemoteStatusUpdate`, compared by content:
//! a branch only rebased onto a newer baseline has nothing to push), for the app and `bd` alike
//! (docs/design/rust-domain-authority.md, R1 and D4):
//!
//! - `action`: create, push (a fast-forward), force (rebased, older versions replaced, remote commits dropped) or up to
//!   date; `sync_core::remote_status` stores it on the status, so the page and `bd` read this one answer
//! - `needs_push`: the push changes the remote branch (the lack of it, commits it lacks, versions it replaces or drops)
//! - `replaces_older_versions`: the remote has older versions of commits of the branch, of its stack ancestors or of the
//!   baseline, or an older piece of a stacked branch
//! - `drops_remote_commits`: the push drops remote commits the branch lacks, someone's work: it asks first
//! - `over_moved_remote`: a push refused because the remote branch moved since its status goes again, once, over what
//!   the remote has now, only when nothing of it is lost
//! - `kept`: what keeps a branch out of a push whatever its remote has (`Kept`: parked, landed, held, not built)
//! - `push_plan`: what a push of several branches covers, in push order, parents first, and what each of them does
//!   (`Verdict`): Push Stack, the branches a branch is built on (the chain ends at the nearest one a running Safe Push
//!   holds, `PushPlan::with_ancestors`), and Push with Children, the branches stacked on it by declared parent; the
//!   adapters say each verdict (the page leaves a branch out and its row says why, `bd` refuses where the user must act)
//! - `start_push`: what a Safe Push or a Dry Run start of a branch pushes, the `Push` effect of `land::decide` that the
//!   app's start and `bd safe-push` send: Push Stack of it over the holds of every robot but the one the start
//!   replaces, the ancestors that ask first included (a push that drops remote commits refuses the start unless allowed)
//! - `branch_push`: the pushes of a branch the view sends (`BranchPush`): Push Stack, the push of the push button and
//!   of an action (it starts above the nearest branch that asks first), and Push with Children with the stack branch of
//!   the whole stack of a root (`stack_branch_along`: once it is on the remote or its pull request is open, built, not
//!   held, its remote lacking it)

use crate::facts::{RemoteBranch, Repository};
use crate::reasons::stack_not_built;
use crate::stack::{build_ancestors, descendants};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView};
use domain_types::session::{BranchPush, ChildrenPush, StackBranchAlong};
use domain_types::stack::StackBranchUpdate;
use domain_types::{PushAction, RemoteStatusUpdate};

/// What a push of the branch does over `remote`. The status compares content: a branch only rebased onto a newer
/// baseline is up to date, the remote has its work; one reworded since is not, a robot would land the remote's
/// messages. A push that is not a fast-forward (the remote has commits the branch lacks, older versions of its
/// commits, or the branch was rebased) is a force push
#[must_use]
pub const fn action(remote: &RemoteStatusUpdate) -> PushAction {
  if !remote.remote_exists {
    return PushAction::Create;
  }
  if !needs_push(remote) {
    return PushAction::UpToDate;
  }
  let drops = drops_remote_commits(remote);
  let replaces = replaces_older_versions(remote);
  if drops || replaces || !remote.fast_forward {
    PushAction::Force { drops, replaces }
  } else {
    PushAction::Push
  }
}

/// A push of the branch would change its remote branch: the remote lacks it or commits of it, or has commits a push
/// replaces or drops
#[must_use]
pub const fn needs_push(remote: &RemoteStatusUpdate) -> bool {
  !remote.remote_exists || !remote.unpushed_commits.is_empty() || drops_remote_commits(remote) || replaces_older_versions(remote)
}

/// The remote has older versions of commits of the branch (amended here, or reworded: the same changes with another
/// message or author), of its stack ancestors (restacked) or of the baseline (landed), or an older piece: it lacks new
/// commits of the branches it is built on, which a Safe Push of the branch lands with it
#[must_use]
pub const fn replaces_older_versions(remote: &RemoteStatusUpdate) -> bool {
  remote.commits_amended > 0 || remote.commits_restacked > 0 || remote.commits_landed > 0 || remote.commits_reworded > 0 || remote.ancestor_commits_ahead > 0
}

/// The push drops remote commits the branch lacks that are no older version of a commit it has: someone's work, which
/// the push asks for first (the app's Force Push, `bd push --force`)
#[must_use]
pub const fn drops_remote_commits(remote: &RemoteStatusUpdate) -> bool {
  remote.commits_behind > 0
}

/// Whether a push the remote refused because the remote branch moved since the status the user saw goes again, once,
/// over `moved`, the status fetched after the refusal: only when the remote has no commits the branch lacks (nobody's
/// work is lost) and the branch has work for it. A remote with someone's commits, or one that has the branch as it is,
/// is told instead; so is one that moves again
#[must_use]
pub const fn over_moved_remote(moved: &RemoteStatusUpdate) -> bool {
  !drops_remote_commits(moved) && needs_push(moved)
}

/// What the order of a push reads of a branch (`push_plan`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PushState {
  /// What its push does (`action`); `None` before a remote status said it
  pub action: Option<PushAction>,
  /// What keeps it out of a push whatever its remote has
  pub kept: Option<Kept>,
}

/// What keeps a branch out of a push whatever its remote has; the first that applies counts
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
  /// It stays local: parked, or built on a parked branch
  Parked,
  /// Nothing of it is left to land: a merged Safe Push landed it whole, or the fetched baseline has every commit of it
  Landed,
  /// A running Safe Push holds it, other than the robot the push replaces: its robot lands it as it took it
  Held,
  /// The last sync did not build it (blocked, failed): no push sends an older build
  NotBuilt,
}

/// What keeps a branch out of a push whatever its remote has, the first that applies: it stays local (`parked`), nothing
/// of it is left to land (`landed`: the fetched baseline has every commit of it; or `hold` is of a merged Safe Push that
/// landed it whole), a running Safe Push holds it (`hold`) other than `replaces`, the robot the push cancels, the last
/// sync did not build it (`not_built`)
#[must_use]
pub fn kept(parked: bool, landed: bool, hold: Option<&HoldView>, replaces: Option<&str>, not_built: bool) -> Option<Kept> {
  if parked {
    Some(Kept::Parked)
  } else if landed || hold.is_some_and(|hold| hold.kind == HoldKind::Landed) {
    Some(Kept::Landed)
  } else if hold.is_some_and(|hold| Some(hold.by.robot_id.as_str()) != replaces) {
    Some(Kept::Held)
  } else if not_built {
    Some(Kept::NotBuilt)
  } else {
    None
  }
}

/// What a branch of a push plan does
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
  /// It goes: its push changes the remote branch
  Goes,
  /// It has work, but its push drops remote commits it lacks (someone's work): it goes once the user confirmed that
  AsksFirst,
  /// The remote has it as it is, or no remote status said yet what the remote has
  UpToDate,
  /// The last sync did not build it
  NotBuilt,
  /// A running Safe Push holds it
  Held,
  /// Nothing of it is left to land
  Landed,
  /// It stays local
  Parked,
}

/// What a push of several branches covers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushScope {
  /// Push Stack: the branches the subject is built on, root first, then the subject
  WithAncestors,
  /// Push with Children: the subject, then the branches stacked on it by declared parent, each after its parent
  WithChildren,
}

/// What a push covers, in push order, parents first, with what each branch does
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PushPlan<'a> {
  /// `WithAncestors`: the chain of the effective bases, root first, from the nearest held ancestor on (the first step
  /// then; the branches it is built on are not covered: its robot lands them), the subject last. `WithChildren`: the
  /// subject first, then its descendants
  pub steps: Vec<(&'a str, Verdict)>,
}

impl<'a> PushPlan<'a> {
  /// The branches the push sends: those that go, and with `confirmed` those that ask first too
  #[must_use]
  pub fn order(&self, confirmed: bool) -> Vec<&'a str> {
    self
      .steps
      .iter()
      .filter(|(_, verdict)| *verdict == Verdict::Goes || (confirmed && *verdict == Verdict::AsksFirst))
      .map(|(name, _)| *name)
      .collect()
  }

  /// The branches whose push asks first
  #[must_use]
  pub fn ask_first(&self) -> Vec<&'a str> {
    self.steps.iter().filter(|(_, verdict)| *verdict == Verdict::AsksFirst).map(|(name, _)| *name).collect()
  }

  /// What `name` does in the plan; `None` for a branch it does not cover
  #[must_use]
  pub fn verdict(&self, name: &str) -> Option<Verdict> {
    self.steps.iter().find(|(step, _)| *step == name).map(|(_, verdict)| *verdict)
  }

  /// Push Stack of a branch: `ancestors`, the nearest first, each with what it does, up to the nearest held one (its
  /// robot lands it and every branch it is built on), root first; then the branch, `own` what it does
  #[must_use]
  pub fn with_ancestors(ancestors: impl IntoIterator<Item = (&'a str, Verdict)>, own: (&'a str, Verdict)) -> Self {
    let mut steps = Vec::new();
    for step in ancestors {
      steps.push(step);
      if step.1 == Verdict::Held {
        break;
      }
    }
    steps.reverse();
    steps.push(own);
    Self { steps }
  }
}

/// What a branch kept out of a push does in it
const fn kept_verdict(kept: Kept) -> Verdict {
  match kept {
    Kept::Parked => Verdict::Parked,
    Kept::Landed => Verdict::Landed,
    Kept::Held => Verdict::Held,
    Kept::NotBuilt => Verdict::NotBuilt,
  }
}

/// What a branch does in a push, by its state
#[must_use]
pub const fn verdict(state: &PushState) -> Verdict {
  if let Some(kept) = state.kept {
    return kept_verdict(kept);
  }
  match state.action {
    None | Some(PushAction::UpToDate) => Verdict::UpToDate,
    Some(PushAction::Force { drops: true, .. }) => Verdict::AsksFirst,
    Some(_) => Verdict::Goes,
  }
}

/// What a branch does in a push, by what keeps it out (`kept`) and what the rules read of its remote branch (`None`
/// before a remote status said it), as `verdict` says it of the action of a remote status: it asks first while the push
/// drops remote commits, goes while the remote lacks something of it, and is up to date otherwise
#[must_use]
pub const fn remote_verdict(kept: Option<Kept>, remote: Option<RemoteBranch<'_>>) -> Verdict {
  if let Some(kept) = kept {
    return kept_verdict(kept);
  }
  match remote {
    Some(remote) if remote.drops > 0 => Verdict::AsksFirst,
    Some(remote) if !remote.up_to_date => Verdict::Goes,
    _ => Verdict::UpToDate,
  }
}

/// A branch a start of a branch is built on, as `start_push` reads it
#[derive(Clone, Copy, Debug)]
pub struct StartAncestor<'a> {
  pub name: &'a str,
  /// What keeps it out of the push (`kept`, over the holds of every robot but the one the start replaces)
  pub kept: Option<Kept>,
  /// What the rules read of its remote branch; `None` before a remote status said it
  pub remote: Option<RemoteBranch<'a>>,
}

/// What a start of a branch pushes (`start_push`)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartPush<'a> {
  /// The branches it pushes, root first, the branch last while its remote lacks something of it
  pub branches: Vec<&'a str>,
  /// A branch the push covers has no remote status yet: what the start pushes is not known
  pub unknown: bool,
}

/// What a Safe Push or a Dry Run start of `name` pushes, the `Push` effect of `land::decide` that the app's start and `bd
/// safe-push` send (and whether it pushes, `BranchStart::pushes`): Push Stack of it (`PushPlan::with_ancestors`) over
/// `ancestors`, the nearest first, the branches that go or ask first (the push of one that drops remote commits refuses
/// the start unless the user allowed it, D12), root first; the branch itself by its remote branch `own` alone: its own
/// hold and its landing refuse the start, they don't keep it out of the push
#[must_use]
pub fn start_push<'a>(name: &'a str, own: Option<RemoteBranch<'a>>, ancestors: impl IntoIterator<Item = StartAncestor<'a>>) -> StartPush<'a> {
  let mut unknown = own.is_none();
  let mut covered = Vec::new();
  for ancestor in ancestors {
    unknown |= ancestor.kept.is_none() && ancestor.remote.is_none();
    covered.push((ancestor.name, remote_verdict(ancestor.kept, ancestor.remote)));
    if ancestor.kept == Some(Kept::Held) {
      break;
    }
  }
  let plan = PushPlan::with_ancestors(covered, (name, remote_verdict(None, own)));
  StartPush {
    branches: plan.order(true),
    unknown,
  }
}

/// What a push of `scope` from `name` covers (`PushPlan`), `state` of each branch; empty for a branch the table lacks
#[must_use]
pub fn push_plan<'a>(repo: &Repository<'a>, state: impl Fn(&str) -> PushState, name: &str, scope: PushScope) -> PushPlan<'a> {
  let Some(name) = repo.branch(name).map(|branch| branch.name) else {
    return PushPlan::default();
  };
  match scope {
    // `with_ancestors` stops at the nearest held ancestor: no verdict is asked of the branches its robot lands
    PushScope::WithAncestors => {
      let mut verdicts = Vec::new();
      for ancestor in build_ancestors(repo, name).into_iter().rev() {
        let verdict = verdict(&state(ancestor));
        verdicts.push((ancestor, verdict));
        if verdict == Verdict::Held {
          break;
        }
      }
      PushPlan::with_ancestors(verdicts, (name, verdict(&state(name))))
    }
    PushScope::WithChildren => {
      let mut steps = vec![(name, verdict(&state(name)))];
      steps.extend(descendants(repo, name).into_iter().map(|descendant| (descendant, verdict(&state(descendant)))));
      PushPlan { steps }
    }
  }
}

/// What Push with Children of the root of a stack that forks does with its stack branch (`StackBranchAlong`): only one
/// the sync built goes along, once it is on the remote or its pull request is open (`pull_request_open`; a stack branch
/// is no branch to publish by itself, its pull request or its Safe Push puts it on the remote), while its remote lacks
/// commits of it and has none it lacks, and never while its push is held (`hold`, `holds::stack_branch_hold`). `None`
/// for a stack that does not fork
#[must_use]
pub fn stack_branch_along(stack: &StackBranchUpdate, pull_request_open: bool, hold: Option<&Reason>) -> Option<StackBranchAlong> {
  if stack.members.is_empty() {
    return None;
  }
  let remote = stack.remote.as_ref();
  if !remote.is_some_and(|remote| remote.remote_exists) && !pull_request_open {
    return Some(StackBranchAlong::Unpublished);
  }
  if let Some(reason) = hold {
    return Some(StackBranchAlong::Held { reason: reason.clone() });
  }
  if let Some(reason) = stack_not_built(stack) {
    return Some(StackBranchAlong::NotBuilt { reason });
  }
  Some(match remote.map(|remote| remote.action) {
    Some(PushAction::UpToDate) => StackBranchAlong::UpToDate,
    Some(PushAction::Force { drops: true, .. }) => StackBranchAlong::DropsRemote,
    // also the remote not compared yet, with its pull request open: leased as absent, a remote branch that is there is
    // fetched
    _ => StackBranchAlong::Along,
  })
}

/// What `branch_push` reads of the stack a branch heads
#[derive(Clone, Copy, Debug)]
pub struct HeadedStack<'a> {
  /// Its stack branch, while the stack forks
  pub stack: &'a StackBranchUpdate,
  /// The pull request of the whole stack is open
  pub pull_request_open: bool,
  /// What holds the push of its stack branch (`holds::stack_branch_hold`)
  pub hold: Option<&'a Reason>,
}

/// The pushes of `name` (`BranchPush`): `state` of each branch with the holds of every running robot, `stack` the stack
/// branch `name` is the root of. What a start pushes is its decision's (`start_push`)
#[must_use]
pub fn branch_push(repo: &Repository<'_>, state: impl Fn(&str) -> PushState, stack: Option<HeadedStack<'_>>, name: &str) -> BranchPush {
  let owned = |names: &[&str]| names.iter().map(|name| (*name).to_string()).collect::<Vec<_>>();
  let ancestors = push_plan(repo, &state, name, PushScope::WithAncestors);
  let Some(&(_, own)) = ancestors.steps.last() else {
    return BranchPush::default();
  };
  let before = &ancestors.steps[..ancestors.steps.len() - 1];
  let stack_order: Vec<&str> = PushPlan { steps: before.to_vec() }.order(true);
  let action = above_ask_first(before);
  let children = children_push(repo, &state, stack, name);
  BranchPush {
    stack: (stack_order != build_ancestors(repo, name)).then(|| owned(&stack_order)),
    button: own == Verdict::Goes && !action.is_empty(),
    action: (action != stack_order).then(|| owned(&action)),
    children,
  }
}

/// What the push of an action that needs a branch on the remote makes first (`action_push`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionPush<'a> {
  /// The branches it is built on that go, root first, above the nearest one whose push asks first (`BranchPush::action`)
  pub ancestors: Vec<&'a str>,
  /// What the branch itself does in the push
  pub own: Verdict,
}

/// What the push of an action that needs `name` on the remote sends (Create Pull Request, Request Review: the push of
/// the push button too, `BranchPush::action`): Push Stack of it (`push_plan`) without the ancestors from the nearest one
/// whose push asks first on, which its own row asks for, then the branch by what it does; `None` for a branch the table
/// lacks
#[must_use]
pub fn action_push<'a>(repo: &Repository<'a>, state: impl Fn(&str) -> PushState, name: &str) -> Option<ActionPush<'a>> {
  let plan = push_plan(repo, state, name, PushScope::WithAncestors);
  let (&(_, own), before) = plan.steps.split_last()?;
  Some(ActionPush {
    ancestors: above_ask_first(before),
    own,
  })
}

/// Of the ancestors of a plan, root first, those that go above the nearest one that asks first: a push started without
/// asking leaves it, and what it is built on, to its own row
fn above_ask_first<'a>(ancestors: &[(&'a str, Verdict)]) -> Vec<&'a str> {
  let from = ancestors.iter().rposition(|(_, verdict)| *verdict == Verdict::AsksFirst).map_or(0, |at| at + 1);
  PushPlan {
    steps: ancestors[from..].to_vec(),
  }
  .order(false)
}

/// Push with Children of `name`; `None` for a branch nothing but parked branches is stacked on
fn children_push(repo: &Repository<'_>, state: impl Fn(&str) -> PushState, stack: Option<HeadedStack<'_>>, name: &str) -> Option<ChildrenPush> {
  let plan = push_plan(repo, state, name, PushScope::WithChildren);
  let (subject, children) = plan.steps.split_first()?;
  if children.iter().all(|(_, verdict)| *verdict == Verdict::Parked) {
    return None;
  }
  let mut order = Vec::new();
  // its own push that drops remote commits goes with it: the push of the branch it is
  if matches!(subject.1, Verdict::Goes | Verdict::AsksFirst) {
    order.push(subject.0);
  }
  order.extend(children.iter().filter(|(_, verdict)| *verdict == Verdict::Goes).map(|(name, _)| *name));
  let stack_branch = stack.and_then(|stack| stack_branch_along(stack.stack, stack.pull_request_open, stack.hold));
  let everything = order.len() == plan.steps.len();
  Some(ChildrenPush {
    offers: order.iter().any(|it| *it != name) || stack_branch == Some(StackBranchAlong::Along),
    ask_first: children
      .iter()
      .filter(|(_, verdict)| *verdict == Verdict::AsksFirst)
      .map(|(name, _)| (*name).to_string())
      .collect(),
    order: (!everything).then(|| order.iter().map(|name| (*name).to_string()).collect()),
    stack_branch,
  })
}
