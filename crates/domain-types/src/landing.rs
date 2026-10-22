//! What refuses a Safe Push or a Dry Run before anything is pushed, as the landing gate (`sync_core::landing_gate`)
//! builds it for `bd safe-push`, the start of the app and the landing needs of the repository view: typed, so the CLI
//! and the app each name it in their own words, and each refusal carries the fix the user can run (`LandingFix`).

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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReferenceKind {
  Removed,
  Changed,
  Duplicate,
}

/// How the user gets the missing declaration, or the change of a dangling use, into what lands
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[cfg(test)]
#[path = "landing_test.rs"]
mod landing_test;
