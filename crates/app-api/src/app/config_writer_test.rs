use crate::app::config_writer::{ConfigWriters, config_file_of, page_write};
use crate::app::repository_config::write_companion;
use crate::transport::{Call, PageId, WindowId};
use domain_types::error::ApiError;
use domain_types::settings::WriteOrder;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::repository_config::{RepositoryConfig, RepositoryConfigChange, WriteScope, read_repository_config, set_config};
use test_utils::git_test_utils::{TestRepo, append_config};
use test_utils::process_budget::{counting, processes};

fn settings_page() -> Call {
  Call {
    window: WindowId::new("settings"),
    page: PageId(1),
  }
}

/// The write of `change` numbered `seq` of the Settings page through `writers` with `git`, ordered by its key
async fn write_with(
  writers: &ConfigWriters,
  git: &GitCommandExecutor,
  repo: &TestRepo,
  change: RepositoryConfigChange,
  seq: Option<u32>,
) -> Result<Option<RepositoryConfig>, ApiError> {
  let (git, path) = (git.clone(), repo.path_str().to_owned());
  let order = seq.map(|seq| {
    (
      page_write(
        &settings_page(),
        WriteOrder {
          page: "settings-1".to_owned(),
          seq,
        },
      ),
      format!("{:?}", change.key()),
    )
  });
  writers
    .write(repo.path_str(), WriteScope::Local, order, move || set_config(&git, &path, &change, WriteScope::Local))
    .await
}

async fn write(writers: &ConfigWriters, repo: &TestRepo, change: RepositoryConfigChange, seq: Option<u32>) -> Result<Option<RepositoryConfig>, ApiError> {
  write_with(writers, repo.git_executor(), repo, change, seq).await
}

/// A repository whose git refuses a locked config at once, as 2.54 and 2.55 do, so a write that met a lock of the app
/// shows as a write tried again
fn repository_without_a_lock_wait() -> TestRepo {
  let repo = TestRepo::new();
  append_config(repo.path(), &[("core.configLockTimeout", "0")]);
  repo
}

/// The rounds of writes at once: each meets the other write of its round in the config of the repository
const ROUNDS: u32 = 8;

/// The writes of round `round`: the review plan switched and a prefix typed, numbered after the rounds before
fn round_of(round: u32) -> [(RepositoryConfigChange, u32); 2] {
  [
    (RepositoryConfigChange::ReviewExperimental(Some(round % 2 == 0)), 2 * round + 1),
    (RepositoryConfigChange::BranchPrefix(Some(format!("jane{round}"))), 2 * round + 2),
  ]
}

/// Settings saves a typed prefix on blur and a switch at once: two writes of different keys of the app, which the one
/// writer of the config of the repository runs one after the other, both on disk, never a "Cannot set …" and never a
/// retry for a `config.lock` the other write held: the writes at once cost the git processes the same writes one by one
/// cost
#[tokio::test]
async fn two_writes_of_different_keys_at_once_are_both_on_disk() {
  let one_by_one = repository_without_a_lock_wait();
  let git = counting();
  let writers = ConfigWriters::default();
  for round in 0..ROUNDS {
    for (change, seq) in round_of(round) {
      assert!(write_with(&writers, &git, &one_by_one, change, Some(seq)).await.unwrap().is_some());
    }
  }

  let at_once = repository_without_a_lock_wait();
  let git_at_once = counting();
  let writers = Arc::new(ConfigWriters::default());
  for round in 0..ROUNDS {
    let [(switch, switch_seq), (prefix, prefix_seq)] = round_of(round);
    let (switch, prefix) = tokio::join!(
      write_with(&writers, &git_at_once, &at_once, switch, Some(switch_seq)),
      write_with(&writers, &git_at_once, &at_once, prefix, Some(prefix_seq)),
    );
    assert!(switch.unwrap().is_some() && prefix.unwrap().is_some(), "both written");
  }
  let config = read_repository_config(at_once.git_executor(), at_once.path_str()).unwrap();
  assert_eq!((config.review_experimental.value, config.branch_prefix.value.as_deref()), (false, Some("jane7")));
  assert_eq!(processes(&git_at_once), processes(&git), "no write was tried again");
}

