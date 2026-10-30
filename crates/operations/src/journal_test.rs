use crate::engine::run;
use crate::events::EventSink;
use crate::journal::{LIMIT, LateListener, MAX_AGE, clear, journal_path, journal_path_in, kept, read, read_activity, read_page, record_page};
use crate::registry::registry_test::sessions;
use crate::registry::{Answer, Registry, RegistryFiles, RunningGuard};
use crate::registry_file::ProcessRegistry;
use crate::registry_file::registry_file_test::{id, safe_push, sent, session};
use domain_types::activity::{ActivityKind, PageActivity};
use domain_types::error::{ApiError, ErrorCode, HolderTool};
use domain_types::fixtures::land_intent;
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::operation::{EffectRecord, EffectStatus, JournalEntry, JournalTransition, OperationEvent, OperationResult, OperationStats, Outcome};
use domain_types::patronus::PatronusPushMode;
use git_executor::git_command_executor::GitCommandExecutor;
use lease_pool::lease::{Pool, Slot, canonical_path};
use pretty_assertions::assert_eq;
use repository_model::session::SessionKey;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use test_utils::git_test_utils::TestRepo;

/// Hears nothing
struct Deaf;

impl EventSink for Deaf {
  fn send(&self, _event: OperationEvent) {}
}

/// The outcome of `op` whose one effect, the start, went as `status`
fn started(op: &domain_types::operation::OperationId, status: EffectStatus) -> Outcome {
  Outcome {
    op: op.clone(),
    effects: vec![EffectRecord::new(
      PlannedEffect::StartRobot {
        route: StartRoute::Api { profile: None },
      },
      status,
    )],
    news_key: None,
    version_after: None,
    result: OperationResult::None,
    undo: None,
    stats: OperationStats::default(),
  }
}

/// A registry of the app whose operations are recorded in the common dir `common` of a session in the worktree of
/// `repo`
pub(crate) struct AppRun {
  /// Kept while the run lives: the worktree of the session
  _repo: TestRepo,
  pub(crate) common: tempfile::TempDir,
  pub(crate) registry: Registry,
  pub(crate) key: SessionKey,
}

impl AppRun {
  pub(crate) fn new() -> Self {
    let repo = TestRepo::new();
    let common = tempfile::tempdir().unwrap();
    let common_dir = common.path().to_path_buf();
    let registry = Registry::new(sessions(), RegistryFiles::new(HolderTool::App, move |_| Some(common_dir.clone())).unflushed());
    let key = SessionKey {
      worktree: repo.path().to_string_lossy().into_owned(),
      prefix: "user".to_string(),
    };
    Self {
      _repo: repo,
      common,
      registry,
      key,
    }
  }

  /// Runs the Safe Push of auth through the engine, its body one git process on the blocking pool and one HTTP request,
  /// ending as `ended` says of its id
  pub(crate) async fn safe_push(&self, ended: impl FnOnce(&domain_types::operation::OperationId) -> Answer) -> Answer {
    let worktree = self.key.worktree.clone();
    run(&self.registry, &self.key, safe_push("auth"), Arc::new(Deaf), async |guard: &RunningGuard| {
      tokio::task::spawn_blocking(in_current_span(move || GitCommandExecutor::new().execute_command(&["rev-parse", "--git-dir"], &worktree)))
        .await
        .unwrap()
        .unwrap();
      tracing::info!("the body of the operation ran");
      sync_utils::operation_tally::http_request();
      ended(guard.id())
    })
    .await
  }
}

/// The id of each entry of the journal of `common`, with what `of` tells of it
fn told<T>(common: &tempfile::TempDir, of: impl Fn(&JournalEntry) -> T) -> Vec<(String, T)> {
  read(common.path()).unwrap().iter().map(|entry| (entry.id.0.clone(), of(entry))).collect()
}

/// The operation `op` of `key` that asks `request`, admitted into `registry`, ended as `ended` says
fn finished(registry: &ProcessRegistry, op: &str, key: &SessionKey, request: &crate::registry::OperationRequest, ended: Result<&Outcome, &ApiError>) {
  let entry = registry.admit(&id(op), HolderTool::Bd, key, request).unwrap();
  let stats = entry.stats();
  entry.finish(ended, stats);
}

