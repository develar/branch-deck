//! The history of the `review_modules` template (`repo_template::templates::review_modules`): a large step of modules
//! (spec/branch/review-tour.spec.md, "Groups of a Step"), the branch `media` of a plugin `player` of the shape of an
//! IntelliJ plugin, in two commits with bodies after the baseline. The first gives an embed of the module `view.gui`
//! the extension of its file; the second adds the module `view.gui.media`:
//!
//! - its `*.iml`, its `BUILD.bazel` and its content module descriptor with a service and a registry key, the content
//!   entry in `plugin.xml`, its line of `.idea/modules.xml` and of the generated `build/module-list.txt`;
//! - nine classes in two packages that use the classes of `view.gui` the same commit changes, and two tests;
//! - five other changed classes of `view.gui`, a changed test and four new texts of its bundle;
//! - a function of `shared/code` it opens wider (`internal fun` to `fun`) and uses;
//! - a spec with its front matter and a table, whose requirements, `targets:` and rows it adds to, and a sentence of
//!   `AGENTS.md` that names the new module in code.
//!
//! Its plan cuts the second step into Contract, `view.gui`, `view.gui.media` (also `shared/code`) and the Wiring.

use std::fmt::Write;

/// A commit of the history: its message and the files it writes, each by its path and text
pub struct ModulesCommit {
  pub message: &'static str,
  pub files: Vec<(String, String)>,
}

/// The directory of the plugin
const ROOT: &str = "plugins/player";

/// The commits of the template, the baseline first
pub fn review_modules_commits() -> Vec<ModulesCommit> {
  vec![
    ModulesCommit {
      message: "Initial project setup",
      files: base(),
    },
    ModulesCommit {
      message: "(media) the embed names the extension of its file\n\nAn embed knows the extension of its file, so a module can tell a video from an image before it reads a byte.",
      files: vec![(gui("embeds/EmbedSource.kt"), embed_source(EXTENSION))],
    },
    ModulesCommit {
      message: "(media) play the media embeds of the chat inline\n\nThe chat plays a video or a sound in its embed through a player of its own, the new module \
                `view.gui.media`: `MediaEmbeds` registers a view of each media kind in `EmbedViews` of `view.gui`, and `MediaPlayer` \
                decodes the file through AVFoundation on macOS.\n\nInline playback is a feature of nightly builds (`MediaPlaybackFlag`), which a \
                registry key turns off.\n\nLook closely: a file the decoder cannot read stays a file embed, never an empty frame.",
      files: change(),
    },
  ]
}

/// The extension an embed of the first commit knows
const EXTENSION: &str = "\n  val extension: String\n    get() = name.substringAfterLast('.', \"\")\n";

fn base() -> Vec<(String, String)> {
  vec![
    (".idea/modules.xml".to_string(), modules_xml("")),
    ("build/module-list.txt".to_string(), module_list("")),
    (format!("{ROOT}/AGENTS.md"), agents("")),
    (format!("{ROOT}/spec/transcript-embeds.spec.md"), spec("", "", "")),
    (
      format!("{ROOT}/plugin/BUILD.bazel"),
      bazel("intellij.player.plugin", "plugin", &["//plugins/player/view.gui", "//plugins/player/shared/code"]),
    ),
    (
      format!("{ROOT}/plugin/intellij.player.plugin.iml"),
      iml(&sources(true, false), &["intellij.player.view.gui", "intellij.player.shared.code"]),
    ),
    (format!("{ROOT}/plugin/resources/META-INF/plugin.xml"), plugin_xml("")),
    (format!("{ROOT}/shared/code/BUILD.bazel"), bazel("intellij.player.shared.code", "code", &[])),
    (format!("{ROOT}/shared/code/intellij.player.shared.code.iml"), iml(&sources(false, false), &[])),
    (format!("{ROOT}/shared/code/src/com/example/player/shared/NightlyBuild.kt"), nightly("internal ", "")),
    (
      format!("{ROOT}/view.gui/BUILD.bazel"),
      bazel("intellij.player.view.gui", "gui", &["//plugins/player/shared/code"]),
    ),
    (
      format!("{ROOT}/view.gui/intellij.player.view.gui.iml"),
      iml(&sources(true, true), &["intellij.player.shared.code"]),
    ),
    (format!("{ROOT}/view.gui/resources/messages/PlayerBundle.properties"), bundle("")),
    (gui("embeds/EmbedKind.kt"), embed_kind("")),
    (gui("embeds/EmbedModel.kt"), embed_model("")),
    (gui("embeds/EmbedSource.kt"), embed_source("")),
    (gui("embeds/EmbedView.kt"), embed_view("JLabel(model.title)")),
    (gui("embeds/EmbedViews.kt"), embed_views("")),
    (gui("markdown/MarkdownEmbeds.kt"), markdown("")),
    (format!("{ROOT}/view.gui/testSrc/com/example/player/gui/embeds/EmbedModelTest.kt"), gui_test("")),
  ]
}

