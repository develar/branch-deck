//! What a session derives of its table for the view by the rules of `domain-policy` (docs/design/rust-domain-authority.md,
//! s8-policy), so the page and `bd` read one answer and the page decides nothing: `repository` is the input of the rules
//! read from a table (`domain_policy::facts::Repository`), `place` the structure of each branch (its stack by declared
//! parent, its ancestors by effective base, its main line, whether it is parked and its section) and the rows of each
//! section, with the whole stack of each root (`domain_policy::stack::whole_stack_of`, by declared parent). The table
//! places its branches once per grouping (`TableState::apply`), the root of a whole stack again when its stack branch is
//! reported; a patch carries a structure only when it changed. `push_view` is what the pushes of each branch send
//! (`domain_policy::push::branch_push`) over the table, the holds of the Patronus view and the pull requests of the
//! whole stacks: the view derives it with each patch (`state::View::diff`), so a push button, a menu and an island read
//! one order. `open_pull_requests_view` is the open pull request of each subject as the rules read it (where it goes, a
//! draft, the code owners Request Review… offers), derived with each patch too.
//!
//! `action_push_of` is the push an action that needs a branch on the remote makes first (a pull request, a review
//! request), in the order of the push button and with why it can't be made, over the holds of the Patronus view as they
//! stand; `with_pull_request_sources` the sources such an action is planned on, which need no Patronus view.
//!
//! `decide_landing` is what the rules decide over the Patronus facts of a table (`patronus_view`), for the view of the
//! session, the guards of the app and `bd` alike: the hold of each branch (`domain_policy::holds::branch_hold`) and of
//! each stack branch, why a Safe Push or a Dry Run can't start (`domain_policy::land::start_refusal`), the robot a Safe
//! Push start replaces, why the push an action needs can't be made (`push_blocker`), and the restart each robot offers
//! (`restart_action`, `restart_refusal`). `branch_start` and `stack_start` are the input of a start, which the guards and
//! `bd` read for their own intent (a restart, a dry run that pushes what `bd` pushes); `branch_subject` and
//! `stack_subject` the subject part of the decision of a start (`domain_policy::land::decide`), which `bd safe-push`
//! decides by.
//!
//! `lands_of` is what a Safe Push and a Dry Run of each subject do now and what stands in their way, the decisions of
//! their default intents (`domain_policy::land::decide` over `LandSources`: the table, the Patronus view, the robots,
//! the profiles, the pull requests, the readiness, the rules and the landing needs as the session holds them), which the
//! view derives with each patch but while a sync runs; `land_decision` the decision of any intent (a chosen profile, a
//! start without approvals) with the profiles it offers, over the sources of a session (`with_land_sources`) or of a
//! command, which `operations::land` answers as the plan of the start (`prepare`).
//!
//! `next_steps` is the next step and the node of every row and of every whole stack that forks
//! (`domain_policy::next_step`, `domain_policy::landing_node` over `StepSources`: the table of the last sync, its rows as
//! they are now, the Patronus view, the robots and the facts of the remote services), which the view derives with
//! every patch, a sync of a branch included (row 1), and `bd status` and `bd safe-push --check` over what they read
//! (`grouped_rows`); `wanted_readiness` the shown subjects whose step waits for the readiness, the only ones the
//! session asks it of.

use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::facts::{FactRecord, FactStore, listed_robots};
use crate::table::{TableParts, TableState};
use domain_policy::facts::{RemoteBranch, Repository, Robots};
use domain_policy::holds::{PushStep, StackHoldFacts, branch_hold, holds_pushes, is_active, is_pushing, push_blocker, restart_action, stack_branch_hold};
use domain_policy::land::{
  BranchStart, BranchSubject, LandFacts, LandsBranch, StackStart, StackSubject, Start, SubjectFacts, TargetConflictFact, decide, offered_profiles, on_last_commit, restart_refusal,
  standing, start_refusal,
};
use domain_policy::landed::{LandedFacts, LandedSubject, Of, landed_reason, landed_state};
use domain_policy::landing_node::{NodeFacts, landing_node};
use domain_policy::next_step::{Listing, Needs, PatronusStanding, StepFacts, StepPullRequest, StepRobot, next_step};
use domain_policy::parked::{parked_state, whole_stack_parked};
use domain_policy::pull_request::base_of;
use domain_policy::push::{HeadedStack, Kept, PushState, StartAncestor, StartPush, Verdict, action_push, branch_push, kept, start_push};
use domain_policy::readiness::wanted;
use domain_policy::reasons::{held, stack_not_built};
use domain_policy::review::candidates;
use domain_policy::stack::{WholeStack, build_ancestors, children, declared_parent, descendants, main_line, whole_stack_members, whole_stack_of, whole_stack_rest};
use domain_types::branch_names::baseline_on_remote;
use domain_types::landing::{Land, LandDecision, LandDecisions, LandProfiles, Landed, LandingRefusal, ProfileChoice, Subject};
use domain_types::next_step::{NextStepView, StackPullRequest};
use domain_types::patronus::{LandingAllowances, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotStatus, PatronusStartOptions, ReviewReadiness};
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestAvailability, PullRequestState, PullRequestView};
use domain_types::reason::Reason;
use domain_types::repository_view::{BranchLanding, BranchPatronus, HoldKind, HoldView, LandingNeeds, PatronusView, RestartAction, RobotOf, RobotRestart, SpanView, StackLanding};
use domain_types::session::{
  BranchPush, BranchSection, BranchStatus, BranchStructure, BranchView, Demand, Fact, FactState, LandedSection, Parked, ParkedMember, PullRequestsView, RobotsView,
  SubjectReadiness, WholeStackView, stack_readiness_key,
};
use domain_types::stack::{BranchStack, StackBranchUpdate};
use domain_types::{GroupedBranchInfo, PushAction};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// The input of the rules over the table of `table`, as its view has it
#[must_use]
pub fn repository(table: &TableState) -> Repository<'_> {
  parts_repository(&table.parts)
}

fn parts_repository(parts: &TableParts) -> Repository<'_> {
  Repository::of_view(&parts.order, &parts.branches, &parts.header.parked)
}

/// The structure of every branch of `parts`, its section included, written into it: the rows of each section are the
/// order split by it, which the view does not send twice
pub(crate) fn place(parts: &mut TableParts) {
  let structures: BTreeMap<String, BranchStructure> = {
    let repo = parts_repository(parts);
    repo.order().map(|name| (name.to_string(), structure_of(&repo, &parts.stacks, name))).collect()
  };
  for (name, structure) in structures {
    if let Some(branch) = parts.branches.get_mut(&name) {
      branch.structure = structure;
    }
  }
}

/// The parked member of the stack branch of `root` and its whole stack written into the structure of `root`, once its
/// stack branch is reported
pub(crate) fn place_stack(parts: &mut TableParts, root: &str) {
  let (stack_parked, whole_stack) = {
    let repo = parts_repository(parts);
    (stack_parked(&repo, parts.stacks.get(root)), whole_stack_view(&repo, &parts.stacks, root))
  };
  if let Some(branch) = parts.branches.get_mut(root) {
    branch.structure.stack_parked = stack_parked;
    branch.structure.whole_stack = whole_stack;
  }
}

/// The whole stack of `name` while it heads one (`whole_stack_of`): its stack branch while the stack forks, the last
/// branch of a straight chain of two or more
fn whole_stack_view(repo: &Repository<'_>, stacks: &BTreeMap<String, StackBranchUpdate>, name: &str) -> Option<WholeStackView> {
  let forking = |root: &str| stacks.get(root).is_some_and(|stack| !stack.members.is_empty());
  match whole_stack_of(repo, forking, name)? {
    WholeStack::Forking { root } if root == name => Some(WholeStackView::Forking {
      not_built: stacks.get(root).and_then(stack_not_built),
    }),
    WholeStack::Chain { root, leaf } if root == name && leaf != root => Some(WholeStackView::Chain { leaf: leaf.to_string() }),
    _ => None,
  }
}

