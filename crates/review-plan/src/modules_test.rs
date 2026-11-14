use crate::ReviewPlan;
use crate::files::MechanicalKind;
use crate::modules::{ModuleFiles, ModuleState, labels_of};
use crate::test_support::{DeclaredModules, ModulesBy, add_media, branch, mark, plan_by_accio, plan_of, plan_of_range, range};
use file_diffs::accio::SymbolIndex;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// Each module: (root, name, label, state)
fn modules(plan: &ReviewPlan) -> Vec<(&str, &str, &str, ModuleState)> {
  plan
    .modules
    .iter()
    .map(|module| (module.root.as_str(), module.name.as_str(), module.label.as_str(), module.state))
    .collect()
}

/// The module of each file, by its label
fn module_of_files(plan: &ReviewPlan) -> Vec<(&str, &str)> {
  let mut found: Vec<(&str, &str)> = plan
    .files
    .iter()
    .map(|facts| (facts.path.as_str(), facts.module.map_or("", |module| plan.modules[module as usize].label.as_str())))
    .collect();
  found.sort_unstable();
  found
}

const IML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<module type=\"JAVA_MODULE\" version=\"4\">\n  <component name=\"NewModuleRootManager\">\n    <orderEntry type=\"module\" module-name=\"intellij.gui\" />\n  </component>\n</module>\n";

