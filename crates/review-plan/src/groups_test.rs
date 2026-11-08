use crate::ReviewPlan;
use crate::groups::GroupKind;
use crate::test_support::plan_of_range;
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

/// A commit that adds a module and changes two: its spec first, then each module after those whose declarations it
/// newly uses, its tests at its end; a small module the new one uses joins its group; the build files last
#[test]
fn a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add the GUI",
    &[
      ("gui/BUILD.bazel", "# gui\n"),
      ("gui/src/a/Box.kt", "package a\n\nclass Box\n"),
      ("shared/BUILD.bazel", "# shared\n"),
      ("shared/src/s/Flag.kt", "package s\n\nclass Flag\n"),
      ("spec/player.spec.md", "# Player\n\nThe chat shows a card.\n"),
      ("build/module-list.txt", "gui\nshared\n"),
    ],
  );
  let mut files: Vec<(String, String)> = Vec::new();
  files.extend(classes("gui/media/src/b", "b", "Player", 6, Some(("a.Embed0", "Embed0"))));
  files.extend(classes("gui/media/testSrc/b", "b", "PlayerTest", 2, None));
  files.extend(classes("gui/src/a", "a", "Embed", 5, None));
  files.push((
    "gui/media/src/b/Flags.kt".to_string(),
    "package b\n\nimport s.Flag\n\nclass Flags(val flag: Flag)\n".to_string(),
  ));
  files.push(("gui/media/BUILD.bazel".to_string(), "# media\n".to_string()));
  files.push(("shared/src/s/Flag.kt".to_string(), "package s\n\nclass Flag(val on: Boolean)\n".to_string()));
  files.push(("spec/player.spec.md".to_string(), "# Player\n\nThe chat plays a video in its card.\n".to_string()));
  files.push(("build/module-list.txt".to_string(), "gui\ngui/media\nshared\n".to_string()));
  let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
  told_first(&repo);
  let head = repo.create_commit_with_files("Play a video\n\nThe chat plays a video in its card through a player of its own.", &files);
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
    [(GroupKind::Folder, "ui/embeds".to_string(), 14), (GroupKind::Folder, "ui/markdown".to_string(), 12)]
  );
}
