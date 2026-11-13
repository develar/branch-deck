//! The accio process of a lookup: waited for as long as it runs, killed once the navigation is canceled, below the app
//! for a plan read ahead of its window

#[cfg(unix)]
use crate::accio::AccioSearch;
use crate::accio::{
  AccioCli, CallArgument, DeclarationAt, DescriptorClass, DescriptorService, HitKind, IndexUnavailable, Lenient, OfBuild, ReferenceHit, SearchCanceled, SurfaceDeclaration,
  SurfaceSite, SurfaceUse, SymbolHit, SymbolIndex,
};
use crate::fixture_test::{RENAMED_STORE_ANSWER, commit_all, renamed_store, repo_path, unhurried, write};
use crate::model::{DiffContent, DiffSource};
use diff_marks::index::{Access, MovesAnswer, RenameAnswer, RenamePair, RenameUse, Resolved};
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use sync_utils::child_process::{ChildPriority, windowless_command_at};
use test_log::test;
use test_utils::env_vars::EnvVars;
use test_utils::fake_executable::{fake_executable, shell_path};
use text_diff::model::RenameProof;
use text_diff::model::WhitespacePolicy;

const ROOT: &str = "/work/repository";

/// A fake accio in `dir` that touches `started` and then runs `then`
fn fake_accio(dir: &Path, started: &Path, then: &str) -> AccioCli {
  AccioCli::at(fake_executable(dir, "accio", &format!(": > {}\n{then}", shell_path(started))))
}

/// Waits until accio touched `started`: the query runs
fn wait_for(started: &Path) {
  while !started.exists() {
    std::thread::sleep(Duration::from_millis(10));
  }
}

#[test]
fn waits_for_a_slow_index_as_long_as_it_runs() {
  let dir = tempfile::tempdir().unwrap();
  let (started, go) = (dir.path().join("started"), dir.path().join("go"));
  // accio answers only once the test lets it, as one on a loaded machine: no deadline cuts the query meanwhile
  let answer = r#"[{"hits":[{"file":"src/Base.kt","line":1,"kind":"function","name":"outsider"}]}]"#;
  let accio = fake_accio(
    dir.path(),
    &started,
    &format!("while [ ! -e {} ]; do sleep 0.05; done\nprintf '%s' '{answer}'", shell_path(&go)),
  );
  let canceled = || false;
  let search = accio.search(&canceled);
  let hits = std::thread::scope(|scope| {
    let query = scope.spawn(|| search.symbols(ROOT, "outsider", Some("HEAD")));
    wait_for(&started);
    assert!(!query.is_finished(), "the query answered before accio did");
    std::fs::write(&go, "").unwrap();
    query.join().unwrap()
  });
  assert_eq!(
    hits.unwrap(),
    Some(vec![SymbolHit {
      path: "src/Base.kt".to_string(),
      line: 1,
      column: None,
      end_column: None,
      kind: HitKind::Function,
      group: None,
      name: "outsider".to_string(),
      parent: None,
    }])
  );
}

/// A hit carries the declaration in the words a review shares when accio answers them; one from an accio older than
/// the field has none, and the review folds its kind itself instead of dropping the hit
#[test]
fn a_hit_carries_the_word_a_review_shares_when_accio_answers_one() {
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let answer = r#"[{"hits":[{"file":"src/Base.kt","line":1,"kind":"method","group":"function","name":"run"},{"file":"src/Old.kt","line":2,"kind":"method","name":"run"}]}]"#;
  let accio = fake_accio(dir.path(), &started, &format!("printf '%s' '{answer}'"));
  let canceled = || false;
  let hits = accio.search(&canceled).symbols(ROOT, "run", None).unwrap().unwrap();
  assert_eq!(
    hits.iter().map(|it| (it.kind, it.group.as_deref())).collect::<Vec<_>>(),
    [(HitKind::Method, Some("function")), (HitKind::Method, None)]
  );
}

#[test]
fn a_canceled_search_kills_its_query() {
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let accio = fake_accio(dir.path(), &started, "exec sleep 600");
  let flag = AtomicBool::new(false);
  let canceled = || flag.load(Ordering::SeqCst);
  let search = accio.search(&canceled);
  let (result, waited) = std::thread::scope(|scope| {
    let query = scope.spawn(|| {
      let result = search.references(ROOT, "helper", None);
      (result, Instant::now())
    });
    wait_for(&started);
    let canceled_at = Instant::now();
    flag.store(true, Ordering::SeqCst);
    let (result, answered_at) = query.join().unwrap();
    (result, answered_at - canceled_at)
  });
  let error = result.unwrap_err();
  assert!(error.is::<SearchCanceled>(), "{error:#}");
  // killed within the poll interval, not after the 600 s of its sleep
  assert!(waited < Duration::from_secs(60), "{waited:?}");
}

/// The queries of a plan read ahead of its window run below the app, at nice 19, which the processes accio starts
/// inherit; those of a search the user waits for run at the priority of the app, whatever the one the tests run at
#[cfg(unix)]
#[test]
fn a_search_in_the_background_runs_its_queries_at_the_lowest_priority() {
  let dir = tempfile::tempdir().unwrap();
  let niceness = dir.path().join("niceness");
  // its own niceness, then the one of the test that started it
  let body = format!(
    "ps -o nice= -p $$ > {path}\nps -o nice= -p $PPID >> {path}\nprintf '%s' '[{{\"hits\":[]}}]'",
    path = shell_path(&niceness)
  );
  let accio = AccioCli::at(fake_executable(dir.path(), "accio", &body));
  let canceled = || false;
  let read = |search: AccioSearch<'_>| -> (i32, i32) {
    assert_eq!(search.symbols(ROOT, "outsider", None).unwrap(), Some(Vec::new()));
    let text = std::fs::read_to_string(&niceness).unwrap();
    let values: Vec<i32> = text.split_whitespace().map(|value| value.parse().unwrap()).collect();
    (values[0], values[1])
  };
  let (normal, test) = read(accio.search(&canceled));
  assert_eq!(normal, test, "a search the user waits for runs as the app");
  assert_eq!(read(accio.search(&canceled).in_background()), (19, test));
}