fn structure_of(repo: &Repository<'_>, stacks: &BTreeMap<String, StackBranchUpdate>, name: &str) -> BranchStructure {
  let owned = |names: Vec<&str>| names.into_iter().map(str::to_string).collect();
  let parked = parked_state(repo, name);
  let main_line = repo.branch(name).map_or_default(|branch| owned(main_line(repo, branch.name)));
  BranchStructure {
    parent: declared_parent(repo, name).map(str::to_string),
    children: owned(children(repo, name).to_vec()),
    ancestors: owned(build_ancestors(repo, name)),
    descendants: owned(descendants(repo, name)),
    main_line,
    section: if parked.is_some() { BranchSection::Parked } else { BranchSection::Main },
    parked,
    stack_parked: stack_parked(repo, stacks.get(name)),
    whole_stack: whole_stack_view(repo, stacks, name),
  }
}

fn stack_parked(repo: &Repository<'_>, stack: Option<&StackBranchUpdate>) -> Option<ParkedMember> {
  whole_stack_parked(repo, stack?.members.iter().map(String::as_str))
}

/// What the pushes of each branch of `parts` send (`domain_policy::push::branch_push`), over the holds of the robots in
/// the Patronus view of `facts` and the pull requests of the whole stacks it lists; a branch whose pushes send it alone
/// is not listed. A branch the fetched baseline has whole by the table of the last sync (`BranchTable::in_baseline`, as
/// the decision of a start, the gate of the view and the guards read it, never the landing section a running sync
/// moves) is landed: the update archives it, its push lands nothing
pub(crate) fn push_view(parts: &TableParts, facts: &FactStore) -> BTreeMap<String, BranchPush> {
  let repo = parts_repository(parts);
  let (patronus, pull_requests) = (&facts.patronus.part().view, facts.pull_requests.view());
  let push_facts = PushFacts {
    parts,
    patronus,
    in_baseline: facts.table.value().map(|table| &table.in_baseline),
  };
  let held = |name: &str| push_facts.state(name);
  repo
    .order()
    .filter_map(|name| {
      let stack = parts.stacks.get(name).map(|stack| HeadedStack {
        stack,
        pull_request_open: pull_requests
          .stack_pull_requests
          .get(name)
          .is_some_and(|pull_request| pull_request.state == PullRequestState::Open),
        hold: patronus.stacks.get(name).and_then(|facts| facts.landing.hold.as_ref()),
      });
      let push = branch_push(&repo, held, stack, name);
      (push != BranchPush::default()).then(|| (name.to_string(), push))
    })
    .collect()
}

/// The open pull request of each subject of a table (a branch `branches` has, the whole stack of a root `stacks` has as
/// `stack:<root>`) as the rules read it, by the pull requests GitHub listed and the review readiness Patronus answered
/// (`readiness`, by subject): where it goes (`domain_policy::pull_request::base_of`, `target` the branch the baseline is
/// on the remote, the virtual branches those of `prefix`), whether it is a draft, and the code owners Request Review…
/// offers (`domain_policy::review::candidates`). A subject without an open pull request is not listed. The view derives
/// it with each patch; `bd`'s scenario facts read it too
pub fn open_pull_requests_view<'a>(
  target: &str,
  branches: impl Fn(&str) -> bool,
  stacks: impl Fn(&str) -> bool,
  pull_requests: &PullRequestsView,
  readiness: impl Fn(&str) -> Option<&'a ReviewReadiness>,
  prefix: &str,
) -> BTreeMap<String, PullRequestView> {
  let view_of = |key: String, pull_request: &PullRequest| {
    let view = PullRequestView {
      number: pull_request.number,
      base: base_of(pull_request, target, prefix),
      draft: pull_request.is_draft,
      candidates: candidates(readiness(&key), pull_request).candidates,
    };
    (key, view)
  };
  let open = |(_, pull_request): &(&String, &PullRequest)| pull_request.state == PullRequestState::Open;
  let of_branches = pull_requests
    .pull_requests
    .iter()
    .filter(|(name, _)| branches(name))
    .filter(open)
    .map(|(name, pull_request)| view_of(name.clone(), pull_request));
  let of_stacks = pull_requests
    .stack_pull_requests
    .iter()
    .filter(|(root, _)| stacks(root))
    .filter(open)
    .map(|(root, pull_request)| view_of(stack_readiness_key(root), pull_request));
  of_branches.chain(of_stacks).collect()
}

/// The push an action that needs a branch on the remote makes first (Create Pull Request, Request Review), ordered as
/// the push button of the view orders it (`BranchPush::action`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionPushView {
  /// What it sends, root first: the ancestors with work above the nearest one whose push asks first, then the branch
  /// while it has work (`domain_policy::push::action_push`); empty when the remote has it all
  pub branches: Vec<String>,
  /// Why it can't be made for the user: the branch stays local, a running Safe Push holds it, the last sync did not
  /// build it, its push drops remote commits (its row asks first), or a branch of its chain the sync did not build or
  /// whose push drops remote commits (`BranchLanding::push`)
  pub blocker: Option<Reason>,
}

/// The push an action that needs `name` on the remote makes first, over the table of `state` and the holds of the
/// Patronus view of `facts` as they stand (none known before a view: nothing holds it), as `push_view` orders the pushes;
/// a branch with work for the remote that it can't push (its push drops remote commits, it stays local, a Safe Push holds
/// it) has the blocker and no branch to send
#[must_use]
pub fn action_push_of(state: &TableState, facts: &FactStore, name: &str) -> ActionPushView {
  let parts = &state.parts;
  let patronus = &facts.patronus.part().view;
  let push_facts = PushFacts {
    parts,
    patronus,
    in_baseline: facts.table.value().map(|table| &table.in_baseline),
  };
  let repo = parts_repository(parts);
  let Some(push) = action_push(&repo, |branch| push_facts.state(branch), name) else {
    return ActionPushView::default();
  };
  let mut branches: Vec<String> = push.ancestors.iter().map(|branch| (*branch).to_string()).collect();
  if push.own == Verdict::Goes {
    branches.push(name.to_string());
  }
  // the remote has it all: there is no push to make, nor a reason it can't be made
  let own_work = parts
    .branches
    .get(name)
    .and_then(|branch| branch.remote.status.as_ref())
    .is_some_and(|status| !matches!(status.action, PushAction::UpToDate));
  if branches.is_empty() && !own_work {
    return ActionPushView::default();
  }
  let landing = patronus.branches.get(name).map(|facts| &facts.landing);
  let table = facts.table.value();
  let own = match push.own {
    Verdict::Parked => parts.branches.get(name).and_then(|branch| branch.structure.parked.as_ref()).map(|parked| Reason::Parked {
      branch: name.to_string(),
      with: parked.with.clone(),
      note: parked.note.clone(),
    }),
    Verdict::Held => landing.and_then(|landing| landing.hold.as_ref()).and_then(|hold| {
      let robots = listed_robots(facts.robots.view());
      robots.iter().find(|robot| robot.id == hold.by.robot_id).map(|robot| held(name, hold, robot))
    }),
    Verdict::NotBuilt => table.and_then(|table| table.not_built.get(name)).map(|block| Reason::CantPush {
      branch: name.to_string(),
      block: *block,
    }),
    Verdict::AsksFirst => parts.branches.get(name).and_then(|branch| branch.remote.status.as_ref()).map(|status| Reason::DropsRemote {
      branch: name.to_string(),
      behind: status.commits_behind,
    }),
    Verdict::Goes | Verdict::UpToDate | Verdict::Landed => None,
  };
  ActionPushView {
    branches,
    blocker: own.or_else(|| landing.and_then(|landing| landing.push.clone())),
  }
}

/// What `push_view` reads of a branch
struct PushFacts<'a> {
  parts: &'a TableParts,
  patronus: &'a PatronusView,
  /// The branches the fetched baseline has whole, of the table of the last sync; none before its first
  in_baseline: Option<&'a BTreeSet<String>>,
}

