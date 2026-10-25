//! What the Safe Push of a branch needs of a branch outside what it lands, as a row and a start say it
//! (spec/branch/next-step.spec.md): the refusals of the landing gate (`trial_check::landing_gate::landing_needs`) with the
//! running Safe Push that lands the branch adding a missing name, which the start of this one would fail beside.

use domain_types::landing::LandingRefusal;
use domain_types::repository_view::RobotOf;

/// Names in each missing declaration of `refusals` the running Safe Push that lands the branch adding the name
/// (`landing_of`, by branch): a start beside it fails all the same, its robot tests that branch without this one. A name
/// an unassigned commit adds has no branch, so no robot lands it
pub fn name_landing_robots(refusals: &mut [LandingRefusal], landing_of: impl Fn(&str) -> Option<RobotOf>) {
  for refusal in refusals {
    if let LandingRefusal::MissingDeclaration { declared_by, landing_in, .. } = refusal {
      *landing_in = if declared_by.branch.is_empty() { None } else { landing_of(&declared_by.branch) };
    }
  }
}