/// The write numbered 2 of a key comes before the one numbered 1 of the same page, as the close of Settings sends an edit
/// while the write before it still runs: 2 is on disk, and 1 is superseded and writes nothing
#[tokio::test]
async fn an_older_write_of_a_key_of_the_page_is_superseded_and_writes_nothing() {
  let repo = TestRepo::new();
  let writers = ConfigWriters::default();
  let newer = write(&writers, &repo, RepositoryConfigChange::BranchPrefix(Some("second".to_owned())), Some(2))
    .await
    .unwrap();
  assert_eq!(newer.unwrap().branch_prefix.value.as_deref(), Some("second"));
  let older = write(&writers, &repo, RepositoryConfigChange::BranchPrefix(Some("first".to_owned())), Some(1))
    .await
    .unwrap();
  assert_eq!(older, None, "superseded");
  let config = read_repository_config(repo.git_executor(), repo.path_str()).unwrap();
  assert_eq!(config.branch_prefix.value.as_deref(), Some("second"));
  // a write without a number is applied in the order it came
  let unordered = write(&writers, &repo, RepositoryConfigChange::BranchPrefix(Some("third".to_owned())), None).await.unwrap();
  assert_eq!(unordered.unwrap().branch_prefix.value.as_deref(), Some("third"));
}

/// Two spellings of one repository, its path and a symlink to it (`/var/…` and `/private/var/…` on macOS), are one config
/// file: their writes go to one writer task, one after the other
#[cfg(unix)]
#[tokio::test]
async fn two_spellings_of_one_repository_write_through_one_task() {
  let repo = TestRepo::new();
  let links = tempfile::tempdir().unwrap();
  let link = links.path().join("repo");
  std::os::unix::fs::symlink(repo.path(), &link).unwrap();
  let writers = ConfigWriters::default();
  for path in [repo.path_str().to_owned(), link.to_str().unwrap().to_owned()] {
    let (git, written) = (repo.git_executor().clone(), path.clone());
    let config = writers
      .write(&path, WriteScope::Local, None, move || {
        set_config(&git, &written, &RepositoryConfigChange::ReviewExperimental(Some(true)), WriteScope::Local)
      })
      .await
      .unwrap();
    assert!(config.is_some());
  }
  assert_eq!(writers.tasks.lock().unwrap().len(), 1, "one task for the config file of both spellings");
}

/// A companion added from Settings holds the writers of both configs it writes, the primary's and the companion's, so a
/// write of Settings to either at once never races it for `config.lock`
#[tokio::test]
async fn a_companion_write_holds_the_writers_of_both_configs() {
  let (app, ci) = (TestRepo::new(), TestRepo::new());
  let writers = ConfigWriters::default();
  let config = write_companion(&writers, app.git_executor(), app.path_str(), "ci".to_owned(), Some(ci.path_str().to_owned()))
    .await
    .unwrap();
  assert_eq!(config.companions.iter().map(|it| it.name.as_str()).collect::<Vec<_>>(), ["ci"]);
  let primary = ConfigSnapshot::read(ci.git_executor(), ci.path_str()).unwrap().primary().map(str::to_owned);
  assert!(primary.is_some(), "the companion names its primary");
  assert_eq!(writers.tasks.lock().unwrap().len(), 2, "one writer for each config");
}

/// Polls `future` once, which queues its first write in the writer of its config, the one step it takes before it waits
fn queue(future: std::pin::Pin<&mut impl Future>) {
  let _ = future.poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
}

/// A write that holds the writer of the config of `repo` until `gate` opens, queued at once
fn hold(writers: &ConfigWriters, repo: &TestRepo) -> (std::sync::mpsc::Sender<()>, impl Future<Output = Result<Option<()>, ApiError>>) {
  let (open, gate) = std::sync::mpsc::channel::<()>();
  let mut held = Box::pin(writers.write(repo.path_str(), WriteScope::Local, None, move || {
    let _ = gate.recv();
    Ok(())
  }));
  queue(held.as_mut());
  (open, held)
}

/// Waits until exactly `count` jobs sent to the writers have not started: the writes started before it queued theirs
async fn until_queued(writers: &ConfigWriters, count: usize) {
  while writers.queued() != count {
    tokio::task::yield_now().await;
  }
}

/// The write of the companion `ci` of `app` through `writers` on a task of its own: an add of `ci`, or its removal
fn spawn_write(writers: &Arc<ConfigWriters>, app: &TestRepo, ci: Option<&TestRepo>) -> tokio::task::JoinHandle<Result<RepositoryConfig, ApiError>> {
  let (writers, git, app) = (Arc::clone(writers), app.git_executor().clone(), app.path_str().to_owned());
  let ci = ci.map(|ci| ci.path_str().to_owned());
  tokio::spawn(async move { write_companion(&writers, &git, &app, "ci".to_owned(), ci).await })
}

