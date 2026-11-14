//! What an approval of a pull request to review counts for where Patronus merges the repository
//! (openspec/specs/repository/review-requests/spec.md, "Review window: Approve of a layer says when Patronus counts it";
//! docs/design/change-review.md, decision 31; `approval_landing`). Patronus starts a Safe Push of a pull request into its
//! base, checks the approvals of that pull request alone, once at its start, on the paths of all it lands, and takes the
//! latest review of each owner whatever commit it was on. So the approval of a pull request into the target counts for
//! its own Safe Push only, never for another layer of its stack; one into another base (a layer of a stack goes into the
//! branch of the one below it) counts for no Safe Push into the target until its base is the target, and then for all
//! its Safe Push lands. Where Patronus doesn't merge, before the target is known, and for an own pull request (GitHub
//! takes no review of its author) there is nothing to say.

use crate::actions::Target;
use domain_types::session::ApprovalLanding;

/// What an approval of the pull request into `base` counts for, `target` what is known of the branch Patronus pushes
/// into for the baseline (the same fact its Safe Push is offered by, `actions::review_actions`), `own` the user's own
/// pull request
#[must_use]
pub fn approval_landing(target: Target<'_>, base: &str, own: bool) -> Option<ApprovalLanding> {
  let Target::Branch(target) = target else {
    return None;
  };
  if own {
    return None;
  }
  Some(if base == target {
    ApprovalLanding::OwnSafePush
  } else {
    ApprovalLanding::OnceIntoTarget {
      base: base.to_owned(),
      target: target.to_owned(),
    }
  })
}
