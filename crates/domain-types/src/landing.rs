//! What refuses a Safe Push or a Dry Run before anything is pushed, as the landing gate (`trial_check::landing_gate`)
//! builds it for `bd safe-push`, the start of the app and the landing needs of the repository view: typed, so the CLI
//! and the app each name it in their own words, and each refusal carries the fix the user can run (`LandingFix`). And a
//! start of a robot as the contract's `Land` intent names it, with what `domain_policy::land::decide` answers of it
//! (`LandDecision`: blockers, confirmations, waits and effects), one decision of `bd safe-push` and of the app. What
//! landed a subject (`Landed`, `domain_policy::landed::landed_state`) and the pull requests a merge closes
//! (`LandedPullRequest`).

use crate::error::ApiError;
use crate::patronus::{LandingAllowances, PatronusProfile, PatronusPushMode};
use crate::reason::Reason;
use crate::repository_view::RobotOf;
use serde::{Deserialize, Serialize};

/// The owned subject of a landing, the one shape of "a branch, or the whole stack of a root" (`bd safe-push`, the
/// landing gate, the robot of the repository view): a branch with what it is built on, or the whole stack of a root.
/// Struct variants, as an internally tagged enum can't serialize a newtype variant of a string; on the wire
/// `{"kind":"branch","name":…}` and `{"kind":"wholeStack","root":…}` (`landing_test.rs`)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Subject {
  /// The branch and the branches it is built on: one chain, one robot
  Branch { name: String },
  /// Every branch of the stack of this root, which may fork: the robot of the whole stack
  WholeStack { root: String },
}

impl Subject {
  /// The branch `name` with the branches it is built on
  pub fn branch(name: &str) -> Self {
    Self::Branch { name: name.to_string() }
  }

  /// The whole stack of `root`
  pub fn whole_stack(root: &str) -> Self {
    Self::WholeStack { root: root.to_string() }
  }

  /// The branch it names: the branch, or the root of the whole stack
  pub fn name(&self) -> &str {
    match self {
      Self::Branch { name } => name,
      Self::WholeStack { root } => root,
    }
  }

  /// The key of the subject in the view: the branch, `stack:<root>` the whole stack (`session::stack_readiness_key`)
  pub fn key(&self) -> String {
    match self {
      Self::Branch { name } => name.clone(),
      Self::WholeStack { root } => crate::session::stack_readiness_key(root),
    }
  }

  /// The whole stack of a root, not one branch
  pub const fn is_whole_stack(&self) -> bool {
    matches!(self, Self::WholeStack { .. })
  }

  /// The branch `name` itself, not the whole stack of it: the robot of this subject is the branch's own
  pub fn is_branch(&self, name: &str) -> bool {
    matches!(self, Self::Branch { name: own } if own == name)
  }

  /// "auth", or "the whole auth stack": the subject as a sentence names it, the one wording of `bd` and the app's errors
  pub fn words(&self) -> String {
    match self {
      Self::Branch { name } => name.clone(),
      Self::WholeStack { root } => format!("the whole {root} stack"),
    }
  }

  /// The robot `robot_id` that ran on this subject, as the repository view names it
  pub fn robot_of(&self, robot_id: &str) -> RobotOf {
    RobotOf {
      branch: self.name().to_string(),
      robot_id: robot_id.to_string(),
      whole_stack: self.is_whole_stack(),
    }
  }
}

/// A commit ahead of the baseline, as `StackIndependence` and the refusals of the landing gate name it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackCommit {
  pub id: String,
  /// The subject without its prefix; an autosquash marker stays
  pub subject: String,
  /// The branch the commit is in
  pub branch: String,
}

