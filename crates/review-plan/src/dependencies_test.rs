use crate::ReviewPlan;
use crate::dependencies::{DependencyChange, DependencyChangeKind, DependencyKind, DependencyScope};
use crate::files::MechanicalKind;
use crate::test_support::{branch, mark, plan_of, range};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A change of `name` of `kind` in `manifest` of `module`, in the main scope with no version
fn change(manifest: &str, module: &str, kind: DependencyKind, name: &str, change: DependencyChangeKind) -> DependencyChange {
  DependencyChange {
    manifest: manifest.to_string(),
    module: module.to_string(),
    kind,
    name: name.to_string(),
    change,
    version_before: None,
    version_after: None,
    scope: DependencyScope::Main,
    scope_before: None,
  }
}

/// The plan of a repository whose base has `before` and whose head writes `after` (a `None` text deletes the file)
fn plan_of_change(before: &[(&str, &str)], after: &[(&str, Option<&str>)]) -> ReviewPlan {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add files", before);
  for (path, text) in after {
    let file = repo.path().join(path);
    match text {
      Some(text) => {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
      }
      None => {
        repo.git(&["rm", "-q", path]);
      }
    }
  }
  repo.git(&["add", "-A"]);
  let head = repo.commit_with(&["-m", "Change files"], &[]);
  plan_of(&repo, None, &range(&repo, &base, &head)).unwrap()
}

/// The mechanical kind of each file of `plan`, by path
fn mechanical(plan: &ReviewPlan) -> Vec<(&str, Option<MechanicalKind>)> {
  let mut kinds: Vec<(&str, Option<MechanicalKind>)> = plan.files.iter().map(|facts| (facts.path.as_str(), facts.mechanical)).collect();
  kinds.sort_unstable_by_key(|(path, _)| *path);
  kinds
}

const IML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<module type="JAVA_MODULE" version="4">
  <component name="NewModuleRootManager" inherit-compiler-output="true">
    <exclude-output />
    <content url="file://$MODULE_DIR$">
      <sourceFolder url="file://$MODULE_DIR$/src" isTestSource="false" />
    </content>
    <orderEntry type="inheritedJdk" />
    <orderEntry type="sourceFolder" forTests="false" />
    <orderEntry type="module" module-name="intellij.platform.core" />
    <orderEntry type="module" module-name="intellij.platform.old" />
    <orderEntry type="library" name="kotlin-stdlib" level="project" />
    <orderEntry type="library" scope="TEST" name="JUnit5" level="project" />
  </component>
</module>"#;

