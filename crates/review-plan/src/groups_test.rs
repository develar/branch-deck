use crate::ReviewPlan;
use crate::entries::{EntryPoint, EntryRegistration, EntrySite, ModuleEntries};
use crate::fan_in_test::Counts;
use crate::groups::GroupKind;
use crate::steps::StepTests;
use crate::test_support::{plan_of, plan_of_range, range};
use file_diffs::accio::{FixtureIndex, ReachingTest, TestClass, TestReach};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A group as the tests see it: (kind, label of its module or its folder, the labels it also holds, its files, its tests)
type Group<'a> = (GroupKind, String, Vec<&'a str>, Vec<&'a str>, Vec<&'a str>);

/// The groups of each step
fn groups(plan: &ReviewPlan) -> Vec<Vec<Group<'_>>> {
  plan
    .steps
    .steps
    .iter()
    .map(|step| {
      step
        .groups
        .iter()
        .map(|group| {
          let name = |module: u32| plan.modules[module as usize].label.as_str();
          let files = |range: std::ops::Range<u32>| {
            let mut found: Vec<&str> = Vec::new();
            for stop in &step.stops[range.start as usize..range.end as usize] {
              let file = stop.path.rsplit('/').next().unwrap_or(&stop.path);
              if !found.contains(&file) {
                found.push(file);
              }
            }
            found
          };
          let label = group.folder.clone().or_else(|| group.module.map(|module| name(module).to_string())).unwrap_or_default();
          (
            group.kind,
            label,
            group.also.iter().map(|&module| name(module)).collect(),
            files(group.start..group.tests),
            files(group.tests..group.end),
          )
        })
        .collect()
    })
    .collect()
}

/// A first commit with a body before the change: a range of two commits that tell their steps, whose steps are the
/// commits (a range of one commit has its concerns, which are never cut)
fn told_first(repo: &TestRepo) {
  repo.create_commit_with_files("Name the card\n\nThe card of a file names it.", &[("gui/src/a/Card.kt", "package a\n\nclass Card\n")]);
}

/// `count` files of a Kotlin package in `dir`, each a class that uses `uses` when given
fn classes(dir: &str, package: &str, prefix: &str, count: usize, uses: Option<(&str, &str)>) -> Vec<(String, String)> {
  (0..count)
    .map(|index| {
      let text = match uses {
        Some((import, name)) => format!("package {package}\n\nimport {import}\n\nclass {prefix}{index}(val used: {name})\n"),
        None => format!("package {package}\n\nclass {prefix}{index}\n"),
      };
      (format!("{dir}/{prefix}{index}.kt"), text)
    })
    .collect()
}

/// A range of a large step: the GUI, a small shared module and a spec at the base (and a list of the modules with
/// `list`), a told first commit, then one that adds the module `gui/media` (`players` classes that use `a.Embed0`, the
/// `tests` given, its flags over `s.Flag`, its build file), adds 5 classes to `gui`, and changes the flag of `shared`
/// and the spec (and the list): its base and its head
fn media_range(repo: &TestRepo, players: usize, tests: Vec<(String, String)>, list: bool) -> (String, String) {
  let mut at_base = vec![
    ("gui/BUILD.bazel", "# gui\n"),
    ("gui/src/a/Box.kt", "package a\n\nclass Box\n"),
    ("shared/BUILD.bazel", "# shared\n"),
    ("shared/src/s/Flag.kt", "package s\n\nclass Flag\n"),
    ("spec/player.spec.md", "# Player\n\nThe chat shows a card.\n"),
  ];
  if list {
    at_base.push(("build/module-list.txt", "gui\nshared\n"));
  }
  let base = repo.create_commit_with_files("Add the GUI", &at_base);
  let mut files: Vec<(String, String)> = Vec::new();
  files.extend(classes("gui/media/src/b", "b", "Player", players, Some(("a.Embed0", "Embed0"))));
  files.extend(tests);
  files.extend(classes("gui/src/a", "a", "Embed", 5, None));
  files.push((
    "gui/media/src/b/Flags.kt".to_string(),
    "package b\n\nimport s.Flag\n\nclass Flags(val flag: Flag)\n".to_string(),
  ));
  files.push(("gui/media/BUILD.bazel".to_string(), "# media\n".to_string()));
  files.push(("shared/src/s/Flag.kt".to_string(), "package s\n\nclass Flag(val on: Boolean)\n".to_string()));
  files.push(("spec/player.spec.md".to_string(), "# Player\n\nThe chat plays a video in its card.\n".to_string()));
  if list {
    files.push(("build/module-list.txt".to_string(), "gui\ngui/media\nshared\n".to_string()));
  }
  let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
  told_first(repo);
  let head = repo.create_commit_with_files("Play a video\n\nThe chat plays a video in its card through a player of its own.", &files);
  (base, head)
}

