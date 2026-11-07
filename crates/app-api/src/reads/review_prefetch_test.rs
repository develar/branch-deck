//! The plans read ahead after a rewrite (`review_prefetch`): a commit of the table that moved a branch a closed window
//! asked reads its plan once, below the app; no other branch; a newer commit supersedes the reading in flight; a window
//! that asks the plan being read joins that reading, and reads it itself when the reading ends without a plan

use crate::context::PrefixInputs;
use crate::reads::review_plan::{Asked, Canceled, PlanReader, ReviewPlans, Turn, ask_plan, joined};
use crate::reads::review_prefetch::read_ahead;
use crate::reads::symbols::SymbolIndexes;
use crate::repository_state::RepositoryStateCache;
use crate::session::windows::Windows;
use crate::transport::{EventSink, SinkGone, WindowId};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use file_diffs::accio::{DeclarationAt, References, SearchCanceled, SymbolHit, SymbolIndex, TestReach};
use file_diffs::service::FileDiffs;
use pretty_assertions::assert_eq;
use repository_model::branch_table::BranchTable;
use repository_model::session::SessionKey;
use review_plan::concerns::ConcernEdges;
use review_plan::{ReviewPlan, ReviewPlanEvent, ReviewPlanParams, ReviewSubject};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_utils::child_process::ChildPriority;
use sync_utils::single_flight::Join;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;
use tokio::sync::mpsc;

const PREFIX: &str = "jane";

/// An index whose queries of the usages answer none, each recorded with the priority of its search; while it is held,
/// a query waits until it is released or its search is canceled
#[derive(Default)]
struct Index {
  /// The revision of each query of the usages and the priority of its search, in order
  asked: Mutex<Vec<(String, ChildPriority)>>,
  held: AtomicBool,
  /// The queries the cancel of their search killed
  killed: AtomicU32,
  /// The revision of each query of the usages as it starts
  started: Mutex<Option<mpsc::UnboundedSender<String>>>,
}

struct Indexes(Arc<Index>);

impl SymbolIndexes for Indexes {
  fn search<'a>(&'a self, canceled: &'a (dyn Fn() -> bool + Sync), priority: ChildPriority) -> Option<Box<dyn SymbolIndex + 'a>> {
    Some(Box::new(Search {
      index: &self.0,
      canceled,
      priority,
    }))
  }
}

struct Search<'a> {
  index: &'a Index,
  canceled: &'a (dyn Fn() -> bool + Sync),
  priority: ChildPriority,
}

impl SymbolIndex for Search<'_> {
  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(None)
  }

  fn references(&self, _repository_root: &str, _name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    Ok(None)
  }

  fn references_at(&self, _repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    self.index.asked.lock().unwrap_or_else(PoisonError::into_inner).push((revision.to_string(), self.priority));
    if let Some(started) = self.index.started.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
      let _ = started.send(revision.to_string());
    }
    // a poll, as the one of accio's process
    while self.index.held.load(Ordering::SeqCst) {
      if (self.canceled)() {
        self.index.killed.fetch_add(1, Ordering::SeqCst);
        return Err(SearchCanceled.into());
      }
      std::thread::sleep(Duration::from_millis(5));
    }
    let none = References {
      hits: Vec::new(),
      by_name: 0,
      complete: true,
      unavailable: false,
    };
    Ok(Some(declarations.iter().map(|_| none.clone()).collect()))
  }

  fn tests(&self, _repository_root: &str, declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    let reach = TestReach {
      complete: true,
      ..TestReach::default()
    };
    Ok(Some(declarations.iter().map(|_| reach.clone()).collect()))
  }
}

/// The page of a window: the events of its plans
#[derive(Default)]
struct Page(Mutex<Vec<ReviewPlanEvent>>);

impl EventSink<ReviewPlanEvent> for Page {
  fn send(&self, event: ReviewPlanEvent) -> Result<(), SinkGone> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).push(event);
    Ok(())
  }
}

/// A repository of the prefix `jane` that reads review plans: its `master` declares `fun <name>(x: Int)` in
/// `src/<name>.kt` for each branch, and the virtual branch of each adds a parameter
struct Fixture {
  repo: TestRepo,
  base: String,
  index: Arc<Index>,
  reader: PlanReader,
  windows: Windows,
  _settings: tempfile::TempDir,
}