/// An `*.iml` that adds and removes a module entry and changes the scope of a library: each entry once, the scope as
/// one `changed` entry, and the file of the build model. An `*.iml` that does not parse after the commits has no
/// change and stays in the tree
#[test]
fn the_entries_of_an_iml_are_its_dependency_changes() {
  let after = IML
    .replace(
      r#"    <orderEntry type="module" module-name="intellij.platform.old" />"#,
      r#"    <orderEntry type="module" module-name="intellij.platform.x" />"#,
    )
    .replace(r#"name="kotlin-stdlib" level="project""#, r#"scope="PROVIDED" name="kotlin-stdlib" level="project""#);
  let broken = IML.replace("</module>", "").replace("intellij.platform.old", "intellij.platform.new");
  let plan = plan_of_change(
    &[("platform/foo/intellij.platform.foo.iml", IML), ("platform/bar/intellij.platform.bar.iml", IML)],
    &[
      ("platform/foo/intellij.platform.foo.iml", Some(&after)),
      ("platform/bar/intellij.platform.bar.iml", Some(&broken)),
    ],
  );
  let manifest = "platform/foo/intellij.platform.foo.iml";
  let module = "intellij.platform.foo";
  assert_eq!(
    plan.dependencies,
    [
      change(manifest, module, DependencyKind::Module, "intellij.platform.old", DependencyChangeKind::Removed),
      change(manifest, module, DependencyKind::Module, "intellij.platform.x", DependencyChangeKind::Added),
      DependencyChange {
        scope: DependencyScope::Provided,
        scope_before: Some(DependencyScope::Main),
        ..change(manifest, module, DependencyKind::Library, "kotlin-stdlib", DependencyChangeKind::Changed)
      },
    ]
  );
  assert_eq!(
    mechanical(&plan),
    [
      ("platform/bar/intellij.platform.bar.iml", None),
      ("platform/foo/intellij.platform.foo.iml", Some(MechanicalKind::BuildModel))
    ]
  );
}

const CARGO_A: &str =
  "[package]\nname = \"a\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1.0.1\"\ntokio = { version = \"1\", features = [\"rt\"] }\ngone = { path = \"../gone\" }\n";
const CARGO_B: &str = "[package]\nname = \"b\"\nedition = \"2021\"\n\n[dependencies]\nserde = \"1\"\n";

/// A `Cargo.toml` whose new version, new scope, new target entry and removal are its only change is of the build
/// model, the version and the scope each one `changed` entry. One whose edition changes too, one whose features of an
/// entry change, one that does not parse, and a new crate stay in the tree; a deleted crate is `Deleted`; a reformat
/// with no other change is `Whitespace`
#[test]
fn the_dependency_tables_of_a_cargo_toml_are_its_dependency_changes() {
  let a = "[package]\nname = \"a\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1.0.2\"\n\n[dev-dependencies]\ntokio = { version = \"1\", features = [\"rt\"] }\n\n[target.'cfg(windows)'.dependencies]\nwinapi = \"0.3\"\n";
  let b = CARGO_B.replace("2021", "2024").replace("serde = \"1\"\n", "serde = \"1\"\nlog = \"0.4\"\n");
  let c = CARGO_A.replace("name = \"a\"", "name = \"c\"").replace("[\"rt\"]", "[\"rt\", \"net\"]");
  let broken = CARGO_B.replace("[dependencies]", "[dependencies").replace("name = \"b\"", "name = \"broken\"");
  let reformat = CARGO_B.replace("name = \"b\"", "name = \"fmt\"").replace("serde = \"1\"", "serde=\"1\"");
  let plan = plan_of_change(
    &[
      ("crates/a/Cargo.toml", CARGO_A),
      ("crates/b/Cargo.toml", CARGO_B),
      ("crates/c/Cargo.toml", &CARGO_A.replace("name = \"a\"", "name = \"c\"")),
      ("crates/broken/Cargo.toml", &CARGO_B.replace("name = \"b\"", "name = \"broken\"")),
      ("crates/fmt/Cargo.toml", &CARGO_B.replace("name = \"b\"", "name = \"fmt\"")),
      ("crates/gone/Cargo.toml", &CARGO_B.replace("name = \"b\"", "name = \"gone\"")),
    ],
    &[
      ("crates/a/Cargo.toml", Some(a)),
      ("crates/b/Cargo.toml", Some(&b)),
      ("crates/c/Cargo.toml", Some(&c)),
      ("crates/broken/Cargo.toml", Some(&broken.replace("serde = \"1\"", "serde = \"2\""))),
      ("crates/fmt/Cargo.toml", Some(&reformat)),
      ("crates/gone/Cargo.toml", None),
      // unlike the deleted one, so that git sees no rename
      (
        "crates/new/Cargo.toml",
        Some("[package]\nname = \"new\"\nversion = \"0.1.0\"\nlicense = \"MIT\"\npublish = false\n\n[lints]\nworkspace = true\n"),
      ),
    ],
  );
  let a = "crates/a/Cargo.toml";
  let version = |before: Option<&str>, after: Option<&str>, change: DependencyChange| DependencyChange {
    version_before: before.map(str::to_string),
    version_after: after.map(str::to_string),
    ..change
  };
  assert_eq!(
    plan.dependencies,
    [
      change(a, "a", DependencyKind::Crate, "gone", DependencyChangeKind::Removed),
      version(Some("1.0.1"), Some("1.0.2"), change(a, "a", DependencyKind::Crate, "serde", DependencyChangeKind::Changed)),
      DependencyChange {
        scope: DependencyScope::Dev,
        scope_before: Some(DependencyScope::Main),
        ..version(Some("1"), Some("1"), change(a, "a", DependencyKind::Crate, "tokio", DependencyChangeKind::Changed))
      },
      version(None, Some("0.3"), change(a, "a", DependencyKind::Crate, "winapi", DependencyChangeKind::Added)),
      version(
        None,
        Some("0.4"),
        change("crates/b/Cargo.toml", "b", DependencyKind::Crate, "log", DependencyChangeKind::Added)
      ),
      change("crates/gone/Cargo.toml", "gone", DependencyKind::NewModule, "gone", DependencyChangeKind::Removed),
      change("crates/new/Cargo.toml", "new", DependencyKind::NewModule, "new", DependencyChangeKind::Added),
    ]
  );
  assert_eq!(
    mechanical(&plan),
    [
      ("crates/a/Cargo.toml", Some(MechanicalKind::BuildModel)),
      ("crates/b/Cargo.toml", None),
      ("crates/broken/Cargo.toml", None),
      ("crates/c/Cargo.toml", None),
      ("crates/fmt/Cargo.toml", Some(MechanicalKind::Whitespace)),
      ("crates/gone/Cargo.toml", Some(MechanicalKind::Deleted)),
      ("crates/new/Cargo.toml", None),
    ]
  );
}

const WORKSPACE: &str = "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\nserde = \"1.0.1\"\n";

/// A new version together with new features of the same entry is a `changed` entry, and the `Cargo.toml` stays in
/// the tree. A workspace `Cargo.toml` and a `package.json` at the root without a name are the module `(root)`; a new
/// version of a workspace dependency alone is of the build model
#[test]
fn a_new_version_with_new_features_keeps_the_manifest_in_the_tree() {
  let tokio = CARGO_A.replace(
    "tokio = { version = \"1\", features = [\"rt\"] }",
    "tokio = { version = \"2\", features = [\"rt\", \"net\"] }",
  );
  let plan = plan_of_change(
    &[
      ("Cargo.toml", WORKSPACE),
      ("package.json", "{\n  \"private\": true,\n  \"devDependencies\": { \"vite\": \"^6.0.0\" }\n}\n"),
      ("crates/a/Cargo.toml", CARGO_A),
    ],
    &[
      ("Cargo.toml", Some(&WORKSPACE.replace("1.0.1", "1.0.2"))),
      ("package.json", Some("{\n  \"private\": true,\n  \"devDependencies\": { \"vite\": \"^7.0.0\" }\n}\n")),
      ("crates/a/Cargo.toml", Some(&tokio)),
    ],
  );
  let version = |before: &str, after: &str, scope: DependencyScope, change: DependencyChange| DependencyChange {
    version_before: Some(before.to_string()),
    version_after: Some(after.to_string()),
    scope,
    ..change
  };
  assert_eq!(
    plan.dependencies,
    [
      version(
        "1",
        "2",
        DependencyScope::Main,
        change("crates/a/Cargo.toml", "a", DependencyKind::Crate, "tokio", DependencyChangeKind::Changed)
      ),
      version(
        "1.0.1",
        "1.0.2",
        DependencyScope::Main,
        change("Cargo.toml", "(root)", DependencyKind::Crate, "serde", DependencyChangeKind::Changed)
      ),
      version(
        "^6.0.0",
        "^7.0.0",
        DependencyScope::Dev,
        change("package.json", "(root)", DependencyKind::Package, "vite", DependencyChangeKind::Changed)
      ),
    ]
  );
  assert_eq!(
    mechanical(&plan),
    [
      ("Cargo.toml", Some(MechanicalKind::BuildModel)),
      ("crates/a/Cargo.toml", None),
      ("package.json", Some(MechanicalKind::BuildModel)),
    ]
  );
}

const PACKAGE_JSON: &str = r#"{
  "name": "web",
  "version": "1.0.0",
  "dependencies": {
    "react": "^18.0.0"
  }
}
"#;

