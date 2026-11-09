//! The order of landing the user declared (`brd need`, `refs/branch-deck/needs`;
//! openspec/specs/branch/declared-needs/spec.md): a branch does not start while a branch it needs is not in master, and a branch
//! built on it inherits that need, because one robot lands them together. The rule over the declared lines, the state
//! of each needed branch and the stack chains; the facts come from the table of the session
//! (`repository_model::branch_table::need_states`), never from git here.

use domain_types::landing::{LandingFix, LandingRefusal, NeedState};
use domain_types::repository_view::RobotOf;
use std::collections::BTreeMap;

/// What is known of a branch a need names: how the repository that holds it left it, and the robot that lands it right
/// now. A needed branch that is integrated has no facts: a sync drops such a need
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeedFacts {
  pub state: NeedState,
  /// The running Safe Push that lands it already
  pub landing_in: Option<RobotOf>,
}

impl NeedFacts {
  #[must_use]
  pub const fn of(state: NeedState) -> Self {
    Self { state, landing_in: None }
  }
}

/// The refusals of the declared needs, by branch: the needs of the branch itself, then the needs of the branches it is
/// built on, which it lands with. A need whose branch landed (none in `states`) refuses nothing, and so does a need a
/// branch would inherit on itself
///
/// - `declared`: branch -> the needs it declared, as they are written (`<companion>:<branch>` for one of a companion)
/// - `states`: a declared need -> what is known of the branch it names
/// - `chains`: branch -> the branches it is built on, nearest first
#[must_use]
pub fn declared_refusals(
  declared: &BTreeMap<String, Vec<String>>,
  states: &BTreeMap<String, NeedFacts>,
  chains: &[(String, Vec<String>)],
) -> BTreeMap<String, Vec<LandingRefusal>> {
  if declared.is_empty() {
    return BTreeMap::new();
  }
  let mut refusals: BTreeMap<String, Vec<LandingRefusal>> = BTreeMap::new();
  for (branch, chain) in branches(declared, chains) {
    // its own needs first, then those of the branches it is built on, nearest first: the nearest unstack is the first
    // fix the user sees
    let inherited = chain.iter().map(|parent| (Some(parent.as_str()), parent.as_str()));
    let of_branch = std::iter::once((None, branch)).chain(inherited);
    let mut listed = Vec::new();
    for (inherited_from, owner) in of_branch {
      for need in declared.get(owner).into_iter().flatten() {
        // a branch that inherits a need on itself lands it in the same robot: it is met, not refused
        if need == branch || listed.contains(&need.as_str()) {
          continue;
        }
        let Some(facts) = states.get(need) else { continue };
        listed.push(need.as_str());
        refusals.entry(branch.to_string()).or_default().push(LandingRefusal::UnlandedNeed {
          branch: branch.to_string(),
          needs: need.clone(),
          state: facts.state.clone(),
          inherited_from: inherited_from.map(str::to_string),
          fix: inherited_from.map(|from| LandingFix::Unstack {
            branch: branch.to_string(),
            from: from.to_string(),
          }),
          landing_in: facts.landing_in.clone(),
        });
      }
    }
  }
  refusals
}

/// Each branch that may inherit a need with the branches it is built on: the branches of `chains`, and the branches
/// that declared a need without being in them (one of a branch of another worktree)
fn branches<'a>(declared: &'a BTreeMap<String, Vec<String>>, chains: &'a [(String, Vec<String>)]) -> impl Iterator<Item = (&'a str, &'a [String])> {
  let known = chains.iter().map(|(branch, chain)| (branch.as_str(), chain.as_slice()));
  let rest = declared
    .keys()
    .filter(|branch| !chains.iter().any(|(name, _)| name == *branch))
    .map(|branch| (branch.as_str(), [].as_slice()));
  known.chain(rest)
}
