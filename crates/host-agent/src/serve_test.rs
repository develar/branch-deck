use super::*;
use pretty_assertions::assert_eq;
use sync_utils::fs_retry;
use test_utils::git_test_utils::TestRepo;

/// The agent of a temporary home, as `serve` makes it
fn agent(home: &std::path::Path) -> Agent {
  let identity = Identity::generate().expect("an identity");
  Agent::new(
    Home::new(home.to_path_buf()),
    ServerTls::new(&identity, home.join("clients")).expect("the TLS setup"),
    PathBuf::from("bd-host"),
    PathBuf::from("launchctl"),
    "0.0.0".to_owned(),
  )
}

/// The agent opens a repository once: a path that a request opened stays open while it is a directory, and a missing one
/// is looked at again at each request
#[test]
fn a_repository_that_a_request_opened_needs_no_second_look() {
  let home = tempfile::tempdir().expect("a temporary home");
  let agent = agent(home.path());
  assert_eq!(agent.repo("~/repo").expect_err("no repository yet").code, Code::RepoMissing);
  let repo = TestRepo::new();
  let path = repo.path().display().to_string();
  let opened = agent.repo(&path).expect("a repository");
  assert_eq!(opened.path, repo.path());
  // without its .git a fresh open refuses the directory; the agent answers from what it opened
  fs_retry::rename(repo.path().join(".git"), home.path().join("moved.git")).expect("the .git moved away");
  assert_eq!(agent.repo(&path).expect("an opened repository").path, repo.path());
  fs_retry::remove_dir_all(repo.path()).expect("the directory removed");
  assert_eq!(agent.repo(&path).expect_err("no directory").code, Code::RepoMissing);
}

/// One peer fills its own cap and leaves room for the others, and the total cap holds for all peers
#[test]
fn a_peer_gets_its_own_cap_of_handshakes_within_the_total_cap() {
  let handshakes = Arc::new(Handshakes::default());
  let peer = |last: u8| IpAddr::from([192, 168, 1, last]);
  let busy: Vec<Admitted> = (0..MAX_HANDSHAKES_PER_PEER)
    .map(|_| Handshakes::admit(&handshakes, peer(1)).expect("room for the peer"))
    .collect();
  assert!(Handshakes::admit(&handshakes, peer(1)).is_none(), "the cap of the peer is full");
  let peers = u8::try_from(MAX_HANDSHAKES / MAX_HANDSHAKES_PER_PEER).expect("a peer count of one byte");
  let others: Vec<Admitted> = (2..=peers)
    .flat_map(|last| (0..MAX_HANDSHAKES_PER_PEER).map(move |_| last))
    .map(|last| Handshakes::admit(&handshakes, peer(last)).expect("room for another peer"))
    .collect();
  assert_eq!(busy.len() + others.len(), MAX_HANDSHAKES);
  assert!(Handshakes::admit(&handshakes, peer(peers + 1)).is_none(), "the total cap is full");
  drop(busy);
  assert!(Handshakes::admit(&handshakes, peer(1)).is_some(), "an ended handshake frees its count");
  drop(others);
  assert!(Handshakes::admit(&handshakes, peer(peers + 1)).is_some(), "the other peers freed the total cap");
}

#[test]
fn an_affinity_names_a_base() {
  check_affinity("checkout-0123456789ab").expect("an affinity");
  assert_eq!(check_affinity("../x").expect_err("refused").code, Code::BadRequest);
  assert_eq!(check_affinity("").expect_err("refused").code, Code::BadRequest);
}
