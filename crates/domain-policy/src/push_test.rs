use crate::facts::RemoteBranch;
use crate::push::{
  ActionPush, HeadedStack, Kept, PushScope, PushState, StartAncestor, StartPush, UpToPush, UpToStop, Verdict, action_push, branch_push, kept as kept_by, push_plan,
  stack_branch_along, start_push, up_to_push,
};
use crate::stack_test::{Spec, repository, stacks_of};
use domain_types::fixtures::{RemoteCounts, remote_status_with, robot_of};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView};
use domain_types::session::{ChildrenPush, StackBranchAlong};
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use domain_types::{PushAction, RemoteStatusUpdate};
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashMap};

const fn force(drops: bool, replaces: bool) -> PushAction {
  PushAction::Force { drops, replaces }
}

/// A branch built on its declared parent
const fn on(name: &'static str, parent: Option<&'static str>) -> Spec {
  (name, parent, parent)
}

/// auth ← login ← oauth, auth ← session, in the order of the table (newest first)
const STACK: [Spec; 4] = [on("oauth", Some("login")), on("session", Some("auth")), on("login", Some("auth")), on("auth", None)];

const fn with(action: PushAction) -> PushState {
  PushState { action: Some(action), kept: None }
}

const fn kept(kept: Kept) -> PushState {
  PushState { kept: Some(kept), ..PUSH }
}

const UP_TO_DATE: PushState = with(PushAction::UpToDate);
const PUSH: PushState = with(PushAction::Push);
const DROPS: PushState = with(force(true, false));

fn states(entries: &[(&'static str, PushState)]) -> impl Fn(&str) -> PushState {
  let map: HashMap<&str, PushState> = entries.iter().copied().collect();
  move |name: &str| map.get(name).copied().unwrap_or(UP_TO_DATE)
}

/// Push Stack covers the chain of the effective bases root first up to the nearest held ancestor, whose robot lands
/// the rest; each branch says what it does: an amended one goes, one a merged Safe Push landed is left out, one the sync
/// did not build too
#[test]
fn push_stack_covers_the_ancestors_up_to_the_nearest_held_one() {
  let (stacks, parked) = (stacks_of(&STACK), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let plan = push_plan(&repo, states(&[("auth", PUSH), ("login", PUSH), ("oauth", PUSH)]), "oauth", PushScope::WithAncestors);
  assert_eq!(plan.order(false), ["auth", "login", "oauth"]);
  // an amended or restacked ancestor goes and asks nothing: its push replaces older versions, it drops nobody's work
  let replaced = with(force(false, true));
  let plan = push_plan(&repo, states(&[("auth", PUSH), ("login", replaced), ("oauth", PUSH)]), "oauth", PushScope::WithAncestors);
  assert_eq!((plan.order(false), plan.ask_first()), (vec!["auth", "login", "oauth"], vec![]));
  let held = kept(Kept::Held);
  let plan = push_plan(&repo, states(&[("auth", PUSH), ("login", held), ("oauth", PUSH)]), "oauth", PushScope::WithAncestors);
  assert_eq!(plan.steps, [("login", Verdict::Held), ("oauth", Verdict::Goes)]);
  let landed = kept(Kept::Landed);
  let not_built = kept(Kept::NotBuilt);
  let plan = push_plan(
    &repo,
    states(&[("auth", not_built), ("login", landed), ("oauth", DROPS)]),
    "oauth",
    PushScope::WithAncestors,
  );
  assert_eq!(plan.steps, [("auth", Verdict::NotBuilt), ("login", Verdict::Landed), ("oauth", Verdict::AsksFirst)]);
  assert_eq!((plan.order(false), plan.order(true), plan.ask_first()), (vec![], vec!["oauth"], vec!["oauth"]));
}

/// What keeps a branch out of a push, the first that applies: parked, landed (by the fetched baseline or a merged Safe
/// Push), held by a running Safe Push other than the one the push cancels, not built
#[test]
fn a_branch_is_kept_out_parked_then_landed_then_held_unless_replaced_then_not_built() {
  let hold = |kind| HoldView {
    kind,
    by: robot_of("login", "login-run"),
    carried: None,
  };
  let (span, landed) = (hold(HoldKind::Span), hold(HoldKind::Landed));
  assert_eq!(kept_by(true, true, Some(&span), None, true), Some(Kept::Parked));
  assert_eq!(kept_by(false, true, Some(&span), None, true), Some(Kept::Landed));
  assert_eq!(kept_by(false, false, Some(&landed), Some("login-run"), false), Some(Kept::Landed));
  assert_eq!(kept_by(false, false, Some(&span), None, true), Some(Kept::Held));
  assert_eq!(kept_by(false, false, Some(&span), Some("login-run"), true), Some(Kept::NotBuilt));
  assert_eq!(kept_by(false, false, None, None, false), None);
}

const UNPUSHED: RemoteBranch<'static> = RemoteBranch {
  tip: None,
  up_to_date: false,
  drops: 0,
  others: 0,
};
const ON_REMOTE: RemoteBranch<'static> = RemoteBranch {
  tip: Some("tip"),
  up_to_date: true,
  ..UNPUSHED
};

const fn ancestor(name: &'static str, kept: Option<Kept>, remote: Option<RemoteBranch<'static>>) -> StartAncestor<'static> {
  StartAncestor { name, kept, remote }
}