impl PushFacts<'_> {
  /// The state of `name` for a push, with the hold of every running robot
  fn state(&self, name: &str) -> PushState {
    let Some(branch) = self.parts.branches.get(name) else {
      return PushState::default();
    };
    let hold = self.patronus.branches.get(name).and_then(|facts| facts.landing.hold.as_ref());
    PushState {
      action: branch.remote.status.as_ref().map(|status| status.action),
      kept: kept(
        branch.structure.parked.is_some(),
        self.in_baseline.is_some_and(|landed| landed.contains(name)),
        hold,
        None,
        branch.sync.status == BranchStatus::Blocked || branch.sync.has_error,
      ),
    }
  }
}

/// What the rules read of a table and its Patronus facts: the robots by id, the holds of the branches
pub struct LandingFacts<'a> {
  table: &'a BranchTable,
  remote: &'a RemoteFacts,
  view: &'a PatronusView,
  pub robots: Robots<'a>,
  repo: Repository<'a>,
  holds: HashMap<&'a str, HoldView>,
}

impl<'a> LandingFacts<'a> {
  /// The facts of `table` with `view`, its Patronus facts, the remote branches `remote` and the user's `robots`;
  /// `running_spans`, by branch, the first running robot that lands it (`patronus_view`). A view whose holds are decided
  /// already (`decide_landing`) passes none: its holds are read from it
  pub fn new(table: &'a BranchTable, remote: &'a RemoteFacts, view: &'a PatronusView, robots: &'a [PatronusRobot], running_spans: Option<&HashMap<String, SpanView>>) -> Self {
    let robots = Robots::new(robots);
    let holds = table
      .branches
      .iter()
      .filter_map(|branch| {
        let name = branch.name.as_str();
        let facts = view.branches.get(name);
        let hold = match running_spans {
          Some(spans) => {
            let robot = facts.and_then(|facts| facts.robot_id.as_deref()).and_then(|id| robots.get(id));
            branch_hold(name, facts.and_then(|facts| facts.landed_by.as_ref()), robot, spans.get(name))
          }
          None => facts.and_then(|facts| facts.landing.hold.clone()),
        }?;
        Some((name, hold))
      })
      .collect();
    Self {
      table,
      remote,
      view,
      robots,
      repo: table.repository(),
      holds,
    }
  }