/// The commits of `repo` that add the module `gui/media` beside `gui`: its `*.iml`, its `BUILD.bazel`, its code, tests
/// and bundle, a use of the code of `gui`, the plugin descriptor that holds it as content and the list of the module
/// directories; (base, head)
fn add_the_player(repo: &TestRepo) -> (String, String) {
  let base = repo.create_commit_with_files(
    "Add the GUI",
    &[
      ("gui/intellij.gui.iml", "<module/>\n"),
      ("gui/src/a/Box.kt", "package a\n\nclass Box\n"),
      (
        "plugin/resources/META-INF/plugin.xml",
        "<idea-plugin>\n  <id>com.example.app</id>\n  <content>\n    <module name=\"intellij.gui\"/>\n  </content>\n</idea-plugin>\n",
      ),
      ("build/module-list.txt", "gui\nplugin\n"),
      ("plugin/intellij.app.dev-plan.json", "[\n  {\n    \"module\": \"intellij.gui\"\n  }\n]\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Add the player",
    &[
      ("gui/media/intellij.gui.media.iml", IML),
      ("gui/media/BUILD.bazel", "# media\n"),
      ("gui/media/src/b/Player.kt", "package b\n\nimport a.Box\n\nclass Player(val box: Box)\n"),
      ("gui/media/src/b/ui/Seek.kt", "package b.ui\n\nclass Seek\n"),
      ("gui/media/testSrc/b/PlayerTest.kt", "package b\n\nclass PlayerTest\n"),
      ("gui/media/resources/messages/Media.properties", "media.play=Play\n"),
      ("gui/src/a/Box.kt", "package a\n\nclass Box(val size: Int)\n"),
      (
        "plugin/resources/META-INF/plugin.xml",
        "<idea-plugin>\n  <id>com.example.app</id>\n  <content>\n    <module name=\"intellij.gui\"/>\n    <module name=\"intellij.gui.media\"/>\n  </content>\n</idea-plugin>\n",
      ),
      ("build/module-list.txt", "gui\ngui/media\nplugin\n"),
      (
        "plugin/intellij.app.dev-plan.json",
        "[\n  {\n    \"module\": \"intellij.gui\"\n  },\n  {\n    \"module\": \"intellij.gui.media\"\n  }\n]\n",
      ),
    ],
  );
  (base, head)
}

/// A module the commits add comes first, with its files by role, its lines and what it newly uses: the module of
/// another file of the change its code links to, declared by its `*.iml` too; the plugin descriptor that adds it as
/// content names it, and the list of the module directories that adds its line is its wiring
#[test]
fn a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses() {
  let repo = TestRepo::new();
  let (base, head) = add_the_player(&repo);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    modules(&plan),
    [
      ("gui/media", "intellij.gui.media", "gui/media", ModuleState::Added),
      ("gui", "intellij.gui", "gui", ModuleState::Changed),
      ("", "(root)", "(root)", ModuleState::Changed),
    ]
  );
  let media = &plan.modules[0];
  assert_eq!(
    media.files,
    ModuleFiles {
      code: 2,
      tests: 1,
      resources: 1,
      build: 2,
      prose: 0
    }
  );
  assert_eq!((media.additions, media.deletions), (19, 0));
  let directories: Vec<(&str, u32)> = media.directories.iter().map(|directory| (directory.path.as_str(), directory.files)).collect();
  assert_eq!(directories, [("", 2), ("b", 2), ("b/ui", 1), ("messages", 1)]);
  let uses: Vec<(&str, usize, bool)> = media
    .uses
    .iter()
    .map(|used| (plan.modules[used.module as usize].label.as_str(), used.links.len(), used.declared))
    .collect();
  assert_eq!(uses, [("gui", 1, true)]);
  assert_eq!(plan.modules[1].used_by, [0]);
  assert_eq!(media.content_of, ["com.example.app"]);
  assert_eq!(
    module_of_files(&plan),
    [
      ("build/module-list.txt", "(root)"),
      ("gui/media/BUILD.bazel", "gui/media"),
      ("gui/media/intellij.gui.media.iml", "gui/media"),
      ("gui/media/resources/messages/Media.properties", "gui/media"),
      ("gui/media/src/b/Player.kt", "gui/media"),
      ("gui/media/src/b/ui/Seek.kt", "gui/media"),
      ("gui/media/testSrc/b/PlayerTest.kt", "gui/media"),
      ("gui/src/a/Box.kt", "gui"),
      ("plugin/intellij.app.dev-plan.json", "(root)"),
      ("plugin/resources/META-INF/plugin.xml", "(root)"),
    ]
  );
  // a list of directories, and a JSON list of module names whose other lines are the punctuation of its entries
  for path in ["build/module-list.txt", "plugin/intellij.app.dev-plan.json"] {
    let list = plan.files.iter().find(|facts| facts.path == path).unwrap();
    assert_eq!(list.mechanical, Some(MechanicalKind::ModuleList), "{path}");
  }
}

/// The modules of a review are those the accio of this machine answers for the base and the head (`accio modules`):
/// the same roots, names and states as the index of these tests gives (`test_support::declared_modules`), a Python
/// project accio declares among them. Nothing is asked on a machine without an accio that answers them
#[test]
fn the_modules_of_a_review_are_those_accio_answers() {
  let repo = TestRepo::new();
  let (base, _) = add_the_player(&repo);
  let head = repo.create_commit_with_files(
    "Add the scripts",
    &[
      ("tools/pyproject.toml", "[project]\nname = \"tools\"\n"),
      ("tools/scripts/report.py", "def report():\n    pass\n"),
    ],
  );
  let Some(plan) = plan_by_accio(&repo, &base, &head) else {
    eprintln!("no accio that answers modules on this machine: nothing asked");
    return;
  };
  let expected = plan_of_range(&repo, &base, &head);
  assert_eq!(modules(&plan), modules(&expected));
  assert_eq!(module_of_files(&plan), module_of_files(&expected));
  assert!(modules(&plan).contains(&("tools", "tools", "tools", ModuleState::Added)), "{:?}", modules(&plan));
}

/// A module is of the roots accio answers at the two sides, not of the build files the commits touch: one whose build
/// file the commits rename is changed, and without both answers every file is of the root of the repository
#[test]
fn a_module_is_added_or_deleted_by_the_roots_of_the_two_sides() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the GUI", &[("gui/BUILD", "# gui\n"), ("gui/src/Box.kt", "class Box\n")]);
  repo.git(&["mv", "gui/BUILD", "gui/BUILD.bazel"]);
  std::fs::write(repo.path().join("gui/src/Box.kt"), "class Box(val size: Int)\n").unwrap();
  repo.git(&["add", "-A"]);
  let head = repo.commit_with(&["-m", "Rename the build file"], &[]);
  assert_eq!(modules(&plan_of_range(&repo, &base, &head)), [("gui", "gui", "gui", ModuleState::Changed)]);
  let unanswered = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  assert_eq!(modules(&unanswered), [("", "(root)", "(root)", ModuleState::Changed)]);
}

