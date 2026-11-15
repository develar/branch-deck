use crate::app::config_writer::{ConfigWriters, page_write};
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

/// A companion added from Settings writes the key of the primary in the writer of the primary's config and the
/// `branchdeck.primary` of the companion in the writer of the companion's, so a write of either repository at once never
/// races it for `config.lock`
#[tokio::test]
async fn a_companion_writes_each_side_in_the_writer_of_its_own_config() {
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

/// The add of `ci` as the companion `ci` of `primary` through `writers`, not polled yet
fn add_ci<'a>(writers: &'a ConfigWriters, primary: &'a TestRepo, ci: &TestRepo) -> std::pin::Pin<Box<impl Future<Output = Result<RepositoryConfig, ApiError>> + 'a>> {
  Box::pin(write_companion(
    writers,
    primary.git_executor(),
    primary.path_str(),
    "ci".to_owned(),
    Some(ci.path_str().to_owned()),
  ))
}

/// The writes queued in the writer of the config of `repo` before it are done once it is
async fn after_queued(writers: &ConfigWriters, repo: &TestRepo) {
  writers.write(repo.path_str(), WriteScope::Local, None, || Ok(())).await.unwrap();
}

/// Two primaries that add one companion at once: each checks its own config in its writer, then the companion's side
/// with the check that it has one primary in the companion's writer, so whatever their order the second is refused
/// there and its primary never names the companion. The writer of the companion holds until both checked their primary,
/// the order in which a check of the companion's config in the primary's writer let both pass
#[tokio::test]
async fn of_two_primaries_that_add_one_companion_at_once_the_second_is_refused_in_the_writer_of_the_companion() {
  let (first, second, ci) = (TestRepo::new(), TestRepo::new(), TestRepo::new());
  let writers = ConfigWriters::default();
  let (open, held) = hold(&writers, &ci);
  let (mut of_first, mut of_second) = (add_ci(&writers, &first, &ci), add_ci(&writers, &second, &ci));
  queue(of_first.as_mut());
  queue(of_second.as_mut());
  after_queued(&writers, &first).await;
  after_queued(&writers, &second).await;
  open.send(()).unwrap();
  let (of_first, of_second, held) = tokio::join!(of_first, of_second, held);
  held.unwrap();
  let refused: Vec<String> = [&of_first, &of_second]
    .into_iter()
    .filter_map(|added| added.as_ref().err())
    .map(|error| error.message.clone())
    .collect();
  assert_eq!(refused.len(), 1, "one of the two is refused: {refused:?}");
  assert!(refused[0].contains("a companion has one primary"), "{refused:?}");
  let (winner, loser) = if of_first.is_ok() { (&first, &second) } else { (&second, &first) };
  let primary = ConfigSnapshot::read(ci.git_executor(), ci.path_str()).unwrap().primary().map(str::to_owned);
  let winner_main = ConfigSnapshot::read(winner.git_executor(), winner.path_str()).unwrap();
  assert!(
    primary.is_some_and(|primary| std::path::Path::new(&primary) == std::fs::canonicalize(winner.path()).unwrap()),
    "the companion names the first that claimed it"
  );
  assert_eq!(winner_main.companions().keys().collect::<Vec<_>>(), ["ci"]);
  assert!(
    ConfigSnapshot::read(loser.git_executor(), loser.path_str()).unwrap().companions().is_empty(),
    "the refused primary names no companion"
  );
}

/// An add whose primary became a companion while the add waited for the writer of the companion is refused in the
/// writer of the primary, and the primary the companion named just now is taken back in the writer of the companion
#[tokio::test]
async fn an_add_whose_primary_became_a_companion_meanwhile_takes_back_the_primary_of_the_companion() {
  let (app, ci, other) = (TestRepo::new(), TestRepo::new(), TestRepo::new());
  let writers = ConfigWriters::default();
  let (open, held) = hold(&writers, &ci);
  let mut added = add_ci(&writers, &app, &ci);
  queue(added.as_mut());
  after_queued(&writers, &app).await;
  write_companion(&writers, other.git_executor(), other.path_str(), "app".to_owned(), Some(app.path_str().to_owned()))
    .await
    .unwrap();
  open.send(()).unwrap();
  let (added, held) = tokio::join!(added, held);
  held.unwrap();
  let refused = added.unwrap_err().message;
  assert!(refused.contains("only a primary has companions"), "{refused}");
  assert_eq!(ConfigSnapshot::read(ci.git_executor(), ci.path_str()).unwrap().primary(), None, "the claim is taken back");
  assert!(ConfigSnapshot::read(app.git_executor(), app.path_str()).unwrap().companions().is_empty());
}