  fn remote_of(&self, name: &str) -> Option<RemoteBranch<'a>> {
    self.remote.branches.get(name).and_then(RemoteFact::branch)
  }

  fn robot_of(&self, name: &str) -> Option<&'a PatronusRobot> {
    self.view.branches.get(name)?.robot_id.as_deref().and_then(|id| self.robots.get(id))
  }

  /// The hold of the push of `name`
  #[must_use]
  pub fn hold(&self, name: &str) -> Option<&HoldView> {
    self.holds.get(name)
  }

  /// The robot of `name` on the commit it is at (`on_last_commit`)
  fn last_commit(&self, name: &str) -> Option<&'a PatronusRobot> {
    self.robot_of(name).filter(|robot| on_last_commit(robot, self.table.tip_of(name), self.remote_of(name)))
  }

  /// The robot a Safe Push start of `name` replaces: its own, holding it from an older commit, which the push cancels
  #[must_use]
  pub fn replaces(&self, name: &str) -> Option<&str> {
    let hold = self.holds.get(name).filter(|hold| hold.kind == HoldKind::Own)?;
    self.last_commit(name).is_none().then_some(hold.by.robot_id.as_str())
  }

  /// The branches `name` is built on, the nearest first
  fn ancestors_nearest_first(&self, name: &str) -> Vec<&'a str> {
    let mut ancestors = build_ancestors(&self.repo, name);
    ancestors.reverse();
    ancestors
  }

  /// What keeps `name` out of the push of a start (`push::kept`): `replaces` the robot the start cancels; landed, what
  /// a merged Safe Push landed whole and what the fetched baseline has whole (`BranchTable::in_baseline`)
  fn kept_from_start(&self, name: &str, replaces: Option<&str>) -> Option<Kept> {
    kept(
      false,
      self.landed_whole(name).is_some() || self.table.in_baseline.contains(name),
      self.holds.get(name),
      replaces,
      self.table.not_built.contains_key(name),
    )
  }

  /// What a start of `name` pushes (`push::start_push`), over the holds of every robot but `replaces`, the robot the
  /// start cancels
  fn start_push(&self, name: &'a str, replaces: Option<&str>) -> StartPush<'a> {
    start_push(
      name,
      self.remote_of(name),
      self.ancestors_nearest_first(name).into_iter().map(|ancestor| StartAncestor {
        name: ancestor,
        kept: self.kept_from_start(ancestor, replaces),
        remote: self.remote_of(ancestor),
      }),
    )
  }

  /// Whether a start of `name` pushes (`push::start_push`, past what landed as `decide` passes it) over the holds of
  /// every robot: only the refusal of a Dry Run reads it (D15), and a Dry Run replaces none. A remote not compared yet
  /// pushes nothing known
  #[must_use]
  pub fn start_pushes(&self, name: &str) -> bool {
    self.table.branch(name).is_some_and(|branch| !self.start_push(&branch.name, None).branches.is_empty())
  }

  /// The input of a start of `name`; `pushes` whether it pushes (`start_pushes`, or what `bd` pushes)
  #[must_use]
  pub fn branch_start(&self, name: &'a str, pushes: bool) -> BranchStart<'_> {
    let facts = self.view.branches.get(name);
    BranchStart {
      name,
      not_built: self.table.not_built.get(name).copied(),
      stack: self.repo.stack(name),
      last_commit: self.last_commit(name),
      span: facts.and_then(|facts| facts.span.as_ref()),
      landing_ancestor: facts.and_then(|facts| facts.landing_ancestor.as_ref()),
      hold: self.holds.get(name),
      pushes,
    }
  }

  /// The robot of the whole stack of the root of `stack`
  fn stack_robot(&self, stack: &StackBranchUpdate) -> Option<&'a PatronusRobot> {
    self.view.stacks.get(&stack.root)?.robot_id.as_deref().and_then(|id| self.robots.get(id))
  }

  fn stack_last_commit(&self, stack: &StackBranchUpdate) -> Option<&'a PatronusRobot> {
    let remote = self.remote.stacks.get(&stack.root).and_then(RemoteFact::branch);
    self.stack_robot(stack).filter(|robot| on_last_commit(robot, stack.tip.as_deref(), remote))
  }

  /// The input of a start of the whole stack of `stack`
  #[must_use]
  pub fn stack_start<'s>(&'s self, stack: &'s StackBranchUpdate) -> StackStart<'s> {
    StackStart {
      stack,
      last_commit: self.stack_last_commit(stack),
      landing_member: self.view.stacks.get(&stack.root).and_then(|facts| facts.landing_member.as_ref()),
    }
  }

  /// What landed `name` whole, with its robot
  fn landed_whole(&self, name: &str) -> Option<(&'a RobotOf, &'a PatronusRobot)> {
    let landed = self.view.branches.get(name)?.landed_by.as_ref().filter(|landed| landed.after.is_empty())?;
    Some((&landed.by, self.robots.get(&landed.by.robot_id)?))
  }

  /// Merged Safe Pushes landed every member of `stack` whole: what landed each
  fn stack_landed_whole(&self, stack: &StackBranchUpdate) -> Option<Vec<(&'a RobotOf, &'a PatronusRobot)>> {
    stack.members.iter().map(|member| self.landed_whole(member)).collect()
  }

  /// What `domain_policy::landed` reads of these facts with the landing report `baseline`
  const fn landed_facts<'s>(&'s self, baseline: &'s LandedSection) -> LandedFacts<'s> {
    LandedFacts {
      baseline,
      view: Some(self.view),
      robots: &self.robots,
    }
  }

  /// The hold of the push of the stack branch of `stack` (`stack_branch_hold`)
  #[must_use]
  pub fn stack_hold(&self, stack: &StackBranchUpdate) -> Option<Reason> {
    let parked = whole_stack_parked(&self.repo, stack.members.iter().map(String::as_str));
    stack_branch_hold(&StackHoldFacts {
      stack,
      parked: parked.as_ref(),
      landed: self.stack_landed_whole(stack),
      robot: self.stack_robot(stack),
    })
  }

  fn branch_landing(&self, name: &'a str) -> BranchLanding {
    let start = Start::Branch(self.branch_start(name, self.start_pushes(name)));
    let replaces = self.replaces(name).map(str::to_string);
    // the push of an action replaces what a Safe Push start replaces: its robot lands the ancestors it holds no longer;
    // one that landed (a merged Safe Push or the fetched baseline has it whole) is left as it is, as a start leaves it
    // (`push::start_push`)
    let ancestors = self
      .ancestors_nearest_first(name)
      .into_iter()
      .map(|ancestor| (ancestor, self.kept_from_start(ancestor, replaces.as_deref())))
      .filter(|(_, kept)| *kept != Some(Kept::Landed));
    let chain = std::iter::once((name, None)).chain(ancestors).map(|(branch, kept)| PushStep {
      name: branch,
      not_built: self.table.not_built.get(branch).copied(),
      remote: self.remote_of(branch),
      held: kept == Some(Kept::Held),
    });
    BranchLanding {
      hold: self.holds.get(name).cloned(),
      start: start_refusal(&start, PatronusPushMode::RebaseAutosquash, &self.robots),
      dry_run: start_refusal(&start, PatronusPushMode::DryRun, &self.robots),
      push: push_blocker(chain),
      replaces,
    }
  }

  fn stack_landing(&self, stack: &StackBranchUpdate) -> StackLanding {
    let start = Start::WholeStack(self.stack_start(stack));
    let replaces = self.stack_replaces(stack).map(str::to_string);
    let members = whole_stack_members(&self.repo, &stack.root, &stack.members);
    let rest = whole_stack_rest(&stack.root, &members, stack.commit_count, |name| {
      let landed = self.view.branches.get(name).and_then(|facts| facts.landed_by.as_ref());
      (landed, self.table.branch(name).map_or(0, |branch| u32::try_from(branch.commits.len()).unwrap_or(u32::MAX)))
    });
    StackLanding {
      hold: self.stack_hold(stack),
      start: start_refusal(&start, PatronusPushMode::RebaseAutosquash, &self.robots),
      dry_run: start_refusal(&start, PatronusPushMode::DryRun, &self.robots),
      replaces,
      rest,
    }
  }

  /// The subject part of the decision of a start of `name` (`domain_policy::land::decide`): its robots, what landed it,
  /// why it stays local, what it lands with the remote and the hold of each branch, the robot a Safe Push replaces, and
  /// the open pull request of the whole stack it is a branch of (of `pull_requests`, the listing of the session or of
  /// `bd`); what landed it by the landing report `baseline`, what lands past what the table has landed whole
  /// (`BranchTable::in_baseline`, as the view's pushes read it). `None` for a branch the table lacks
  #[must_use]
  pub fn branch_subject(&self, name: &str, pull_requests: Option<&PullRequestsView>, baseline: &LandedSection) -> Option<BranchSubject<'_>> {
    let name = self.table.branch(name)?.name.as_str();
    let replaces = self.replaces(name);
    let chain = build_ancestors(&self.repo, name)
      .into_iter()
      .chain(std::iter::once(name))
      .map(|branch| LandsBranch {
        name: branch,
        landed_whole: self.landed_whole(branch).is_some(),
        in_baseline: self.table.in_baseline.contains(branch),
        remote: self.remote_of(branch),
        hold: self.holds.get(branch),
        not_built: self.table.not_built.contains_key(branch),
      })
      .collect();
    let whole_stack_pull_request = self
      .table
      .stack_branches
      .iter()
      .find(|stack| stack.members.iter().any(|member| member == name))
      .and_then(|stack| pull_requests?.stack_pull_requests.get(&stack.root))
      .filter(|pull_request| pull_request.state == PullRequestState::Open)
      .map(|pull_request| pull_request.number);
    Some(BranchSubject {
      start: self.branch_start(name, false),
      parked: parked_state(&self.repo, name),
      landed: landed_reason(&self.landed_facts(baseline), &landed_subject(self.table, self.remote, Of::Branch(name), pull_requests)),
      chain,
      tip: self.table.tip_of(name),
      replaces,
      whole_stack_pull_request,
    })
  }

  /// The subject part of the decision of a start of the whole stack of `stack` (`domain_policy::land::decide`): its
  /// robots, the parked member, what landed every branch of it, its branches in the order of the stack branch, the
  /// remote of its stack branch, the robot of the whole stack a Safe Push replaces and the one that holds its pushes
  #[must_use]
  pub fn stack_subject<'s>(&'s self, stack: &'s StackBranchUpdate, pull_requests: Option<&PullRequestsView>, baseline: &LandedSection) -> StackSubject<'s> {
    let root = stack.root.as_str();
    let of = Of::WholeStack { root, members: &stack.members };
    StackSubject {
      replaces: self.stack_replaces(stack),
      start: self.stack_start(stack),
      parked: whole_stack_parked(&self.repo, stack.members.iter().map(String::as_str)),
      landed: landed_reason(&self.landed_facts(baseline), &landed_subject(self.table, self.remote, of, pull_requests)),
      members: stack.members.iter().map(|member| (member.as_str(), self.landed_whole(member).is_some())).collect(),
      remote: self.remote.stacks.get(root).and_then(RemoteFact::branch),
      held_by: self.stack_holder(stack),
    }
  }

  /// The robot of the whole stack of `stack` that holds the pushes of its stack branch: it runs on it as a Safe Push
  fn stack_holder(&self, stack: &StackBranchUpdate) -> Option<&'a PatronusRobot> {
    self
      .stack_robot(stack)
      .filter(|robot| holds_pushes(robot) && robot.stack_root.as_deref() == Some(stack.root.as_str()))
  }

  /// The robot of the whole stack a Safe Push start of it replaces: the one that holds its stack branch from an older
  /// commit
  fn stack_replaces(&self, stack: &StackBranchUpdate) -> Option<&'a str> {
    self.stack_holder(stack).filter(|_| self.stack_last_commit(stack).is_none()).map(|robot| robot.id.as_str())
  }

  /// The restart `robot` offers (`restart_action`) and why it can't start now (`restart_refusal`): of its branch, while
  /// the branch has a row; of its whole stack, the stack branch as the sync left it, none for a stack without one. What
  /// the table refuses comes first: a subject that stays local (parked, built on parked, a whole stack with a parked
  /// member), and a branch the sync did not build for a restart that pushes it first
  fn restart(&self, robot: &PatronusRobot, needs_review: bool) -> Option<RobotRestart> {
    let mode = robot.push_mode;
    let (action, refusal) = match (&robot.branch_name, &robot.stack_root) {
      (Some(name), _) => {
        let action = restart_action(robot, self.table.tip_of(name), self.remote_of(name), needs_review)?;
        let refusal = self.table.branch(name).and_then(|branch| {
          // what merged Safe Pushes landed: the view derives without the landing report and the pull requests
          let landed = landed_reason(&self.landed_facts(&NO_LANDING), &landed_subject(self.table, self.remote, Of::Branch(&branch.name), None));
          let start = self.branch_start(&branch.name, false);
          let not_built = start.not_built.filter(|_| action == RestartAction::PushAndRestart).map(|block| Reason::CantPush {
            branch: branch.name.clone(),
            block,
          });
          let table = parked_state(&self.repo, &branch.name)
            .map(|parked| Reason::Parked {
              branch: branch.name.clone(),
              with: parked.with,
              note: parked.note,
            })
            .or(not_built);
          restart_refusal(&Start::Branch(start), mode, &self.robots, table, landed)
        });
        (action, refusal)
      }
      (None, Some(root)) => {
        let stack = self.table.stack_branches.iter().find(|stack| stack.root == *root);
        let remote = self.remote.stacks.get(root).and_then(RemoteFact::branch);
        let action = restart_action(robot, stack.and_then(|stack| stack.tip.as_deref()), remote, needs_review)?;
        let refusal = match stack {
          None => Some(Reason::NoStackBranch { root: root.clone() }),
          Some(stack) => {
            let of = Of::WholeStack { root, members: &stack.members };
            let landed = landed_reason(&self.landed_facts(&NO_LANDING), &landed_subject(self.table, self.remote, of, None));
            let table = whole_stack_parked(&self.repo, stack.members.iter().map(String::as_str)).map(|parked| Reason::StackParked {
              root: root.clone(),
              member: parked.parked.with.clone().unwrap_or_else(|| parked.member.clone()),
            });
            restart_refusal(&Start::WholeStack(self.stack_start(stack)), mode, &self.robots, table, landed)
          }
        };
        (action, refusal)
      }
      (None, None) => return None,
    };
    Some(RobotRestart { action, refusal })
  }
}