/// A list that adds a line naming no module the commits add or delete is real work, and so is one whose module came
/// before the change
#[test]
fn a_list_that_names_no_module_the_commits_add_is_real_work() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the GUI", &[("gui/BUILD.bazel", "# gui\n"), ("build/module-list.txt", "gui\n")]);
  let head = repo.create_commit_with_files("List the tools", &[("tools/BUILD.bazel", "# tools\n"), ("build/module-list.txt", "gui\ntools\nnotes\n")]);
  let plan = plan_of_range(&repo, &base, &head);
  let list = plan.files.iter().find(|facts| facts.path == "build/module-list.txt").unwrap();
  assert_eq!(list.mechanical, None);
}

/// The files of a module the commits delete stay in it, though the head has no build file there
#[test]
fn the_files_of_a_module_the_commits_delete_stay_in_it() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add the modules",
    &[
      ("gui/BUILD.bazel", "# gui\n"),
      ("gui/old/BUILD.bazel", "# old\n"),
      ("gui/old/src/Old.kt", "class Old\n"),
      ("gui/src/Box.kt", "class Box\n"),
    ],
  );
  repo.git(&["rm", "-q", "-r", "gui/old"]);
  std::fs::write(repo.path().join("gui/src/Box.kt"), "class Box(val size: Int)\n").unwrap();
  repo.git(&["add", "-A"]);
  let head = repo.commit_with(&["-m", "Drop the old module"], &[]);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    modules(&plan),
    [("gui", "gui", "gui", ModuleState::Changed), ("gui/old", "old", "gui/old", ModuleState::Deleted)]
  );
  assert_eq!(
    module_of_files(&plan),
    [("gui/old/BUILD.bazel", "gui/old"), ("gui/old/src/Old.kt", "gui/old"), ("gui/src/Box.kt", "gui")]
  );
}

/// A label is the last directory when it has a `.`, `-` or `_`, else the last two, longer while another module shares it
#[test]
fn a_label_is_the_shortest_suffix_no_other_module_shares() {
  let roots = [
    "plugins/air/frontend/session/view.gui",
    "plugins/air/frontend/session/view.gui.media",
    "plugins/air/shared/code",
    "plugins/other/shared/code",
    "crates/review-plan",
    "plugins/air",
    "",
  ];
  assert_eq!(
    labels_of(&roots),
    ["view.gui", "view.gui.media", "air/shared/code", "other/shared/code", "review-plan", "plugins/air", "(root)"]
  );
}

/// A file the range adds counts the lines its commits leave, as the diff of the range does, though a later commit
/// changed what an earlier one added
#[test]
fn a_file_the_range_adds_counts_the_lines_its_commits_leave() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the GUI", &[("gui/BUILD.bazel", "# gui\n")]);
  repo.create_commit_with_files("Add the box", &[("gui/src/Box.kt", "package a\n\nclass Box\n\nfun one() = 1\n")]);
  let head = repo.create_commit_with_files("Shrink the box", &[("gui/src/Box.kt", "package a\n\nclass Box(val size: Int)\n")]);
  let plan = plan_of_range(&repo, &base, &head);
  let gui = plan.modules.iter().find(|module| module.root == "gui").unwrap();
  assert_eq!((gui.additions, gui.deletions), (3, 0));
}