/// Every query runs with `--no-build`; accio without a usable index answers `available: false`, which the lookup answers
/// as no index, its reason kept for the plan, never as a failure
#[test]
fn an_index_accio_cannot_use_answers_no_index_with_its_reason() {
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let unavailable = r#"[{"available":false,"note":"accio is building the index of this repository","target":"helper"}]"#;
  let accio = fake_accio(
    dir.path(),
    &started,
    &format!("[ \"$2\" = --no-build ] || {{ echo \"no --no-build: $*\" >&2; exit 2; }}\nprintf '%s' '{unavailable}'"),
  );
  let canceled = || false;
  let search = accio.search(&canceled);
  let lenient = Lenient::new(&search);
  assert_eq!(lenient.indexed_references(ROOT, "helper", Some("HEAD")).unwrap(), None);
  assert_eq!(lenient.unavailable().as_deref(), Some("accio is building the index of this repository"));
  assert_eq!(lenient.error(), None);
}

#[test]
fn an_index_format_mismatch_says_which_accio_version_to_run() {
  let dir = tempfile::tempdir().unwrap();
  let canceled = || false;
  for (note, version, action) in [
    ("archive /Users/develar/.cache/accio/index.shard has format 34, expected 32", "a newer", "update accio"),
    ("archive /Users/develar/.cache/accio/index.shard has format 30, expected 32", "an older", "run accio index"),
  ] {
    let unavailable = format!(r#"[{{"available":false,"note":"{note}","target":"helper"}}]"#);
    let executable = fake_executable(dir.path(), "accio", &format!("printf '%s' '{unavailable}'"));
    let accio = AccioCli::at(executable.clone());
    let search = accio.search(&canceled);
    let lenient = Lenient::new(&search);
    assert_eq!(lenient.indexed_references(ROOT, "helper", Some("HEAD")).unwrap(), None);
    let expected = format!("the index of this repository is from {version} accio than {}: {action}", executable.display());
    assert_eq!(lenient.unavailable().as_deref(), Some(expected.as_str()));
  }
}

/// The descriptors of one side are read in one process, `parse --descriptor --resolve` at the revision, the paths on its
/// standard input: one line per path in their order, a path the revision lacks `missing`; without a usable index a line
/// has no classes and its note says which accio to run, as a query's does
#[test]
fn the_descriptors_of_a_side_are_read_in_one_process() {
  let dir = tempfile::tempdir().unwrap();
  let (args, stdin) = (dir.path().join("args"), dir.path().join("stdin"));
  let service = r#"{"area":"app","interface":"a.Players","implementation":"a.PlayersImpl","test":null,"headless":null,"schemaKey":null,"overrides":false,"open":false,"preload":"FALSE","client":null,"os":null}"#;
  let lines = [
    format!(
      r#"{{"path":"gui/resources/intellij.gui.xml","root":"idea-plugin","id":null,"content":[],"services":[{service}],"listeners":[],"extensions":[],"actions":[],"later":1,"classes":[{{"role":"services.interface","name":"a.Players","resolved":"exact","path":"gui/src/Players.kt","line":3}}]}}"#
    ),
    r#"{"path":"gui/media/resources/intellij.gui.media.xml","missing":true}"#.to_string(),
    r#"{"path":"plugin/resources/META-INF/plugin.xml","root":"idea-plugin","classes":null,"note":"archive /x/index.shard has format 30, expected 32"}"#.to_string(),
  ];
  let executable = fake_executable(
    dir.path(),
    "accio",
    &format!(
      "printf '%s\\n' \"$@\" > {}\ncat > {}\nprintf '%s\\n' '{}'",
      shell_path(&args),
      shell_path(&stdin),
      lines.join("' '")
    ),
  );
  let accio = AccioCli::at(executable.clone());
  let canceled = || false;
  let search = accio.search(&canceled);
  let paths = [
    "gui/resources/intellij.gui.xml".to_string(),
    "gui/media/resources/intellij.gui.media.xml".to_string(),
    "plugin/resources/META-INF/plugin.xml".to_string(),
  ];
  let descriptors = search.descriptors(ROOT, &paths, "head").unwrap().unwrap();
  let asked = std::fs::read_to_string(&args).unwrap();
  assert_eq!(
    asked.lines().collect::<Vec<_>>(),
    [
      "parse",
      "--descriptor",
      "--resolve",
      "--no-build",
      "--project",
      ROOT,
      "--root",
      ROOT,
      "--ref",
      "head",
      "--files-from",
      "-"
    ]
  );
  assert_eq!(std::fs::read_to_string(&stdin).unwrap(), format!("{}\n", paths.join("\n")));
  assert_eq!(
    descriptors[0].services,
    [DescriptorService {
      area: "app".to_string(),
      interface: Some("a.Players".to_string()),
      implementation: Some("a.PlayersImpl".to_string()),
      preload: Some("FALSE".to_string()),
      ..DescriptorService::default()
    }]
  );
  assert_eq!(
    descriptors[0].classes.as_deref(),
    Some(
      [DescriptorClass {
        role: "services.interface".to_string(),
        name: "a.Players".to_string(),
        resolved: "exact".to_string(),
        count: None,
        path: Some("gui/src/Players.kt".to_string()),
        line: Some(3),
      }]
      .as_slice()
    )
  );
  assert!(descriptors[0].is_read());
  assert_eq!((descriptors[1].missing, descriptors[1].is_read()), (true, false));
  let expected = format!("the index of this repository is from an older accio than {}: run accio index", executable.display());
  assert_eq!((descriptors[2].classes.as_ref(), descriptors[2].note.as_deref()), (None, Some(expected.as_str())));
}

/// What enters the new modules of a side is asked in one process at its revision, `--module` each, within the budget of
/// the review: one answer per module in their order, each declaration with the files that use it and its uses (a
/// registration with its point and what it is registered for), and why an answer is partial
#[test]
fn the_entry_points_of_the_new_modules_of_a_side_are_read_in_one_process() {
  let dir = tempfile::tempdir().unwrap();
  let args = dir.path().join("args");
  let answer = r#"[{"module":"gui/media","checked":75,"complete":true,"quality":"structural","unresolved":0,"declarations":[{"file":"gui/media/src/PlayersImpl.kt","line":31,"kind":"class","name":"PlayersImpl","qualified":"a.media.PlayersImpl","total":1,"files":0,"unresolved":0,"used_by":[{"file":"gui/media/resources/media.xml","line":23}],"uses":[{"file":"gui/media/resources/media.xml","line":23,"label":"descriptor","role":"services.implementation","point":"com.intellij.applicationService","of":"a.gui.Players","resolved":"typed","test":false}]}]},{"module":"tools","checked":3,"complete":false,"quality":"structural","unresolved":2,"note":"the budget of 15s ran out: 4 declarations not asked","declarations":[]}]"#;
  let started = dir.path().join("started");
  let accio = fake_accio_with_versions(
    dir.path(),
    &started,
    ANSWERS_SURFACE,
    &format!("printf '%s\\n' \"$@\" > {}\nprintf '%s\\n' '{answer}'", shell_path(&args)),
  );
  let canceled = || false;
  let search = accio.search(&canceled);
  let modules = ["gui/media".to_string(), "tools".to_string()];
  let surfaces = search.surface(ROOT, &modules, "head").unwrap().unwrap();
  // the arguments the fake accio wrote, one a line
  let arguments: Vec<String> = std::fs::read_to_string(&args).unwrap().lines().map(str::to_string).collect();
  let mut expected = vec!["surface", "--no-build", "--project", ROOT, "--ref", "head", "--budget", "15", "--format", "json"];
  expected.extend(modules.iter().flat_map(|module| ["--module", module.as_str()]));
  assert_eq!(arguments, expected);
  assert_eq!(
    surfaces[0].declarations,
    [SurfaceDeclaration {
      file: "gui/media/src/PlayersImpl.kt".to_string(),
      line: 31,
      kind: "class".to_string(),
      name: "PlayersImpl".to_string(),
      qualified: Some("a.media.PlayersImpl".to_string()),
      used_by: vec![SurfaceSite {
        file: "gui/media/resources/media.xml".to_string(),
        line: 23,
        test: false,
      }],
      uses: vec![SurfaceUse {
        file: "gui/media/resources/media.xml".to_string(),
        line: 23,
        label: "descriptor".to_string(),
        point: Some("com.intellij.applicationService".to_string()),
        of: Some("a.gui.Players".to_string()),
        resolved: Some("typed".to_string()),
        test: false,
      }],
    }]
  );
  assert_eq!(
    surfaces.iter().map(|it| (it.module.as_str(), it.complete, it.note.as_deref())).collect::<Vec<_>>(),
    [("gui/media", true, None), ("tools", false, Some("the budget of 15s ran out: 4 declarations not asked"))]
  );
}

/// The places of a plan are asked in one process, `--at` each, at the revision of the answer: one entry per place in
/// their order, each hit with what ties it to the declaration, and a call with its last line, its arguments, the
/// parameter each binds and whether it fits
#[test]
fn the_places_of_a_plan_are_asked_in_one_process() {
  let dir = tempfile::tempdir().unwrap();
  let (started, args) = (dir.path().join("started"), dir.path().join("args"));
  let call = r#"{"file":"lib/Welcome.kt","line":54,"end_line":57,"column":3,"kind":"call","resolved":"typed","fits":true,"arguments":[{"name":"pane","parameter":"pane"},{"position":1,"parameter":"id"}]}"#;
  let answer = format!(
    r#"[{{"at":"c1:src/Policy.kt:3","complete":true,"hits":[{call},{{"file":"lib/a.py","line":2,"column":1,"kind":"call","resolved":"name"}}]}},{{"at":"c0^:src/Old.kt:5","complete":false,"hits":[]}}]"#
  );
  let accio = fake_accio(dir.path(), &started, &format!("printf '%s\\n' \"$@\" > {}\nprintf '%s' '{answer}'", shell_path(&args)));
  let canceled = || false;
  let search = accio.search(&canceled);
  let places = [
    DeclarationAt {
      revision: Some("c1".to_string()),
      path: "src/Policy.kt".to_string(),
      line: 3,
      column: None,
    },
    DeclarationAt {
      revision: Some("c0^".to_string()),
      path: "src/Old.kt".to_string(),
      line: 5,
      column: None,
    },
  ];
  let answers = search.references_at(ROOT, &places, "head").unwrap().unwrap();
  let asked = std::fs::read_to_string(&args).unwrap();
  let asked: Vec<&str> = asked.lines().collect();
  assert_eq!(
    asked,
    [
      "references",
      "--no-build",
      "--format",
      "json",
      "--project",
      ROOT,
      "--limit",
      "100",
      "--ref",
      "head",
      "--at",
      "c1:src/Policy.kt:3",
      "--at",
      "c0^:src/Old.kt:5"
    ]
  );
  let hits: Vec<(&str, Resolved, Option<u32>, Option<bool>)> = answers[0].hits.iter().map(|hit| (hit.path.as_str(), hit.resolved, hit.end_line, hit.fits)).collect();
  assert_eq!(hits, [("lib/Welcome.kt", Resolved::Typed, Some(57), Some(true)), ("lib/a.py", Resolved::Name, None, None)]);
  assert_eq!(
    answers[0].hits[0].arguments.as_deref().unwrap()[1],
    CallArgument {
      position: Some(1),
      parameter: Some("id".to_string()),
      ..CallArgument::default()
    }
  );
  assert_eq!((answers[1].hits.len(), answers[1].complete), (0, false));
}

/// accio lists a line once with each usage on it as a mark; an older accio lists each usage. Both answer the same
/// usages, in the answer's order.
#[test]
fn a_line_of_marks_answers_a_usage_for_each_mark() {
  let lines = r#"{"hits":[
    {"file":"lib/Use.kt","line":2,"snippet":"twice(twice(1))","marks":[
      {"column":0,"kind":"call","resolved":"typed","end_line":2,"arguments":[{"position":0}],"fits":true},
      {"column":6,"kind":"call","resolved":"name"}]},
    {"file":"lib/Use.kt","line":5,"snippet":"import a.twice","marks":[{"column":9,"kind":"import","resolved":"import"}]}],
    "total":2,"total_marks":3,"by_name":0,"complete":true}"#;
  let flat = r#"{"hits":[
    {"file":"lib/Use.kt","line":2,"column":0,"snippet":"twice(twice(1))","kind":"call","resolved":"typed","end_line":2,"arguments":[{"position":0}],"fits":true},
    {"file":"lib/Use.kt","line":2,"column":6,"snippet":"twice(twice(1))","kind":"call","resolved":"name"},
    {"file":"lib/Use.kt","line":5,"column":9,"snippet":"import a.twice","kind":"import","resolved":"import"}],
    "total":3,"by_name":0,"complete":true}"#;
  let by_line: crate::accio::References = serde_json::from_str(lines).unwrap();
  let by_usage: crate::accio::References = serde_json::from_str(flat).unwrap();
  assert_eq!(by_line, by_usage);
  let places: Vec<(u32, u32, Resolved)> = by_line.hits.iter().map(|hit| (hit.line, hit.column, hit.resolved)).collect();
  assert_eq!(places, [(2, 0, Resolved::Typed), (2, 6, Resolved::Name), (5, 9, Resolved::Import)]);
  assert_eq!((by_line.hits[0].fits, by_line.hits[0].arguments.as_ref().map(Vec::len)), (Some(true), Some(1)));
}

