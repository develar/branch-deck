//! The plans read ahead after a rewrite (`review_prefetch`): a commit of the table that moved a branch a closed window
//! asked reads its plan once, below the app; no other branch; a newer commit supersedes the reading in flight; a window
//! that asks the plan being read ahead makes that reading yield and reads the plan itself. And the one reading of a
//! plan (`PlanReader::read`): a window that joins the reading of another gets each stage it published, then the plan,
//! and reads the plan itself when that reading ends without one

use crate::context::PrefixInputs;
use crate::reads::review_plan::{Asked, PlanReader, ReviewPlans, ask_plan};
use crate::reads::review_prefetch::read_ahead;
use crate::reads::symbols::SymbolIndexes;
use crate::repository_state::RepositoryStateCache;
use crate::session::windows::Windows;
use crate::transport::{EventSink, SinkGone, WindowId};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use diff_marks::index::{RenameAnswer, RenamePair};
use domain_types::error::{ApiError, ErrorCode};
use file_diffs::accio::{DeclarationAt, References, SearchCanceled, SymbolHit, SymbolIndex, TestReach};
use file_diffs::service::FileDiffs;
use pretty_assertions::assert_eq;
use repository_model::branch_table::BranchTable;
use repository_model::session::SessionKey;
use review_plan::concerns::ConcernEdges;
use review_plan::{PlanWait, ReviewPlanEvent, ReviewPlanParams, ReviewSubject};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_utils::child_process::ChildPriority;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;
use tokio::sync::mpsc;

const PREFIX: &str = "jane";

/// An index whose queries of the usages answer none and whose queries of the renames of a subject answer that it
/// knows no such answer, each recorded with the priority of its search; while it is held, a query waits until it is
/// released or its search is canceled. Its query of the tests answers that none reaches, held apart, unrecorded
#[derive(Default)]
pub(crate) struct Index {
  /// The revision of each query and the priority of its search, in order
  pub(crate) asked: Mutex<Vec<(String, ChildPriority)>>,
  pub(crate) held: AtomicBool,
  /// The query of the tests waits while it is set
  pub(crate) tests_held: AtomicBool,
  /// The queries the cancel of their search killed
  pub(crate) killed: AtomicU32,
  /// The revision of each query as it starts
  pub(crate) started: Mutex<Option<mpsc::UnboundedSender<String>>>,
}

pub(crate) struct Indexes(pub(crate) Arc<Index>);

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

impl Search<'_> {
  /// A query of `revision`: recorded, said to have started, and held while the index is, until it is released or
  /// the search is canceled, which kills it
  fn query(&self, revision: &str) -> anyhow::Result<()> {
    self.index.asked.lock().unwrap_or_else(PoisonError::into_inner).push((revision.to_string(), self.priority));
    if let Some(started) = self.index.started.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
      let _ = started.send(revision.to_string());
    }
    self.wait(&self.index.held)
  }

  /// Waits while `held` is set, until the search is canceled, which kills the query: a poll, as the one of accio's
  /// process
  fn wait(&self, held: &AtomicBool) -> anyhow::Result<()> {
    while held.load(Ordering::SeqCst) {
      if (self.canceled)() {
        self.index.killed.fetch_add(1, Ordering::SeqCst);
        return Err(SearchCanceled.into());
      }
      std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
  }
}

impl SymbolIndex for Search<'_> {
  fn symbols(&self, _repository_root: &str, _query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(None)
  }

  fn references(&self, _repository_root: &str, _name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    Ok(None)
  }

  fn references_at(&self, _repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    self.query(revision)?;
    let none = References {
      hits: Vec::new(),
      by_name: 0,
      complete: true,
      unavailable: false,
    };
    Ok(Some(declarations.iter().map(|_| none.clone()).collect()))
  }

  fn tests(&self, _repository_root: &str, declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    self.wait(&self.index.tests_held)?;
    let reach = TestReach {
      complete: true,
      ..TestReach::default()
    };
    Ok(Some(declarations.iter().map(|_| reach.clone()).collect()))
  }

  fn renames(&self, _repository_root: &str, _pairs: &[RenamePair], _base: &str, head: &str) -> anyhow::Result<Option<Vec<RenameAnswer>>> {
    self.query(head)?;
    Ok(None)
  }
}

/// The page of a window: the events of its plans, and who hears what each plan still waits for as it comes
#[derive(Default)]
struct Page {
  events: Mutex<Vec<ReviewPlanEvent>>,
  heard: Option<mpsc::UnboundedSender<Option<PlanWait>>>,
}

impl Page {
  /// A page whose plans are heard as they come: what each still waits for
  fn heard() -> (Arc<Self>, mpsc::UnboundedReceiver<Option<PlanWait>>) {
    let (heard, plans) = mpsc::unbounded_channel();
    let page = Self {
      events: Mutex::default(),
      heard: Some(heard),
    };
    (Arc::new(page), plans)
  }
}