/// Whether `app` lists `ci` and whether `ci` names a primary: both sides of one link
fn sides(app: &TestRepo, ci: &TestRepo) -> (bool, bool) {
  let lists = ConfigSnapshot::read(app.git_executor(), app.path_str()).unwrap().companions().contains_key("ci");
  let names = ConfigSnapshot::read(ci.git_executor(), ci.path_str()).unwrap().primary().is_some();
  (lists, names)
}

/// The order of the review: an add of a linked companion again and its removal at once, both queued in the writers
/// before either runs, in either order. Each holds both writers and both companion locks for its whole span, so they
/// run one after the other and both sides agree: unlinked after the add and the removal, linked after the removal and
/// the add. Of a write in steps, each in the writer of its config, the steps interleave: the add finds the companion
/// named and leaves it, the removal unsets it, the add writes the key of the primary: a half link
#[tokio::test]
async fn an_add_and_a_removal_of_one_linked_pair_at_once_run_one_after_the_other() {
  for add_first in [true, false] {
    let (app, ci) = (TestRepo::new(), TestRepo::new());
    let writers = Arc::new(ConfigWriters::default());
    write_companion(&writers, app.git_executor(), app.path_str(), "ci".to_owned(), Some(ci.path_str().to_owned()))
      .await
      .unwrap();
    let (open_app, held_app) = hold(&writers, &app);
    let (open_ci, held_ci) = hold(&writers, &ci);
    until_queued(&writers, 0).await;
    let order = if add_first { [Some(&ci), None] } else { [None, Some(&ci)] };
    let mut writes = Vec::new();
    for (queued, path) in (1..).zip(order) {
      writes.push(spawn_write(&writers, &app, path));
      until_queued(&writers, queued).await;
    }
    open_app.send(()).unwrap();
    open_ci.send(()).unwrap();
    let (held_app, held_ci) = tokio::join!(held_app, held_ci);
    held_app.unwrap();
    held_ci.unwrap();
    for write in writes {
      write.await.unwrap().unwrap();
    }
    let linked = !add_first;
    assert_eq!(sides(&app, &ci), (linked, linked), "both sides agree, the add first: {add_first}");
  }
}

/// The add of `ci` as the companion `ci` of `primary` through `writers`
async fn add_ci(writers: &ConfigWriters, primary: &TestRepo, ci: &TestRepo) -> Result<RepositoryConfig, ApiError> {
  write_companion(writers, primary.git_executor(), primary.path_str(), "ci".to_owned(), Some(ci.path_str().to_owned())).await
}

/// Two primaries that add one companion at once: the writes hold the writer of the companion and its companion lock,
/// so one after the other, and whatever their order the second is refused (a companion has one primary) and its
/// primary never names the companion
#[tokio::test]
async fn of_two_primaries_that_add_one_companion_at_once_the_second_is_refused() {
  let (first, second, ci) = (TestRepo::new(), TestRepo::new(), TestRepo::new());
  let writers = ConfigWriters::default();
  let (of_first, of_second) = tokio::join!(add_ci(&writers, &first, &ci), add_ci(&writers, &second, &ci));
  let refused: Vec<String> = [&of_first, &of_second]
    .into_iter()
    .filter_map(|added| added.as_ref().err())
    .map(|error| error.message.clone())
    .collect();
  assert_eq!(refused.len(), 1, "one of the two is refused: {refused:?}");
  assert!(refused[0].contains("a companion has one primary"), "{refused:?}");
  let (winner, loser) = if of_first.is_ok() { (&first, &second) } else { (&second, &first) };
  let primary = ConfigSnapshot::read(ci.git_executor(), ci.path_str()).unwrap().primary().map(str::to_owned);
  assert!(
    primary.is_some_and(|primary| std::path::Path::new(&primary) == std::fs::canonicalize(winner.path()).unwrap()),
    "the companion names the one that added it"
  );
  assert_eq!(
    ConfigSnapshot::read(winner.git_executor(), winner.path_str())
      .unwrap()
      .companions()
      .keys()
      .collect::<Vec<_>>(),
    ["ci"]
  );
  assert!(
    ConfigSnapshot::read(loser.git_executor(), loser.path_str()).unwrap().companions().is_empty(),
    "the refused primary names no companion"
  );
}

