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
