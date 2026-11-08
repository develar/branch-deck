use crate::ReviewPlan;
use crate::test_support::plan_of_range;
use crate::texts::{Entry, UiTextChange, entries};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A text of the plan: its path, key, change, values before and after, line, and the label of its module
type Text<'a> = (&'a str, &'a str, UiTextChange, Option<&'a str>, Option<&'a str>, u32, &'a str);

/// Each text of `plan`, in its order
fn texts(plan: &ReviewPlan) -> Vec<Text<'_>> {
  plan
    .texts
    .iter()
    .map(|text| {
      (
        text.path.as_str(),
        text.key.as_str(),
        text.change,
        text.before.as_deref(),
        text.after.as_deref(),
        text.line,
        text.module.map_or("", |module| plan.modules[module as usize].label.as_str()),
      )
    })
    .collect()
}

const BUNDLE: &str = "gui/resources/messages/GuiBundle.properties";

/// The keys a bundle adds, removes or changes are its texts, each with its values, its line and its module: a value
/// broken over another line or a key escaped is read as Java reads it; a comment, a blank line and a value broken
/// over other lines alone change no text
#[test]
fn the_keys_a_bundle_adds_removes_or_changes_are_its_texts() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add the GUI",
    &[
      ("gui/intellij.gui.iml", "<module/>\n"),
      (
        BUNDLE,
        "# The texts of the GUI\ngui.play=Play\ngui.pause=Pause\ngui.seek=Playback \\\n    position\ngui.stop = Stop\ngui.cancel=Cancel\n",
      ),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change the texts",
    &[(
      BUNDLE,
      "# The texts of the GUI, sorted\ngui.play=Play\ngui.pause=Pause playback\ngui.seek=Playback position\n\ngui.stop: Stop\ngui.download=Download\\u2026\ngui.hint=Click to return \\\n  to chat\n",
    )],
  );
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    texts(&plan),
    [
      (BUNDLE, "gui.pause", UiTextChange::Changed, Some("Pause"), Some("Pause playback"), 3, "gui"),
      (BUNDLE, "gui.cancel", UiTextChange::Removed, Some("Cancel"), None, 7, "gui"),
      (BUNDLE, "gui.download", UiTextChange::Added, None, Some("Download…"), 7, "gui"),
      (BUNDLE, "gui.hint", UiTextChange::Added, None, Some("Click to return to chat"), 8, "gui"),
    ]
  );
}

/// A bundle the commits add has every key added, in the module the commits add; a bundle by its name outside a
/// `messages` directory is one too, while a `gradle.properties` and the keys of a registry hold no text of the UI
#[test]
fn an_added_bundle_has_every_key_added_and_another_properties_file_none() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add the GUI",
    &[
      ("gradle.properties", "kotlin.code.style=official\n"),
      ("gui/intellij.gui.iml", "<module/>\n"),
      ("gui/resources/misc/registry.properties", "gui.media.enabled=false\n"),
      ("gui/resources/strings/PlayerMessages.properties", "player.title=Player\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Add the player",
    &[
      ("gradle.properties", "kotlin.code.style=official\norg.gradle.jvmargs=-Xmx2g\n"),
      ("gui/resources/misc/registry.properties", "gui.media.enabled=true\n"),
      ("gui/resources/strings/PlayerMessages.properties", "player.title=Media Player\n"),
      ("gui/media/intellij.gui.media.iml", "<module/>\n"),
      (
        "gui/media/resources/messages/Media.properties",
        "media.play=Play\n! the state of a clip\nmedia.cannot.play=The chat cannot play this file\n",
      ),
    ],
  );
  let plan = plan_of_range(&repo, &base, &head);
  let media = "gui/media/resources/messages/Media.properties";
  let player = "gui/resources/strings/PlayerMessages.properties";
  assert_eq!(
    texts(&plan),
    [
      (media, "media.play", UiTextChange::Added, None, Some("Play"), 1, "gui/media"),
      (
        media,
        "media.cannot.play",
        UiTextChange::Added,
        None,
        Some("The chat cannot play this file"),
        3,
        "gui/media"
      ),
      (player, "player.title", UiTextChange::Changed, Some("Player"), Some("Media Player"), 1, "gui"),
    ]
  );
}

