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
//! view derives with each patch but while a sync runs; `land_answer_of` the decision of another intent (a chosen
//! profile, a start without approvals) with the profiles it offers, which the app reads by `decide_land`.

use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::facts::{FactRecord, FactStore, listed_robots};
use crate::table::{TableParts, TableState};
use domain_policy::facts::{RemoteBranch, Repository, Robots};
use domain_policy::holds::{PushStep, StackHoldFacts, branch_hold, holds_pushes, push_blocker, restart_action, stack_branch_hold};
use domain_policy::land::{
  BranchStart, BranchSubject, LandFacts, LandsBranch, StackStart, StackSubject, Start, SubjectFacts, decide, offered_profiles, on_last_commit, restart_refusal, standing,
  start_refusal,
};
use domain_policy::parked::{parked_state, whole_stack_parked};
use domain_policy::pull_request::base_of;
use domain_policy::push::{HeadedStack, Kept, PushState, branch_push};
use domain_policy::reasons::{landed, stack_landed, stack_not_built};
use domain_policy::review::candidates;
use domain_policy::stack::{WholeStack, build_ancestors, children, declared_parent, descendants, main_line, whole_stack_members, whole_stack_of, whole_stack_rest};
use domain_types::branch_names::baseline_on_remote;
use domain_types::landing::{Land, LandAnswer, LandDecision, LandDecisions, ProfileChoice, Subject};
use domain_types::patronus::{LandingAllowances, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusStartOptions, ReviewReadiness};
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestState, PullRequestView};
use domain_types::reason::Reason;
use domain_types::repository_view::{BranchLanding, HoldKind, HoldView, LandingNeeds, PatronusView, RobotOf, RobotRestart, SpanView, StackLanding};
use domain_types::session::{
  BranchPush, BranchSection, BranchStatus, BranchStructure, Fact, FactState, ParkedMember, PullRequestsView, RowsView, SubjectReadiness, WholeStackView, stack_readiness_key,
};
use domain_types::stack::StackBranchUpdate;
use std::collections::{BTreeMap, HashMap, HashSet};

/// The input of the rules over the table of `table`, as its view has it
#[must_use]
pub fn repository(table: &TableState) -> Repository<'_> {
  parts_repository(&table.parts)
}

fn parts_repository(parts: &TableParts) -> Repository<'_> {
  Repository::of_view(&parts.order, &parts.branches, &parts.header.parked)
}