fn path(name: &str) -> String {
  format!("src/{name}.kt")
}

fn declaration(name: &str, parameters: &str) -> String {
  format!("package a\n\nfun {name}({parameters}): Int = x\n")
}

fn virtual_branch(name: &str) -> String {
  format!("{PREFIX}/virtual/{name}")
}

impl Fixture {
  fn new(branches: &[&str]) -> Self {
    let files: Vec<(String, String)> = branches.iter().map(|name| (path(name), declaration(name, "x: Int"))).collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
    let (repo, [base]) = TestRepo::with_commits(&[HistoryCommit::new("Declare the API", &files)]);
    repo.set_config("branchdeck.branchPrefix", PREFIX).unwrap();
    repo.set_config("branchdeck.reviewExperimental", "true").unwrap();
    let settings = tempfile::tempdir().unwrap();
    let index = Arc::new(Index::default());
    let reader = PlanReader::new(
      PrefixInputs::new(
        repo.git_executor(),
        &SettingsService::open(settings.path().to_path_buf()).unwrap(),
        &BranchPrefixes::default(),
      ),
      RepositoryStateCache::without_object_caches(),
      FileDiffs::default(),
      Arc::new(Indexes(Arc::clone(&index))),
      ReviewPlans::default(),
    );
    let fixture = Self {
      repo,
      base,
      index,
      reader,
      windows: Windows::default(),
      _settings: settings,
    };
    for name in branches {
      fixture.repo.create_branch_at(&virtual_branch(name), &fixture.base).unwrap();
      fixture.move_branch(name, "x: Int, y: Int");
    }
    fixture
  }

  /// A commit on top of the virtual branch `name` that gives its declaration `parameters`: its new head
  fn move_branch(&self, name: &str, parameters: &str) -> String {
    let [head] = self.repo.commits_on(
      &format!("refs/heads/{}", virtual_branch(name)),
      &[HistoryCommit::new(
        format!("Take {parameters} in {name}"),
        &[(path(name).as_str(), declaration(name, parameters).as_str())],
      )],
    );
    head
  }

  fn head(&self, name: &str) -> String {
    self.repo.rev_parse(&virtual_branch(name)).unwrap()
  }

  /// The plan of the branch `name` as its review window asks it: on the base of its part, the commit it is built on
  fn params(&self, name: &str) -> ReviewPlanParams {
    ReviewPlanParams {
      repository_path: self.repo.path_str().to_string(),
      subject: ReviewSubject::Branch {
        name: name.to_string(),
        base: Some(self.base.clone()),
        baseline_branch: None,
      },
      since_reviewed: false,
      concern_edges: ConcernEdges::default(),
    }
  }

  /// The events of the plan of `name` that `window` asks: the head of each plan, and whether it is pending
  async fn ask(&self, window: &str, name: &str) -> Vec<(String, bool)> {
    let page = Arc::new(Page::default());
    ask_plan(
      self.reader.clone(),
      &self.windows,
      &WindowId::new(window),
      self.params(name),
      Arc::clone(&page) as Arc<dyn EventSink<ReviewPlanEvent>>,
    )
    .await
    .unwrap();
    let events = page.0.lock().unwrap();
    events
      .iter()
      .map(|event| match event {
        ReviewPlanEvent::Plan { plan, pending } => (plan.head.clone(), *pending),
        ReviewPlanEvent::Disabled => panic!("the repository reads review plans"),
      })
      .collect()
  }

  /// `window` closed (`AppContext::forget_window`)
  fn close(&self, window: &str) {
    let window = WindowId::new(window);
    self.windows.forget_reads(&window);
    self.reader.plans.forget_window(&window);
  }

  /// The plan of `name` resolved as its window asks it now
  fn resolve(&self, name: &str) -> Asked {
    self.reader.resolve(self.params(name)).unwrap()
  }

  /// A sync committed the table of the repository with the heads of `branches` now: its read ahead, if any
  fn commit(&self, branches: &[&str]) -> Option<impl Future<Output = ()> + Send + use<>> {
    let table = BranchTable {
      prefix: PREFIX.to_string(),
      baseline: "master".to_string(),
      tips: branches.iter().map(|name| (virtual_branch(name), self.head(name))).collect(),
      ..BranchTable::default()
    };
    let key = SessionKey {
      worktree: self.repo.path_str().to_string(),
      prefix: PREFIX.to_string(),
    };
    read_ahead(self.reader.clone(), &key, &table)
  }