/// A `package.json` whose new devDependency, new peer dependency and new version are its only change is of the build
/// model; one whose scripts change too stays in the tree, named by its directory without a `name`
#[test]
fn the_dependencies_of_a_package_json_are_its_dependency_changes() {
  let web = r#"{
  "name": "web",
  "version": "1.0.0",
  "dependencies": {
    "react": "^19.0.0"
  },
  "devDependencies": {
    "vitest": "^2.0.0"
  },
  "peerDependencies": {
    "react-dom": "^19.0.0"
  }
}
"#;
  let tools = "{\n  \"scripts\": { \"build\": \"vite\" },\n  \"dependencies\": { \"vite\": \"^6.0.0\" }\n}\n";
  let plan = plan_of_change(
    &[("web/package.json", PACKAGE_JSON), ("tools/package.json", "{\n  \"scripts\": { \"build\": \"tsc\" }\n}\n")],
    &[("web/package.json", Some(web)), ("tools/package.json", Some(tools))],
  );
  let version = |before: Option<&str>, after: Option<&str>, scope: DependencyScope, change: DependencyChange| DependencyChange {
    version_before: before.map(str::to_string),
    version_after: after.map(str::to_string),
    scope,
    ..change
  };
  let web = "web/package.json";
  assert_eq!(
    plan.dependencies,
    [
      version(
        None,
        Some("^6.0.0"),
        DependencyScope::Main,
        change("tools/package.json", "tools", DependencyKind::Package, "vite", DependencyChangeKind::Added)
      ),
      version(
        Some("^18.0.0"),
        Some("^19.0.0"),
        DependencyScope::Main,
        change(web, "web", DependencyKind::Package, "react", DependencyChangeKind::Changed)
      ),
      version(
        None,
        Some("^19.0.0"),
        DependencyScope::Peer,
        change(web, "web", DependencyKind::Package, "react-dom", DependencyChangeKind::Added)
      ),
      version(
        None,
        Some("^2.0.0"),
        DependencyScope::Dev,
        change(web, "web", DependencyKind::Package, "vitest", DependencyChangeKind::Added)
      ),
    ]
  );
  assert_eq!(mechanical(&plan), [("tools/package.json", None), ("web/package.json", Some(MechanicalKind::BuildModel))]);
}

