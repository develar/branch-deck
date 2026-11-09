//! The accio process of a lookup: waited for as long as it runs, killed once the navigation is canceled, below the app
//! for a plan read ahead of its window

#[cfg(unix)]
use crate::accio::AccioSearch;
use crate::accio::{
  Access, AccioCli, CallArgument, DeclarationAt, DescriptorClass, DescriptorService, HitKind, Lenient, ReferenceHit, Resolved, SearchCanceled, SurfaceDeclaration, SurfaceSite,
  SurfaceUse, SymbolHit, SymbolIndex,
};
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use test_log::test;
use test_utils::fake_executable::{fake_executable, shell_path};

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
  let executable = fake_executable(dir.path(), "accio", &format!("printf '%s\\n' \"$@\" > {}\nprintf '%s\\n' '{answer}'", shell_path(&args)));
  let accio = AccioCli::at(executable);
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
        line: 23
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