/// The structure of every branch of `parts` and the rows of each section, written into them
pub(crate) fn place(parts: &mut TableParts) {
  let (structures, rows) = {
    let repo = parts_repository(parts);
    let structures: BTreeMap<String, BranchStructure> = repo.order().map(|name| (name.to_string(), structure_of(&repo, &parts.stacks, name))).collect();
    let (parked, main): (Vec<&String>, Vec<&String>) = parts
      .order
      .iter()
      .filter(|name| structures.contains_key(*name))
      .partition(|name| structures[*name].section == BranchSection::Parked);
    let rows = RowsView {
      main: main.into_iter().cloned().collect(),
      parked: parked.into_iter().cloned().collect(),
    };
    (structures, rows)
  };
  for (name, structure) in structures {
    if let Some(branch) = parts.branches.get_mut(&name) {
      branch.structure = structure;
    }
  }
  parts.rows = rows;
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
/// `patronus` and the pull requests of the whole stacks in `pull_requests`; a branch whose pushes send it alone is not
/// listed. A branch the fetched baseline has whole is landed: the update archives it, its push lands nothing
pub(crate) fn push_view(parts: &TableParts, patronus: &PatronusView, pull_requests: &PullRequestsView) -> BTreeMap<String, BranchPush> {
  let repo = parts_repository(parts);
  let facts = PushFacts {
    parts,
    patronus,
    fully_landed: parts
      .sections
      .landed
      .landing
      .iter()
      .filter(|branch| branch.remaining == 0)
      .filter_map(|branch| branch.name.as_deref())
      .collect(),
  };
  let facts = &facts;
  let held = |name: &str| facts.state(name, None);
  repo
    .order()
    .filter_map(|name| {
      // the robot a Safe Push start replaces: the start cancels it, and pushes what it held
      let replaces = patronus.branches.get(name).and_then(|facts| facts.landing.replaces.as_deref());
      let start = replaces.map(|replaces| move |branch: &str| facts.state(branch, Some(replaces)));
      let stack = parts.stacks.get(name).map(|stack| HeadedStack {
        stack,
        pull_request_open: pull_requests
          .stack_pull_requests
          .get(name)
          .is_some_and(|pull_request| pull_request.state == PullRequestState::Open),
        hold: patronus.stacks.get(name).and_then(|facts| facts.landing.hold.as_ref()),
      });
      let push = branch_push(&repo, held, start.as_ref().map(|start| start as &dyn Fn(&str) -> PushState), stack, name);
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

/// What `push_view` reads of a branch
struct PushFacts<'a> {
  parts: &'a TableParts,
  patronus: &'a PatronusView,
  /// The branches the fetched baseline has whole
  fully_landed: HashSet<&'a str>,
}

impl PushFacts<'_> {
  /// The state of `name` for a push; `replaces`: the robot the push cancels, which holds nothing then
  fn state(&self, name: &str, replaces: Option<&str>) -> PushState {
    let Some(branch) = self.parts.branches.get(name) else {
      return PushState::default();
    };
    let hold = self.patronus.branches.get(name).and_then(|facts| facts.landing.hold.as_ref());
    let kept = if branch.structure.parked.is_some() {
      Some(Kept::Parked)
    } else if hold.is_some_and(|hold| hold.kind == HoldKind::Landed) || self.fully_landed.contains(name) {
      Some(Kept::Landed)
    } else if hold.is_some_and(|hold| Some(hold.by.robot_id.as_str()) != replaces) {
      Some(Kept::Held)
    } else if branch.sync.status == BranchStatus::Blocked || branch.sync.has_error {
      Some(Kept::NotBuilt)
    } else {
      None
    };
    PushState {
      action: branch.remote.status.as_ref().map(|status| status.action),
      kept,
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

  /// What a start of `name` pushes: the branch, and the ancestors with work up to the first held one, which its robot
  /// lands; a remote not compared yet pushes nothing known
  #[must_use]
  pub fn start_pushes(&self, name: &str) -> bool {
    let has_work = |branch: &str| self.remote_of(branch).is_some_and(|remote| !remote.up_to_date);
    has_work(name)
      || self
        .ancestors_nearest_first(name)
        .into_iter()
        .take_while(|ancestor| !self.holds.contains_key(ancestor))
        .any(has_work)
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

  /// Why nothing of `name` is left to land: a merged Safe Push landed all of it
  fn landed_reason(&self, name: &str) -> Option<Reason> {
    let by = self.view.branches.get(name)?.landed_by.as_ref().filter(|landed| landed.after.is_empty())?;
    Some(landed(name, by, self.robots.get(&by.by.robot_id)?))
  }

  /// Merged Safe Pushes landed every member of `stack` whole: what landed each
  fn stack_landed_whole(&self, stack: &StackBranchUpdate) -> Option<Vec<(&'a RobotOf, &'a PatronusRobot)>> {
    stack.members.iter().map(|member| self.landed_whole(member)).collect()
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
    // the push of an action replaces what a Safe Push start replaces: its robot lands the ancestors it holds no longer
    let held = |ancestor: &str| self.holds.get(ancestor).is_some_and(|hold| Some(hold.by.robot_id.as_str()) != replaces.as_deref());
    let chain = std::iter::once(name).chain(self.ancestors_nearest_first(name)).map(|branch| PushStep {
      name: branch,
      not_built: self.table.not_built.get(branch).copied(),
      remote: self.remote_of(branch),
      held: held(branch),
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
  /// `bd`). `None` for a branch the table lacks
  #[must_use]
  pub fn branch_subject(&self, name: &str, pull_requests: Option<&PullRequestsView>) -> Option<BranchSubject<'_>> {
    let name = self.table.branch(name)?.name.as_str();
    let replaces = self.replaces(name);
    let held = |branch: &str| self.holds.get(branch).is_some_and(|hold| Some(hold.by.robot_id.as_str()) != replaces);
    let chain = build_ancestors(&self.repo, name)
      .into_iter()
      .chain(std::iter::once(name))
      .map(|branch| LandsBranch {
        name: branch,
        landed_whole: self.landed_whole(branch).is_some(),
        remote: self.remote_of(branch),
        held: held(branch),
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
      landed: self.landed_reason(name),
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
  pub fn stack_subject<'s>(&'s self, stack: &'s StackBranchUpdate) -> StackSubject<'s> {
    let root = stack.root.as_str();
    StackSubject {
      replaces: self.stack_replaces(stack),
      start: self.stack_start(stack),
      parked: whole_stack_parked(&self.repo, stack.members.iter().map(String::as_str)),
      landed: self.stack_landed_whole(stack).and_then(|landed| stack_landed(root, &landed)),
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
  /// the branch has a row; of its whole stack, the stack branch as the sync left it, none for a stack without one
  fn restart(&self, robot: &PatronusRobot, needs_review: bool) -> Option<RobotRestart> {
    let mode = robot.push_mode;
    let (action, refusal) = match (&robot.branch_name, &robot.stack_root) {
      (Some(name), _) => {
        let action = restart_action(robot, self.table.tip_of(name), self.remote_of(name), needs_review)?;
        let refusal = self.table.branch(name).and_then(|branch| {
          let landed = self.landed_reason(&branch.name);
          restart_refusal(&Start::Branch(self.branch_start(&branch.name, false)), mode, &self.robots, landed)
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
            let landed = self.stack_landed_whole(stack).and_then(|landed| stack_landed(root, &landed));
            restart_refusal(&Start::WholeStack(self.stack_start(stack)), mode, &self.robots, landed)
          }
        };
        (action, refusal)
      }
      (None, None) => return None,
    };
    Some(RobotRestart { action, refusal })
  }
}

/// What the starts of a table are decided on (`lands_of`, `land_answer`): the table of the last sync with the remote
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
}

/// The decisions of the default intents of every subject of `state` (`LandDecisions`: a Safe Push with the default
/// profile, a Dry Run), by subject key; none before the Patronus view of the session (`land_sources`)
#[must_use]
pub fn lands_of(state: &TableState, facts: &FactStore, prefix: &str) -> BTreeMap<String, LandDecisions> {
  with_land_sources(state, facts, prefix, decide_lands).unwrap_or_default()
}

/// The answer of the start `land` of the session (`decide_land` of the app): its decision and the profiles it offers;
/// `None` before the Patronus view, or for a subject the table lacks
#[must_use]
pub fn land_answer_of(state: &TableState, facts: &FactStore, prefix: &str, land: &Land) -> Option<LandAnswer> {
  with_land_sources(state, facts, prefix, |sources| land_answer(sources, land)).flatten()
}

/// `decide` over the sources of the session: its table, its Patronus view while it stands, its facts
fn with_land_sources<T>(state: &TableState, facts: &FactStore, prefix: &str, decide: impl FnOnce(&LandSources<'_>) -> T) -> Option<T> {
  let part = facts.patronus.part();
  if !matches!(part.state, FactState::Fresh | FactState::Stale { .. }) {
    return None;
  }
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
      Some((subject_key(&subject), decisions))
    })
    .collect()
}

/// The decision of `land` with the profiles it offers; `None` for a subject the table lacks
#[must_use]
pub fn land_answer(sources: &LandSources<'_>, land: &Land) -> Option<LandAnswer> {
  let landing = LandingFacts::new(sources.table, sources.remote, sources.view, sources.robots, None);
  let (decision, last) = decide_with(sources, &landing, land)?;
  let profiles = standing(sources.profiles).map_or_default(|options| offered_profiles(options, last, land.mode, land.without_approvals));
  Some(LandAnswer { decision, profiles })
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

/// The key of a subject in the view: the branch, `stack:<root>` the whole stack
fn subject_key(subject: &Subject) -> String {
  match subject {
    Subject::Branch { name } => name.clone(),
    Subject::WholeStack { root } => stack_readiness_key(root),
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
        SubjectFacts::Branch(landing.branch_subject(name, Some(sources.pull_requests))?),
        landing.robot_of(name),
        needs,
      )
    }
    Subject::WholeStack { root } => {
      let stack = sources.table.stack_branches.iter().find(|stack| stack.root == *root && !stack.members.is_empty())?;
      // the landing needs are of the branches: the gate of the start checks the whole stack
      (SubjectFacts::WholeStack(landing.stack_subject(stack)), landing.stack_robot(stack), Some(&[][..]))
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
    readiness: (sources.readiness)(&subject_key(&land.subject)),
    rules: sources.rules,
    landing: needs,
    target_conflict: None,
  };
  Some((decide(&facts, land), last))
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