/// An operation of the app and one of `bd` that ended go into the one journal of their repository, each once, in the
/// order they ended, with what they did and what they cost from their admission: the git processes and the HTTP
/// requests of the app's body, on its threads too, are its outcome's and its entry's. A line of another version stays
/// and is skipped by the reader; the records left the registry
#[tokio::test(flavor = "multi_thread")]
async fn an_operation_of_the_app_and_one_of_bd_are_journaled_once_each_with_what_they_cost() {
  let app = AppRun::new();
  let (common, key) = (&app.common, &app.key);
  let outcome = app.safe_push(|op| Ok(started(op, EffectStatus::Done))).await.unwrap();
  assert_eq!(
    (outcome.stats.git_processes.clone(), outcome.stats.http_requests),
    (BTreeMap::from([("rev-parse".to_string(), 1)]), 1)
  );
  // a line of another version of Branch Deck
  let path = common.path().join("branch-deck").join("journal.jsonl");
  let foreign = format!("{{\"endedAt\":{},\"kind\":\"newer\"}}", unix_now());
  std::fs::write(&path, format!("{}{foreign}\n", std::fs::read_to_string(&path).unwrap())).unwrap();

  let files = ProcessRegistry::of(common.path()).unflushed();
  let entry = files.admit(&id("bd"), HolderTool::Bd, &session("user"), &safe_push("docs")).unwrap();
  entry.tally().scope(sync_utils::operation_tally::http_request);
  let stats = entry.stats();
  entry.finish(Err(&ApiError::new(ErrorCode::PlanChanged, "auth moved")), stats);

  let journal = read(common.path()).unwrap();
  let [app, bd] = journal.as_slice() else { panic!("{journal:#?}") };
  assert_eq!(
    (
      &app.id,
      app.tool,
      &app.worktree,
      app.lease_slot,
      app.subject.as_deref(),
      &app.effects,
      &app.stats,
      &app.error
    ),
    (&outcome.op, HolderTool::App, &key.worktree, false, Some("auth"), &outcome.effects, &outcome.stats, &None)
  );
  assert_eq!(app.intent, land_intent("auth", PatronusPushMode::RebaseAutosquash));
  assert_eq!(
    (
      bd.id.0.as_str(),
      bd.tool,
      bd.effects.len(),
      bd.error.as_ref().map(|error| error.code),
      bd.stats.http_requests
    ),
    ("bd", HolderTool::Bd, 0, Some(ErrorCode::PlanChanged), 1)
  );
  assert_eq!(std::fs::read_to_string(&path).unwrap().lines().nth(1), Some(foreign.as_str()));
  assert_eq!(files.records().unwrap(), Some(Vec::new()));
}

/// A start whose answer was lost is journaled when its operation ends, with its start unknown, and again under its id
/// when a read of Patronus settles it or the user forgets it; a settle that finds it gone journals nothing
#[test]
fn a_start_whose_answer_was_lost_is_journaled_again_once_settled_or_forgotten() {
  let common = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(common.path()).unflushed();
  for (op, branch) in [("lost", "auth"), ("forgotten", "docs")] {
    let entry = registry.admit(&id(op), HolderTool::App, &session("user"), &safe_push(branch)).unwrap();
    entry.handle().send_start(sent(branch, unix_now())).unwrap();
    entry.handle().unknown().unwrap();
    let stats = entry.stats();
    entry.finish(Ok(&started(&id(op), EffectStatus::Unknown)), stats);
  }
  let lost = registry.records().unwrap().unwrap().into_iter().find(|record| record.id == id("lost")).unwrap();
  assert!(registry.settle(&lost, JournalTransition::Started { robot: None }).unwrap());
  assert!(!registry.settle(&lost, JournalTransition::NotStarted).unwrap());
  registry.forget(&id("forgotten")).unwrap();

  let told = told(&common, |entry| (entry.effects.first().map(|effect| effect.status.clone()), entry.transition.clone()));
  assert_eq!(
    told,
    [
      ("lost".to_string(), (Some(EffectStatus::Unknown), None)),
      ("forgotten".to_string(), (Some(EffectStatus::Unknown), None)),
      ("lost".to_string(), (None, Some(JournalTransition::Started { robot: None }))),
      ("forgotten".to_string(), (None, Some(JournalTransition::Forgotten))),
    ]
  );
}