/// Why a start is refused
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LandingRefusal {
  /// A commit of `branch` removes or changes a declaration, or adds one again, that the tree it lands still uses; the
  /// commit outside it that removes the use proves it
  DanglingReference {
    branch: String,
    reference: ReferenceKind,
    identifier: String,
    /// Where the use is
    file: String,
    line: u32,
    removed_by: StackCommit,
    fixed_by: StackCommit,
    /// How the commit that changes the use gets into what lands
    fix: LandingFix,
  },
  /// A commit of `branch` uses a name that a commit outside what lands adds
  MissingDeclaration {
    branch: String,
    identifier: String,
    /// Where the use is
    file: String,
    line: u32,
    used_by: StackCommit,
    declared_by: StackCommit,
    fix: LandingFix,
    /// The running Safe Push that lands the branch of `declared_by` (its own, of a branch built on it, of its whole
    /// stack): the commit it tests lacks this start's commits, so a start beside it fails all the same; the repository
    /// view says it, a start does not ask it
    #[serde(default)]
    landing_in: Option<RobotOf>,
  },
}

/// What a branch does to the declaration of a dangling reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReferenceKind {
  Removed,
  Changed,
  Duplicate,
}

/// How the user gets the missing declaration, or the change of a dangling use, into what lands
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LandingFix {
  /// Stack `branch`, the root of what lands, on `onto`, the branch of the commit it needs: the start lands it too
  Restack { branch: String, onto: String },
  /// Move the commit it needs into `into`, the branch that needs it: the commit is unassigned, stacked branches are off,
  /// or its branch is built on what lands, so a restack would close a cycle
  MoveCommit { commit: StackCommit, into: String },
}

impl LandingRefusal {
  /// The commit outside what lands that the start needs: the one that adds the name, or that changes the dangling use
  pub const fn needed(&self) -> &StackCommit {
    match self {
      Self::DanglingReference { fixed_by, .. } => fixed_by,
      Self::MissingDeclaration { declared_by, .. } => declared_by,
    }
  }

  /// In a sentence: what breaks the build of what lands
  pub fn reason(&self) -> String {
    let named = |commit: &StackCommit| {
      if commit.branch.is_empty() {
        format!("\"{}\" (unassigned)", commit.subject)
      } else {
        format!("\"{}\" of {}", commit.subject, commit.branch)
      }
    };
    match self {
      Self::DanglingReference {
        identifier,
        file,
        removed_by,
        fixed_by,
        reference,
        ..
      } => {
        let change = match reference {
          ReferenceKind::Removed => "removes",
          ReferenceKind::Changed => "changes",
          ReferenceKind::Duplicate => "declares again",
        };
        format!("{} {change} {identifier}, which {file} still uses; {} changes that use", named(removed_by), named(fixed_by))
      }
      Self::MissingDeclaration {
        identifier,
        file,
        used_by,
        declared_by,
        landing_in,
        ..
      } => {
        let landing = landing_in.as_ref().map_or_default(|robot| {
          let subject = robot.subject();
          let of = if subject.is_branch(&declared_by.branch) {
            "its Safe Push".to_string()
          } else {
            format!("the Safe Push of {}", subject.words())
          };
          format!("; {} is landing in {of}, which tests it without this start", declared_by.branch)
        });
        format!("{} uses {identifier} in {file}, which {} adds{landing}", named(used_by), named(declared_by))
      }
    }
  }
}

/// The merge profile a start asks for
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProfileChoice {
  /// The profile of the last robot of the subject while the target offers it for the mode, else the default checks
  #[default]
  Default,
  /// A profile the target offers for the mode
  Id { id: String },
  /// The default checks of the target
  None,
}

/// A start of a robot on a subject (the contract's `Land`; docs/design/rust-domain-authority.md, "Контракты")
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Land {
  pub subject: Subject,
  /// A Safe Push (`RebaseAutosquash`) or a Dry Run
  pub mode: PatronusPushMode,
  pub profile: ProfileChoice,
  /// Start although the code owners haven't approved (`--without-approvals`, the app's Safe Push without review)
  #[serde(default)]
  pub without_approvals: bool,
  /// Push and open the pull request of the subject (or take the open one) instead of a start, where the code owners
  /// approve (`bd safe-push --create-pull-request`)
  #[serde(default)]
  pub create_pull_request: bool,
  #[serde(default)]
  pub allow: LandingAllowances,
}