/// The folders of a module of packages (`com/example/player/…`) start below the folders its code and its tests share: a
/// test of a package under `testSrc` is in the folder of that package under `src`, a bundle under `resources` shares no
/// package with them; a module of no package prefix names a folder of its tests as that of its code (`ui/embeds`)
#[test]
fn the_folders_of_a_module_start_below_the_packages_its_files_share() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the GUI", &[("gui/BUILD.bazel", "# gui\n"), ("view/BUILD.bazel", "# view\n")]);
  let head = repo.create_commit_with_files(
    "Embed media",
    &[
      ("gui/src/com/example/player/gui/embeds/A.kt", "class A\n"),
      ("gui/src/com/example/player/gui/embeds/B.kt", "class B\n"),
      ("gui/src/com/example/player/gui/markdown/C.kt", "class C\n"),
      ("gui/testSrc/com/example/player/gui/embeds/ATest.kt", "class ATest\n"),
      ("gui/resources/messages/GuiBundle.properties", "embed.open=Open\n"),
      ("gui/BUILD.bazel", "# gui\n# with the embeds\n"),
      ("view/src/references/R.kt", "class R\n"),
      ("view/src/ui/embeds/E.kt", "class E\n"),
      ("view/testSrc/ui/embeds/ETest.kt", "class ETest\n"),
    ],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let gui = plan.modules.iter().find(|module| module.root == "gui").unwrap();
  let directories: Vec<(&str, u32)> = gui.directories.iter().map(|directory| (directory.path.as_str(), directory.files)).collect();
  // the build file right in the root of the module takes no part in what the others share
  assert_eq!(directories, [("embeds", 3), ("", 1), ("markdown", 1), ("messages", 1)]);
  let view = plan.modules.iter().find(|module| module.root == "view").unwrap();
  let directories: Vec<(&str, u32)> = view.directories.iter().map(|directory| (directory.path.as_str(), directory.files)).collect();
  assert_eq!(directories, [("ui/embeds", 2), ("references", 1)]);
  // the folder of each file, after its name in the tree: below every folder its kind shares, all of its levels, ""
  // in its own
  let mut folders: Vec<(&str, &str)> = plan.files.iter().map(|facts| (facts.path.as_str(), facts.folder.as_str())).collect();
  folders.sort_unstable();
  assert_eq!(
    folders,
    [
      ("gui/BUILD.bazel", ""),
      ("gui/resources/messages/GuiBundle.properties", ""),
      ("gui/src/com/example/player/gui/embeds/A.kt", "embeds"),
      ("gui/src/com/example/player/gui/embeds/B.kt", "embeds"),
      ("gui/src/com/example/player/gui/markdown/C.kt", "markdown"),
      ("gui/testSrc/com/example/player/gui/embeds/ATest.kt", "embeds"),
      ("view/src/references/R.kt", "references"),
      ("view/src/ui/embeds/E.kt", "ui/embeds"),
      ("view/testSrc/ui/embeds/ETest.kt", "ui/embeds"),
    ]
  );
}

/// The lines of a module are what `git diff --numstat` of the range counts, also where the engine aligns a block
/// otherwise than git: a call moved into the test above it, which the engine matches as an important line before the
/// closing brace and the blank line, deleted and added again (+3 −3,
/// `diff_scan_test::read_commit_diffs_gives_git_counts_beside_the_engines`)
#[test]
fn the_lines_of_a_module_are_what_git_counts_where_the_engine_aligns_otherwise() {
  let repo = TestRepo::new();
  let old = "class FooTest {\n  fun a() {\n    one()\n  }\n\n  fun b() {\n    two()\n  }\n}\n";
  let new = "class FooTest {\n  fun a() {\n    one()\n    two()\n  }\n\n  fun b() {\n  }\n}\n";
  let base = repo.create_commit_with_files("Add the tests", &[("tests/BUILD.bazel", "# tests\n"), ("tests/testSrc/a/FooTest.kt", old)]);
  let head = repo.create_commit_with_files("Test two in a", &[("tests/testSrc/a/FooTest.kt", new)]);
  let plan = plan_of_range(&repo, &base, &head);
  let tests = plan.modules.iter().find(|module| module.root == "tests").unwrap();
  let engine = line_diff::model::diff_texts(old, new, line_diff::policy::WhitespacePolicy::Default, &line_diff::model::DiffBudget::NONE);
  assert_eq!((engine.additions, engine.deletions), (3, 3), "the engine aligns the blocks otherwise");
  assert_eq!((tests.additions, tests.deletions), (1, 1));
}

/// A file that several commits change counts the blocks of its pair, the engine's count of the range (git's would take
/// a process of its own, and the sum of its commits' is no count of the range): without the empty line after the final
/// line end, which a block that reaches the end of the file holds where the range adds that line end, and no count
/// takes. Here git counts +4 −2: the call moved as `the_lines_of_a_module_are_what_git_counts_where_the_engine_aligns_otherwise`,
/// and the last line of the old text as changed, which the engine compares without its line end
#[test]
fn the_lines_of_a_file_several_commits_change_are_the_engines_count_of_its_pair() {
  let repo = TestRepo::new();
  let old = "class FooTest {\n  fun a() {\n    one()\n  }\n\n  fun b() {\n    two()\n  }\n}";
  let base = repo.create_commit_with_files("Add the tests", &[("tests/BUILD.bazel", "# tests\n"), ("tests/testSrc/a/FooTest.kt", old)]);
  repo.create_commit_with_files(
    "Test two in a",
    &[(
      "tests/testSrc/a/FooTest.kt",
      "class FooTest {\n  fun a() {\n    one()\n    two()\n  }\n\n  fun b() {\n  }\n}",
    )],
  );
  let last = "class FooTest {\n  fun a() {\n    one()\n    two()\n  }\n\n  fun b() {\n  }\n}\n\nprivate fun d() = 1\n";
  let head = repo.create_commit_with_files("Add the helper", &[("tests/testSrc/a/FooTest.kt", last)]);
  let plan = plan_of_range(&repo, &base, &head);
  let tests = plan.modules.iter().find(|module| module.root == "tests").unwrap();
  let engine = line_diff::model::diff_texts(old, last, line_diff::policy::WhitespacePolicy::Default, &line_diff::model::DiffBudget::NONE);
  let blocks: u32 = engine.blocks.iter().map(|block| block.new_end - block.new_start).sum();
  assert_eq!((engine.additions, engine.deletions, blocks), (5, 3, 6), "a block holds the empty last line");
  assert_eq!((tests.additions, tests.deletions), (5, 3));
}