/// No landing report: what the Patronus view decides alone (the restarts) reads no fetched baseline
const NO_LANDING: LandedSection = LandedSection {
  baseline: None,
  landing: Vec::new(),
  landed: Vec::new(),
};

/// The subject `of` of `table` as `domain_policy::landed` reads it: the tip of its virtual branch (of the stack branch of
/// a whole stack), its remote branch as `remote` knows it, its pull request in `pull_requests` in any state
#[must_use]
pub fn landed_subject<'a>(table: &'a BranchTable, remote: &'a RemoteFacts, of: Of<'a>, pull_requests: Option<&'a PullRequestsView>) -> LandedSubject<'a> {
  match of {
    Of::Branch(name) => LandedSubject {
      of,
      tip: table.tip_of(name),
      remote: remote.branches.get(name).and_then(RemoteFact::branch),
      pull_request: pull_requests.and_then(|listing| listing.pull_requests.get(name)),
    },
    Of::WholeStack { root, .. } => LandedSubject {
      of,
      tip: table.stack(root).and_then(|stack| stack.tip.as_deref()),
      remote: remote.stacks.get(root).and_then(RemoteFact::branch),
      pull_request: pull_requests.and_then(|listing| listing.stack_pull_requests.get(root)),
    },
  }
}

/// What landed each subject of `state` (`domain_policy::landed::landed_state`), by subject key: every branch and every
/// whole stack that forks, over the landing report of the table, the Patronus view while it stands (what merged Safe
/// Pushes landed) and the listing of the pull requests; none before the first sync
#[must_use]
pub fn landed_states_of(state: &TableState, facts: &FactStore) -> BTreeMap<String, Landed> {
  let Some(table) = facts.table_facts(state) else {
    return BTreeMap::new();
  };
  let part = facts.patronus.part();
  let robots = listed_robots(facts.robots.view());
  let robots = Robots::new(&robots);
  let landed = LandedFacts {
    baseline: &state.parts.sections.landed,
    view: matches!(part.state, FactState::Fresh | FactState::Stale { .. }).then_some(&part.view),
    robots: &robots,
  };
  landed_states(&table.table, &table.remote, &landed, facts.pull_requests.view())
}

/// What landed each subject of `table` (`domain_policy::landed::landed_state`) over `landed` and the listing
/// `pull_requests`, by subject key: every branch and every whole stack that forks
#[must_use]
pub fn landed_states(table: &BranchTable, remote: &RemoteFacts, landed: &LandedFacts<'_>, pull_requests: &PullRequestsView) -> BTreeMap<String, Landed> {
  let pull_requests = Some(pull_requests);
  let branches = table.branches.iter().map(|branch| (branch.name.clone(), Of::Branch(&branch.name)));
  let stacks = table.stack_branches.iter().filter(|stack| !stack.members.is_empty()).map(|stack| {
    (
      stack_readiness_key(&stack.root),
      Of::WholeStack {
        root: &stack.root,
        members: &stack.members,
      },
    )
  });
  branches
    .chain(stacks)
    .filter_map(|(key, of)| Some((key, landed_state(landed, &landed_subject(table, remote, of, pull_requests))?)))
    .collect()
}

/// What the starts of a table are decided on (`lands_of`, `land_decision`): the table of the last sync with the remote
/// branches as the deciding side knows them, its Patronus view and the user's robots, and the facts of the remote
/// services as the session holds them (the scenario facts of `bd` hold their reads so)
pub struct LandSources<'a> {
  pub table: &'a BranchTable,
  pub remote: &'a RemoteFacts,
  pub view: &'a PatronusView,
  pub robots: &'a [PatronusRobot],
  /// The branch of origin the robots push into ("master")
  pub target: &'a str,
  /// The branch prefix: the virtual branches a pull request may go into
  pub prefix: &'a str,
  /// Patronus names the GitHub repository the branches are mirrored to
  pub github: bool,
  pub profiles: &'a Fact<PatronusStartOptions>,
  pub pull_requests: &'a PullRequestsView,
  /// The review readiness of a subject (`stack:<root>` the whole stack); `None` while nobody asked
  pub readiness: &'a dyn Fn(&str) -> Option<&'a Fact<SubjectReadiness>>,
  /// The rules of the target, for a push into an approved pull request
  pub rules: Option<&'a Fact<BranchRules>>,
  /// How the landing needs of `view` stand: a failed computation waits for nothing (the gate of the start checks)
  pub landing_needs: &'a FactState,
  /// The landing report of the table: a subject the fetched baseline has whole has nothing left to land
  pub baseline: &'a LandedSection,
  /// What the landing gate the deciding side ran of this start refuses (`bd`'s, of a whole stack too), in place of the
  /// landing needs of the view; `None`: the view's (a session runs the gate at the start)
  pub gate: Option<&'a [LandingRefusal]>,
  /// A conflict the check of the deciding side against the newest target found; `None` without one, or when nothing
  /// checked (a session checks at the start)
  pub target_conflict: Option<TargetConflictFact<'a>>,
}

/// The decisions of the default intents of every subject of `state` (`LandDecisions`: a Safe Push with the default
/// profile, a Dry Run), by subject key; none before the Patronus view of the session (`land_sources`)
#[must_use]
pub fn lands_of(state: &TableState, facts: &FactStore, prefix: &str) -> BTreeMap<String, LandDecisions> {
  with_land_sources(state, facts, prefix, decide_lands).unwrap_or_default()
}

