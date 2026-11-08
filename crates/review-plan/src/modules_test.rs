use crate::ReviewPlan;
use crate::files::MechanicalKind;
use crate::modules::{ModuleFiles, ModuleState, labels_of};
use crate::test_support::plan_of_range;
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

/// A module the commits add comes first, with its files by role, its lines and what it newly uses: the module of
/// another file of the change its code links to, declared by its `*.iml` too; the plugin descriptor that adds it as
/// content names it, and the list of the module directories that adds its line is its wiring
#[test]
fn a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses() {
  let repo = TestRepo::new();
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
      ("view/src/references/R.kt", "class R\n"),
      ("view/src/ui/embeds/E.kt", "class E\n"),
      ("view/testSrc/ui/embeds/ETest.kt", "class ETest\n"),
    ],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let gui = plan.modules.iter().find(|module| module.root == "gui").unwrap();
  let directories: Vec<(&str, u32)> = gui.directories.iter().map(|directory| (directory.path.as_str(), directory.files)).collect();
  assert_eq!(directories, [("embeds", 3), ("markdown", 1), ("messages", 1)]);
  let view = plan.modules.iter().find(|module| module.root == "view").unwrap();
  let directories: Vec<(&str, u32)> = view.directories.iter().map(|directory| (directory.path.as_str(), directory.files)).collect();
  assert_eq!(directories, [("ui/embeds", 2), ("references", 1)]);
}