fn change() -> Vec<(String, String)> {
  let media_row = "      <module fileurl=\"file://$PROJECT_DIR$/plugins/player/view.gui.media/intellij.player.view.gui.media.iml\" filepath=\"$PROJECT_DIR$/plugins/player/view.gui.media/intellij.player.view.gui.media.iml\" />\n";
  let mut files = vec![
    (".idea/modules.xml".to_string(), modules_xml(media_row)),
    ("build/module-list.txt".to_string(), module_list("plugins/player/view.gui.media\n")),
    (format!("{ROOT}/AGENTS.md"), agents(", the inline media player of its embeds in `view.gui.media`")),
    (
      format!("{ROOT}/spec/transcript-embeds.spec.md"),
      spec(
        "  - ../view.gui.media/src/com/example/player/media/MediaEmbedView.kt\n",
        "- A video or a sound must play inline in its embed, its controls under its frame, while inline playback is on\n  (nightly builds only).\n- A file the player cannot decode must stay a file embed, with \"The chat cannot play this file\" in its hover.\n",
        "| Video | mp4, mov | view.gui.media |\n| Sound | mp3, m4a | view.gui.media |\n",
      ),
    ),
    (
      format!("{ROOT}/plugin/resources/META-INF/plugin.xml"),
      plugin_xml("    <module name=\"intellij.player.view.gui.media\"/>\n"),
    ),
    (
      format!("{ROOT}/shared/code/src/com/example/player/shared/NightlyBuild.kt"),
      nightly("", ", val defaultEnabled: Boolean = false"),
    ),
    (
      format!("{ROOT}/view.gui/resources/messages/PlayerBundle.properties"),
      bundle(
        "embed.video.accessible.name=Video: {0}\nembed.audio.accessible.name=Sound: {0}\nembed.media.cannot.play=The chat cannot play this file\nembed.media.download=Download…\n",
      ),
    ),
    (gui("embeds/EmbedKind.kt"), embed_kind("  VIDEO,\n  AUDIO,\n")),
    (
      gui("embeds/EmbedModel.kt"),
      embed_model("\n  /** A video or a sound: the media module plays it inline */\n  val playable: Boolean\n    get() = kind == EmbedKind.VIDEO || kind == EmbedKind.AUDIO\n"),
    ),
    (gui("embeds/EmbedView.kt"), embed_view("EmbedViews.create(model)")),
    (
      gui("embeds/EmbedViews.kt"),
      embed_views("\n  /** Whether a module registered the view of `kind` */\n  fun supports(kind: EmbedKind): Boolean = kind in factories\n"),
    ),
    (
      gui("markdown/MarkdownEmbeds.kt"),
      markdown("    \"mp4\", \"mov\" -> EmbedKind.VIDEO\n    \"mp3\", \"m4a\" -> EmbedKind.AUDIO\n"),
    ),
    (
      format!("{ROOT}/view.gui/testSrc/com/example/player/gui/embeds/EmbedModelTest.kt"),
      gui_test("\n  @Test\n  fun `a video plays`() {\n    assertEquals(true, EmbedModel(EmbedSource(Path.of(\"/tmp/cat.mp4\")), EmbedKind.VIDEO).playable)\n  }\n"),
    ),
    (
      format!("{ROOT}/view.gui.media/BUILD.bazel"),
      bazel("intellij.player.view.gui.media", "media", &["//plugins/player/view.gui", "//plugins/player/shared/code"]),
    ),
    (
      format!("{ROOT}/view.gui.media/intellij.player.view.gui.media.iml"),
      iml(&sources(true, true), &["intellij.player.view.gui", "intellij.player.shared.code"]),
    ),
    (format!("{ROOT}/view.gui.media/resources/intellij.player.view.gui.media.xml"), MEDIA_DESCRIPTOR.to_string()),
  ];
  files.extend(media_classes());
  files.extend(media_tests());
  files
}