/// A bundle is read before the first commit that changes it and after the last: a value changed and changed back is no
/// text, and a bundle changed and then deleted removes the keys it had before the first
#[test]
fn a_bundle_is_read_before_the_first_commit_and_after_the_last() {
  let repo = TestRepo::new();
  let old = "gui/resources/messages/OldBundle.properties";
  let base = repo.create_commit_with_files(
    "Add the GUI",
    &[
      ("gui/intellij.gui.iml", "<module/>\n"),
      (BUNDLE, "gui.play=Play\ngui.stop=Stop\n"),
      (old, "old.open=Open\n"),
    ],
  );
  repo.create_commit_with_files("Change the texts", &[(BUNDLE, "gui.play=Start\ngui.stop=Stop now\n"), (old, "old.open=Open…\n")]);
  repo.create_commit_with_files("Change a text back", &[(BUNDLE, "gui.play=Play\ngui.stop=Stop now\n")]);
  repo.git(&["rm", "-q", old]);
  let head = repo.commit_with(&["-m", "Delete the old texts"], &[]);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    texts(&plan),
    [
      (BUNDLE, "gui.stop", UiTextChange::Changed, Some("Stop"), Some("Stop now"), 2, "gui"),
      (old, "old.open", UiTextChange::Removed, Some("Open"), None, 1, "gui"),
    ]
  );
}

/// A bundle binary on a side has no text there, so no texts, never every key removed; another bundle of the commit has
/// its own
#[test]
fn a_bundle_binary_on_a_side_has_no_texts() {
  let repo = TestRepo::new();
  let binary = "gui/resources/messages/BinaryBundle.properties";
  let base = repo.create_commit_with_files(
    "Add the GUI",
    &[("gui/intellij.gui.iml", "<module/>\n"), (BUNDLE, "gui.play=Play\n"), (binary, "gui.open=Open\n")],
  );
  std::fs::write(repo.path().join(BUNDLE), "gui.play=Play the clip\n").unwrap();
  std::fs::write(repo.path().join(binary), b"gui.open=Open\0\n").unwrap();
  repo.git(&["add", "-A"]);
  let head = repo.commit_with(&["-m", "Change the texts"], &[]);
  let plan = plan_of_range(&repo, &base, &head);
  assert!(plan.files.iter().any(|facts| facts.path == binary), "{:?}", plan.files);
  assert_eq!(texts(&plan), [(BUNDLE, "gui.play", UiTextChange::Changed, Some("Play"), Some("Play the clip"), 1, "gui")]);
}

/// The entries of a properties text are read as `java.util.Properties::load` reads them: the three separators, the
/// whitespace around them, escaped separators in a key, comments that continue on no line, an escaped `\` that joins no
/// line, a surrogate pair, a `\u` without its digits as written, a key without a value, the last of a key given twice
/// and a `\` that ends the text
#[test]
fn the_entries_are_read_as_java_reads_a_properties_file() {
  let text = concat!(
    "a=1\n",
    "b : 2\r\n",
    "c 3\n",
    "  d\t=\t4  \n",
    "\\:e\\=f=5\n",
    "! a comment\n",
    "# a comment that ends with \\\n",
    "g=6\n",
    "h=7\\\\\n",
    "i=8\n",
    "emoji=\\uD83D\\uDE00 \\u00e9\n",
    "bad=\\u12z\n",
    "empty\n",
    "a=last\n",
    "end=the end\\",
  );
  let found = entries(text);
  let entry = |key: &str| found.get(key).map(|Entry { value, line }| (value.as_str(), *line));
  let keys = ["a", "b", "c", "d", ":e=f", "g", "h", "i", "emoji", "bad", "empty", "end"];
  assert_eq!(
    keys.map(entry),
    [
      Some(("last", 14)),
      Some(("2", 2)),
      Some(("3", 3)),
      Some(("4  ", 4)),
      Some(("5", 5)),
      Some(("6", 8)),
      Some(("7\\", 9)),
      Some(("8", 10)),
      Some(("😀 é", 11)),
      Some(("\\u12z", 12)),
      Some(("", 13)),
      Some(("the end", 15)),
    ]
  );
  assert_eq!(found.len(), keys.len());
}
