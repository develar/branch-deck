//! How the work of a repository lands into master (spec/branch/landing-routes.spec.md): by a robot of Patronus, by a
//! push of the copy into the branch of the baseline, or by a merge of its pull request on GitHub. The setting of the
//! repository that names it (`branchdeck.landing`, `LandingSetting`), what the sync reads of git for it
//! (`LandingRemote`), and the route `domain_policy::landing_route::route` decides with the facts of GitHub
//! (`LandingRoute`, each with its gate).

use crate::error::ApiError;
use crate::reason::Reason;
use serde::{Deserialize, Serialize};

/// The git config key of the route (`patronus`, `push`, `pull-request`)
pub const LANDING_CONFIG_KEY: &str = "branchdeck.landing";

/// The route a repository names (`branchdeck.landing`); unset, the route is Patronus on Space and the pull request
/// elsewhere (`domain_policy::landing_route::route`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LandingSetting {
  /// A Safe Push of Patronus lands it: the robot rebases it onto the target, runs the checks and pushes
  Patronus,
  /// A push of the copy into the branch of the baseline on its remote lands it, as a fast-forward; only the repository's
  /// own config names it (`git config --local`), never the global one
  Push,
  /// A merge of its pull request on GitHub lands it
  PullRequest,
}

impl LandingSetting {
  /// The value of the config, as git keeps it; `None` for another (trimmed, any case)
  #[must_use]
  pub fn parse(value: &str) -> Option<Self> {
    match value.trim().to_ascii_lowercase().as_str() {
      "patronus" => Some(Self::Patronus),
      "push" => Some(Self::Push),
      "pull-request" => Some(Self::PullRequest),
      _ => None,
    }
  }

  /// The value Branch Deck writes
  #[must_use]
  pub const fn config_value(self) -> &'static str {
    match self {
      Self::Patronus => "patronus",
      Self::Push => "push",
      Self::PullRequest => "pull-request",
    }
  }
}

/// What the route is decided on of git, as a sync reads it with its listing and its config (no process of its own):
/// the setting, and where the remotes push. A table read before any sync has the default
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingRemote {
  /// `branchdeck.landing`; `None` unset
  pub setting: Option<LandingSetting>,
  /// The push URL of `origin` is a repository on Space (`git.jetbrains.team`), where Patronus lands
  pub on_space: bool,
  /// `owner/name` of the github.com repository the push URL of the baseline's remote names; `None` for another host,
  /// and for a local baseline
  pub push_repository: Option<String>,
  /// The branch of the baseline on its remote (`master` of `origin/master`), which a push lands into; `None` for a
  /// local baseline
  pub target: Option<String>,
}

/// The route of a repository into master, each with its gate (`domain_policy::landing_route::route`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LandingRoute {
  /// A Safe Push of Patronus; refused off Space
  Patronus { gate: RouteGate },
  /// A push of the copy into the target, by the write access and the rules of the target as GitHub tells them
  Push { gate: RouteGate },
  /// A merge of the pull request on GitHub, by the user's write access; without it the route stays and others merge
  PullRequest { merge: RouteGate },
}

/// Whether a route may land now
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RouteGate {
  /// Nothing known holds it: what is not known of the remote, the server answers
  Open,
  /// A fact of GitHub has no answer yet, or its read `failed`: never read as "no"
  Wait { fact: RouteFact, failed: Option<ApiError> },
  /// It may land once the user accepts `confirmations`, in their order
  Confirm { confirmations: Vec<Reason> },
  /// It can't land: `reason`
  Refused { reason: Reason },
}

/// The facts of GitHub a gate reads
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RouteFact {
  /// What the token may do in the repository (the listing of the pull requests)
  Permission,
  /// The rules of the target (`branch_rules`)
  TargetRules,
}

#[cfg(test)]
#[path = "landing_route_test.rs"]
mod landing_route_test;