const MEDIA_DESCRIPTOR: &str = "<idea-plugin package=\"com.example.player.media\">\n  <dependencies>\n    <module name=\"intellij.player.view.gui\"/>\n    <module name=\"intellij.player.shared.code\"/>\n  </dependencies>\n  <extensions defaultExtensionNs=\"com.intellij\">\n    <applicationService serviceInterface=\"com.example.player.media.MediaDecoder\" serviceImplementation=\"com.example.player.media.avfoundation.AvFoundationDecoder\"/>\n    <registryKey key=\"player.media.inline.playback\" defaultValue=\"true\" description=\"Play the media embeds of the chat inline\"/>\n  </extensions>\n</idea-plugin>\n";

/// The classes of the new module: seven in its package, two in `avfoundation`
fn media_classes() -> Vec<(String, String)> {
  let class = |file: &str, imports: &str, body: &str| {
    let package = if file.contains('/') {
      "com.example.player.media.avfoundation"
    } else {
      "com.example.player.media"
    };
    (
      format!("{ROOT}/view.gui.media/src/com/example/player/media/{file}"),
      format!("package {package}\n\n{imports}\n{body}"),
    )
  };
  vec![
    class(
      "MediaEmbedView.kt",
      "import com.example.player.gui.embeds.EmbedModel\nimport javax.swing.JComponent\nimport javax.swing.JPanel\n",
      "/** The embed of a video or a sound: its frame and its controls, which play it inline */\nclass MediaEmbedView(private val model: EmbedModel, private val player: MediaPlayer) {\n  fun component(): JComponent = JPanel().apply {\n    add(MediaControls(player).component())\n    toolTipText = model.title\n  }\n}\n",
    ),
    class(
      "MediaPlayer.kt",
      "import com.example.player.gui.embeds.EmbedSource\n",
      "/** Plays one media file: a decoder for its bytes, the timeline of its position */\nclass MediaPlayer(private val source: EmbedSource, private val decoder: MediaDecoder) {\n  val timeline = MediaTimeline(decoder.durationOf(source))\n\n  fun play() = decoder.start(source, timeline)\n\n  fun pause() = decoder.stop()\n}\n",
    ),
    class(
      "MediaControls.kt",
      "import javax.swing.JButton\nimport javax.swing.JComponent\n",
      "/** Play, pause and the position of a player */\nclass MediaControls(private val player: MediaPlayer) {\n  fun component(): JComponent = JButton(\"Play\").apply { addActionListener { player.play() } }\n}\n",
    ),
    class(
      "MediaTimeline.kt",
      "",
      "/** The position of a player in its file, in milliseconds */\nclass MediaTimeline(val durationMs: Long) {\n  var positionMs: Long = 0\n    private set\n\n  fun seek(to: Long) {\n    positionMs = to.coerceIn(0, durationMs)\n  }\n}\n",
    ),
    class(
      "MediaDecoder.kt",
      "import com.example.player.gui.embeds.EmbedSource\n",
      "/** Decodes the frames of a media file on the platform */\ninterface MediaDecoder {\n  fun durationOf(source: EmbedSource): Long\n\n  fun start(source: EmbedSource, timeline: MediaTimeline)\n\n  fun stop()\n}\n",
    ),
    class(
      "MediaEmbeds.kt",
      "import com.example.player.gui.embeds.EmbedKind\nimport com.example.player.gui.embeds.EmbedViews\nimport com.example.player.media.avfoundation.AvFoundationDecoder\n",
      "/** Registers the views of the media embeds while inline playback is on */\nobject MediaEmbeds {\n  fun register() {\n    if (!MediaPlaybackFlag.enabled) return\n    for (kind in listOf(EmbedKind.VIDEO, EmbedKind.AUDIO)) {\n      EmbedViews.register(kind) { model -> MediaEmbedView(model, MediaPlayer(model.source, AvFoundationDecoder())).component() }\n    }\n  }\n}\n",
    ),
    class(
      "MediaPlaybackFlag.kt",
      "import com.example.player.shared.NightlyFeature\nimport com.example.player.shared.isNightlyBuild\n",
      "/** Inline playback of the chat: nightly builds only, unless the registry key turns it off */\nobject MediaPlaybackFlag {\n  private val feature = NightlyFeature(\"player.media.inline.playback\", defaultEnabled = true)\n\n  val enabled: Boolean\n    get() = isNightlyBuild() && feature.enabled\n}\n",
    ),
    class(
      "avfoundation/AvFoundationDecoder.kt",
      "import com.example.player.gui.embeds.EmbedSource\nimport com.example.player.media.MediaDecoder\nimport com.example.player.media.MediaTimeline\n",
      "/** The decoder of macOS: AVFoundation through its bridge */\nclass AvFoundationDecoder : MediaDecoder {\n  override fun durationOf(source: EmbedSource): Long = AvFoundationBridge.duration(source.path.toString())\n\n  override fun start(source: EmbedSource, timeline: MediaTimeline) = AvFoundationBridge.play(source.path.toString(), timeline.positionMs)\n\n  override fun stop() = AvFoundationBridge.stop()\n}\n",
    ),
    class(
      "avfoundation/AvFoundationBridge.kt",
      "",
      "/** The native calls of AVFoundation */\ninternal object AvFoundationBridge {\n  external fun duration(path: String): Long\n\n  external fun play(path: String, fromMs: Long)\n\n  external fun stop()\n}\n",
    ),
  ]
}