/// A commit that adds a module and changes two: its spec first, then each module after those whose declarations it
/// newly uses, its tests at its end; a small module the new one uses joins its group; the build files last
#[test]
fn a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring() {
  let repo = TestRepo::new();
  let (base, head) = media_range(&repo, 6, classes("gui/media/testSrc/b", "b", "PlayerTest", 2, None), true);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    groups(&plan),
    [
      vec![],
      vec![
        (GroupKind::Contract, String::new(), vec![], vec!["player.spec.md"], vec![]),
        (
          GroupKind::Module,
          "gui".to_string(),
          vec![],
          vec!["Embed0.kt", "Embed1.kt", "Embed2.kt", "Embed3.kt", "Embed4.kt"],
          vec![]
        ),
        (
          GroupKind::Module,
          "gui/media".to_string(),
          vec!["shared"],
          vec!["Player0.kt", "Player1.kt", "Player2.kt", "Player3.kt", "Player4.kt", "Player5.kt", "Flag.kt", "Flags.kt"],
          vec!["PlayerTest0.kt", "PlayerTest1.kt"]
        ),
        (GroupKind::Mechanical, String::new(), vec![], vec!["BUILD.bazel", "module-list.txt"], vec![]),
      ]
    ]
  );
  // every stop is in one group, once
  let step = &plan.steps.steps[1];
  assert_eq!(step.groups.last().map(|group| group.end as usize), Some(step.stops.len()));
  assert_eq!(plan.steps.coverage.twice, 0);
  assert_eq!(plan.steps.coverage.missing, 0);
}

/// The tests of a step as a group reads them: its test files, each test with what of the code it reaches, its code no
/// test of the change reaches, and how many of its declarations one does
type GroupTested<'a> = (Vec<(&'a str, Vec<(&'a str, Vec<&'a str>)>)>, Vec<&'a str>, u32);

fn group_tested(tested: &StepTests) -> GroupTested<'_> {
  let proof = tested
    .proof
    .iter()
    .map(|file| {
      let tests = file
        .tests
        .iter()
        .map(|test| (test.name.as_str(), test.reaches.iter().map(String::as_str).collect()))
        .collect();
      (file.path.rsplit('/').next().unwrap_or(&file.path), tests)
    })
    .collect();
  let mut unreached: Vec<&str> = tested.unreached.iter().map(|it| it.name.as_str()).collect();
  unreached.sort_unstable();
  (proof, unreached, tested.reached)
}