/// `decide` over the sources of the session: its table, its Patronus view while it stands, its facts; `None` before
/// its Patronus view
pub fn with_land_sources<T>(state: &TableState, facts: &FactStore, prefix: &str, decide: impl FnOnce(&LandSources<'_>) -> T) -> Option<T> {
  with_checked_land_sources(state, facts, prefix, &StartChecks::default(), decide)
}

/// What a start checked itself besides the facts of its session: the refusals of the landing gate it ran on the refs of
/// the last sync (`None`: the landing needs of the view decide), and the commit that conflicts with the newest target
#[derive(Debug, Clone, Default)]
pub struct StartChecks {
  pub gate: Option<Vec<LandingRefusal>>,
  /// The commit (a copy of the virtual branch) and its files
  pub target_conflict: Option<(String, Vec<String>)>,
}

/// `with_land_sources` with what the start checked itself (`StartChecks`)
pub fn with_checked_land_sources<T>(state: &TableState, facts: &FactStore, prefix: &str, checks: &StartChecks, decide: impl FnOnce(&LandSources<'_>) -> T) -> Option<T> {
  let part = facts.patronus.part();
  if !matches!(part.state, FactState::Fresh | FactState::Stale { .. }) {
    return None;
  }
  sources_of(state, facts, prefix, checks, decide)
}

/// The sources of the session as they stand, its Patronus view as it is (none known before the first: no robot holds
/// or lands anything): what a pull request or a review request is planned on, which needs no robot of its own; `None`
/// before the first table
pub fn with_pull_request_sources<T>(state: &TableState, facts: &FactStore, prefix: &str, decide: impl FnOnce(&LandSources<'_>) -> T) -> Option<T> {
  sources_of(state, facts, prefix, &StartChecks::default(), decide)
}

/// `decide` over the sources of the session of `state` and `facts`; `None` before the first table
fn sources_of<T>(state: &TableState, facts: &FactStore, prefix: &str, checks: &StartChecks, decide: impl FnOnce(&LandSources<'_>) -> T) -> Option<T> {
  let part = facts.patronus.part();
  let table = facts.table_facts(state)?;
  let robots = listed_robots(facts.robots.view());
  let target = baseline_on_remote(&state.parts.header.baseline_branch);
  let readiness = |key: &str| facts.readiness.fact(key);
  let sources = LandSources {
    table: &table.table,
    remote: &table.remote,
    view: &part.view,
    robots: &robots,
    target,
    prefix,
    github: facts.robots.view().head.value.as_ref().is_some_and(|head| head.github_repository.is_some()),
    profiles: facts.profiles.fact(),
    pull_requests: facts.pull_requests.view(),
    readiness: &readiness,
    rules: facts.rules.get(target).map(FactRecord::fact),
    landing_needs: &part.landing_needs,
    baseline: &state.parts.sections.landed,
    gate: checks.gate.as_deref(),
    target_conflict: checks.target_conflict.as_ref().map(|(commit, files)| TargetConflictFact { commit, files }),
  };
  Some(decide(&sources))
}

/// The decisions of the default intents of every branch and of every whole stack that forks
#[must_use]
pub fn decide_lands(sources: &LandSources<'_>) -> BTreeMap<String, LandDecisions> {
  let landing = LandingFacts::new(sources.table, sources.remote, sources.view, sources.robots, None);
  let branches = sources.table.branches.iter().map(|branch| Subject::branch(&branch.name));
  let stacks = sources
    .table
    .stack_branches
    .iter()
    .filter(|stack| !stack.members.is_empty())
    .map(|stack| Subject::whole_stack(&stack.root));
  branches
    .chain(stacks)
    .filter_map(|subject| {
      let decide = |mode| decide_with(sources, &landing, &default_land(subject.clone(), mode)).map(|(decision, _)| decision);
      let decisions = LandDecisions {
        safe_push: decide(PatronusPushMode::RebaseAutosquash)?,
        dry_run: decide(PatronusPushMode::DryRun)?,
      };
      Some((subject.key(), decisions))
    })
    .collect()
}

/// The decision of `land` with the profiles it offers; `None` for a subject the table lacks, or a whole stack that
/// doesn't fork
#[must_use]
pub fn land_decision(sources: &LandSources<'_>, land: &Land) -> Option<(LandDecision, LandProfiles)> {
  let landing = LandingFacts::new(sources.table, sources.remote, sources.view, sources.robots, None);
  let (decision, last) = decide_with(sources, &landing, land)?;
  let profiles = standing(sources.profiles).map_or_default(|options| offered_profiles(options, last, land.mode, land.without_approvals));
  Some((decision, profiles))
}

/// A start of `subject` in `mode` with the default profile, nothing allowed
fn default_land(subject: Subject, mode: PatronusPushMode) -> Land {
  Land {
    subject,
    mode,
    profile: ProfileChoice::Default,
    without_approvals: false,
    create_pull_request: false,
    allow: LandingAllowances::default(),
  }
}

/// `decide` of `land` over `sources`, with the profile of the last robot of its subject (the default); `None` for a
/// subject the table lacks, or a whole stack that doesn't fork
fn decide_with<'s>(sources: &LandSources<'s>, landing: &LandingFacts<'s>, land: &Land) -> Option<(LandDecision, Option<&'s PatronusRobotProfile>)> {
  let (subject, robot, needs) = match &land.subject {
    Subject::Branch { name } => {
      let needs = match sources.view.branches.get(name.as_str()).map(|facts| &facts.landing_needs) {
        Some(LandingNeeds::Known { refusals }) => Some(refusals.as_slice()),
        _ if matches!(sources.landing_needs, FactState::Unavailable { .. }) => Some(&[][..]),
        _ => None,
      };
      (
        SubjectFacts::Branch(landing.branch_subject(name, Some(sources.pull_requests), sources.baseline)?),
        landing.robot_of(name),
        sources.gate.or(needs),
      )
    }
    Subject::WholeStack { root } => {
      let stack = sources.table.stack_branches.iter().find(|stack| stack.root == *root && !stack.members.is_empty())?;
      // the landing needs are of the branches: the gate of the start checks the whole stack
      (
        SubjectFacts::WholeStack(landing.stack_subject(stack, Some(sources.pull_requests), sources.baseline)),
        landing.stack_robot(stack),
        Some(sources.gate.unwrap_or_default()),
      )
    }
  };
  let last = robot.and_then(|robot| robot.profile.as_ref());
  let facts = LandFacts {
    target: sources.target,
    prefix: sources.prefix,
    github: sources.github,
    robots: &landing.robots,
    subject,
    profiles: sources.profiles,
    last_profile: last,
    pull_requests: sources.pull_requests,
    readiness: (sources.readiness)(&land.subject.key()),
    rules: sources.rules,
    landing: needs,
    target_conflict: sources.target_conflict,
  };
  Some((decide(&facts, land), last))
}

/// What a row of the table says of its subject for its next step and its node: the sync of it, its failure, its
/// commits, its block and whether it stays local. A session reads it of its rows (`row_of_view`), `bd status` of the
/// grouping it read without a sync (`grouped_rows`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowFacts {
  pub syncing: bool,
  pub status: BranchStatus,
  pub sync_failed: bool,
  /// The reason of its stack problem, where the sync says it
  pub problem: Option<String>,
  pub commit_count: usize,
  /// The parent of a blocked branch (`blocking_parent`)
  pub blocked_by: Option<String>,
  pub parked: Option<Parked>,
  /// Of a root: the parked member its stack branch holds, with the branch it is parked with
  pub stack_parked: Option<Parked>,
}

/// The row facts of a branch of a session
#[must_use]
pub fn row_of_view(branch: &BranchView) -> RowFacts {
  let stack = branch.head.stack.as_ref();
  RowFacts {
    syncing: branch.sync.status == BranchStatus::Syncing,
    status: branch.sync.status,
    sync_failed: branch.sync.has_error,
    problem: stack.and_then(|stack| stack.problems.first()).map(|problem| problem.reason.clone()),
    commit_count: branch.commits.len(),
    blocked_by: blocking_parent(branch.sync.status, stack),
    parked: branch.structure.parked.clone(),
    stack_parked: branch.structure.stack_parked.as_ref().map(member_parked),
  }
}

/// The parent a blocked branch waits for: the one its block names, else the branch it is built on, else its declared
/// parent; none for a branch that is not blocked
pub(crate) fn blocking_parent(status: BranchStatus, stack: Option<&BranchStack>) -> Option<String> {
  if status != BranchStatus::Blocked {
    return None;
  }
  let stack = stack?;
  stack
    .blocked_by
    .as_ref()
    .map(|block| block.parent.clone())
    .or_else(|| stack.effective_base.clone())
    .or_else(|| stack.declared_parent.clone())
}

/// The rows of `table` as a grouping read without a sync has them (`bd status`, `bd safe-push --check` over the table of
/// its sync), in its order
#[must_use]
pub fn grouped_rows(table: &BranchTable) -> Vec<(String, RowFacts)> {
  let repo = Repository::of_grouping(&table.branches, &table.parked);
  table
    .branches
    .iter()
    .map(|branch| (branch.name.clone(), row_of_grouped(&repo, branch, table.stack(&branch.name))))
    .collect()
}

/// The row facts of a grouped branch of `repo`: nothing syncs, a failure is not known; `stack` the stack branch of a
/// root
fn row_of_grouped(repo: &Repository<'_>, branch: &GroupedBranchInfo, stack: Option<&StackBranchUpdate>) -> RowFacts {
  let placed = branch.stack.as_ref();
  let blocked_by = placed.and_then(|placed| placed.blocked_by.as_ref()).map(|block| block.parent.clone());
  RowFacts {
    syncing: false,
    status: if blocked_by.is_some() { BranchStatus::Blocked } else { BranchStatus::Unchanged },
    sync_failed: false,
    problem: placed.and_then(|placed| placed.problems.first()).map(|problem| problem.reason.clone()),
    commit_count: branch.commits.len(),
    blocked_by,
    parked: parked_state(repo, &branch.name),
    stack_parked: stack_parked(repo, stack).as_ref().map(member_parked),
  }
}

/// The parked member a stack branch holds, as the step of the whole stack names it: the branch it is parked with
fn member_parked(member: &ParkedMember) -> Parked {
  Parked {
    note: member.parked.note.clone(),
    with: Some(member.parked.with.clone().unwrap_or_else(|| member.member.clone())),
  }
}

/// What the next steps and the nodes of a table are decided on (`next_steps`): the table of the last sync with the
/// remote branches as the deciding side knows them, its rows as they are now, the Patronus view and the robots, and the
/// facts of the remote services as the session holds them (`bd` holds its own reads, those it didn't read unknown)
pub struct StepSources<'a> {
  pub table: &'a BranchTable,
  pub remote: &'a RemoteFacts,
  /// The rows, in the order of the table, by branch
  pub rows: &'a [(String, RowFacts)],
  pub view: &'a PatronusView,
  pub robots: &'a RobotsView,
  /// How Patronus stands for the repository (`PatronusStanding::of` its head and the state of the view)
  pub patronus: &'a PatronusStanding,
  /// How the landing needs of `view` stand
  pub landing_needs: &'a FactState,
  pub pull_requests: &'a PullRequestsView,
  /// Where each open pull request goes (`open_pull_requests_view`), by subject key
  pub open_pull_requests: &'a BTreeMap<String, PullRequestView>,
  /// What landed each subject (`landed_states`), by subject key
  pub landed: &'a BTreeMap<String, Landed>,
  /// The review readiness of a subject (`stack:<root>` the whole stack); `None` while nobody asked
  pub readiness: &'a dyn Fn(&str) -> Option<&'a Fact<SubjectReadiness>>,
  /// The branch of origin the robots push into ("master")
  pub target: &'a str,
  /// The stack branches as the view of a session has them, by root, whose tips its rows show (`TableState::stack`);
  /// `None`: those of `table`
  pub view_stacks: Option<&'a BTreeMap<String, StackBranchUpdate>>,
}

impl StepSources<'_> {
  fn robot(&self, id: Option<&str>) -> Option<&PatronusRobot> {
    self.robots.robots.get(id?)
  }

  /// The robot of a subject with what the session learned of it; `restart` whether its restart is offered
  fn step_robot(&self, id: Option<&str>, restart: impl Fn(&PatronusRobot) -> bool) -> Option<StepRobot<'_>> {
    let robot = self.robot(id)?;
    let offered = self
      .view
      .restarts
      .get(&robot.id)
      .is_some_and(|offered| offered.action == RestartAction::Restart || restart(robot));
    Some(StepRobot {
      robot,
      needs_review: self.robots.needs_review.get(&robot.id).copied(),
      problems_failed: self.robots.problems_failed.get(&robot.id).map(String::as_str),
      failing: self.robots.failing.contains(&robot.id),
      restart: offered,
    })
  }

  /// The open pull request of the whole stack `name` is a branch of
  fn stack_pull_request_of(&self, name: &str) -> Option<StackPullRequest> {
    let stack = self.table.stack_branches.iter().find(|stack| stack.members.iter().any(|member| member == name))?;
    let pull_request = self.pull_requests.stack_pull_requests.get(&stack.root).filter(|it| it.state == PullRequestState::Open)?;
    Some(StackPullRequest {
      number: pull_request.number,
      root: stack.root.clone(),
    })
  }

  /// The GitHub repository of the branches is known
  fn repository(&self) -> bool {
    github_repository_known(self.pull_requests, self.robots)
  }
}