const PLUGIN_XML: &str = r#"<idea-plugin>
  <id>com.example.x</id>
  <depends>com.intellij.modules.platform</depends>
  <dependencies>
    <plugin id="com.intellij.java"/>
  </dependencies>
  <extensions defaultExtensionNs="com.intellij">
    <applicationService serviceImplementation="a.B"/>
  </extensions>
</idea-plugin>
"#;

/// A plugin descriptor whose `dependencies`, `content` and `depends` are its only change is of the build model; a
/// module descriptor that adds an extension too stays in the tree, named by its file; an `*.xml` of a resources
/// directory that is no plugin descriptor has no dependency change
#[test]
fn the_dependencies_and_the_content_of_a_plugin_descriptor_are_its_dependency_changes() {
  let plugin = PLUGIN_XML
    .replace("  <depends>com.intellij.modules.platform</depends>\n", "")
    .replace(
      "    <plugin id=\"com.intellij.java\"/>\n",
      "    <plugin id=\"com.intellij.java\"/>\n    <module name=\"intellij.platform.y\"/>\n",
    )
    .replace("  <extensions", "  <content>\n    <module name=\"com.example.x.sub\"/>\n  </content>\n  <extensions");
  let module = "<idea-plugin>\n  <dependencies>\n    <module name=\"a\"/>\n  </dependencies>\n</idea-plugin>\n";
  let module_after = "<idea-plugin>\n  <dependencies>\n    <module name=\"a\"/>\n    <module name=\"b\"/>\n  </dependencies>\n  <extensions defaultExtensionNs=\"com.intellij\">\n    <applicationService serviceImplementation=\"y.C\"/>\n  </extensions>\n</idea-plugin>\n";
  let plan = plan_of_change(
    &[
      ("plugins/x/resources/META-INF/plugin.xml", PLUGIN_XML),
      ("plugins/y/resources/intellij.y.xml", module),
      ("plugins/z/resources/other.xml", "<component name=\"z\">\n  <option value=\"1\"/>\n</component>\n"),
    ],
    &[
      ("plugins/x/resources/META-INF/plugin.xml", Some(&plugin)),
      ("plugins/y/resources/intellij.y.xml", Some(module_after)),
      ("plugins/z/resources/other.xml", Some("<component name=\"z\">\n  <option value=\"2\"/>\n</component>\n")),
    ],
  );
  let x = "plugins/x/resources/META-INF/plugin.xml";
  assert_eq!(
    plan.dependencies,
    [
      change(x, "com.example.x", DependencyKind::Module, "intellij.platform.y", DependencyChangeKind::Added),
      change(x, "com.example.x", DependencyKind::Plugin, "com.intellij.modules.platform", DependencyChangeKind::Removed),
      change(x, "com.example.x", DependencyKind::Content, "com.example.x.sub", DependencyChangeKind::Added),
      change("plugins/y/resources/intellij.y.xml", "intellij.y", DependencyKind::Module, "b", DependencyChangeKind::Added),
    ]
  );
  assert_eq!(
    mechanical(&plan),
    [
      ("plugins/x/resources/META-INF/plugin.xml", Some(MechanicalKind::BuildModel)),
      ("plugins/y/resources/intellij.y.xml", None),
      ("plugins/z/resources/other.xml", None),
    ]
  );
}

