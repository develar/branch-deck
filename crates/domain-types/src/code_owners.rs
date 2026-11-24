//! A group of code owners as the `CodeOwners` service answers it (`codeowners::client`): its members and its reviewers, each
//! with the GitHub login a review is asked of, whether they are available and whether the rotation of the group passes
//! them by. The owner groups of the changed paths come from the `OWNERSHIP` files (`git_ops::ownership`); whom a review
//! after landing asks of them is `domain_policy::shown::show_candidates`.

use serde::{Deserialize, Serialize};

/// A group of code owners by its name, as an `OWNERSHIP` file names it (`owner 'Name'`)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerGroup {
  pub name: String,
  /// Those the group names to review its changes; empty for a group that names none, whose members review
  pub reviewers: Vec<GroupMember>,
  pub members: Vec<GroupMember>,
}

/// A member or a reviewer of a group of code owners
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
  /// The login on Space, which names them where no GitHub login does
  pub username: String,
  /// The GitHub login a review is asked of; `None` for one without
  pub github_login: Option<String>,
  /// Not on vacation or away (`availability.state` `AVAILABLE`)
  pub available: bool,
  /// The rotation of the group passes them by
  pub rotated: bool,
}

/// Whom a review after landing asks when the user names nobody (`domain_policy::shown::show_candidates`): the picker of
/// Safe Push and Request Review… starts with them
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ShowReviewers {
  /// The owner groups of the paths the branch changes, by the `OWNERSHIP` files of the target; empty where none names an
  /// owner of them
  pub groups: Vec<String>,
  /// One available GitHub login of each group, each once, never the author
  pub reviewers: Vec<String>,
  /// The groups none of whose members or reviewers can be asked
  pub without_reviewer: Vec<String>,
}

/// What `get_show_reviewers` asks: the default reviewers of the review after landing of `branch_name`
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetShowReviewersParams {
  pub repository_path: String,
  pub branch_name: String,
}