/// The GitHub repository of the branches is known: GitHub lists it, or Patronus names the mirror
pub(crate) fn github_repository_known(pull_requests: &PullRequestsView, robots: &RobotsView) -> bool {
  matches!(pull_requests.head.value, Some(PullRequestAvailability::Available { .. })) || robots.head.value.as_ref().is_some_and(|head| head.github_repository.is_some())
}

/// What holds the push of a branch now, with its robot: a hold of the view (`BranchLanding::hold`) whose robot holds
/// pushes, or merged for a hold of what landed
pub(crate) fn live_hold<'a>(facts: &'a BranchPatronus, robots: &'a RobotsView) -> Option<(&'a HoldView, &'a PatronusRobot)> {
  let hold = facts.landing.hold.as_ref()?;
  let robot = robots.robots.get(&hold.by.robot_id)?;
  let holds = match hold.kind {
    HoldKind::Landed => robot.status == PatronusRobotStatus::Success,
    HoldKind::Own | HoldKind::Span => holds_pushes(robot),
  };
  holds.then_some((hold, robot))
}

/// The next step and the node of every branch of the rows of `sources` and of every whole stack that forks, by subject
/// key (`domain_policy::next_step`, `domain_policy::landing_node`)
#[must_use]
pub fn next_steps(sources: &StepSources<'_>) -> BTreeMap<String, NextStepView> {
  let repo = Repository::of_grouping(&sources.table.branches, &sources.table.parked);
  let repository = sources.repository();
  let patronus = sources.patronus.answer();
  let listing = Listing::of(&sources.pull_requests.head);
  let rows: HashMap<&str, &RowFacts> = sources.rows.iter().map(|(name, row)| (name.as_str(), row)).collect();
  let mut steps = BTreeMap::new();
  for (name, row) in sources.rows {
    let facts = sources.view.branches.get(name);
    let landing = facts.map(|facts| &facts.landing);
    let robot = sources.step_robot(facts.and_then(|facts| facts.robot_id.as_deref()), |_| true);
    let landed_by = facts.and_then(|facts| facts.landed_by.as_ref());
    let own_robot_landed = landed_by
      .zip(robot)
      .is_some_and(|(landed, robot)| landed.own && landed.after.is_empty() && landed.by.robot_id == robot.robot.id);
    let hold = facts.and_then(|facts| live_hold(facts, sources.robots));
    let subject = Subject::branch(name);
    let stack_pull_request = sources.stack_pull_request_of(name);
    let remote = sources.remote.branches.get(name.as_str()).and_then(RemoteFact::branch);
    let landed_whole = |ancestor: &&str| {
      let landed = sources.view.branches.get(*ancestor).and_then(|facts| facts.landed_by.as_ref());
      landed.is_some_and(|landed| landed.after.is_empty() && sources.robot(Some(&landed.by.robot_id)).is_some_and(|robot| robot.status == PatronusRobotStatus::Success))
    };
    let ancestors = build_ancestors(&repo, name);
    let step = StepFacts {
      subject: &subject,
      patronus,
      syncing: row.syncing,
      sync_failed: row.sync_failed,
      problem: row.problem.as_deref(),
      commit_count: row.commit_count,
      blocked: row.status == BranchStatus::Blocked,
      start: landing.and_then(|landing| landing.start.as_ref()),
      dry_run_start: landing.and_then(|landing| landing.dry_run.as_ref()),
      landing_part: facts
        .and_then(|facts| facts.landing_ancestor.as_ref())
        .is_some_and(|ancestor| sources.robot(Some(&ancestor.robot_id)).is_some_and(is_active)),
      held: hold.is_some(),
      parked: row.parked.as_ref(),
      landed: sources.landed.get(name.as_str()),
      tip: sources.table.tip_of(name),
      remote,
      robot,
      own_robot_landed,
      listing,
      pull_request: sources.pull_requests.pull_requests.get(name.as_str()).map(|pull_request| StepPullRequest {
        pull_request,
        base: sources.open_pull_requests.get(name.as_str()).map(|open| &open.base),
      }),
      target: sources.target,
      needs: Needs::of(facts.map(|facts| &facts.landing_needs), sources.landing_needs),
      readiness: (sources.readiness)(name),
      stack_pull_request: stack_pull_request.as_ref(),
      repository,
      lands: u32::try_from(ancestors.iter().filter(|ancestor| !landed_whole(ancestor)).count() + 1).unwrap_or(u32::MAX),
    };
    let node = NodeFacts {
      step,
      status: row.status,
      blocked_by: row.blocked_by.as_deref(),
      lands_with: facts
        .and_then(|facts| facts.span.as_ref())
        .filter(|span| sources.robot(Some(&span.by.robot_id)).is_some_and(is_pushing))
        .map(|span| &span.by),
      push_pending: remote.is_some_and(|remote| !remote.up_to_date),
    };
    let view = NextStepView {
      step: next_step(&step),
      node: landing_node(&node),
    };
    steps.insert(name.clone(), view);
  }
  for stack in sources.table.stack_branches.iter().filter(|stack| !stack.members.is_empty()) {
    let key = stack_readiness_key(&stack.root);
    let facts = sources.view.stacks.get(&stack.root);
    let landing = facts.map(|facts| &facts.landing);
    let rest = landing.map(|landing| &landing.rest);
    let robot = sources.step_robot(facts.and_then(|facts| facts.robot_id.as_deref()), |robot| {
      robot.branch_name.is_none() && robot.stack_root.is_some()
    });
    let subject = Subject::whole_stack(&stack.root);
    let root_row = rows.get(stack.root.as_str());
    let held = matches!(landing.and_then(|landing| landing.hold.as_ref()), Some(Reason::StackHeld { robot_id, .. }) if sources.robot(Some(robot_id)).is_some_and(holds_pushes));
    // before the view names what is left of it, every branch of its stack branch
    let lands = rest
      .filter(|rest| !rest.members.is_empty() || !rest.landed.is_empty())
      .map_or(stack.members.len(), |rest| rest.members.len());
    let step = StepFacts {
      subject: &subject,
      patronus,
      syncing: stack.members.iter().any(|member| rows.get(member.as_str()).is_some_and(|row| row.syncing)),
      sync_failed: false,
      problem: None,
      commit_count: usize::try_from(stack.commit_count).unwrap_or(usize::MAX),
      blocked: false,
      start: landing.and_then(|landing| landing.start.as_ref()),
      dry_run_start: landing.and_then(|landing| landing.dry_run.as_ref()),
      landing_part: facts
        .and_then(|facts| facts.landing_member.as_ref())
        .is_some_and(|member| sources.robot(Some(&member.robot_id)).is_some_and(is_active)),
      held,
      parked: root_row.and_then(|row| row.stack_parked.as_ref()),
      landed: sources.landed.get(&key),
      // the tip of its stack branch as the view shows it, as the remote status is the view's (`session_remote`)
      tip: sources
        .view_stacks
        .and_then(|stacks| stacks.get(&stack.root))
        .map_or(stack.tip.as_deref(), |view| view.tip.as_deref()),
      remote: sources.remote.stacks.get(&stack.root).and_then(RemoteFact::branch),
      robot,
      own_robot_landed: rest.is_some_and(|rest| rest.by_own_robot && rest.members.is_empty()),
      listing,
      pull_request: sources.pull_requests.stack_pull_requests.get(&stack.root).map(|pull_request| StepPullRequest {
        pull_request,
        base: sources.open_pull_requests.get(&key).map(|open| &open.base),
      }),
      target: sources.target,
      // the landing needs are of the branches: the gate of the start checks the whole stack
      needs: Needs::Unknown,
      readiness: (sources.readiness)(&key),
      stack_pull_request: None,
      repository,
      lands: u32::try_from(lands).unwrap_or(u32::MAX),
    };
    let node = NodeFacts {
      step,
      status: BranchStatus::Unchanged,
      blocked_by: None,
      lands_with: None,
      push_pending: false,
    };
    let view = NextStepView {
      step: next_step(&step),
      node: landing_node(&node),
    };
    steps.insert(key, view);
  }
  steps
}

