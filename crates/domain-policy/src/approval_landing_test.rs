use crate::actions::Target;
use crate::approval_landing::approval_landing;
use domain_types::session::ApprovalLanding;
use pretty_assertions::assert_eq;

const MASTER: Target<'static> = Target::Branch("master");

fn once_into_master(base: &str) -> ApprovalLanding {
  ApprovalLanding::OnceIntoTarget {
    base: base.to_string(),
    target: "master".to_string(),
  }
}

/// Where Patronus merges into master, the approval of the bottom of a stack, into master, counts for its own Safe Push;
/// a layer into the branch of the one below, or a pull request into another branch, counts once its base is master.
/// Where Patronus doesn't merge, before the target is known and for an own pull request nothing is said
#[test]
fn an_approval_counts_for_the_safe_push_of_its_own_pull_request_into_the_target_and_a_layer_above_once_it_goes_there() {
  let cases: [(&str, Target<'_>, &str, bool, Option<ApprovalLanding>); 7] = [
    ("the bottom, into master", MASTER, "master", false, Some(ApprovalLanding::OwnSafePush)),
    (
      "a layer into the one below",
      MASTER,
      "alice/parts/33-auth",
      false,
      Some(once_into_master("alice/parts/33-auth")),
    ),
    ("another base", MASTER, "release/253", false, Some(once_into_master("release/253"))),
    ("an own pull request", MASTER, "alice/parts/33-auth", true, None),
    ("no Patronus", Target::NoPatronus, "alice/parts/33-auth", false, None),
    ("before the first sync", Target::Unread, "master", false, None),
    ("the baseline no branch of origin", Target::None, "master", false, None),
  ];
  for (case, target, base, own, expected) in cases {
    assert_eq!(approval_landing(target, base, own), expected, "{case}");
  }
}
