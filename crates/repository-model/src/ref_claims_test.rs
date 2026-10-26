use crate::ref_claims::{claim_free_refs, claim_refs};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A fetch takes only the refs no push writes; a push of a ref the fetch holds waits until the fetch ended, a push of
/// another ref of the same repository and the same ref of another repository don't
#[tokio::test]
async fn a_push_waits_for_the_fetch_of_its_ref_and_a_fetch_leaves_the_pushed_refs() {
  let repository = Path::new("/tmp/ref-claims-test/a/.git");
  let names = |names: &[&str]| names.iter().map(ToString::to_string).collect::<Vec<_>>();

  let pushing = claim_refs(repository, &names(&["p/virtual/login"]), || panic!("nothing holds login")).await;
  let (fetch, fetched) = claim_free_refs(repository, &names(&["p/virtual/auth", "p/virtual/login"]));
  assert_eq!(fetched, BTreeSet::from(["p/virtual/auth".to_string()]));
  drop(pushing);

  // another repository's auth and this one's login go at once
  let other = claim_refs(Path::new("/tmp/ref-claims-test/b/.git"), &names(&["p/virtual/auth"]), || panic!("auth of b is free")).await;
  let login = claim_refs(repository, &names(&["p/virtual/login"]), || panic!("login is free")).await;
  drop((other, login));

  let waited = Arc::new(AtomicBool::new(false));
  let on_wait = Arc::clone(&waited);
  let push = tokio::spawn(async move {
    let names = vec!["p/virtual/auth".to_string()];
    let _claim = claim_refs(Path::new("/tmp/ref-claims-test/a/.git"), &names, move || on_wait.store(true, Ordering::SeqCst)).await;
  });
  while !waited.load(Ordering::SeqCst) {
    tokio::task::yield_now().await;
  }
  assert!(!push.is_finished(), "the push of auth waits while its fetch runs");
  drop(fetch);
  push.await.unwrap();
  let (_again, free) = claim_free_refs(repository, &names(&["p/virtual/auth"]));
  assert_eq!(free.len(), 1, "the push released auth");
}