/// An operation dropped before it ended is journaled as one that ended without an answer; one that did not run (it
/// joined another) is not journaled
#[test]
fn a_dropped_operation_is_journaled_as_internal_and_a_discarded_one_not_at_all() {
  let common = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(common.path()).unflushed();
  drop(registry.admit(&id("dropped"), HolderTool::App, &session("user"), &safe_push("auth")).unwrap());
  registry.admit(&id("joined"), HolderTool::App, &session("user"), &safe_push("docs")).unwrap().discard();

  assert_eq!(
    told(&common, |entry| entry.error.as_ref().map(|error| error.code)),
    [("dropped".to_string(), Some(ErrorCode::Internal))]
  );
  assert_eq!(registry.records().unwrap(), Some(Vec::new()));
}

/// The entry of an operation whose worktree is a slot of the lease pool says so: the operation of an agent
#[test]
fn an_operation_in_a_slot_of_the_lease_pool_is_journaled_as_an_agents() {
  let common = tempfile::tempdir().unwrap();
  let slot = tempfile::tempdir().unwrap();
  Pool::new(common.path())
    .update(Duration::from_secs(2), |store| {
      store.slots.push(Slot {
        name: "1".to_string(),
        path: canonical_path(slot.path()),
        dirty: false,
        returned: None,
        lease: None,
        reclaimed: None,
      });
      Ok(())
    })
    .unwrap();
  let registry = ProcessRegistry::of(common.path()).unflushed();
  let in_slot = SessionKey {
    worktree: slot.path().to_string_lossy().into_owned(),
    prefix: "user".to_string(),
  };
  for (op, key) in [("agent", &in_slot), ("user", &session("user"))] {
    finished(&registry, op, key, &safe_push(op), Ok(&started(&id(op), EffectStatus::Done)));
  }

  assert_eq!(told(&common, |entry| entry.lease_slot), [("agent".to_string(), true), ("user".to_string(), false)]);
}

/// A start of the push route is journaled with the target it pushes into, by the remote-tracking ref it held, also when
/// it was refused before its first effect, so Activity names it a push into master; a start of a robot has none
#[test]
fn a_start_of_the_push_route_is_journaled_with_its_target_also_when_refused_before_its_first_effect() {
  let common = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(common.path()).unflushed();
  let mut push = safe_push("auth");
  push.resources.push(domain_types::operation::Resource::Ref {
    name: "refs/remotes/origin/master".to_string(),
  });
  finished(&registry, "push", &session("user"), &push, Err(&ApiError::new(ErrorCode::BaselineMoved, "master moved")));
  finished(&registry, "robot", &session("user"), &safe_push("docs"), Ok(&started(&id("robot"), EffectStatus::Done)));

  assert_eq!(
    told(&common, |entry| entry.into.clone()),
    [("push".to_string(), Some("master".to_string())), ("robot".to_string(), None)]
  );
}

/// The journal keeps the newest `LIMIT` entries of the last `MAX_AGE` seconds, in their order; a line without its end is
/// no entry
#[test]
fn the_journal_keeps_the_newest_entries_of_the_last_thirty_days() {
  const NOW: u32 = 1_800_000_000;
  let line = |ended_at: u32, n: usize| format!("{{\"endedAt\":{ended_at},\"n\":{n}}}");
  let mut lines = vec![line(NOW - MAX_AGE - 1, 0), "not json".to_string(), line(NOW - MAX_AGE, 1)];
  lines.extend((2..LIMIT + 6).map(|n| line(NOW, n)));
  let kept = kept(lines.iter().map(String::as_str), NOW);
  assert_eq!(kept.len(), LIMIT);
  assert_eq!((kept[0], kept[LIMIT - 1]), (line(NOW, 6).as_str(), line(NOW, LIMIT + 5).as_str()));
  assert_eq!(super::kept([line(NOW - MAX_AGE, 1).as_str()].into_iter(), NOW), [line(NOW - MAX_AGE, 1)]);
}

