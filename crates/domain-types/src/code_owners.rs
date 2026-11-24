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
