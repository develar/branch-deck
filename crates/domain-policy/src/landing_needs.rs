//! What the Safe Push of a branch needs of a branch outside what it lands, as a row and a start say it
//! (openspec/specs/branch/next-step/spec.md): the refusals of the landing gate (`trial_check::landing_gate::landing_needs`) and
//! the declared order of landing (`declared_needs`), with the running Safe Push that lands the branch the start needs,
//! which the start of this one would fail beside.

use domain_types::landing::LandingRefusal;
use domain_types::repository_view::RobotOf;

/// Names in each refusal of `refusals` the running Safe Push that lands the branch the start needs (`landing_of`, by
/// branch): the branch adding a missing name, or the branch a declared need names. A start beside it fails all the
/// same, its robot tests that branch without this one. A name an unassigned commit adds has no branch, so no robot
/// lands it, and neither does a branch of a companion repository, whose robots are its own
pub fn name_landing_robots(refusals: &mut [LandingRefusal], landing_of: impl Fn(&str) -> Option<RobotOf>) {
  for refusal in refusals {
    match refusal {
      LandingRefusal::MissingDeclaration { declared_by, landing_in, .. } => {
        *landing_in = if declared_by.branch.is_empty() { None } else { landing_of(&declared_by.branch) };
      }
      LandingRefusal::UnlandedNeed { needs, landing_in, .. } => {
        *landing_in = if needs.contains(':') { None } else { landing_of(needs) };
      }
      LandingRefusal::DanglingReference { .. } => {}
    }
  }
}