/// Where the index answers the modules of one side alone, of the head or of the base, none answering or failing,
/// every file is of the root of the repository, changed: no side stands in for the other
#[test]
fn the_modules_of_one_side_alone_put_every_file_at_the_root() {
  let repo = TestRepo::new();
  let (base, head) = add_media(&repo);
  assert_eq!(
    modules(&plan_of_range(&repo, &base, &head)),
    [("media", "media", "media", ModuleState::Added), ("gui", "gui", "gui", ModuleState::Changed)]
  );
  for (answered, fails) in [(&head, false), (&head, true), (&base, false), (&base, true)] {
    let index = ModulesBy(|repository_root: &str, paths: &[String], revision: &str| {
      if revision == answered.as_str() {
        DeclaredModules.modules(repository_root, paths, revision)
      } else if fails {
        anyhow::bail!("accio failed at {revision}")
      } else {
        Ok(None)
      }
    });
    let plan = plan_of(&repo, Some(&index), &range(&repo, &base, &head)).unwrap();
    let side = if answered == &head { "head" } else { "base" };
    assert_eq!(modules(&plan), [("", "(root)", "(root)", ModuleState::Changed)], "{side}, the other fails: {fails}");
    assert!(plan.files.iter().all(|facts| facts.module == Some(0)), "{side}, the other fails: {fails}");
  }
}

/// Since Your Review whose carry conflicts reads the new and the rewritten commits alone, each against its parent: their
/// modules are compared with the reviewed tip, so a module the reviewer read is changed, not added again
#[test]
fn since_the_review_whose_carry_conflicts_compares_the_modules_with_the_reviewed_tip() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Add readme", &[("README.md", "readme\n"), ("shared.txt", "one\n")]);
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  let player = |shared: &str| {
    for (path, content) in [("media/BUILD.bazel", "# media\n"), ("media/src/Player.kt", "class Player\n"), ("shared.txt", shared)] {
      std::fs::create_dir_all(repo.path().join(path).parent().unwrap()).unwrap();
      std::fs::write(repo.path().join(path), content).unwrap();
    }
    repo.git(&["add", "-A"]);
    repo.commit_with(
      &["-m", "(auth) Add the player"],
      &[("GIT_AUTHOR_DATE", "1700000100 +0000"), ("GIT_COMMITTER_DATE", "1700000100 +0000")],
    )
  };
  player("two\n");
  mark(&repo);
  // the baseline changes the line the review changed, and the branch is carried onto it by hand: the carry conflicts
  repo.git(&["switch", "-q", "master"]);
  repo.create_commit_with_files("Change shared", &[("shared.txt", "three\n")]);
  repo.git(&["switch", "-q", "-C", "jane/virtual/auth"]);
  player("two three\n");
  repo.create_commit_with_files("(auth) Play a clip", &[("media/src/Player.kt", "class Player(val clip: String)\n")]);

  let plan = plan_of(&repo, Some(&DeclaredModules), &branch(&repo, true)).unwrap();
  assert!(plan.interdiff.as_ref().is_some_and(|interdiff| interdiff.scope.is_none()), "{:?}", plan.interdiff);
  assert_eq!(
    modules(&plan),
    [("media", "media", "media", ModuleState::Changed), ("", "(root)", "(root)", ModuleState::Changed)]
  );
}