impl EventSink<ReviewPlanEvent> for Page {
  fn send(&self, event: ReviewPlanEvent) -> Result<(), SinkGone> {
    if let (ReviewPlanEvent::Plan { waits, .. }, Some(heard)) = (&event, &self.heard) {
      let _ = heard.send(*waits);
    }
    self.events.lock().unwrap_or_else(PoisonError::into_inner).push(event);
    Ok(())
  }
}

/// The plans a page got, in order: the head of each and what it still waits for
type Plans = Vec<(String, Option<PlanWait>)>;

fn head_of(event: &ReviewPlanEvent) -> String {
  match event {
    ReviewPlanEvent::Plan { plan, .. } => plan.head.clone(),
    ReviewPlanEvent::Disabled => panic!("the repository reads review plans"),
  }
}

fn waits_of(event: &ReviewPlanEvent) -> Option<PlanWait> {
  match event {
    ReviewPlanEvent::Plan { waits, .. } => *waits,
    ReviewPlanEvent::Disabled => panic!("the repository reads review plans"),
  }
}

/// The stages of a reading that asks the index its usages alone, then its plan, all at `head`
fn staged(head: &str) -> Plans {
  vec![(head.to_string(), Some(PlanWait::Index)), (head.to_string(), None)]
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

  /// A commit on top of the virtual branch `name` that gives its declaration a third parameter, with a test that calls
  /// it: its new head
  fn test_branch(&self, name: &str) -> String {
    let test = format!("package a\n\nimport org.junit.Test\n\nclass ApiTest {{\n  @Test fun `takes three`() = check({name}(1, 2, 3) == 1)\n}}\n");
    let [head] = self.repo.commits_on(
      &format!("refs/heads/{}", virtual_branch(name)),
      &[HistoryCommit::new(
        format!("Test {name}"),
        &[
          (path(name).as_str(), declaration(name, "x: Int, y: Int, z: Int").as_str()),
          ("test/ApiTest.kt", test.as_str()),
        ],
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

  /// The plans of `name` that `window` asks and gets
  async fn ask(&self, window: &str, name: &str) -> Plans {
    self.ask_on(window, name, Arc::new(Page::default())).await.expect("the plan")
  }

  /// The plans of `name` that `window` asks and gets on `page`, or why its ask failed
  async fn ask_on(&self, window: &str, name: &str, page: Arc<Page>) -> Result<Plans, ApiError> {
    ask_plan(
      self.reader.clone(),
      &self.windows,
      &WindowId::new(window),
      self.params(name),
      Arc::clone(&page) as Arc<dyn EventSink<ReviewPlanEvent>>,
    )
    .await?;
    let events = page.events.lock().unwrap();
    Ok(events.iter().map(|event| (head_of(event), waits_of(event))).collect())
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
}

/// A read ahead that holds in its query of the index (`Fixture::read_ahead_held`)
struct Held {
  /// The head it reads the plan of
  moved: String,
  reading: tokio::task::JoinHandle<()>,
  /// The revision of each query of the usages from now on, as it starts
  started: mpsc::UnboundedReceiver<String>,
}

/// A commit of the table that moved the branch a closed window asked the plan of reads that plan once, its query of
/// the index below the app: the window that opens it again finds it whole, and a commit that leaves the head there
/// reads nothing
#[tokio::test(flavor = "multi_thread")]
async fn a_commit_that_moved_a_branch_a_window_asked_reads_its_plan_ahead_once_below_the_app() {
  let fixture = Fixture::new(&["auth"]);
  let asked = fixture.head("auth");
  assert_eq!(fixture.ask("review", "auth").await, staged(&asked));
  fixture.close("review");
  assert!(fixture.commit(&["auth"]).is_none(), "the head the window read");

  let moved = fixture.move_branch("auth", "x: Int, y: Int, z: Int");
  fixture.commit(&["auth"]).expect("a read ahead of the moved branch").await;
  assert_eq!(fixture.asked(), [(asked, ChildPriority::Normal), (moved.clone(), ChildPriority::Background)]);
  assert!(fixture.commit(&["auth"]).is_none(), "read once");
  assert_eq!(fixture.ask("review", "auth").await, [(moved, None)]);
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
  assert_eq!(fixture.ask("review", "auth").await, [(fixture.head("auth"), None)]);
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
  assert_eq!(fixture.ask("review", "auth").await, [(newest.clone(), None)]);
  assert_eq!(
    fixture.asked(),
    [(asked, ChildPriority::Normal), (between, ChildPriority::Background), (newest, ChildPriority::Background)]
  );
}

/// A window that asks the plan being read ahead joins that reading, which shows it the plan as it stands and then
/// yields to it: the query of the read ahead is killed, the reading ends without a plan, and the window reads the
/// plan itself, its query at the priority of the app
#[tokio::test(flavor = "multi_thread")]
async fn a_read_ahead_yields_to_the_window_that_asks_its_plan() {
  let fixture = Fixture::new(&["auth"]);
  let asked = fixture.head("auth");
  let Held { moved, reading, mut started } = fixture.read_ahead_held().await;

  let window = fixture.ask("review", "auth");
  let released = async {
    // the query of the window itself, once the read ahead gave way
    assert_eq!(started.recv().await.as_ref(), Some(&moved));
    fixture.release();
  };
  let (plans, ()) = tokio::join!(window, released);
  reading.await.unwrap();
  // the stage the read ahead had published, then the stages of its own reading and the plan
  assert_eq!(plans, [vec![(moved.clone(), Some(PlanWait::Index))], staged(&moved)].concat());
  assert_eq!(fixture.index.killed.load(Ordering::SeqCst), 1, "the query of the read ahead is killed");
  assert_eq!(
    fixture.asked(),
    [(asked, ChildPriority::Normal), (moved.clone(), ChildPriority::Background), (moved, ChildPriority::Normal)]
  );
  assert!(fixture.reader.plans.keeps(&fixture.resolve("auth")));
}

/// A window that asks the plan another window reads joins that reading and gets what it published: the latest stage
/// at once, each later one as it comes, then the plan; the index is asked once
#[tokio::test(flavor = "multi_thread")]
async fn a_window_that_joins_a_reading_gets_each_stage_it_published_then_the_plan() {
  let fixture = Fixture::new(&["auth"]);
  // a test of the change, so that the tests of its steps are a query of the index
  let head = fixture.test_branch("auth");
  let mut started = fixture.hold();
  fixture.index.tests_held.store(true, Ordering::SeqCst);
  let (page, mut heard) = Page::heard();

  let first = fixture.ask("review-1", "auth");
  let second = async {
    // the first window reads: its plan of git alone is published, its usages are asked
    assert_eq!(started.recv().await.as_ref(), Some(&head));
    fixture.ask_on("review-2", "auth", page).await.expect("the plan")
  };
  let released = async {
    assert_eq!(heard.recv().await, Some(Some(PlanWait::Index)), "the stage published before it joined");
    fixture.release();
    assert_eq!(heard.recv().await, Some(Some(PlanWait::Tests)), "the stage published while it waits");
    fixture.index.tests_held.store(false, Ordering::SeqCst);
  };
  let (first, second, ()) = tokio::join!(first, second, released);
  let stages = vec![(head.clone(), Some(PlanWait::Index)), (head.clone(), Some(PlanWait::Tests)), (head, None)];
  assert_eq!((first, second), (stages.clone(), stages));
  assert_eq!(fixture.asked().len(), 1, "one reading");
  // only the plan the reading answered is kept: the one a window that opens it again gets, whole
  let again = fixture.ask("review-3", "auth").await;
  assert_eq!(again.iter().map(|(_, waits)| *waits).collect::<Vec<_>>(), [None]);
}

/// A reading whose window asked a newer plan ends without a plan, its query killed: the window that waited for it
/// reads the plan itself and gets it, and the request the window went on from gets no plan after the stage it had
#[tokio::test(flavor = "multi_thread")]
async fn a_window_that_waited_for_a_superseded_reading_reads_the_plan_itself() {
  let fixture = Fixture::new(&["auth", "docs"]);
  let (auth, docs) = (fixture.head("auth"), fixture.head("docs"));
  let mut started = fixture.hold();
  let (page, mut heard) = Page::heard();

  let superseded = Arc::new(Page::default());
  let first = fixture.ask_on("review-1", "auth", Arc::clone(&superseded));
  let others = async {
    assert_eq!(started.recv().await.as_ref(), Some(&auth));
    let second = async { fixture.ask_on("review-2", "auth", page).await.expect("the plan") };
    let newer = async {
      // the second window waits for the reading of the first, which then asks another plan
      assert_eq!(heard.recv().await, Some(Some(PlanWait::Index)));
      let plans = fixture.ask("review-1", "docs");
      let released = async {
        // the query of the second window, which reads the plan itself, and the one of the newer plan
        let mut asked = [started.recv().await.unwrap(), started.recv().await.unwrap()];
        asked.sort();
        let mut expected = [auth.clone(), docs.clone()];
        expected.sort();
        assert_eq!(asked, expected);
        fixture.release();
      };
      tokio::join!(plans, released).0
    };
    tokio::join!(second, newer)
  };
  let (first, (second, newer)) = tokio::join!(first, others);
  assert_eq!(first.unwrap_err().code, ErrorCode::Canceled);
  let before: Vec<Option<PlanWait>> = superseded.events.lock().unwrap().iter().map(waits_of).collect();
  assert_eq!(before, [Some(PlanWait::Index)], "the plan of git alone, sent before the window asked another");
  assert_eq!(fixture.index.killed.load(Ordering::SeqCst), 1, "the query of the superseded reading is killed");
  assert_eq!(second, [vec![(auth.clone(), Some(PlanWait::Index))], staged(&auth)].concat());
  assert_eq!(newer, staged(&docs));
}