fn media_tests() -> Vec<(String, String)> {
  let test = |name: &str, body: &str| {
    (
      format!("{ROOT}/view.gui.media/testSrc/com/example/player/media/{name}.kt"),
      format!("package com.example.player.media\n\nimport org.junit.jupiter.api.Assertions.assertEquals\nimport org.junit.jupiter.api.Test\n\nclass {name} {{\n{body}}}\n"),
    )
  };
  vec![
    test(
      "MediaTimelineTest",
      "  @Test\n  fun `a seek stays inside the file`() {\n    val timeline = MediaTimeline(1000)\n    timeline.seek(5000)\n    assertEquals(1000, timeline.positionMs)\n  }\n",
    ),
    test(
      "MediaPlayerTest",
      "  @Test\n  fun `a player starts at the start of its file`() {\n    assertEquals(0, MediaTimeline(1000).positionMs)\n  }\n",
    ),
  ]
}

fn gui(file: &str) -> String {
  format!("{ROOT}/view.gui/src/com/example/player/gui/{file}")
}

fn iml(sources: &str, modules: &[&str]) -> String {
  let entries = modules.iter().fold(String::new(), |mut entries, module| {
    let _ = writeln!(entries, "    <orderEntry type=\"module\" module-name=\"{module}\" />");
    entries
  });
  format!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<module type=\"JAVA_MODULE\" version=\"4\">\n  <component name=\"NewModuleRootManager\" inherit-compiler-output=\"true\">\n    <exclude-output />\n    <content url=\"file://$MODULE_DIR$\">\n{sources}    </content>\n    <orderEntry type=\"inheritedJdk\" />\n    <orderEntry type=\"sourceFolder\" forTests=\"false\" />\n    <orderEntry type=\"library\" name=\"kotlin-stdlib\" level=\"project\" />\n{entries}  </component>\n</module>\n"
  )
}

