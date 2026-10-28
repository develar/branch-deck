//! The catalog of the reasons (`reasons.json`): every code of `Reason` with an example, which the page's formatter
//! is tested on (`src/presenters/reasons.test.ts`), so a code Rust gives is one the page words.
//! `UPDATE_REASON_CATALOG=1 cargo nextest run -p domain-policy -E 'test(/reason/)'` writes it; without the variable a
//! stale catalog fails.

use crate::catalog_test::assert_catalog;
use domain_types::fixtures::{carried, robot_of, stack_commit as commit, whole_stack_robot_of};
use domain_types::landing::{LandingFix, LandingRefusal, ReferenceKind};
use domain_types::patronus::PatronusPushMode;
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, RobotOf};
use domain_types::stack::{BlockReason, StackBranchConflict, StackBranchState};

/// The position of the code of `reason` among the codes: a new code fails to compile here until it has an example
const fn code_index(reason: &Reason) -> usize {
  match reason {
    Reason::Blocked { .. } => 0,
    Reason::LastCommit { .. } => 1,
    Reason::Descendant { .. } => 2,
    Reason::Ancestor { .. } => 3,
    Reason::Landed { .. } => 4,
    Reason::Held { .. } => 5,
    Reason::NoStackBranch { .. } => 6,
    Reason::StackNotBuilt { .. } => 7,
    Reason::StackParked { .. } => 8,
    Reason::StackLanded { .. } => 9,
    Reason::StackHeld { .. } => 10,
    Reason::CantPush { .. } => 11,
    Reason::DropsRemote { .. } => 12,
    Reason::Parked { .. } => 13,
    Reason::Unprotected { .. } => 14,
    Reason::Draft { .. } => 15,
    Reason::OtherBase { .. } => 16,
    Reason::ApprovalsMissing { .. } => 17,
    Reason::DismissesApprovals { .. } => 18,
    Reason::LandingRefused { .. } => 19,
    Reason::TargetConflict { .. } => 20,
    Reason::LandedIn { .. } => 21,
    Reason::PullRequestMerged { .. } => 22,
    Reason::NeedsPush { .. } => 23,
    Reason::NoReviewer { .. } => 24,
    Reason::RewrittenSincePush { .. } => 25,
    Reason::NotOnSpace => 26,
    Reason::NoWriteAccess { .. } => 27,
    Reason::BaselineRequiresPullRequest { .. } => 28,
    Reason::BaselineRequiresMergeQueue { .. } => 29,
    Reason::BaselineRestrictsUpdates { .. } => 30,
    Reason::BaselineRequiresSignatures { .. } => 31,
    Reason::BaselineRequiresStatusChecks { .. } => 32,
    Reason::LocalBaseline { .. } => 33,
    Reason::GitHubMirror { .. } => 34,
    Reason::NotOnGitHub => 35,
  }
}

const CODES: usize = 36;

fn of(branch: &str, robot_id: &str, whole_stack: bool) -> RobotOf {
  if whole_stack {
    whole_stack_robot_of(branch, robot_id)
  } else {
    robot_of(branch, robot_id)
  }
}

