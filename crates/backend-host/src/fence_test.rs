//! The version fence (`Host::fence`; openspec/specs/repository/backend-protocol/spec.md, "Commands: An answer that names a
//! version follows that version"): an answer that names a version of a repository waits for the actions up to it to
//! each window of its connection, behind a resync when one is due, and at once when none subscribes

use crate::channel::WindowSubscription;
use crate::corpus_test::Fixture;
use domain_types::session::Version;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const EPOCH: u32 = 7;

fn at(epoch: u32, seq: u32) -> Version {
  Version { epoch, seq }
}

/// A window of the connection subscribed to the channel of the fixture, holding `delivered`
async fn subscribed(fixture: &Fixture, delivered: Version) -> (Arc<crate::channel::Channel>, app_api::transport::ConnectionId, crate::corpus_test::Link) {
  let link = fixture.bridge().await;
  let channel = fixture.host.channel_of_path(&fixture.repo);
  {
    let mut state = channel.state();
    state.epoch = Some(EPOCH);
    state.windows.insert(
      (link.id(), "main".to_owned()),
      WindowSubscription {
        connection: Arc::downgrade(link.connection()),
        subscription: None,
        ended: Arc::new(AtomicBool::new(false)),
        delivered: Some(delivered),
        joining: None,
      },
    );
  }
  (channel, link.id(), link)
}

fn deliver(fixture: &Fixture, channel: &crate::channel::Channel, of: app_api::transport::ConnectionId, version: Version) {
  let mut state = channel.state();
  state.epoch = Some(version.epoch);
  state.windows.get_mut(&(of, "main".to_owned())).unwrap().delivered = Some(version);
  drop(state);
  fixture.host.published.notify_waiters();
}

/// The answer naming V waits until the window got the action to V
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_waits_for_the_actions_up_to_its_version() {
  let fixture = Fixture::new().await;
  let (channel, of, _link) = subscribed(&fixture, at(EPOCH, 1)).await;
  let host = Arc::clone(&fixture.host);
  let repo = fixture.repo.clone();
  let fenced = tokio::spawn(async move { host.fence(of, &repo, at(EPOCH, 3)).await });
  tokio::time::sleep(Duration::from_millis(100)).await;
  assert!(!fenced.is_finished(), "the window holds 1 of 3");
  deliver(&fixture, &channel, of, at(EPOCH, 2));
  tokio::time::sleep(Duration::from_millis(50)).await;
  assert!(!fenced.is_finished(), "the window holds 2 of 3");
  deliver(&fixture, &channel, of, at(EPOCH, 3));
  tokio::time::timeout(Duration::from_secs(5), fenced).await.unwrap().unwrap();
}

/// A window that holds the view of the epoch before the one of the answer waits for its resync
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_waits_behind_a_due_resync() {
  let fixture = Fixture::new().await;
  let (channel, of, _link) = subscribed(&fixture, at(EPOCH - 1, 40)).await;
  let host = Arc::clone(&fixture.host);
  let repo = fixture.repo.clone();
  let fenced = tokio::spawn(async move { host.fence(of, &repo, at(EPOCH, 2)).await });
  tokio::time::sleep(Duration::from_millis(100)).await;
  assert!(!fenced.is_finished(), "the window holds the epoch before");
  deliver(&fixture, &channel, of, at(EPOCH, 2));
  tokio::time::timeout(Duration::from_secs(5), fenced).await.unwrap().unwrap();
}

/// No window of the connection subscribes to the repository: the answer goes at once (an undo without a subscription)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_without_a_subscription_goes_at_once() {
  let fixture = Fixture::new().await;
  let (_channel, _of, _link) = subscribed(&fixture, at(EPOCH, 1)).await;
  let other = fixture.bridge().await;
  tokio::time::timeout(Duration::from_secs(1), fixture.host.fence(other.id(), &fixture.repo, at(EPOCH, 9)))
    .await
    .expect("no window of the other connection subscribes");
  tokio::time::timeout(Duration::from_secs(1), fixture.host.fence(other.id(), "/no/channel", at(EPOCH, 9)))
    .await
    .expect("no channel of the path");
}