/// The open pull request of each subject of `state` as the rules read it (`open_pull_requests_view`), over the pull
/// requests and the readiness of the session of `prefix`
#[must_use]
pub fn session_open_pull_requests(state: &TableState, facts: &FactStore, prefix: &str) -> BTreeMap<String, PullRequestView> {
  let parts = &state.parts;
  let readiness = |key: &str| facts.readiness.fact(key)?.value.as_ref().map(|subject| &subject.readiness);
  open_pull_requests_view(
    baseline_on_remote(&parts.header.baseline_branch),
    |name| parts.branches.contains_key(name),
    |root| parts.stacks.contains_key(root),
    facts.pull_requests.view(),
    readiness,
    prefix,
  )
}

/// The next step and the node of every row of `state` and of every whole stack that forks (`next_steps`), over the
/// facts of the session: where each open pull request goes (`open`, `open_pull_requests_view`) and what landed each
/// subject (`landed`, `landed_states_of`); none before the first sync
#[must_use]
pub fn next_steps_of(state: &TableState, facts: &FactStore, open: &BTreeMap<String, PullRequestView>, landed: &BTreeMap<String, Landed>) -> BTreeMap<String, NextStepView> {
  let Some(table) = facts.table_facts(state) else {
    return BTreeMap::new();
  };
  let parts = &state.parts;
  let rows: Vec<(String, RowFacts)> = parts.order.iter().filter_map(|name| Some((name.clone(), row_of_view(parts.branches.get(name)?)))).collect();
  let part = facts.patronus.part();
  let robots = facts.robots.view();
  let patronus = PatronusStanding::of(&robots.head, &part.state);
  let readiness = |key: &str| facts.readiness.fact(key);
  let sources = StepSources {
    table: &table.table,
    remote: &table.remote,
    rows: &rows,
    view: &part.view,
    robots,
    patronus: &patronus,
    landing_needs: &part.landing_needs,
    pull_requests: facts.pull_requests.view(),
    open_pull_requests: open,
    landed,
    readiness: &readiness,
    target: baseline_on_remote(&parts.header.baseline_branch),
    view_stacks: Some(&parts.stacks),
  };
  next_steps(&sources)
}

/// The subjects a window shows (`Demand::visible_branches`, `visible_stacks` by root) whose next step waits for the
/// readiness (`domain_policy::readiness::wanted`): the session asks only those of them, by subject key
#[must_use]
pub fn wanted_readiness(state: &TableState, facts: &FactStore, prefix: &str, demand: &Demand) -> HashSet<String> {
  let open = session_open_pull_requests(state, facts, prefix);
  let steps = next_steps_of(state, facts, &open, &landed_states_of(state, facts));
  let stacks: Vec<String> = demand.visible_stacks.iter().map(|root| stack_readiness_key(root)).collect();
  let shown = demand.visible_branches.iter().chain(&stacks).map(String::as_str);
  wanted(shown, |key| steps.get(key).map(|view| &view.step)).into_iter().map(str::to_owned).collect()
}

/// What the rules decide of `table` over the Patronus facts of `view` (module docs), written into it. `running_spans`:
/// by branch, the first running robot that lands it; `needs_review`: by robot id, the robots that failed for missing
/// approvals
pub(crate) fn decide_landing(
  view: &mut PatronusView,
  table: &BranchTable,
  remote: &RemoteFacts,
  robots: &[PatronusRobot],
  running_spans: &HashMap<String, SpanView>,
  needs_review: &BTreeMap<String, bool>,
) {
  let (branches, stacks, restarts) = {
    let facts = LandingFacts::new(table, remote, view, robots, Some(running_spans));
    let branches: Vec<(String, BranchLanding)> = table.branches.iter().map(|branch| (branch.name.clone(), facts.branch_landing(&branch.name))).collect();
    let stacks: Vec<(String, StackLanding)> = table
      .stack_branches
      .iter()
      .filter(|stack| !stack.members.is_empty())
      .map(|stack| (stack.root.clone(), facts.stack_landing(stack)))
      .collect();
    let restarts: BTreeMap<String, RobotRestart> = robots
      .iter()
      .filter_map(|robot| Some((robot.id.clone(), facts.restart(robot, needs_review.get(&robot.id).copied().unwrap_or(false))?)))
      .collect();
    (branches, stacks, restarts)
  };
  for (name, landing) in branches {
    if landing != BranchLanding::default() {
      view.branches.entry(name).or_default().landing = landing;
    }
  }
  for (root, landing) in stacks {
    if landing != StackLanding::default() {
      view.stacks.entry(root).or_default().landing = landing;
    }
  }
  view.restarts = restarts;
}

#[cfg(test)]
#[path = "derive_test.rs"]
mod derive_test;

#[cfg(test)]
#[path = "derive_budget_test.rs"]
pub(crate) mod derive_budget_test;