#[test]
fn resolved_usages_are_asked_at_a_name_column_and_count_name_only_hits() {
  let dir = tempfile::tempdir().unwrap();
  let (started, args) = (dir.path().join("started"), dir.path().join("args"));
  let answer = r#"[{"hits":[{"file":"lib/Use.kt","line":2,"column":3,"kind":"call","resolved":"typed"}],"by_name":1,"complete":true}]"#;
  let accio = fake_accio(dir.path(), &started, &format!("printf '%s\\n' \"$@\" > {}\nprintf '%s' '{answer}'", shell_path(&args)));
  let canceled = || false;
  let search = accio.search(&canceled);
  let declarations = [DeclarationAt {
    revision: None,
    path: "src/Store.kt".to_string(),
    line: 4,
    column: Some(7),
  }];

  let answers = search.references_at_resolved(ROOT, &declarations, Some("head")).unwrap().unwrap();
  assert_eq!(answers[0].by_name, 1);
  assert_eq!(answers[0].hits[0].resolved, Resolved::Typed);
  let asked: Vec<String> = std::fs::read_to_string(&args).unwrap().lines().map(str::to_string).collect();
  assert_eq!(
    asked,
    [
      "references",
      "--no-build",
      "--format",
      "json",
      "--project",
      ROOT,
      "--limit",
      "100",
      "--resolved",
      "--ref",
      "head",
      "--at",
      "src/Store.kt:4:7",
    ]
  );
}

