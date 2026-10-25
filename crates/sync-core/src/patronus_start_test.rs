use crate::patronus_start::{carried_ids, carried_words};
use domain_types::repository_view::Carried;
use pretty_assertions::assert_eq;

/// A refusal names the commits a running robot lands as its content told: their number, and for `bd` the first three
/// ids, an amended one marked
#[test]
fn the_commits_a_robot_lands_are_named_by_their_number_and_ids() {
  let ids: Vec<String> = ["aaaaaaaa1", "bbbbbbbb2", "cccccccc3", "dddddddd4"].iter().map(|id| (*id).to_string()).collect();
  let carried = Carried {
    commits: ids.clone(),
    amended: vec![ids[1].clone()],
  };
  assert_eq!(carried_words("login", None), "login");
  assert_eq!(carried_ids(None), "");
  assert_eq!(carried_words("login", Some(&carried)), "4 commits of login");
  assert_eq!(carried_ids(Some(&carried)), " (aaaaaaaa, bbbbbbbb amended since, cccccccc, …)");
}