/// What the page records of `repository`: an error it told
fn told_error(repository: &str, title: &str) -> PageActivity {
  PageActivity {
    repository_path: repository.to_string(),
    kind: ActivityKind::Error,
    title: title.to_string(),
    description: Some("the remote refused it".to_string()),
    warning: false,
    details: None,
    operation_id: Some(id("failed")),
  }
}

/// The page records into its journal in the app data directory, each entry with an id and the time; Clear Log of a
/// repository empties the journal of its repository and takes its entries out of the journal of the page, and the
/// entries of the other repositories stay
#[test]
fn the_page_records_into_its_journal_and_clear_log_forgets_the_log_of_a_repository_in_both() {
  let (common, app_data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
  let registry = ProcessRegistry::of(common.path()).unflushed();
  finished(&registry, "pushed", &session("user"), &safe_push("auth"), Ok(&started(&id("pushed"), EffectStatus::Done)));
  let recorded = record_page(app_data.path(), told_error("/a", "Cannot push auth")).unwrap();
  record_page(app_data.path(), told_error("/b", "Cannot push docs")).unwrap();
  let page = read_page(app_data.path()).unwrap();
  assert_eq!(
    page.iter().map(|entry| entry.activity.title.as_str()).collect::<Vec<_>>(),
    ["Cannot push auth", "Cannot push docs"]
  );
  assert_eq!(page[0], recorded);
  assert!(page[0].ended_at >= unix_now() - 60 && page[0].id != page[1].id, "{page:?}");

  clear(Some(&journal_path_in(registry.dir())), app_data.path(), "/a").unwrap();
  assert_eq!(read(common.path()).unwrap(), []);
  assert_eq!(
    read_page(app_data.path())
      .unwrap()
      .iter()
      .map(|entry| entry.activity.repository_path.as_str())
      .collect::<Vec<_>>(),
    ["/b"]
  );
}

/// An error the page told of an operation shows where the operation ran, so Clear Log of the repository of its journal
/// forgets it too, whichever repository the page recorded it in; an error of the page that names no operation of that
/// journal stays
#[test]
fn clear_log_forgets_the_errors_the_page_told_of_the_operations_of_the_journal() {
  let (common, app_data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
  let registry = ProcessRegistry::of(common.path()).unflushed();
  let failed = ApiError::new(ErrorCode::PushRefused, "the remote refused it");
  finished(&registry, "failed", &session("user"), &safe_push("auth"), Err(&failed));
  // the toast came while the main window showed /b
  record_page(app_data.path(), told_error("/b", "Cannot push auth")).unwrap();
  let of_b = PageActivity {
    operation_id: Some(id("sync-of-b")),
    ..told_error("/b", "Cannot sync docs")
  };
  record_page(app_data.path(), of_b).unwrap();

  clear(Some(&journal_path_in(registry.dir())), app_data.path(), "/a").unwrap();
  assert_eq!(read(common.path()).unwrap(), []);
  let page = read_page(app_data.path()).unwrap();
  assert_eq!(page.iter().map(|entry| entry.activity.title.as_str()).collect::<Vec<_>>(), ["Cannot sync docs"]);
}

/// Activity reads the journal of a repository once for the recent projects that share it (two worktrees), none for a
/// project its journal was never remembered for, and lists a journal it can't read unavailable; the journal of the page
/// comes with them
#[tokio::test(flavor = "multi_thread")]
async fn activity_reads_the_journal_of_each_repository_once_and_lists_one_it_cannot_read() {
  let (common, app_data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
  let registry = ProcessRegistry::of(common.path()).unflushed();
  finished(&registry, "pushed", &session("user"), &safe_push("auth"), Ok(&started(&id("pushed"), EffectStatus::Done)));
  record_page(app_data.path(), told_error("/b", "Cannot push docs")).unwrap();
  let journal = journal_path_in(registry.dir()).to_string_lossy().into_owned();
  // a directory where the journal should be: its read fails at once
  let gone = app_data.path().join("gone");
  std::fs::create_dir_all(&gone).unwrap();
  let projects = [
    ("/a".to_string(), Some(journal.clone())),
    ("/b".to_string(), None),
    ("/a-feature".to_string(), Some(journal.clone())),
    ("/c".to_string(), Some(gone.to_string_lossy().into_owned())),
  ];

  let log = read_activity(&projects, app_data.path(), Duration::from_mins(1), &unheard()).await;
  let journals: Vec<_> = log
    .journals
    .iter()
    .map(|journal| {
      (
        journal.repositories.clone(),
        journal.available,
        journal.entries.iter().map(|entry| entry.id.0.clone()).collect::<Vec<_>>(),
      )
    })
    .collect();
  assert_eq!(
    journals,
    [
      (vec!["/a".to_string(), "/a-feature".to_string()], true, vec!["pushed".to_string()]),
      (vec!["/c".to_string()], false, Vec::new()),
    ]
  );
  assert_eq!(log.page.iter().map(|entry| entry.activity.title.as_str()).collect::<Vec<_>>(), ["Cannot push docs"]);
}

/// The journal of a repository that wrote none yet is an empty journal, but the journal of a repository that is gone (its
/// disk removed: no `$GIT_COMMON_DIR` where the settings remember it) is unavailable, never an empty log
#[tokio::test(flavor = "multi_thread")]
async fn the_journal_of_a_repository_that_is_gone_is_unavailable_and_one_not_written_yet_is_empty() {
  let (common, app_data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
  let gone = common.path().join("removed-disk").join(".git");
  let projects = [
    ("/new".to_string(), Some(journal_path(common.path()).to_string_lossy().into_owned())),
    ("/gone".to_string(), Some(journal_path(&gone).to_string_lossy().into_owned())),
  ];

  let log = read_activity(&projects, app_data.path(), Duration::from_mins(1), &unheard()).await;
  let available: Vec<_> = log.journals.iter().map(|journal| (journal.repositories.clone(), journal.available)).collect();
  assert_eq!(available, [(vec!["/new".to_string()], true), (vec!["/gone".to_string()], false)]);
  assert_eq!(journal_path(common.path()), journal_path_in(ProcessRegistry::of(common.path()).unflushed().dir()));
}

/// A late answer of a journal that nobody hears
fn unheard() -> LateListener {
  Arc::new(|| {})
}

/// What Activity shows of each journal of `log`: whether it is available, and the ids of its entries
#[cfg(unix)]
fn shown(log: &domain_types::activity::ActivityLog) -> Vec<(bool, Vec<String>)> {
  log
    .journals
    .iter()
    .map(|journal| (journal.available, journal.entries.iter().map(|entry| entry.id.0.clone()).collect()))
    .collect()
}

/// A journal at a pipe in `directory`, as on a disk that hangs: a read of it waits until the test answers it
/// (`answer_hung_read`)
#[cfg(unix)]
fn journal_on_a_hung_disk(directory: &std::path::Path) -> std::path::PathBuf {
  let pipe = directory.join("journal.jsonl");
  let made = sync_utils::child_process::windowless_command("mkfifo").arg(&pipe).status().unwrap();
  assert!(made.success());
  pipe
}

/// The writer of the pipe of a journal on a disk that hangs, once a read of it waits for the disk
#[cfg(unix)]
fn hung_disk_writer(pipe: &std::path::Path) -> std::fs::File {
  std::fs::OpenOptions::new().write(true).open(pipe).unwrap()
}

/// More than a pipe holds: 8 KiB of a FIFO on macOS, 64 KiB on Linux
#[cfg(unix)]
const MORE_THAN_A_PIPE_HOLDS: usize = 1 << 17;

/// Answers `text` to the read that waits for the disk of `disk`, the writer of its pipe, which closes then. A read that
/// waits in its open of a pipe misses a writer that opens and closes before the read runs again (macOS: about 1 in 200
/// such answers, and the read waits for the next writer), so the writer stays until the read took a part of what it
/// writes: after `text`, a line the journal skips, longer than a pipe holds
#[cfg(unix)]
fn answer_hung_read(disk: std::fs::File, text: &str) {
  let mut disk = disk;
  std::io::Write::write_all(&mut disk, format!("{text}\n{}\n", " ".repeat(MORE_THAN_A_PIPE_HOLDS)).as_bytes()).unwrap();
}

/// A journal that does not answer within the bound (a disk that hangs or is slow: here a pipe the test writes to late)
/// is unavailable while it never answered, and the read does not wait for it; a read meanwhile waits for the read that
/// begins once that one answered. Once it answers other entries the windows hear it, and the next read shows them, as
/// what it answered last, while the read it waits for waits for the disk again
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_journal_that_answers_after_the_bound_shows_once_it_answered() {
  let directory = tempfile::tempdir().unwrap();
  let pipe = journal_on_a_hung_disk(directory.path());
  let projects = [("/a".to_string(), Some(pipe.to_string_lossy().into_owned()))];
  let app_data = tempfile::tempdir().unwrap();
  let (told, mut heard) = tokio::sync::mpsc::unbounded_channel();
  // the test may stop hearing before a read it started answers
  let late: LateListener = Arc::new(move || {
    let _ = told.send(());
  });

  let log = read_activity(&projects, app_data.path(), Duration::from_millis(50), &late).await;
  assert_eq!(shown(&log), [(false, Vec::new())]);
  let log = read_activity(&projects, app_data.path(), Duration::from_millis(50), &late).await;
  assert_eq!(shown(&log), [(false, Vec::new())]);

  // the disk answers the read that runs: the line of an operation of the repository
  let common = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(common.path()).unflushed();
  finished(&registry, "pushed", &session("user"), &safe_push("auth"), Ok(&started(&id("pushed"), EffectStatus::Done)));
  answer_hung_read(hung_disk_writer(&pipe), &std::fs::read_to_string(journal_path_in(registry.dir())).unwrap());
  heard.recv().await.unwrap();
  let log = read_activity(&projects, app_data.path(), Duration::from_millis(50), &late).await;
  assert_eq!(shown(&log), [(true, vec!["pushed".to_string()])]);
  // the reads that still wait for a writer end with one each, so the runtime of the test can stop: the second answers
  // no entries, which the windows hear, then the third begins
  answer_hung_read(hung_disk_writer(&pipe), "");
  heard.recv().await.unwrap();
  answer_hung_read(hung_disk_writer(&pipe), "");
}

/// A read of Activity asked while a read of the journal runs (a slow disk: here a pipe the test writes to late) never
/// takes the answer of that read, which began before the change the read was asked for: an operation of `bd` ended
/// meanwhile, and the windows heard its journal change. It shows the read that begins once that one answered, with the
/// entry of `bd`
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_read_asked_while_one_runs_shows_the_journal_as_it_is_after_that_one() {
  let directory = tempfile::tempdir().unwrap();
  let path = journal_on_a_hung_disk(directory.path());
  let projects = [("/a".to_string(), Some(path.to_string_lossy().into_owned()))];
  let app_data = tempfile::tempdir().unwrap();
  let common = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(common.path()).unflushed();
  finished(&registry, "pushed", &session("user"), &safe_push("auth"), Ok(&started(&id("pushed"), EffectStatus::Done)));
  let before = std::fs::read_to_string(journal_path_in(registry.dir())).unwrap();
  finished(&registry, "landed", &session("user"), &safe_push("docs"), Ok(&started(&id("landed"), EffectStatus::Done)));

  // the first read opens the journal (the writer of the pipe opens once it did) and waits for the disk
  let log = read_activity(&projects, app_data.path(), Duration::from_millis(50), &unheard()).await;
  assert_eq!(shown(&log), [(false, Vec::new())]);
  let pipe = path.clone();
  let disk = tokio::task::spawn_blocking(move || hung_disk_writer(&pipe)).await.unwrap();
  // the operation of bd ends: its entry is in the journal, and the window reads again while the first read waits
  let written = directory.path().join("written");
  std::fs::copy(journal_path_in(registry.dir()), &written).unwrap();
  std::fs::rename(&written, &path).unwrap();
  let late = unheard();
  let mut again = std::pin::pin!(read_activity(&projects, app_data.path(), Duration::from_mins(1), &late));
  let mut context = std::task::Context::from_waker(std::task::Waker::noop());
  assert!(again.as_mut().poll(&mut context).is_pending());

  // the disk answers the first read with the journal it opened, before the entry of bd
  answer_hung_read(disk, &before);
  assert_eq!(shown(&again.await), [(true, vec!["pushed".to_string(), "landed".to_string()])]);
}