/// Why the approvals don't hold a Safe Push back (`domain_policy::review::skips_approvals`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ApprovalsSkip {
  /// A start through the API with a merge profile known to skip the code-owner review (its options say it), as the
  /// app's Safe Push without review starts; a `/patronus` comment has no profile
  Profile,
  /// The user said to start without them (`--without-approvals`, the app's Start Without Review)
  WithoutApprovals,
}

/// What a start does and what stands in its way, as `domain_policy::land::decide` answers it over the facts it has
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandDecision {
  /// What refuses it, in order; no confirmation lifts one (an allowance of the intent may keep one from being raised)
  pub blockers: Vec<Reason>,
  /// What the user accepts before it starts: a line of the app's popover each, a flag of `bd` for some
  /// (`domain_policy::land::unaccepted`)
  pub confirmations: Vec<Confirmation>,
  /// The facts it waits for, which nobody answered yet (or whose read failed): never read as "no"
  pub waits: Vec<FactWait>,
  /// What it does once nothing refuses it, in order: retarget, cancel, push, then the start or the pull request
  pub effects: Vec<PlannedEffect>,
  /// What the robot lands, root first: the branches it is built on that merged Safe Pushes didn't land whole, then the
  /// branch itself; of a whole stack every branch of it they didn't land whole
  pub lands: Vec<String>,
  /// The open pull request it starts from with a `/patronus` comment, or the one `create_pull_request` takes
  pub pull_request: Option<u32>,
  /// Why the approvals don't hold it back
  pub approvals_skipped_by: Option<ApprovalsSkip>,
}

impl LandDecision {
  /// How the robot starts: through the API with the profile, or from a pull request; none when the start opens the pull
  /// request instead
  #[must_use]
  pub fn start_route(&self) -> Option<&StartRoute> {
    PlannedEffect::start_route_in(&self.effects)
  }
}

/// The decisions of the two starts of a subject the view of the session offers, its default intents
/// (`repository_model::derive::lands_of`): a Safe Push with the default profile, and a Dry Run
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandDecisions {
  pub safe_push: LandDecision,
  pub dry_run: LandDecision,
}

/// The merge profiles a start offers (`domain_policy::land::offered_profiles`), as its popover lists them
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandProfiles {
  /// The profiles of the target the mode allows, with what the options of the last robot said of the review of its own
  pub offered: Vec<PatronusProfile>,
  /// The emergency profiles the mode allows, offered only to a start without approvals: they skip every check
  pub emergency: Vec<PatronusProfile>,
  /// The profile a start runs with unless another is chosen: the last robot's while offered, else the default of the
  /// target; of a start without approvals only one known to skip the review. `None`: the default checks (or the user
  /// chooses)
  pub default: Option<String>,
}

/// What the user accepts before a start (the contract's `Confirmation`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Confirmation {
  /// It lands `count` branches it is built on with the branch, more than its row; of a whole stack the branches of it
  /// besides its root
  LandsMoreThanRow { count: u32 },
  /// It starts with a profile, not the default checks: the last robot's or one chosen
  NonDefaultProfile { id: String },
  /// It cancels the running Safe Push of an older commit it replaces (`LandingAllowances::cancel_running` in `bd`)
  CancelsRobot { robot_id: String },
  /// Patronus judges the approvals only when the robot starts: it doesn't evaluate the pull request, or GitHub didn't
  /// list the pull requests, so the start is not known
  ApprovalsJudgedAtStart,
  /// Patronus couldn't answer the code-owner review (`message`, its pre-check failed): the robot checks the approvals
  /// when it starts (`LandingAllowances::allow_unknown_review` in `bd`, D14)
  ReviewUnknown { message: String },
  /// The push dismisses the approvals of pull request `number`, none of which Patronus counts
  DismissesApprovals { number: u32 },
  /// GitHub didn't answer the rules of the target: whether the push keeps the approvals of pull request `number` is not
  /// known (`LandingAllowances::allow_unknown_rules` in `bd`)
  RulesUnknown { number: u32 },
  /// It starts without the approvals the code owners haven't given (a Safe Push without review)
  WithoutApprovals,
  /// It lands a piece of a stack whose pull request `number` is open: those approvals count for the whole stack only
  StackPullRequestApprovalsDontCount { number: u32 },
  /// Its push replaces `commits` commits of the remote branch of `branch` the branch lacks, someone's work (allowed by
  /// `LandingAllowances::allow_force`, D12)
  ForcePushDrops { branch: String, commits: u32 },
}