/// A start pushes Push Stack of its branch: the ancestors with work, root first, up to the nearest held one and past
/// one that landed, one that asks first too (its drop refuses the start unless allowed), then the branch while its
/// remote lacks something of it; a branch it covers without a remote status leaves what it pushes unknown
#[test]
fn a_start_pushes_the_ancestors_with_work_up_to_the_nearest_held_one_past_what_landed() {
  let behind = RemoteBranch { drops: 1, ..UNPUSHED };
  // nearest first: login, auth, base
  let pushed = start_push(
    "oauth",
    Some(UNPUSHED),
    [
      ancestor("login", Some(Kept::Landed), Some(behind)),
      ancestor("auth", None, Some(behind)),
      ancestor("base", None, Some(UNPUSHED)),
    ],
  );
  assert_eq!(
    pushed,
    StartPush {
      branches: vec!["base", "auth", "oauth"],
      unknown: false
    }
  );
  let pushed = start_push(
    "oauth",
    Some(ON_REMOTE),
    [
      ancestor("login", None, Some(UNPUSHED)),
      ancestor("auth", Some(Kept::Held), None),
      ancestor("base", None, None),
    ],
  );
  assert_eq!(
    pushed,
    StartPush {
      branches: vec!["login"],
      unknown: false
    }
  );
  assert!(start_push("oauth", Some(ON_REMOTE), [ancestor("login", None, None)]).unknown);
  assert!(start_push("oauth", None, []).unknown);
  assert_eq!(
    start_push("oauth", Some(ON_REMOTE), [ancestor("login", Some(Kept::NotBuilt), None)]).branches,
    Vec::<&str>::new()
  );
}