fn sources(resources: bool, tests: bool) -> String {
  let mut folders = String::from("      <sourceFolder url=\"file://$MODULE_DIR$/src\" isTestSource=\"false\" />\n");
  if resources {
    folders.push_str("      <sourceFolder url=\"file://$MODULE_DIR$/resources\" type=\"java-resource\" />\n");
  }
  if tests {
    folders.push_str("      <sourceFolder url=\"file://$MODULE_DIR$/testSrc\" isTestSource=\"true\" />\n");
  }
  folders
}

fn bazel(module: &str, name: &str, deps: &[&str]) -> String {
  let deps = deps.iter().fold(String::new(), |mut list, dep| {
    let _ = writeln!(list, "    \"{dep}\",");
    list
  });
  format!(
    "### auto-generated section `build {module}` start\nload(\"@rules_jvm//:jvm.bzl\", \"jvm_library\", \"jvm_test\")\n\njvm_library(\n  name = \"{name}\",\n  module_name = \"{module}\",\n  srcs = glob([\"src/**/*.kt\"], allow_empty = True),\n  resources = glob([\"resources/**/*\"], allow_empty = True),\n  deps = [\n    \"@lib//:kotlin-stdlib\",\n{deps}  ],\n)\n\njvm_test(\n  name = \"{name}_test\",\n  srcs = glob([\"testSrc/**/*.kt\"], allow_empty = True),\n  associates = [\":{name}\"],\n)\n### auto-generated section `build {module}` end\n"
  )
}

fn plugin_xml(media: &str) -> String {
  format!(
    "<idea-plugin>\n  <id>com.example.player</id>\n  <name>Player</name>\n  <vendor>Example</vendor>\n\n  <content>\n    <module name=\"intellij.player.shared.code\"/>\n    <module name=\"intellij.player.view.gui\"/>\n{media}  </content>\n</idea-plugin>\n"
  )
}

fn modules_xml(media: &str) -> String {
  let row = |root: &str, name: &str| format!("      <module fileurl=\"file://$PROJECT_DIR$/{root}/{name}.iml\" filepath=\"$PROJECT_DIR$/{root}/{name}.iml\" />\n");
  format!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project version=\"4\">\n  <component name=\"ProjectModuleManager\">\n    <modules>\n{}{}{}{media}    </modules>\n  </component>\n</project>\n",
    row("plugins/player/plugin", "intellij.player.plugin"),
    row("plugins/player/shared/code", "intellij.player.shared.code"),
    row("plugins/player/view.gui", "intellij.player.view.gui"),
  )
}

fn module_list(media: &str) -> String {
  format!("plugins/player/plugin\nplugins/player/shared/code\nplugins/player/view.gui\n{media}")
}

/// The shared code: the function the new module reads, `internal` before the change
fn nightly(reach: &str, default: &str) -> String {
  format!(
    "package com.example.player.shared\n\n/** Whether this is a nightly build: a feature still under the question shows only there */\n{reach}fun isNightlyBuild(): Boolean = System.getProperty(\"player.build.nightly\") == \"true\"\n\n/** A feature of nightly builds that a registry key turns off */\nclass NightlyFeature(val key: String{default}) {{\n  val enabled: Boolean\n    get() = isNightlyBuild() && System.getProperty(key, \"\") != \"false\"\n}}\n"
  )
}

fn embed_kind(more: &str) -> String {
  format!("package com.example.player.gui.embeds\n\n/** What an embed of the chat shows */\nenum class EmbedKind {{\n  IMAGE,\n  FILE,\n{more}}}\n")
}