/// The names of one lookup (`Owner.name`, then the bare name) go in one process, which answers an entry per name in
/// their order
#[test]
fn every_name_of_a_lookup_is_asked_in_one_process() {
  let dir = tempfile::tempdir().unwrap();
  let (started, args, runs) = (dir.path().join("started"), dir.path().join("args"), dir.path().join("runs"));
  let answer = r#"[{"hits":[]},{"hits":[{"file":"src/Base.kt","line":1,"column":4,"kind":"function","name":"outsider"}]}]"#;
  let accio = fake_accio(
    dir.path(),
    &started,
    &format!(
      "printf 'run\\n' >> {runs}\nprintf '%s\\n' \"$@\" > {args}\nprintf '%s' '{answer}'",
      runs = shell_path(&runs),
      args = shell_path(&args)
    ),
  );
  let canceled = || false;
  let search = accio.search(&canceled);

  let answers = search.symbols_any(ROOT, &["Store.outsider", "outsider"], Some("head")).unwrap().unwrap();
  assert_eq!(answers.len(), 2);
  assert_eq!(answers[0].len(), 0, "no declaration of the owner's name");
  assert_eq!(answers[1][0].column, Some(4));
  assert_eq!(std::fs::read_to_string(&runs).unwrap().lines().count(), 1, "one accio per lookup");
  let asked: Vec<String> = std::fs::read_to_string(&args).unwrap().lines().map(str::to_string).collect();
  assert_eq!(
    asked,
    [
      "symbol",
      "--no-build",
      "--format",
      "json",
      "--project",
      ROOT,
      "--limit",
      "20",
      "--ref",
      "head",
      "--",
      "Store.outsider",
      "outsider"
    ]
  );
}