  /// The queries of the usages from now on wait for `release`; each, as it starts, says its revision
  fn hold(&self) -> mpsc::UnboundedReceiver<String> {
    let (started, starts) = mpsc::unbounded_channel();
    *self.index.started.lock().unwrap() = Some(started);
    self.index.held.store(true, Ordering::SeqCst);
    starts
  }

  fn release(&self) {
    self.index.held.store(false, Ordering::SeqCst);
  }

  fn asked(&self) -> Vec<(String, ChildPriority)> {
    self.index.asked.lock().unwrap().clone()
  }

  /// A window asked the plan of `auth` and closed, `auth` moved, and the read ahead of the commit after it holds in its
  /// query of the index
  async fn read_ahead_held(&self) -> Held {
    self.ask("review", "auth").await;
    self.close("review");
    let mut started = self.hold();
    let moved = self.move_branch("auth", "x: Int, y: Int, z: Int");
    let reading = tokio::spawn(self.commit(&["auth"]).expect("a read ahead"));
    assert_eq!(started.recv().await.as_ref(), Some(&moved));
    Held { moved, reading, started }
  }

  /// The reading in flight that a window which asks `asked` now joins
  fn join(&self, asked: &Asked) -> Join<Arc<ReviewPlan>> {
    let Turn::Joined(join) = self.reader.lead_or_join(asked, &never(), ChildPriority::Normal, None).unwrap() else {
      panic!("the window read the plan being read ahead again");
    };
    join
  }
}

/// A read ahead that holds in its query of the index (`Fixture::read_ahead_held`)
struct Held {
  /// The head it reads the plan of
  moved: String,
  reading: tokio::task::JoinHandle<()>,
  /// The revision of each query of the usages from now on, as it starts
  started: mpsc::UnboundedReceiver<String>,
}

fn never() -> Canceled {
  Arc::new(|| false)
}

/// A commit of the table that moved the branch a closed window asked the plan of reads that plan once, its query of
/// the index below the app: the window that opens it again finds it whole, and a commit that leaves the head there
/// reads nothing
#[tokio::test(flavor = "multi_thread")]
async fn a_commit_that_moved_a_branch_a_window_asked_reads_its_plan_ahead_once_below_the_app() {
  let fixture = Fixture::new(&["auth"]);
  let asked = fixture.head("auth");
  assert_eq!(fixture.ask("review", "auth").await, [(asked.clone(), true), (asked.clone(), false)]);
  fixture.close("review");
  assert!(fixture.commit(&["auth"]).is_none(), "the head the window read");

  let moved = fixture.move_branch("auth", "x: Int, y: Int, z: Int");
  fixture.commit(&["auth"]).expect("a read ahead of the moved branch").await;
  assert_eq!(fixture.asked(), [(asked, ChildPriority::Normal), (moved.clone(), ChildPriority::Background)]);
  assert!(fixture.commit(&["auth"]).is_none(), "read once");
  assert_eq!(fixture.ask("review", "auth").await, [(moved, false)]);
  assert_eq!(fixture.asked().len(), 2, "the window asks the index nothing");
}

/// Only a branch a window asked is read ahead, and only once no window shows it: an open window reads its own plan when
/// it sees the new head, at the priority of the app, and a branch no window asked is never read
#[tokio::test(flavor = "multi_thread")]
async fn only_a_branch_a_window_asked_and_no_window_shows_is_read_ahead() {
  let fixture = Fixture::new(&["auth", "docs", "feed"]);
  fixture.ask("review-1", "auth").await;
  fixture.ask("review-2", "docs").await;
  fixture.close("review-1");
  let [auth, _, _] = ["auth", "docs", "feed"].map(|name| fixture.move_branch(name, "x: Int, y: Int, z: Int"));

  fixture.commit(&["auth", "docs", "feed"]).expect("a read ahead of auth").await;
  assert_eq!(fixture.asked()[2..], [(auth, ChildPriority::Background)]);
}

