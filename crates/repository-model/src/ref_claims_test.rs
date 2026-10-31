use crate::ref_claims::claim_free_refs;
use std::collections::BTreeSet;
use std::path::Path;

/// A fetch takes only the refs no push writes; a push takes the refs no fetch holds, at once, and leaves a ref a fetch
/// holds to that fetch, which moves it only from the tip it read; another repository's ref of the same name is its own
#[test]
fn a_claim_takes_the_free_refs_at_once_and_leaves_the_held_ones() {
  let repository = Path::new("/tmp/ref-claims-test/a/.git");
  let names = |names: &[&str]| names.iter().map(ToString::to_string).collect::<Vec<_>>();

  let (pushing, pushed) = claim_free_refs(repository, &names(&["p/virtual/login"]));
  assert_eq!(pushed, BTreeSet::from(["p/virtual/login".to_string()]));
  let (fetch, fetched) = claim_free_refs(repository, &names(&["p/virtual/auth", "p/virtual/login"]));
  assert_eq!(fetched, BTreeSet::from(["p/virtual/auth".to_string()]));

  // a push of auth while its fetch runs goes at once, and leaves auth to the fetch
  let (push, free) = claim_free_refs(repository, &names(&["p/virtual/auth"]));
  assert_eq!(free, BTreeSet::new());
  let (other, free) = claim_free_refs(Path::new("/tmp/ref-claims-test/b/.git"), &names(&["p/virtual/auth"]));
  assert_eq!(free.len(), 1, "auth of another repository is free");
  drop((push, other, fetch, pushing));
  let (_again, free) = claim_free_refs(repository, &names(&["p/virtual/auth", "p/virtual/login"]));
  assert_eq!(free.len(), 2, "the claims released their refs");
}