/// An example of every code, some codes with more than one when their words differ by the params
fn examples() -> Vec<Reason> {
  let safe_push = PatronusPushMode::RebaseAutosquash;
  let s = String::from;
  vec![
    Reason::Blocked {
      branch: s("login"),
      by: Some(s("auth")),
    },
    Reason::Blocked { branch: s("login"), by: None },
    Reason::LastCommit {
      robot_id: s("r"),
      mode: PatronusPushMode::DryRun,
    },
    Reason::Descendant {
      branch: s("auth"),
      by: of("oauth", "r", false),
      mode: safe_push,
      carried: Some(carried(&["a1", "a2"])),
    },
    Reason::Descendant {
      branch: s("auth"),
      by: of("air", "w", true),
      mode: safe_push,
      carried: None,
    },
    Reason::Ancestor {
      landing: of("auth", "r", false),
      mode: safe_push,
    },
    Reason::Landed {
      branch: s("auth"),
      by: of("oauth", "m", false),
      own: false,
      target: s("master"),
    },
    Reason::Landed {
      branch: s("auth"),
      by: of("auth", "m", false),
      own: true,
      target: s("master"),
    },
    Reason::Held {
      branch: s("login"),
      hold: HoldView {
        kind: HoldKind::Span,
        by: of("session", "r", false),
        carried: Some(carried(&["l1"])),
      },
      mode: safe_push,
      target: s("master"),
    },
    Reason::Held {
      branch: s("auth"),
      hold: HoldView {
        kind: HoldKind::Own,
        by: of("auth", "r", false),
        carried: None,
      },
      mode: safe_push,
      target: s("master"),
    },
    Reason::NoStackBranch { root: s("auth") },
    Reason::StackNotBuilt {
      root: s("auth"),
      state: StackBranchState::Conflict,
      blocked_by: None,
      conflict: Some(StackBranchConflict {
        commit: s("0123456789abcdef"),
        files: vec![s("a.kt")],
      }),
      error: None,
    },
    Reason::StackNotBuilt {
      root: s("auth"),
      state: StackBranchState::Blocked,
      blocked_by: Some(s("login")),
      conflict: None,
      error: None,
    },
    Reason::StackParked {
      root: s("auth"),
      member: s("login"),
    },
    Reason::StackLanded {
      root: s("auth"),
      by: vec![of("deploy", "m1", false), of("auth", "m2", true)],
      own: false,
      target: s("master"),
    },
    Reason::StackHeld {
      root: s("auth"),
      robot_id: s("w"),
      mode: safe_push,
    },
    Reason::CantPush {
      branch: s("auth"),
      block: BlockReason::Conflict,
    },
    Reason::DropsRemote { branch: s("auth"), behind: 2 },
    Reason::RewrittenSincePush { branch: s("auth") },
    Reason::Parked {
      branch: s("login"),
      with: Some(s("auth")),
      note: s("IJPL-1"),
    },
    Reason::Parked {
      branch: s("auth"),
      with: None,
      note: String::new(),
    },
    Reason::Unprotected { target: s("master") },
    Reason::NeedsPush { branch: s("auth") },
    Reason::NoReviewer {
      groups: vec![s("IJPL Code Platform")],
    },
    Reason::Draft { number: 7 },
    Reason::OtherBase {
      number: 7,
      base: s("release/253"),
      target: s("master"),
    },
    Reason::ApprovalsMissing {
      groups: vec![s("Air Plugin Release Team")],
    },
    Reason::DismissesApprovals { number: 7, target: s("master") },
    Reason::LandingRefused {
      refusal: Box::new(LandingRefusal::MissingDeclaration {
        branch: s("flow"),
        identifier: s("parseFlow"),
        file: s("flow/Flow.kt"),
        line: 3,
        used_by: commit("c1", "Use parseFlow", "flow"),
        declared_by: commit("u1", "Add parseFlow", "util"),
        fix: LandingFix::Restack {
          branch: s("flow"),
          onto: s("util"),
        },
        landing_in: None,
      }),
    },
    Reason::LandingRefused {
      refusal: Box::new(LandingRefusal::DanglingReference {
        branch: s("fake-agent"),
        reference: ReferenceKind::Removed,
        identifier: s("CONTROL_URL_ENV"),
        file: s("acp/Host.kt"),
        line: 7,
        removed_by: commit("r1", "Remove the env", "fake-agent"),
        fixed_by: commit("f1", "Read the url", "acp"),
        fix: LandingFix::MoveCommit {
          commit: commit("f1", "Read the url", "acp"),
          into: s("fake-agent"),
        },
      }),
    },
    Reason::TargetConflict {
      commit: s("0123456789abcdef"),
      files: vec![s("a.kt")],
    },
    Reason::LandedIn { upstream: s("origin/master") },
    Reason::PullRequestMerged { number: 7 },
    Reason::NotOnSpace,
    Reason::NoWriteAccess {
      repository: s("develar/branch-deck"),
      target: s("master"),
    },
    Reason::BaselineRequiresPullRequest { target: s("master") },
    Reason::BaselineRequiresMergeQueue { target: s("master") },
    Reason::BaselineRestrictsUpdates { target: s("master") },
    Reason::BaselineRequiresSignatures { target: s("master") },
    Reason::BaselineRequiresStatusChecks { target: s("master") },
    Reason::LocalBaseline { baseline: s("master") },
    Reason::GitHubMirror {
      repository: s("JetBrains/intellij-community"),
    },
    Reason::NotOnGitHub,
  ]
}

/// Every code has an example, and the catalog the page reads is the one these examples write
#[test]
fn every_reason_code_has_an_example_in_the_catalog_the_page_reads() {
  let examples = examples();
  let mut covered = [false; CODES];
  for reason in &examples {
    covered[code_index(reason)] = true;
  }
  assert!(covered.iter().all(|covered| *covered), "a code without an example: {covered:?}");

  assert_catalog("reasons.json", "UPDATE_REASON_CATALOG", "reason", &examples);
}