/// A group of a large step knows the tests of the change that reach the code of its files and its code none reaches,
/// as the step knows them of all of its code: a test of one group that reaches the code of another is that other's; the
/// code of a small module a group holds is that group's; the contract and the wiring have none
#[test]
fn a_group_knows_the_tests_that_reach_its_code_and_its_code_none_reaches() {
  let repo = TestRepo::new();
  // two tests of the new module, each a test function at line 6
  let tests = (0..2)
    .map(|index| {
      (
        format!("gui/media/testSrc/b/PlayerTest{index}.kt"),
        format!("package b\n\nimport org.junit.Test\n\nclass PlayerTest{index} {{\n  @Test fun `plays {index}`() = Unit\n}}\n"),
      )
    })
    .collect();
  let (base, head) = media_range(&repo, 2, tests, false);
  let test = |path: &str, reads_as: &str| ReachingTest {
    path: path.to_string(),
    line: 6,
    declaration: format!("b.{reads_as}"),
    reads_as: reads_as.to_string(),
    framework: "junit4".to_string(),
    depth: 1,
    via: Vec::new(),
    class: TestClass::Direct,
    through_hub: false,
  };
  let reached = |tests: Vec<ReachingTest>| TestReach {
    tests,
    complete: true,
    unavailable: false,
  };
  // a test of the new module reaches a class of gui, the other one a class of the new module; a place the index does
  // not know no test reaches
  let index = Counts::of(&[]).with_tests([
    ("gui/src/a/Embed0.kt:3", reached(vec![test("gui/media/testSrc/b/PlayerTest0.kt", "plays 0")])),
    ("gui/media/src/b/Player0.kt:5", reached(vec![test("gui/media/testSrc/b/PlayerTest1.kt", "plays 1")])),
  ]);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
  let step = &plan.steps.steps[1];
  let found: Vec<(GroupKind, GroupTested<'_>)> = step.groups.iter().map(|group| (group.kind, group_tested(&group.tested))).collect();
  assert_eq!(
    found,
    [
      (GroupKind::Contract, (vec![], vec![], 0)),
      (
        GroupKind::Module,
        (vec![("PlayerTest0.kt", vec![("plays 0", vec!["Embed0"])])], vec!["Embed1", "Embed2", "Embed3", "Embed4"], 1)
      ),
      (
        GroupKind::Module,
        (vec![("PlayerTest1.kt", vec![("plays 1", vec!["Player0"])])], vec!["Flag", "Flags", "Player1", "on"], 1)
      ),
      (GroupKind::Mechanical, (vec![], vec![], 0)),
    ]
  );
  assert!(step.groups.iter().all(|group| group.tested.answered));
  // the step knows the same of all of its code
  assert_eq!(step.tests.reached, 2);
  assert_eq!(step.tests.unreached.len(), 8);
}

/// A module the change adds names what enters it, from accio at the head: a class its descriptor registers, then a
/// declaration the code of another module uses; a use by the name alone or from test code enters nothing. Its group
/// starts with the files of its entry points, its tests still last, and the files of the step follow its stops
#[test]
fn a_new_module_names_what_enters_it_and_its_group_starts_with_their_files() {
  let repo = TestRepo::new();
  // a step of more than 12 files: large enough to read by its groups
  let (base, head) = media_range(&repo, 3, classes("gui/media/testSrc/b", "b", "PlayerTest", 1, None), false);
  let declaration = |name: &str, line: u32, uses: serde_json::Value| serde_json::json!({ "file": format!("gui/media/src/b/{name}.kt"), "line": line, "kind": "class", "name": name, "qualified": format!("b.{name}"), "used_by": [], "uses": uses });
  let mut flags = declaration(
    "Flags",
    5,
    serde_json::json!([{ "file": "gui/src/a/Embed0.kt", "line": 3, "label": "type", "resolved": "typed", "test": false }]),
  );
  flags["used_by"] = serde_json::json!([{ "file": "gui/src/a/Embed0.kt", "line": 3 }]);
  let surface = serde_json::json!({
    "module": "gui/media",
    "complete": true,
    "declarations": [
      declaration("Player1", 5, serde_json::json!([{ "file": "gui/media/resources/media.xml", "line": 4, "label": "descriptor", "point": "com.intellij.applicationService", "of": "a.Players", "resolved": "typed", "test": false }])),
      flags,
      // by the name alone, and from test code: no entry
      declaration("Player0", 5, serde_json::json!([{ "file": "gui/src/a/Embed1.kt", "line": 3, "label": "call", "resolved": "name", "test": false }])),
      declaration("PlayerTest0", 3, serde_json::json!([{ "file": "gui/testSrc/a/BoxTest.kt", "line": 3, "label": "call", "resolved": "typed", "test": true }])),
    ],
  });
  std::fs::write(
    std::path::Path::new(repo.path_str()).join(".git/branch-deck-accio.json"),
    serde_json::json!({ "surface": { format!("{head}:gui/media"): surface } }).to_string(),
  )
  .unwrap();
  let plan = plan_of(&repo, Some(&FixtureIndex), &range(&repo, &base, &head)).unwrap();
  let media = plan.modules.iter().find(|module| module.root == "gui/media").unwrap();
  assert_eq!(
    media.entries,
    Some(ModuleEntries {
      complete: true,
      points: vec![
        EntryPoint {
          name: "Player1".to_string(),
          qualified: Some("b.Player1".to_string()),
          kind: "class".to_string(),
          path: "gui/media/src/b/Player1.kt".to_string(),
          line: 5,
          registered: Some(EntryRegistration {
            point: "com.intellij.applicationService".to_string(),
            of: Some("a.Players".to_string()),
            path: "gui/media/resources/media.xml".to_string(),
            line: 4,
          }),
          used_by: vec![],
        },
        EntryPoint {
          name: "Flags".to_string(),
          qualified: Some("b.Flags".to_string()),
          kind: "class".to_string(),
          path: "gui/media/src/b/Flags.kt".to_string(),
          line: 5,
          registered: None,
          used_by: vec![EntrySite {
            path: "gui/src/a/Embed0.kt".to_string(),
            line: 3
          }],
        },
      ],
    })
  );
  // the changed modules know none
  assert!(plan.modules.iter().filter(|module| module.root != "gui/media").all(|module| module.entries.is_none()));
  let step = &plan.steps.steps[1];
  let group = step
    .groups
    .iter()
    .find(|group| group.module.is_some_and(|module| plan.modules[module as usize].root == "gui/media"))
    .unwrap();
  let files: Vec<&str> = step.stops[group.start as usize..group.end as usize]
    .iter()
    .map(|stop| stop.path.rsplit('/').next().unwrap())
    .collect();
  assert_eq!(&files[..2], ["Player1.kt", "Flags.kt"], "{files:?}");
  assert_eq!(files.last(), Some(&"PlayerTest0.kt"));
  // the files of the step in the order of its stops
  let mut in_order: Vec<&str> = Vec::new();
  for stop in &step.stops {
    if !in_order.contains(&stop.path.as_str()) {
      in_order.push(&stop.path);
    }
  }
  assert_eq!(&step.paths[..in_order.len()], in_order.as_slice());
}

/// The template of the review window (`review_modules`): a plugin of the shape of an IntelliJ one whose second commit
/// adds a module, its contract the spec and the guide that names it, the small shared module it opens wider in its group
#[test]
fn the_modules_template_reads_as_its_contract_its_modules_and_its_wiring() {
  let repo = TestRepo::new();
  let commits = test_utils::review_modules::review_modules_commits();
  let ids: Vec<String> = commits
    .iter()
    .map(|commit| {
      let files: Vec<(&str, &str)> = commit.files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
      repo.create_commit_with_files(commit.message, &files)
    })
    .collect();
  let plan = plan_of_range(&repo, &ids[0], ids.last().unwrap());
  let labels: Vec<(&str, crate::modules::ModuleState)> = plan.modules.iter().map(|module| (module.label.as_str(), module.state)).collect();
  assert!(labels.contains(&("view.gui.media", crate::modules::ModuleState::Added)), "{labels:?}");
  assert_eq!(
    groups(&plan)[1],
    vec![
      (GroupKind::Contract, String::new(), vec![], vec!["transcript-embeds.spec.md", "AGENTS.md"], vec![]),
      (
        GroupKind::Module,
        "view.gui".to_string(),
        vec![],
        vec![
          "PlayerBundle.properties",
          "EmbedKind.kt",
          "EmbedModel.kt",
          "MarkdownEmbeds.kt",
          "EmbedView.kt",
          "EmbedViews.kt"
        ],
        vec!["EmbedModelTest.kt"]
      ),
      (
        GroupKind::Module,
        "view.gui.media".to_string(),
        vec!["shared/code"],
        // the function of shared/code it opens wider before its uses
        vec![
          "NightlyBuild.kt",
          "MediaPlaybackFlag.kt",
          "MediaEmbeds.kt",
          "intellij.player.view.gui.media.xml",
          "AvFoundationBridge.kt",
          "MediaTimeline.kt",
          "MediaDecoder.kt",
          "AvFoundationDecoder.kt",
          "MediaPlayer.kt",
          "MediaControls.kt",
          "MediaEmbedView.kt"
        ],
        vec!["MediaTimelineTest.kt", "MediaPlayerTest.kt"]
      ),
      (
        GroupKind::Mechanical,
        String::new(),
        vec![],
        vec!["modules.xml", "plugin.xml", "BUILD.bazel", "intellij.player.view.gui.media.iml", "module-list.txt"],
        vec![]
      ),
    ]
  );
}

/// A step of a few files, of one module or of two old ones of a few files each, reads as one list
#[test]
fn a_small_step_has_no_groups() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the modules", &[("gui/BUILD.bazel", "# gui\n"), ("shared/BUILD.bazel", "# shared\n")]);
  let mut files = classes("gui/src/a", "a", "Embed", 10, None);
  files.extend(classes("shared/src/s", "s", "Flag", 3, None));
  let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
  told_first(&repo);
  let head = repo.create_commit_with_files("Embed more\n\nThe chat embeds more kinds of files.", &files);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(groups(&plan), [vec![], vec![]]);
}

/// A step of more than 25 files of one module reads by the folders of the module
#[test]
fn a_large_step_of_one_module_reads_by_its_folders() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the GUI", &[("gui/BUILD.bazel", "# gui\n")]);
  let mut files = classes("gui/src/ui/embeds", "ui.embeds", "Embed", 14, None);
  files.extend(classes("gui/src/ui/markdown", "ui.markdown", "Markdown", 12, None));
  let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
  told_first(&repo);
  let head = repo.create_commit_with_files("Embed media\n\nThe chat embeds media in its Markdown.", &files);
  let plan = plan_of_range(&repo, &base, &head);
  let found: Vec<(GroupKind, String, usize)> = groups(&plan)[1]
    .iter()
    .map(|(kind, label, _, files, tests)| (*kind, label.clone(), files.len() + tests.len()))
    .collect();
  assert_eq!(
    found,
    // the folder every file shares (`ui`) names none of them
    [(GroupKind::Folder, "embeds".to_string(), 14), (GroupKind::Folder, "markdown".to_string(), 12)]
  );
}