fn embed_model(more: &str) -> String {
  format!(
    "package com.example.player.gui.embeds\n\n/** The state of one embed of the transcript */\nclass EmbedModel(val source: EmbedSource, val kind: EmbedKind) {{\n  val title: String\n    get() = source.name\n{more}}}\n"
  )
}

fn embed_source(more: &str) -> String {
  format!(
    "package com.example.player.gui.embeds\n\nimport java.nio.file.Path\n\n/** Where the bytes of an embed come from */\nclass EmbedSource(val path: Path) {{\n  val name: String\n    get() = path.fileName.toString()\n{more}}}\n"
  )
}

fn embed_view(body: &str) -> String {
  format!(
    "package com.example.player.gui.embeds\n\nimport javax.swing.JComponent\nimport javax.swing.JLabel\n\n/** Paints one embed in the transcript */\nclass EmbedView(private val model: EmbedModel) {{\n  fun component(): JComponent = {body}\n}}\n"
  )
}

fn embed_views(more: &str) -> String {
  format!(
    "package com.example.player.gui.embeds\n\nimport javax.swing.JComponent\n\n/** Makes the view of an embed of each kind; a module may add the views of more kinds */\nobject EmbedViews {{\n  private val factories = mutableMapOf<EmbedKind, (EmbedModel) -> JComponent>()\n\n  fun register(kind: EmbedKind, factory: (EmbedModel) -> JComponent) {{\n    factories[kind] = factory\n  }}\n\n  fun create(model: EmbedModel): JComponent = factories[model.kind]?.invoke(model) ?: EmbedView(model).component()\n{more}}}\n"
  )
}

fn markdown(kinds: &str) -> String {
  format!(
    "package com.example.player.gui.markdown\n\nimport com.example.player.gui.embeds.EmbedKind\n\n/** The embeds a Markdown block of the chat may hold, by the extension of their file */\nobject MarkdownEmbeds {{\n  fun kindOf(extension: String): EmbedKind? = when (extension.lowercase()) {{\n    \"png\", \"jpg\" -> EmbedKind.IMAGE\n{kinds}    else -> null\n  }}\n}}\n"
  )
}

fn gui_test(more: &str) -> String {
  format!(
    "package com.example.player.gui.embeds\n\nimport java.nio.file.Path\nimport org.junit.jupiter.api.Assertions.assertEquals\nimport org.junit.jupiter.api.Test\n\nclass EmbedModelTest {{\n  @Test\n  fun `an embed is titled by its file`() {{\n    assertEquals(\"cat.png\", EmbedModel(EmbedSource(Path.of(\"/tmp/cat.png\")), EmbedKind.IMAGE).title)\n  }}\n{more}}}\n"
  )
}

fn bundle(more: &str) -> String {
  format!("embed.image.accessible.name=Image: {{0}}\nembed.file.accessible.name=File: {{0}}\nembed.open=Open\n{more}")
}

fn spec(targets: &str, requirements: &str, rows: &str) -> String {
  format!(
    "---\nname: Transcript Embeds\ndescription: Requirements for the embeds of the chat transcript, the images and files of the GUI and the media a module adds.\ntargets:\n  - ../view.gui/src/com/example/player/gui/embeds/EmbedView.kt\n{targets}---\n\n# Transcript Embeds\n\n## Requirements\n\n- An embed must show the name of its file, its whole path in its hover.\n- An image must paint in its embed at its own size, at most the width of the transcript.\n{requirements}\n## Kinds\n\n| Kind | Extensions | Module |\n| --- | --- | --- |\n| Image | png, jpg | view.gui |\n| File | any | view.gui |\n{rows}"
  )
}

fn agents(modules: &str) -> String {
  format!(
    "# Player\n\nThe player plugin shows the media of the chat transcript.\n\n## Modules\n\nThe session GUI lives in `view.gui`{modules}; `shared/code` holds what the frontend and the backend both read.\nEach module keeps its tests in `testSrc` beside its sources.\n"
  )
}