/// A fact a start waits for: no answer yet, or `failed` with why the read failed; never read as "no"
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FactWait {
  pub fact: LandFact,
  pub failed: Option<ApiError>,
}

/// The facts of a start
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LandFact {
  /// The tip of the subject and the remote status of what its push takes
  Remote,
  /// The merge profiles of the target
  Profiles,
  /// The open pull requests of the branches
  PullRequests,
  /// The code-owner review Patronus answers for the subject
  Readiness,
  /// The rules of the target for a push into the approved pull request `number`
  Rules { number: u32 },
  /// What what lands needs of the branches outside it (the landing gate)
  LandingNeeds,
}

/// What a start does, in the order of its effects (the contract's `PlannedEffect`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PlannedEffect {
  /// The open pull request `number` of `branch` goes from the virtual branch `from` into the target `to` (D11)
  Retarget { branch: String, number: u32, from: String, to: String },
  /// The running Safe Push of an older commit it replaces is canceled
  CancelRobot { robot_id: String },
  /// The branches with work for the remote are pushed, root first: the branch and the ancestors its push takes; of a
  /// whole stack its root, whose stack branch is the only one pushed. `force`: it replaces commits the remote has
  Push { branches: Vec<String>, force: bool },
  /// The robot starts
  StartRobot { route: StartRoute },
  /// The pull request of the subject is opened, where the code owners approve (none is open)
  CreatePullRequest,
}

impl PlannedEffect {
  /// The route of the start among `effects`; none when they start no robot
  #[must_use]
  pub fn start_route_in(effects: &[Self]) -> Option<&StartRoute> {
    effects.iter().find_map(|effect| match effect {
      Self::StartRobot { route } => Some(route),
      _ => None,
    })
  }
}

/// How a robot starts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StartRoute {
  /// Through the API, with `profile` (`None` the default checks)
  Api { profile: Option<PatronusProfile> },
  /// A `/patronus` comment on the open pull request `number`, which runs the default checks of the target
  Comment { number: u32 },
}

/// What landed a subject (`domain_policy::landed::landed_state`): the fetched baseline, merged Safe Pushes, or GitHub
/// merging its pull request; nothing of it is left to land while `after` is empty
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Landed {
  pub by: LandedBy,
  /// The commits added after the landed ones, original ids, newest first: the work left; empty when all of it landed
  pub after: Vec<String>,
}

impl Landed {
  /// All of it landed: nothing is left to push or land until the update archives it
  #[must_use]
  pub const fn is_whole(&self) -> bool {
    self.after.is_empty()
  }
}

/// What landed a subject
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LandedBy {
  /// The fetched baseline (`upstream`, "origin/master") has every commit of it, until the update archives it
  Baseline { upstream: String },
  /// Merged Safe Pushes: of a branch the one that left the fewest commits after it (its own, of a branch built on it,
  /// of the whole stack); of a whole stack what landed each branch of it, each robot once in the order they merged.
  /// `own`: the robot of the subject itself landed all of it; `target` the branch the first one pushed into
  Robots { by: Vec<RobotOf>, own: bool, target: String },
  /// GitHub merged its pull request `number` with the commits the subject has (D13)
  PullRequest { number: u32 },
}

/// An open pull request of a branch a merged Safe Push landed whole besides its own: the app closes it
/// (`domain_policy::landed::pull_requests_to_close`), `bd robot` lists it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandedPullRequest {
  pub branch: String,
  pub number: u32,
}

#[cfg(test)]
#[path = "landing_test.rs"]
mod landing_test;