/// A write together holds the writers of its configs until it ends: writes of both configs queued while it runs start
/// only after it, while a write of a third config runs meanwhile, and the writers of the two had their turn to start
/// what they could before that write ended
#[tokio::test]
async fn a_write_together_holds_the_writers_of_its_configs_until_it_ends() {
  let (a, b, c) = (TestRepo::new(), TestRepo::new(), TestRepo::new());
  let writers = Arc::new(ConfigWriters::default());
  let (runs, running) = std::sync::mpsc::channel();
  let (end, ends) = std::sync::mpsc::channel::<()>();
  let together = tokio::spawn({
    let (writers, pair) = (Arc::clone(&writers), [a.path_str().to_owned(), b.path_str().to_owned()]);
    async move {
      let pair: Vec<&str> = pair.iter().map(String::as_str).collect();
      writers
        .write_together(&pair, move || {
          runs.send(()).unwrap();
          ends.recv().unwrap();
          Ok("together")
        })
        .await
    }
  });
  tokio::task::spawn_blocking(move || running.recv().unwrap()).await.unwrap();
  let mut of_a = Box::pin(writers.write(a.path_str(), WriteScope::Local, None, || Ok("a")));
  queue(of_a.as_mut());
  let mut of_b = Box::pin(writers.write(b.path_str(), WriteScope::Local, None, || Ok("b")));
  queue(of_b.as_mut());
  after_queued(&writers, &c).await;
  assert_eq!(writers.queued(), 2, "the writes of both configs wait for the write together");
  end.send(()).unwrap();
  let (together, of_a, of_b) = tokio::join!(together, of_a, of_b);
  assert_eq!((together.unwrap().unwrap(), of_a.unwrap(), of_b.unwrap()), ("together", Some("a"), Some("b")));
}

/// The writes queued in the writer of the config of `repo` before it are done once it is
async fn after_queued(writers: &ConfigWriters, repo: &TestRepo) {
  writers.write(repo.path_str(), WriteScope::Local, None, || Ok(())).await.unwrap();
}

/// Two writes together of one pair of configs, named in opposite orders, at once: each takes the writers in the order of
/// their files, so the second waits for the first writer the first holds and never holds the other one meanwhile; taken
/// in the order named, each would hold one writer and wait for the other's forever
#[tokio::test]
async fn two_writes_together_of_one_pair_named_in_opposite_orders_both_end() {
  let (a, b) = (TestRepo::new(), TestRepo::new());
  let mut pair = [a.path_str(), b.path_str()];
  pair.sort_by_key(|repository| config_file_of(repository, WriteScope::Local));
  let [first, second] = pair;
  let writers = ConfigWriters::default();
  let (open, held) = hold(&writers, if first == a.path_str() { &a } else { &b });
  let mut in_order = Box::pin(writers.write_together(&[first, second], || Ok("in order")));
  queue(in_order.as_mut());
  let mut reversed = Box::pin(writers.write_together(&[second, first], || Ok("reversed")));
  queue(reversed.as_mut());
  open.send(()).unwrap();
  let (in_order, reversed, held) = tokio::join!(in_order, reversed, held);
  held.unwrap();
  assert_eq!((in_order.unwrap(), reversed.unwrap()), ("in order", "reversed"));
}

/// A companion write and writes of Settings to the configs of both repositories at once, round after round: the writes
/// of Settings wait for the companion write in the writers it holds, so all are on disk and none was tried again for a
/// `config.lock` another write held: the writes at once cost the git processes the same writes one by one cost
#[tokio::test]
async fn a_companion_write_and_settings_writes_of_both_configs_at_once_are_all_on_disk() {
  let rounds = |at_once: bool| async move {
    let (app, ci) = (repository_without_a_lock_wait(), repository_without_a_lock_wait());
    let git = counting();
    let writers = ConfigWriters::default();
    for round in 0..ROUNDS {
      let path = (round % 2 == 0).then(|| ci.path_str().to_owned());
      let companion = write_companion(&writers, &git, app.path_str(), "ci".to_owned(), path);
      let of_app = write_with(&writers, &git, &app, RepositoryConfigChange::BranchPrefix(Some(format!("app{round}"))), None);
      let of_ci = write_with(&writers, &git, &ci, RepositoryConfigChange::BranchPrefix(Some(format!("ci{round}"))), None);
      if at_once {
        let (companion, of_app, of_ci) = tokio::join!(companion, of_app, of_ci);
        companion.unwrap();
        assert!(of_app.unwrap().is_some() && of_ci.unwrap().is_some());
      } else {
        companion.await.unwrap();
        assert!(of_app.await.unwrap().is_some() && of_ci.await.unwrap().is_some());
      }
    }
    assert_eq!(sides(&app, &ci), (false, false), "the last round removed it");
    processes(&git)
  };
  assert_eq!(rounds(true).await, rounds(false).await, "no write was tried again");
}