/// The read ahead watches the 4 branches of a repository a window asked last: a fifth branch forgets the one asked
/// longest ago, and a branch asked again is the newest; the newest is read first
#[tokio::test(flavor = "multi_thread")]
async fn a_fifth_asked_branch_forgets_the_one_asked_longest_ago() {
  let names = ["auth", "docs", "feed", "form", "grid"];
  let fixture = Fixture::new(&names);
  for name in &names[..4] {
    fixture.ask("review", name).await;
  }
  // again from the memory of the plans: the newest watch now, asking the index nothing
  assert_eq!(fixture.ask("review", "auth").await, [(fixture.head("auth"), false)]);
  fixture.ask("review", "grid").await;
  fixture.close("review");
  assert_eq!(fixture.asked().len(), 5);
  let [auth, _, feed, form, grid] = names.map(|name| fixture.move_branch(name, "x: Int, y: Int, z: Int"));

  fixture.commit(&names).expect("a read ahead").await;
  let ahead: Vec<String> = fixture.asked()[5..].iter().map(|(revision, _)| revision.clone()).collect();
  assert_eq!(ahead, [grid, auth, form, feed], "docs, asked longest ago, is read ahead no more");
}

/// A newer commit of the table supersedes the read ahead in flight: its query of the index is killed, and the newer
/// one reads the plan of the newest head, which the window that opens again finds whole
#[tokio::test(flavor = "multi_thread")]
async fn a_newer_commit_supersedes_the_read_ahead_in_flight() {
  let fixture = Fixture::new(&["auth"]);
  let asked = fixture.head("auth");
  let Held {
    moved: between,
    reading: first,
    mut started,
  } = fixture.read_ahead_held().await;

  let newest = fixture.move_branch("auth", "x: Int, z: Int");
  let second = tokio::spawn(fixture.commit(&["auth"]).expect("a read ahead"));
  first.await.unwrap();
  assert_eq!(fixture.index.killed.load(Ordering::SeqCst), 1, "the query of the superseded read ahead is killed");
  assert_eq!(started.recv().await.as_ref(), Some(&newest));
  fixture.release();
  second.await.unwrap();
  assert_eq!(fixture.ask("review", "auth").await, [(newest.clone(), false)]);
  assert_eq!(
    fixture.asked(),
    [(asked, ChildPriority::Normal), (between, ChildPriority::Background), (newest, ChildPriority::Background)]
  );
}

/// A window that asks the plan being read ahead waits for that reading and gets its plan: the index is asked once
#[tokio::test(flavor = "multi_thread")]
async fn a_window_that_asks_the_plan_being_read_ahead_joins_its_reading() {
  let fixture = Fixture::new(&["auth"]);
  let Held { moved, reading, .. } = fixture.read_ahead_held().await;

  let join = fixture.join(&fixture.resolve("auth"));
  fixture.release();
  let plan = joined(join, &never()).await.expect("the plan read ahead");
  reading.await.unwrap();
  assert_eq!((plan.head.as_str(), fixture.asked().len()), (moved.as_str(), 2));
  assert!(fixture.reader.plans.keeps(&fixture.resolve("auth")));
}

/// A read ahead that a newer commit supersedes ends without a plan: the window that waited for it reads the plan itself
/// (or joins the reading of the newer commit), and gets it; the index answers it once
#[tokio::test(flavor = "multi_thread")]
async fn a_window_that_waited_for_a_superseded_read_ahead_still_gets_its_plan() {
  let fixture = Fixture::new(&["auth"]);
  let Held { moved, reading: first, .. } = fixture.read_ahead_held().await;
  let asked = fixture.resolve("auth");
  let join = fixture.join(&asked);

  // a sync that moved nothing commits again: the read ahead of the head not read yet starts over
  let second = tokio::spawn(fixture.commit(&["auth"]).expect("the head is not read yet"));
  assert!(joined(join, &never()).await.is_none(), "the superseded reading ends without a plan");
  first.await.unwrap();
  let reader = fixture.reader.clone();
  let window = tokio::spawn(async move { reader.read(&asked, &never(), ChildPriority::Normal, None).await });
  fixture.release();
  let plan = window.await.unwrap().unwrap();
  second.await.unwrap();
  let answered: Vec<_> = fixture.asked().into_iter().filter(|(revision, _)| *revision == moved).collect();
  assert_eq!((plan.head.as_str(), answered.len()), (moved.as_str(), 2), "one killed, one answered: {answered:?}");
}