/// Push with Children covers the branch and every branch stacked on it by declared parent, each after its parent
#[test]
fn push_with_children_covers_the_branch_and_its_descendants() {
  let (stacks, parked) = (stacks_of(&STACK), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let parked_state = kept(Kept::Parked);
  let plan = push_plan(
    &repo,
    states(&[("login", PUSH), ("session", DROPS), ("oauth", parked_state)]),
    "auth",
    PushScope::WithChildren,
  );
  assert_eq!(
    plan.steps,
    [
      ("auth", Verdict::UpToDate),
      ("session", Verdict::AsksFirst),
      ("login", Verdict::Goes),
      ("oauth", Verdict::Parked)
    ]
  );
  assert_eq!(push_plan(&repo, states(&[]), "gone", PushScope::WithChildren).steps, []);
}

/// The pushes of a branch the view sends: Push Stack sends every ancestor with work, the push of the button and of an
/// action starts above the nearest one that asks first, and the button pushes the stack only while the branch has work
/// and asks nothing
#[test]
fn the_push_button_and_an_action_start_above_the_nearest_ancestor_that_asks_first() {
  let (stacks, parked) = (stacks_of(&STACK), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let push = branch_push(&repo, states(&[("auth", PUSH), ("login", DROPS), ("oauth", PUSH)]), None, "oauth");
  // every ancestor goes: the stack of its structure
  assert_eq!((push.stack, push.action, push.button), (None, Some(vec![]), false));
  // none asks first: the push of the button is Push Stack
  let push = branch_push(&repo, states(&[("auth", PUSH), ("login", PUSH), ("oauth", PUSH)]), None, "oauth");
  assert_eq!((push.stack, push.action, push.button), (None, None, true));
  // the branch alone asks first: the button confirms it alone
  let push = branch_push(&repo, states(&[("login", PUSH), ("oauth", DROPS)]), None, "oauth");
  assert_eq!((push.stack, push.action, push.button), (Some(vec!["login".to_string()]), None, false));
  // a held login: Push Stack starts above it
  let held = kept(Kept::Held);
  let push = branch_push(&repo, states(&[("login", held), ("oauth", PUSH)]), None, "oauth");
  assert_eq!((push.stack, push.action), (Some(vec![]), None));
  // a branch alone sends nothing more
  assert_eq!(branch_push(&repo, states(&[("auth", PUSH)]), None, "auth").stack, None);
}

/// The push of an action that needs the branch on the remote (a pull request, a review request) sends what the button
/// does: the ancestors with work above the nearest one that asks first, root first, and the branch by what it does
#[test]
fn an_action_pushes_the_ancestors_above_the_nearest_that_asks_first_then_the_branch() {
  let (stacks, parked) = (stacks_of(&STACK), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let pushed = |entries: &[(&'static str, PushState)]| action_push(&repo, states(entries), "oauth");
  let goes = |ancestors: Vec<&'static str>, own| Some(ActionPush { ancestors, own });
  assert_eq!(pushed(&[("auth", PUSH), ("login", DROPS), ("oauth", PUSH)]), goes(vec![], Verdict::Goes));
  assert_eq!(pushed(&[("auth", PUSH), ("login", PUSH)]), goes(vec!["auth", "login"], Verdict::UpToDate));
  // a held ancestor ends the chain: its robot lands it and what it is built on
  assert_eq!(pushed(&[("auth", PUSH), ("login", kept(Kept::Held)), ("oauth", DROPS)]), goes(vec![], Verdict::AsksFirst));
  assert_eq!(action_push(&repo, states(&[]), "gone"), None);
}

/// Push Up to Here pushes the ancestors with work whole, root first, then the branch up to the commit; what stops it, the
/// first that applies: the branch kept out, a remote with an older version of the branch (it pushes whole), then, root
/// first, an ancestor held (the nearest held one ends the chain), not built or asking first. A landed or up-to-date
/// ancestor is left out
#[test]
fn push_up_to_here_pushes_the_ancestors_whole_and_stops_at_what_cant_go() {
  let (stacks, parked) = (stacks_of(&STACK), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let up_to = |entries: &[(&'static str, PushState)], rewritten| up_to_push(&repo, states(entries), "oauth", rewritten);
  let goes = |ancestors: Vec<&'static str>| UpToPush { ancestors, stop: None };
  let stops = |name, verdict| UpToPush {
    ancestors: Vec::new(),
    stop: Some(UpToStop::Branch { name, verdict }),
  };
  assert_eq!(up_to(&[("auth", PUSH), ("login", PUSH), ("oauth", PUSH)], false), goes(vec!["auth", "login"]));
  assert_eq!(up_to(&[("auth", kept(Kept::Landed)), ("oauth", PUSH)], false), goes(vec![]));
  assert_eq!(up_to(&[("auth", PUSH), ("login", DROPS), ("oauth", PUSH)], false), stops("login", Verdict::AsksFirst));
  assert_eq!(up_to(&[("auth", kept(Kept::NotBuilt)), ("oauth", PUSH)], false), stops("auth", Verdict::NotBuilt));
  // the nearest held ancestor ends the chain, and its robot takes it as it is: nothing goes up to a commit above it
  assert_eq!(up_to(&[("auth", PUSH), ("login", kept(Kept::Held)), ("oauth", PUSH)], false), stops("login", Verdict::Held));
  assert_eq!(
    up_to(&[("auth", PUSH), ("oauth", PUSH)], true),
    UpToPush {
      ancestors: Vec::new(),
      stop: Some(UpToStop::Rewritten)
    }
  );
  assert_eq!(up_to(&[("login", DROPS), ("oauth", kept(Kept::Parked))], true), stops("oauth", Verdict::Parked));
  assert_eq!(up_to(&[("oauth", kept(Kept::Held))], false), stops("oauth", Verdict::Held));
  assert_eq!(up_to_push(&repo, states(&[]), "gone", false), UpToPush::default());
}

/// Push with Children sends the branch while it has work and the branches stacked on it that have work; one that asks
/// first is left out to its row, one held or landed is left out, and a branch only parked branches are stacked on
/// offers none
#[test]
fn push_with_children_leaves_out_what_asks_first_is_held_or_landed() {
  let (stacks, parked) = (stacks_of(&STACK), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let held = kept(Kept::Held);
  let push = branch_push(&repo, states(&[("auth", DROPS), ("login", held), ("session", DROPS), ("oauth", PUSH)]), None, "auth");
  assert_eq!(
    push.children,
    Some(ChildrenPush {
      order: Some(vec!["auth".to_string(), "oauth".to_string()]),
      ask_first: vec!["session".to_string()],
      stack_branch: None,
      offers: true,
    })
  );
  let parked_state = kept(Kept::Parked);
  assert_eq!(branch_push(&repo, states(&[("oauth", parked_state)]), None, "login").children, None);
  let push = branch_push(&repo, states(&[("auth", PUSH)]), None, "auth");
  assert_eq!(
    push.children.map(|children| (children.order, children.offers)),
    Some((Some(vec!["auth".to_string()]), false))
  );
}

fn forking(state: StackBranchState, remote: Option<RemoteStatusUpdate>) -> StackBranchUpdate {
  StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "user/stack/auth".to_string(),
    members: vec!["auth".to_string(), "login".to_string(), "session".to_string()],
    commit_count: 3,
    my_email: None,
    state,
    tip: Some("tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote,
  }
}

/// The stack branch of a whole stack goes along with Push with Children of its root once it is on the remote or its pull
/// request is open, built, not held, while its remote lacks it and drops nothing; a stack that doesn't fork has none
#[test]
fn the_stack_branch_goes_along_once_published_built_not_held_and_behind() {
  let ahead = remote_status_with(RemoteCounts {
    ahead: 2,
    ..RemoteCounts::default()
  });
  let built = forking(StackBranchState::Built, Some(ahead.clone()));
  assert_eq!(stack_branch_along(&built, false, None), Some(StackBranchAlong::Along));
  let unpublished = forking(
    StackBranchState::Built,
    Some(remote_status_with(RemoteCounts {
      absent: true,
      ahead: 2,
      ..RemoteCounts::default()
    })),
  );
  assert_eq!(stack_branch_along(&unpublished, false, None), Some(StackBranchAlong::Unpublished));
  assert_eq!(stack_branch_along(&forking(StackBranchState::Built, None), true, None), Some(StackBranchAlong::Along));
  let hold = Reason::StackParked {
    root: "auth".to_string(),
    member: "login".to_string(),
  };
  assert_eq!(stack_branch_along(&built, false, Some(&hold)), Some(StackBranchAlong::Held { reason: hold.clone() }));
  let stale = forking(StackBranchState::Stale, Some(ahead));
  assert!(matches!(stack_branch_along(&stale, false, None), Some(StackBranchAlong::NotBuilt { .. })));
  let up_to_date = forking(StackBranchState::Built, Some(remote_status_with(RemoteCounts::default())));
  assert_eq!(stack_branch_along(&up_to_date, false, None), Some(StackBranchAlong::UpToDate));
  let drops = forking(
    StackBranchState::Built,
    Some(remote_status_with(RemoteCounts {
      behind: 1,
      ..RemoteCounts::default()
    })),
  );
  assert_eq!(stack_branch_along(&drops, false, None), Some(StackBranchAlong::DropsRemote));
  let alone = StackBranchUpdate {
    members: Vec::new(),
    ..forking(StackBranchState::Stale, None)
  };
  assert_eq!(stack_branch_along(&alone, true, None), None);
  // Push with Children of the root offers the stack branch along by itself
  let (stacks, parked) = (stacks_of(&STACK), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  let headed = HeadedStack {
    stack: &built,
    pull_request_open: false,
    hold: None,
  };
  let push = branch_push(&repo, states(&[]), Some(headed), "auth");
  assert_eq!(push.children.map(|children| (children.order, children.offers)), Some((Some(vec![]), true)));
}