/// An entry that moves to another table of the same scope gives no dependency change, and its build file stays in
/// the tree also beside a dependency change of its own: a crate that moves from `[dependencies]` into a `target`
/// table next to a new version, and a `depends` of a plugin descriptor that becomes a `plugin` of its `dependencies`
/// next to a new module
#[test]
fn an_entry_that_moves_to_another_table_of_the_same_scope_keeps_the_manifest_in_the_tree() {
  let cargo = "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\nlog = \"0.4\"\n";
  let cargo_after = "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[dependencies]\nlog = \"0.4.20\"\n\n[target.'cfg(windows)'.dependencies]\nserde = \"1\"\n";
  let plugin = PLUGIN_XML.replace("  <depends>com.intellij.modules.platform</depends>\n", "").replace(
    "    <plugin id=\"com.intellij.java\"/>\n",
    "    <plugin id=\"com.intellij.java\"/>\n    <plugin id=\"com.intellij.modules.platform\"/>\n    <module name=\"intellij.platform.y\"/>\n",
  );
  let x = "plugins/x/resources/META-INF/plugin.xml";
  let plan = plan_of_change(
    &[("crates/a/Cargo.toml", cargo), (x, PLUGIN_XML)],
    &[("crates/a/Cargo.toml", Some(cargo_after)), (x, Some(&plugin))],
  );
  let log = DependencyChange {
    version_before: Some("0.4".to_string()),
    version_after: Some("0.4.20".to_string()),
    ..change("crates/a/Cargo.toml", "a", DependencyKind::Crate, "log", DependencyChangeKind::Changed)
  };
  assert_eq!(
    plan.dependencies,
    [log, change(x, "com.example.x", DependencyKind::Module, "intellij.platform.y", DependencyChangeKind::Added)]
  );
  assert_eq!(mechanical(&plan), [("crates/a/Cargo.toml", None), (x, None)]);
}

/// With `since_reviewed`, the dependency changes are those of the diff from the reviewed change to the tip: the entry
/// a rewritten commit replaced, the one it adds instead, the one a new commit adds; an unchanged commit between them adds
/// no entry, also in the same build file
#[test]
fn since_the_review_an_unchanged_commit_between_two_of_one_manifest_adds_no_entry() {
  let repo = TestRepo::new();
  let manifest = |dependencies: &[&str]| {
    dependencies.iter().fold("[package]\nname = \"app\"\n\n[dependencies]\n".to_string(), |mut text, name| {
      text.push_str(name);
      text.push_str(" = \"1\"\n");
      text
    })
  };
  let base = repo.create_commit_with_timestamp("Add manifest", "Cargo.toml", &manifest(&[]), Some(1_700_000_000));
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  repo.create_commit_with_timestamp("(auth) Add a", "Cargo.toml", &manifest(&["a"]), Some(1_700_000_100));
  repo.create_commit_with_timestamp("(auth) Add b", "Cargo.toml", &manifest(&["a", "b"]), Some(1_700_000_200));
  mark(&repo);

  // the first commit rewritten, the second the same lines, a third on top
  repo.git(&["reset", "-q", "--hard", &base]);
  repo.create_commit_with_timestamp("(auth) Add a", "Cargo.toml", &manifest(&["aa"]), Some(1_700_000_100));
  repo.create_commit_with_timestamp("(auth) Add b", "Cargo.toml", &manifest(&["aa", "b"]), Some(1_700_000_200));
  repo.create_commit_with_timestamp("(auth) Add c", "Cargo.toml", &manifest(&["aa", "b", "c"]), Some(1_700_000_300));

  let since = plan_of(&repo, None, &branch(&repo, true)).unwrap();
  let changes: Vec<(&str, DependencyChangeKind)> = since.dependencies.iter().map(|change| (change.name.as_str(), change.change)).collect();
  assert_eq!(
    changes,
    [
      ("a", DependencyChangeKind::Removed),
      ("aa", DependencyChangeKind::Added),
      ("c", DependencyChangeKind::Added)
    ]
  );
}