#[test]
fn unknown_accio_kinds_deserialize_to_unknown() {
  let symbol: SymbolHit = serde_json::from_str(r#"{"file":"src/A.kt","line":2,"kind":"supertype","name":"A"}"#).unwrap();
  let reference: ReferenceHit = serde_json::from_str(r#"{"file":"src/A.kt","line":2,"column":3,"snippet":"a","kind":"config","resolved":"name"}"#).unwrap();

  assert_eq!(symbol.kind, HitKind::Unknown);
  assert_eq!(reference.kind, Access::Unknown);
}

/// A fake accio whose `versions` prints `versions`, and that runs `then` for every other command after it touched
/// `started`
fn fake_accio_with_versions(dir: &Path, started: &Path, versions: &str, then: &str) -> AccioCli {
  let body = format!("if [ \"$1\" = versions ]; then printf '%s' '{versions}'; exit 0; fi\n: > {}\n{then}", shell_path(started));
  AccioCli::at(fake_executable(dir, "accio", &body))
}

const ANSWERS_RENAMES: &str = r#"{"answers":{"moves":1,"renames":1},"index_format":40}"#;

const ANSWERS_SURFACE: &str = r#"{"answers":{"moves":1,"renames":1,"surface":2},"index_format":40}"#;

/// Only an accio that says it answers `surface` as this reader reads it (`SURFACE_ANSWER`) is asked: one without the
/// answer, one of the revision before `used_by` said which file is a test's and one whose `versions` fails answer none,
/// and their `surface` is never started. `renames` and `surface` read one answer of `versions`
#[test]
fn an_accio_that_answers_no_surface_as_it_is_read_is_not_asked() {
  let canceled = || false;
  let modules = ["gui/media".to_string()];
  for versions in [r#"{"index_format":39}"#, ANSWERS_RENAMES, r#"{"answers":{"renames":1,"surface":1}}"#, "not json"] {
    let dir = tempfile::tempdir().unwrap();
    let started = dir.path().join("started");
    let accio = fake_accio_with_versions(dir.path(), &started, versions, "exit 2");
    assert!(!accio.answers_surface(), "{versions}");
    assert_eq!(accio.search(&canceled).surface(ROOT, &modules, "head").unwrap(), None, "{versions}");
    assert!(!started.exists(), "{versions}");
  }
  let dir = tempfile::tempdir().unwrap();
  let asked = dir.path().join("asked");
  let body = format!(
    "if [ \"$1\" = versions ]; then echo asked >> {}; printf '%s' '{ANSWERS_SURFACE}'; exit 0; fi\nexit 2",
    shell_path(&asked)
  );
  let accio = AccioCli::at(fake_executable(dir.path(), "accio", &body));
  assert!(accio.answers_surface() && accio.answers_renames());
  assert_eq!(std::fs::read_to_string(&asked).unwrap().lines().count(), 1);
}

fn store_pair() -> RenamePair {
  RenamePair {
    old: "src/Store.kt:6:5".to_string(),
    new: "src/Store.kt:6:5".to_string(),
  }
}

/// The declarations a subject renamed are asked in one process, `renames --no-build` between its two trees, the
/// pairs of their places on its standard input: one answer per pair in their order, a pair accio could not ask
/// `unavailable` beside the others
#[test]
fn the_renames_of_a_subject_are_asked_in_one_process() {
  let dir = tempfile::tempdir().unwrap();
  let (started, args, stdin) = (dir.path().join("started"), dir.path().join("args"), dir.path().join("stdin"));
  let answered: serde_json::Value = serde_json::from_str(RENAMED_STORE_ANSWER).unwrap();
  let unasked = r#"{"old_at":"src/Body.kt:2","new_at":"src/Body.kt:2","unavailable":true,"reason":"base: no declaration","same":false,"complete":false,"cut":null,"final":true}"#;
  let answer = format!("[{},{unasked}]", answered[0]);
  let accio = fake_accio_with_versions(
    dir.path(),
    &started,
    ANSWERS_RENAMES,
    &format!("printf '%s\\n' \"$@\" > {}\ncat > {}\nprintf '%s' '{answer}'", shell_path(&args), shell_path(&stdin)),
  );
  let canceled = || false;
  let pairs = [
    store_pair(),
    RenamePair {
      old: "src/Body.kt:2:1".to_string(),
      new: "src/Body.kt:2:1".to_string(),
    },
  ];
  let answers = accio.search(&canceled).renames(ROOT, &pairs, "base~1", "head").unwrap().unwrap();
  assert_eq!(
    std::fs::read_to_string(&args).unwrap().lines().collect::<Vec<_>>(),
    [
      "renames",
      "--no-build",
      "--format",
      "json",
      "--project",
      ROOT,
      "--base",
      "base~1",
      "--ref",
      "head",
      "--pairs-from",
      "-"
    ]
  );
  assert_eq!(
    std::fs::read_to_string(&stdin).unwrap(),
    "src/Store.kt:6:5\tsrc/Store.kt:6:5\nsrc/Body.kt:2:1\tsrc/Body.kt:2:1\n"
  );

  let renamed = &answers[0];
  assert!(renamed.same && renamed.complete && !renamed.unavailable && renamed.is_final());
  assert_eq!(renamed.cut, None);
  assert_eq!(renamed.conflicts, []);
  assert_eq!(renamed.old.as_ref().map(|it| it.language.as_str()), Some("kotlin"));
  let (base, head) = (renamed.base.as_ref().unwrap(), renamed.head.as_ref().unwrap());
  assert_eq!((base.total, head.total), (3, 3));
  assert_eq!(
    base.uses[0],
    RenameUse {
      path: "src/Report.kt".to_string(),
      line: 4,
      column: 14,
      end_column: 23,
      kind: Access::Call,
      resolved: Resolved::Import,
      text: None,
    }
  );
  assert_eq!((head.uses[2].kind, head.uses[2].end_column), (Access::Declaration, 16));
  assert_eq!(
    answers[1],
    RenameAnswer {
      unavailable: true,
      is_final: true,
      ..RenameAnswer::default()
    }
  );

  // no pair, no process
  std::fs::remove_file(&started).unwrap();
  assert_eq!(accio.search(&canceled).renames(ROOT, &[], "base~1", "head").unwrap(), Some(Vec::new()));
  assert!(!started.exists());
}

/// Only an accio that says it answers `renames` as this reader reads them is asked: one without the answer (an older
/// accio, whose `versions` names none), one of another revision of it and one whose `versions` fails answer none,
/// and their `renames` is never started. The answer of `versions` is asked once of a build of accio
#[test]
fn an_accio_that_answers_no_renames_as_they_are_read_is_not_asked() {
  let canceled = || false;
  for versions in [r#"{"index_format":39}"#, r#"{"answers":{"moves":1}}"#, r#"{"answers":{"moves":1,"renames":2}}"#, "not json"] {
    let dir = tempfile::tempdir().unwrap();
    let started = dir.path().join("started");
    let accio = fake_accio_with_versions(dir.path(), &started, versions, "exit 2");
    assert!(!accio.answers_renames(), "{versions}");
    assert_eq!(accio.search(&canceled).renames(ROOT, &[store_pair()], "base", "head").unwrap(), None, "{versions}");
    assert!(!started.exists(), "{versions}");
  }
  // an accio whose `versions` fails
  let dir = tempfile::tempdir().unwrap();
  let accio = AccioCli::at(fake_executable(dir.path(), "accio", "echo \"error: unrecognized subcommand\" >&2\nexit 2"));
  assert_eq!(accio.search(&canceled).renames(ROOT, &[store_pair()], "base", "head").unwrap(), None);

  // the one that does is asked its versions once, whatever the number of subjects
  let dir = tempfile::tempdir().unwrap();
  let (started, asked) = (dir.path().join("started"), dir.path().join("asked"));
  let body = format!(
    "if [ \"$1\" = versions ]; then echo asked >> {}; printf '%s' '{ANSWERS_RENAMES}'; exit 0; fi\n: > {}\nprintf '%s' '{}'",
    shell_path(&asked),
    shell_path(&started),
    RENAMED_STORE_ANSWER.replace('\n', " ")
  );
  let accio = AccioCli::at(fake_executable(dir.path(), "accio", &body));
  for _ in 0..3 {
    assert_eq!(
      accio.search(&canceled).renames(ROOT, &[store_pair()], "base", "head").unwrap().map(|answers| answers.len()),
      Some(1)
    );
  }
  assert_eq!(std::fs::read_to_string(&asked).unwrap().lines().count(), 1);
}

/// Only an accio that says it answers `moves` of the revision this reader reads (`MOVES_ANSWER`) is asked them: one
/// without the answer, of the first revision (no `reordered`) or of a later one answers none, its `moves` never
/// started. The one that does is asked `accio moves --no-build --base --ref`, and its answer read; one without an index of
/// the repository answers no index
#[test]
fn an_accio_that_answers_no_moves_as_they_are_read_is_not_asked() {
  let canceled = || false;
  for versions in [
    r#"{"answers":{"renames":1}}"#,
    r#"{"answers":{"moves":1,"renames":1}}"#,
    r#"{"answers":{"moves":3}}"#,
    "not json",
  ] {
    let dir = tempfile::tempdir().unwrap();
    let started = dir.path().join("started");
    let accio = fake_accio_with_versions(dir.path(), &started, versions, "exit 2");
    assert!(!accio.answers_moves(), "{versions}");
    assert_eq!(accio.search(&canceled).moves(ROOT, "base", "head").unwrap(), None, "{versions}");
    assert!(!started.exists(), "{versions}");
  }
  let dir = tempfile::tempdir().unwrap();
  let (started, args) = (dir.path().join("started"), dir.path().join("args"));
  let answer = r#"{"moved":[{"old":{"file":"src/Ledger.kt","line":5,"end_line":8,"name":"reportTitle"},"new":{"file":"src/Report.kt","line":5,"end_line":11,"name":"reportTitle"},"file":true,"owner":false}],"reordered":[],"total":1,"complete":true}"#;
  let accio = fake_accio_with_versions(
    dir.path(),
    &started,
    r#"{"answers":{"moves":2}}"#,
    &format!("echo \"$@\" > {}\nprintf '%s' '{answer}'", shell_path(&args)),
  );
  let moves = accio.search(&canceled).moves(ROOT, "base", "head").unwrap().unwrap();
  assert_eq!((moves.moved.len(), moves.moved[0].new.end_line, moves.complete), (1, 11, true));
  assert_eq!(
    std::fs::read_to_string(&args).unwrap().trim(),
    format!("moves --no-build --format json --project {ROOT} --base base --ref head --limit 500")
  );
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let accio = fake_accio_with_versions(
    dir.path(),
    &started,
    r#"{"answers":{"moves":2}}"#,
    r#"printf '%s' '{"available":false,"note":"no index of this repository"}'"#,
  );
  let error = accio.search(&canceled).moves(ROOT, "base", "head").unwrap_err();
  assert!(error.is::<IndexUnavailable>(), "{error:#}");
}

/// What an executable says of itself is asked once of each build of it: again once its file is another, and every
/// time while there is no file to tell a build by
#[test]
fn what_an_executable_says_of_itself_is_asked_once_of_each_build() {
  let dir = tempfile::tempdir().unwrap();
  let executable = dir.path().join("tool");
  let said: OfBuild<u32> = OfBuild::new();
  let asked = std::cell::Cell::new(0);
  let ask = || {
    asked.set(asked.get() + 1);
    asked.get()
  };
  // no file yet: nothing tells one build from another
  assert_eq!((said.of(&executable, ask), said.of(&executable, ask)), (1, 2));
  std::fs::write(&executable, "a build").unwrap();
  assert_eq!((said.of(&executable, ask), said.of(&executable, ask), asked.get()), (3, 3, 3));
  // another build in its place, of another size
  std::fs::write(&executable, "another build").unwrap();
  assert_eq!((said.of(&executable, ask), said.of(&executable, ask), asked.get()), (4, 4, 4));
}

/// The app outlives an update of accio: what the build before said of itself is not read as the word of the new
/// one. An accio that got the answer is asked from then on, and one whose answer is of another revision now is
/// asked no more, though its `renames` would answer. On Unix, where the fake accio is the script that is rewritten:
/// on Windows it is a shim beside its script, which an update of the script leaves as it is
#[cfg(unix)]
#[test]
fn an_accio_updated_while_the_app_runs_is_asked_again_what_it_answers() {
  let canceled = || false;
  let dir = tempfile::tempdir().unwrap();
  let (started, asked) = (dir.path().join("started"), dir.path().join("asked"));
  let install = |versions: &str| {
    let body = format!(
      "if [ \"$1\" = versions ]; then echo asked >> {}; printf '%s' '{versions}'; exit 0; fi\n: > {}\nprintf '%s' '{}'",
      shell_path(&asked),
      shell_path(&started),
      RENAMED_STORE_ANSWER.replace('\n', " ")
    );
    fake_executable(dir.path(), "accio", &body)
  };
  let renames = |accio: &AccioCli| accio.search(&canceled).renames(ROOT, &[store_pair()], "base", "head").unwrap().map(|answers| answers.len());
  let versions_asked = || std::fs::read_to_string(&asked).unwrap().lines().count();

  // an accio without the answer, as the app found it
  let accio = AccioCli::at(install(r#"{"index_format":39}"#));
  assert_eq!((renames(&accio), renames(&accio)), (None, None));
  assert_eq!((versions_asked(), started.exists()), (1, false));
  // updated to one that answers: asked once more, and from then on it is asked the renames
  install(ANSWERS_RENAMES);
  assert_eq!((renames(&accio), renames(&accio)), (Some(1), Some(1)));
  assert_eq!(versions_asked(), 2);
  // updated to one whose answer is of another revision: its answer would be read as the one it is not
  std::fs::remove_file(&started).unwrap();
  install(r#"{"answers":{"moves":1,"renames":2},"index_format":41}"#);
  assert_eq!(renames(&accio), None);
  assert_eq!((versions_asked(), started.exists()), (3, false));
}

/// Without a usable index of the repository accio answers `available: false` for each pair: no index, never "no use",
/// and never a failure of the diffs. A query that fails, and one that answers another number of pairs, is an error
/// its caller reads as no answer; a canceled one is killed
#[test]
fn renames_without_an_index_answer_no_index_and_a_canceled_query_is_killed() {
  let dir = tempfile::tempdir().unwrap();
  let started = dir.path().join("started");
  let canceled = || false;
  let unavailable = r#"[{"available":false,"note":"no index of this repository","old_at":"src/Store.kt:6:5","new_at":"src/Store.kt:6:5"}]"#;
  let accio = fake_accio_with_versions(dir.path(), &started, ANSWERS_RENAMES, &format!("printf '%s' '{unavailable}'"));
  let error = accio.search(&canceled).renames(ROOT, &[store_pair()], "base", "head").unwrap_err();
  assert_eq!(
    error.downcast_ref::<IndexUnavailable>().map(ToString::to_string).as_deref(),
    Some("no index of this repository")
  );

  let accio = fake_accio_with_versions(dir.path(), &started, ANSWERS_RENAMES, "echo \"fatal: bad revision\" >&2\nexit 1");
  let error = accio.search(&canceled).renames(ROOT, &[store_pair()], "base", "head").unwrap_err();
  assert!(!error.is::<IndexUnavailable>() && !error.is::<SearchCanceled>(), "{error:#}");
  let accio = fake_accio_with_versions(dir.path(), &started, ANSWERS_RENAMES, "printf '%s' '[]'");
  accio.search(&canceled).renames(ROOT, &[store_pair()], "base", "head").unwrap_err();

  std::fs::remove_file(&started).unwrap();
  let accio = fake_accio_with_versions(dir.path(), &started, ANSWERS_RENAMES, "exec sleep 600");
  let flag = AtomicBool::new(false);
  let canceled = || flag.load(Ordering::SeqCst);
  let search = accio.search(&canceled);
  let result = std::thread::scope(|scope| {
    let query = scope.spawn(|| search.renames(ROOT, &[store_pair()], "base", "head"));
    wait_for(&started);
    flag.store(true, Ordering::SeqCst);
    query.join().unwrap()
  });
  assert!(result.unwrap_err().is::<SearchCanceled>());
}

/// The accio of this machine, when it answers `renames` as they are read, asked of a real repository through the
/// diffs of a commit: an index of its own (`ACCIO_HOME`), built once, then the query as the app runs it. The rename
/// of a function with its two calls in another file is proven: the calls are confirmed runs, the declaration is
/// read. The answer is final (`final`): the subject read again through the same diffs starts no accio. A machine
/// without such an accio proves nothing here
#[test]
fn the_accio_of_this_machine_proves_the_rename_of_a_real_repository() {
  let (repo, second) = renamed_store();
  let asked = read_twice_by_accio(&repo, second, AccioCli::answers_renames, |subject, read| {
    let report = subject("src/Report.kt");
    assert_eq!((report.used[0].proof, &report.confirmed), (RenameProof::Confirmed, &vec![0, 1]), "read {read}");
    assert_eq!(subject("src/Store.kt").declared[0].proof, RenameProof::Confirmed, "read {read}");
  });
  // the answer is final: memory keeps it, and the subject read again started no accio
  assert!(asked.is_none_or(|(renames, _)| renames == 1), "{asked:?}");
}

/// Reads the diffs of the commit `id` of `repo` twice through one `FileDiffs`, with the accio of this machine and an
/// index of the repository built for it, and hands what the subject says of each file to `check` each time: how
/// often `renames` and `moves` were asked then. `None`, and nothing read, where the machine has no accio that `answers`
fn read_twice_by_accio(
  repo: &test_utils::git_test_utils::TestRepo,
  id: String,
  answers: fn(&AccioCli) -> bool,
  check: impl Fn(&dyn Fn(&str) -> text_diff::model::SubjectMarks, u32),
) -> Option<(usize, usize)> {
  let Some((accio, _env, _home)) = indexed_by_accio(repo, answers) else {
    eprintln!("no accio that answers what this test asks on this machine: nothing asked");
    return None;
  };
  let canceled = || false;
  let search = accio.search(&canceled);
  let counted = CountedIndex::of(&search);
  let source = DiffSource::Commit { id };
  let diffs = unhurried();
  for read in 0..2 {
    check(&subjects_read(&diffs, repo, &counted, &source), read);
  }
  Some((counted.asked.load(Ordering::SeqCst), counted.moves.load(Ordering::SeqCst)))
}

/// The diffs of `source` read by `diffs` with `index` as a window asks it: what the subject says of a file, by its path
fn subjects_read(
  diffs: &crate::service::FileDiffs,
  repo: &test_utils::git_test_utils::TestRepo,
  index: &dyn SymbolIndex,
  source: &DiffSource,
) -> impl Fn(&str) -> text_diff::model::SubjectMarks {
  let files = diffs
    .read_checked(
      repo.git_executor(),
      repo_path(repo),
      &ObjectCache::disabled(),
      Some(index),
      source,
      WhitespacePolicy::Default,
    )
    .unwrap();
  move |path: &str| {
    let file = files.iter().find(|file| file.path == path).unwrap();
    let DiffContent::Text(diff) = &file.content else { panic!("{path}: {:?}", file.content) };
    *diff.marks.subject.clone().unwrap_or_else(|| panic!("the subject says nothing of {path}"))
  }
}

/// An index of the accio of this machine with a repository of its own: its `ACCIO_HOME` (kept with the returned
/// guard and directory) and an index of `repo` built in it. `None`, and nothing asked, where the machine has no accio
/// that `answers` as this reader reads it
fn indexed_by_accio(repo: &test_utils::git_test_utils::TestRepo, answers: fn(&AccioCli) -> bool) -> Option<(&'static AccioCli, EnvVars, tempfile::TempDir)> {
  let accio = AccioCli::discover().filter(|accio| answers(accio))?;
  let home = tempfile::tempdir().unwrap();
  let env = EnvVars::set(&[("ACCIO_HOME", home.path().as_os_str()), ("ACCIO_NO_LIBS", std::ffi::OsStr::new("1"))]);
  let indexed = windowless_command_at(accio.executable(), ChildPriority::Normal)
    .args(["--project", repo_path(repo), "index", "--quiet"])
    .output()
    .unwrap();
  assert!(indexed.status.success(), "{}", String::from_utf8_lossy(&indexed.stderr));
  Some((accio, env, home))
}

/// The accio of this machine says which declarations a change moved whole (`accio moves`), and the diffs of the
/// commit mark them: a function moved from the ledger to the report with its parameters one a line is one move of
/// its four old lines and its seven new ones, at both ends. Its answer is kept by the two trees: the subject read
/// again starts no accio. A machine without such an accio proves nothing here
#[test]
fn the_accio_of_this_machine_says_a_declaration_moved_whole_in_another_layout() {
  const LEDGER: &str = "package demo\n\nclass Ledger(val accounts: List<String>)\n";
  const TITLE: &str = "\nfun reportTitle(ledger: Ledger, account: String): String {\n  val size = ledger.accounts.size\n  return \"$account: $size\"\n}\n";
  const WRAPPED: &str = "\nfun reportTitle(\n  ledger: Ledger,\n  account: String\n): String {\n  val size = ledger.accounts.size\n  return \"$account: $size\"\n}\n";
  const REPORT: &str = "package demo\n\nfun report(ledger: Ledger): String = ledger.accounts.joinToString()\n";
  let repo = test_utils::git_test_utils::TestRepo::new();
  std::fs::create_dir_all(repo.path().join("src")).unwrap();
  write(&repo, "src/Ledger.kt", format!("{LEDGER}{TITLE}").as_bytes());
  write(&repo, "src/Report.kt", REPORT.as_bytes());
  commit_all(&repo, "Initial");
  write(&repo, "src/Ledger.kt", LEDGER.as_bytes());
  write(&repo, "src/Report.kt", format!("{REPORT}{WRAPPED}").as_bytes());
  let second = commit_all(&repo, "Move the title to the report");
  let asked = read_twice_by_accio(&repo, second, AccioCli::answers_moves, |subject, read| {
    let end = |moved: &text_diff::model::FileMove| (moved.start, moved.len, moved.file, moved.line, moved.declaration.clone().map(|it| (it.name, it.other_len)));
    let with_len = |len: u32| Some(("reportTitle".to_string(), len));
    assert_eq!(
      subject("src/Ledger.kt").moved_out.iter().map(end).collect::<Vec<_>>(),
      [(4, 4, 1, 4, with_len(7))],
      "read {read}"
    );
    assert_eq!(
      subject("src/Report.kt").moved_in.iter().map(end).collect::<Vec<_>>(),
      [(4, 7, 0, 4, with_len(4))],
      "read {read}"
    );
  });
  // kept by the two trees: the subject read again started no accio
  assert!(asked.is_none_or(|(_, moves)| moves == 1), "{asked:?}");
}

/// The accio of this machine says which declarations a change put in another order among those of their owner and
/// whether their order is free (`accio moves`, `reordered`), and the diffs of the commit mark the move of the pair that
/// holds one of free order: a function of a class moved below another as it is, and not an enum entry moved below
/// another, whose place is its ordinal. A machine without such an accio proves nothing here
#[test]
fn the_accio_of_this_machine_says_a_function_moved_below_another_in_its_class_is_a_reorder_of_free_order() {
  const ENTRIES: &str = "package demo\n\nclass Ledger(val entries: List<Long>) {\n";
  const TOTAL: &str = "  fun total(): Long {\n    return entries.sum()\n  }\n";
  const COUNT: &str = "  fun count(): Int {\n    return entries.size\n  }\n";
  const LEVEL: &str = "}\n\nenum class Level(val weight: Int) {\n";
  let ledger = |functions: [&str; 2], entries: [&str; 3]| format!("{ENTRIES}{}\n{}{LEVEL}{}}}\n", functions[0], functions[1], entries.concat());
  let entries = ["  LOW(computeLow()),\n", "  HIGH(computeHigh()),\n", "  HIGHER(computeHigher()),\n"];
  let repo = test_utils::git_test_utils::TestRepo::new();
  std::fs::create_dir_all(repo.path().join("src")).unwrap();
  write(&repo, "src/Ledger.kt", ledger([TOTAL, COUNT], entries).as_bytes());
  commit_all(&repo, "Initial");
  write(&repo, "src/Ledger.kt", ledger([COUNT, TOTAL], [entries[1], entries[2], entries[0]]).as_bytes());
  let second = commit_all(&repo, "Count before the total, the lowest level last");
  read_twice_by_accio(&repo, second, AccioCli::answers_moves, |subject, read| {
    let names: Vec<Vec<String>> = subject("src/Ledger.kt").reordered.into_iter().map(|reorder| reorder.names).collect();
    assert_eq!(names, [vec!["total".to_string()]], "read {read}");
  });
}

/// An index that counts the questions of `renames` and of `moves` it passes on to `index`
struct CountedIndex<'a> {
  index: &'a dyn SymbolIndex,
  asked: std::sync::atomic::AtomicUsize,
  moves: std::sync::atomic::AtomicUsize,
}

impl<'a> CountedIndex<'a> {
  fn of(index: &'a dyn SymbolIndex) -> Self {
    Self {
      index,
      asked: std::sync::atomic::AtomicUsize::new(0),
      moves: std::sync::atomic::AtomicUsize::new(0),
    }
  }
}

impl SymbolIndex for CountedIndex<'_> {
  fn moves(&self, repository_root: &str, base: &str, head: &str) -> anyhow::Result<Option<MovesAnswer>> {
    self.moves.fetch_add(1, Ordering::SeqCst);
    self.index.moves(repository_root, base, head)
  }

  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    self.index.symbols(repository_root, query, revision)
  }

  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> anyhow::Result<Option<crate::accio::References>> {
    self.index.references(repository_root, name, revision)
  }

  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<Option<Vec<crate::accio::References>>> {
    self.index.references_at(repository_root, declarations, revision)
  }

  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> anyhow::Result<Option<Vec<crate::accio::TestReach>>> {
    self.index.tests(repository_root, declarations, given, revision)
  }

  fn renames(&self, repository_root: &str, pairs: &[RenamePair], base: &str, head: &str) -> anyhow::Result<Option<Vec<RenameAnswer>>> {
    self.asked.fetch_add(1, Ordering::SeqCst);
    self.index.renames(repository_root, pairs, base, head)
  }
}
