use crate::fan_in::NO_ACCIO;
use crate::registrations::{ClassFound, ClassRole, ContentLoading, Registered, RegisteredClass, Registration, RegistrationArea, RegistrationChange};
use crate::test_support::{PREFIX, branch, mark, plan_of, range};
use crate::{PlanSources, ReviewPlan, resolve_subject, review_plan_of};
use file_diffs::accio::{DeclarationAt, Descriptor, References, SymbolHit, SymbolIndex, TestReach};
use file_diffs::service::FileDiffs;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;

/// An index that reads each descriptor of `lines` by `<revision>:<path>`, a path it lacks as missing, and keeps every
/// query it was asked: its revision and its paths
#[derive(Default)]
struct Descriptors {
  lines: HashMap<String, Descriptor>,
  asked: Mutex<Vec<(String, Vec<String>)>>,
}

impl Descriptors {
  /// The line of `path` at `revision`: a descriptor (`root` `idea-plugin`) with the fields of `fields`
  fn with(mut self, revision: &str, path: &str, fields: Value) -> Self {
    let mut line = json!({ "path": path, "root": "idea-plugin" });
    let Value::Object(fields) = fields else { panic!("the fields of a line are an object") };
    line.as_object_mut().unwrap().extend(fields);
    self.lines.insert(format!("{revision}:{path}"), serde_json::from_value(line).unwrap());
    self
  }

  /// The queries asked, by revision
  fn asked(&self) -> Vec<(String, Vec<String>)> {
    let mut asked = self.asked.lock().unwrap().clone();
    asked.sort();
    asked
  }
}

/// A plan of descriptors alone asks the index nothing but the descriptors: any other query fails, and its note shows
impl SymbolIndex for Descriptors {
  fn symbols(&self, _repository_root: &str, query: &str, _revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    anyhow::bail!("asked the declarations of {query}")
  }

  fn references(&self, _repository_root: &str, name: &str, _revision: Option<&str>) -> anyhow::Result<Option<References>> {
    anyhow::bail!("asked the usages of {name}")
  }

  fn references_at(&self, _repository_root: &str, declarations: &[DeclarationAt], _revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    anyhow::bail!("asked the usages of {} places", declarations.len())
  }

  fn tests(&self, _repository_root: &str, declarations: &[DeclarationAt], _given: &[String], _revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    anyhow::bail!("asked the tests of {} places", declarations.len())
  }

  fn descriptors(&self, _repository_root: &str, paths: &[String], revision: &str) -> anyhow::Result<Option<Vec<Descriptor>>> {
    self.asked.lock().unwrap_or_else(PoisonError::into_inner).push((revision.to_string(), paths.to_vec()));
    Ok(Some(
      paths
        .iter()
        .map(|path| {
          self.lines.get(&format!("{revision}:{path}")).cloned().unwrap_or_else(|| Descriptor {
            path: path.clone(),
            missing: true,
            ..Descriptor::default()
          })
        })
        .collect(),
    ))
  }
}

const PLUGIN: &str = "plugin/resources/META-INF/plugin.xml";
const GUI: &str = "gui/resources/intellij.gui.xml";
const MEDIA: &str = "gui/media/resources/intellij.gui.media.xml";
const OLD: &str = "old/resources/intellij.old.xml";

/// A plugin of the module `gui`, which holds the descriptors of `gui` and `old`; then a commit that adds the module
/// `gui/media` with a descriptor of its own, adds it to the content of the plugin, changes the descriptor of `gui` and
/// deletes the module `old`, whose files differ enough from those of `gui/media` to be no rename. The descriptors are
/// read by the index, so their text is only what a change needs
fn plugin_repository() -> (TestRepo, [String; 2]) {
  TestRepo::with_commits(&[
    HistoryCommit::new(
      "Add the plugin",
      &[
        ("plugin/intellij.plugin.iml", "<module/>\n"),
        (PLUGIN, "<idea-plugin>\n  <content/>\n</idea-plugin>\n"),
        ("gui/intellij.gui.iml", "<module/>\n"),
        (GUI, "<idea-plugin>\n  <!-- gui -->\n</idea-plugin>\n"),
        (
          "old/intellij.old.iml",
          "<module type=\"JAVA_MODULE\" version=\"4\">\n  <component name=\"NewModuleRootManager\"/>\n</module>\n",
        ),
        (
          OLD,
          "<idea-plugin>\n  <actions>\n    <group id=\"Old.Group\">\n      <action id=\"Old.Run\" class=\"o.RunAction\"/>\n    </group>\n  </actions>\n</idea-plugin>\n",
        ),
      ],
    ),
    HistoryCommit::new(
      "Add the player",
      &[
        (
          PLUGIN,
          "<idea-plugin>\n  <content>\n    <module name=\"intellij.gui.media\"/>\n  </content>\n</idea-plugin>\n",
        ),
        (GUI, "<idea-plugin>\n  <!-- gui, the player -->\n</idea-plugin>\n"),
        ("gui/media/intellij.gui.media.iml", "<module/>\n"),
        (MEDIA, "<idea-plugin>\n  <!-- media -->\n</idea-plugin>\n"),
      ],
    )
    .deleting(&["old/intellij.old.iml", OLD]),
  ])
}

/// The registry key `gui.inline` with the default value `default`
fn registry_key(default: &str) -> Value {
  json!({
    "point": "com.intellij.registryKey",
    "implementation": null,
    "element": { "name": "", "attributes": { "key": "gui.inline", "defaultValue": default, "description": "Plays inline", "restartRequired": "false" }, "children": [] }
  })
}

/// An application service of `interface` by `implementation`
fn app_service(interface: &str, implementation: &str) -> Value {
  json!({ "area": "app", "interface": interface, "implementation": implementation, "preload": "FALSE" })
}

/// The descriptors of the two sides of `plugin_repository`, as accio reads them
fn index_of([base, head]: &[String; 2]) -> Descriptors {
  let listener = json!({ "area": "project", "class": "a.GuiListener", "topic": "a.Topic" });
  let action = json!({
    "kind": "action",
    "element": { "name": "action", "attributes": { "id": "Gui.Play", "class": "a.PlayAction", "text": "Play" }, "children": [] }
  });
  Descriptors::default()
    .with(base, PLUGIN, json!({ "id": "com.example", "content": [] }))
    .with(
      head,
      PLUGIN,
      json!({ "id": "com.example", "content": [{ "name": "intellij.gui.media", "loading": "OPTIONAL" }] }),
    )
    .with(base, GUI, json!({ "listeners": [listener], "extensions": [registry_key("[ON|OFF*]")], "classes": [] }))
    .with(
      head,
      GUI,
      json!({
        "services": [app_service("a.Players", "a.PlayersImpl")],
        "extensions": [registry_key("[ON*|OFF]")],
        "actions": [action],
        "classes": [
          { "role": "services.interface", "name": "a.Players", "resolved": "exact", "path": "gui/src/a/Players.kt", "line": 3 },
          { "role": "services.implementation", "name": "a.PlayersImpl", "resolved": "none" },
          { "role": "actions.class", "name": "a.PlayAction", "resolved": "ambiguous", "count": 2 }
        ]
      }),
    )
    .with(head, MEDIA, json!({ "services": [app_service("b.Window", "b.WindowImpl")] }))
    .with(
      base,
      OLD,
      json!({
        "actions": [{ "kind": "group", "element": { "name": "group", "attributes": { "id": "Old.Group" }, "children": [
          { "name": "action", "attributes": { "id": "Old.Run", "class": "o.RunAction" }, "children": [] }
        ] } }],
        "classes": [{ "role": "actions.class", "name": "o.RunAction", "resolved": "exact", "path": "old/src/o/RunAction.kt", "line": 7 }]
      }),
    )
}

/// Each registration: (descriptor, the label of its module, its change, what it registers, the fields it changed)
fn summary(plan: &ReviewPlan) -> Vec<(&str, &str, RegistrationChange, String, Vec<&str>)> {
  plan
    .registrations
    .iter()
    .map(|registration| {
      let module = registration.module.map_or("", |module| plan.modules[module as usize].label.as_str());
      let changed = registration.changed.iter().map(String::as_str).collect();
      (registration.descriptor.as_str(), module, registration.change, named(&registration.registered), changed)
    })
    .collect()
}

/// What `registered` names it by, as a review would
fn named(registered: &Registered) -> String {
  match registered {
    Registered::Service {
      area, interface, implementation, ..
    } => format!("{area:?} service {interface:?} → {implementation:?}"),
    Registered::ExtensionPoint { qualified_name, .. } => format!("extension point {qualified_name:?}"),
    Registered::Listener { topic, class, .. } => format!("listener {class} of {topic}"),
    Registered::Extension { point, implementation, .. } => format!("extension {point} {implementation:?}"),
    Registered::RegistryKey { key, default_value, .. } => format!("registry key {key} = {default_value:?}"),
    Registered::Action { id, group, .. } => format!("{} {id:?}", if *group { "group" } else { "action" }),
    Registered::ContentModule { name, loading, .. } => format!("{loading:?} content module {name}"),
  }
}

/// The registrations of a change are what its descriptors add, remove and change between the base and the head, as
/// accio reads them: an application service added, a listener removed, the default of a registry key changed, an
/// action added, a content module added, a descriptor added whole and one deleted whole, a group with its nested
/// action; in the order of the modules, then of the descriptor, then of the kind. Each side is one query, of the
/// descriptors it has
#[test]
fn the_registrations_are_what_the_descriptors_add_remove_and_change_between_the_base_and_the_head() {
  let (repo, commits) = plugin_repository();
  let index = index_of(&commits);
  let plan = plan_of(&repo, Some(&index), &range(&repo, &commits[0], &commits[1])).unwrap();
  assert_eq!(
    plan.modules.iter().map(|module| module.label.as_str()).collect::<Vec<_>>(),
    ["gui/media", "gui", "old", "plugin"]
  );
  assert_eq!(
    summary(&plan),
    [
      (
        MEDIA,
        "gui/media",
        RegistrationChange::Added,
        "App service Some(\"b.Window\") → Some(\"b.WindowImpl\")".to_string(),
        vec![]
      ),
      (
        GUI,
        "gui",
        RegistrationChange::Added,
        "App service Some(\"a.Players\") → Some(\"a.PlayersImpl\")".to_string(),
        vec![]
      ),
      (GUI, "gui", RegistrationChange::Removed, "listener a.GuiListener of a.Topic".to_string(), vec![]),
      (
        GUI,
        "gui",
        RegistrationChange::Changed,
        "registry key gui.inline = Some(\"ON\")".to_string(),
        vec!["defaultValue"]
      ),
      (GUI, "gui", RegistrationChange::Added, "action Some(\"Gui.Play\")".to_string(), vec![]),
      (OLD, "old", RegistrationChange::Removed, "group Some(\"Old.Group\")".to_string(), vec![]),
      (OLD, "old", RegistrationChange::Removed, "action Some(\"Old.Run\")".to_string(), vec![]),
      (
        PLUGIN,
        "plugin",
        RegistrationChange::Added,
        "Optional content module intellij.gui.media".to_string(),
        vec![]
      ),
    ]
  );
  // the registry key before the change, its options without the mark
  let key = plan.registrations.iter().find(|registration| registration.change == RegistrationChange::Changed).unwrap();
  assert_eq!(
    (&key.registered, &key.before),
    (
      &Registered::RegistryKey {
        key: "gui.inline".to_string(),
        default_value: Some("ON".to_string()),
        options: vec!["ON".to_string(), "OFF".to_string()],
        description: Some("Plays inline".to_string()),
        restart_required: false,
      },
      &Some(Registered::RegistryKey {
        key: "gui.inline".to_string(),
        default_value: Some("OFF".to_string()),
        options: vec!["ON".to_string(), "OFF".to_string()],
        description: Some("Plays inline".to_string()),
        restart_required: false,
      }),
    )
  );
  // the content module names the module of the review it is
  assert_eq!(
    plan.registrations[7].registered,
    Registered::ContentModule {
      name: "intellij.gui.media".to_string(),
      loading: ContentLoading::Optional,
      review_module: Some(0),
    }
  );
  // the classes as accio resolved them: at the head, of a removed registration at the base; unknown where the side's
  // index gave no answer (the descriptor of `gui/media` has no `classes`)
  let classes = |place: usize| plan.registrations[place].classes.clone();
  assert_eq!(
    classes(1),
    [
      RegisteredClass {
        role: ClassRole::Interface,
        name: "a.Players".to_string(),
        found: Some(ClassFound::Declared {
          path: "gui/src/a/Players.kt".to_string(),
          line: 3,
        }),
      },
      RegisteredClass {
        role: ClassRole::Implementation,
        name: "a.PlayersImpl".to_string(),
        found: Some(ClassFound::NotFound),
      },
    ]
  );
  assert_eq!(classes(4)[0].found, Some(ClassFound::Ambiguous { count: 2 }));
  assert_eq!(
    classes(6)[0].found,
    Some(ClassFound::Declared {
      path: "old/src/o/RunAction.kt".to_string(),
      line: 7,
    })
  );
  assert_eq!(classes(0).iter().map(|class| class.found.clone()).collect::<Vec<_>>(), [None, None]);
  assert_eq!(plan.index_note, None);
  // one query per side: the base lacks the added descriptor, the head the deleted one
  let mut asked = vec![
    (commits[0].clone(), vec![GUI.to_string(), OLD.to_string(), PLUGIN.to_string()]),
    (commits[1].clone(), vec![MEDIA.to_string(), GUI.to_string(), PLUGIN.to_string()]),
  ];
  asked.sort();
  assert_eq!(index.asked(), asked);
}

/// Without a usable index accio still reads the descriptors, as the platform does, but resolves no class: each class is
/// unknown, never missing, and the reason joins the note of the index
#[test]
fn without_an_index_the_registrations_are_listed_and_their_classes_unknown() {
  let (repo, commits) = plugin_repository();
  let note = json!({ "classes": null, "note": "no index of this repository" });
  let index = Descriptors::default().with(&commits[0], GUI, note.clone()).with(&commits[1], GUI, {
    let mut fields = note;
    fields["services"] = json!([app_service("a.Players", "a.PlayersImpl")]);
    fields
  });
  let plan = plan_of(&repo, Some(&index), &range(&repo, &commits[0], &commits[1])).unwrap();
  let added: Vec<&Registration> = plan.registrations.iter().filter(|registration| registration.descriptor == GUI).collect();
  assert_eq!(added.len(), 1);
  assert_eq!(
    added[0].registered,
    Registered::Service {
      area: RegistrationArea::App,
      interface: Some("a.Players".to_string()),
      implementation: Some("a.PlayersImpl".to_string()),
      test_implementation: None,
      headless_implementation: None,
      client: None,
      os: None,
      preload: Some("FALSE".to_string()),
      overrides: false,
      open: false,
    }
  );
  assert_eq!(added[0].classes.iter().map(|class| class.found.clone()).collect::<Vec<_>>(), [None, None]);
  assert_eq!(plan.index_note.as_deref(), Some("no index of this repository"));
}

/// Without accio no registration is guessed: none, and the note says why
#[test]
fn without_accio_a_plan_lists_no_registration_and_says_why() {
  let (repo, commits) = plugin_repository();
  let plan = plan_of(&repo, None, &range(&repo, &commits[0], &commits[1])).unwrap();
  assert_eq!((plan.registrations, plan.index_note.as_deref()), (vec![], Some(NO_ACCIO)));
}

/// A descriptor the platform's reader refuses on a side has no known registration; another XML of a `resources`
/// directory registers nothing; a service keyed by its client is another registration than the same interface for
/// another client; a service whose implementation changed is one changed registration
#[test]
fn a_refused_descriptor_registers_nothing_and_a_service_is_known_by_its_interface_and_client() {
  let (repo, commits) = plugin_repository();
  let local = json!({ "area": "app", "interface": "a.Players", "implementation": "a.LocalPlayers", "client": "local" });
  let remote = json!({ "area": "app", "interface": "a.Players", "implementation": "a.RemotePlayers", "client": "remote" });
  let index = Descriptors::default()
    .with(&commits[0], GUI, json!({ "services": [local, remote] }))
    .with(
      &commits[1],
      GUI,
      json!({ "services": [{ "area": "app", "interface": "a.Players", "implementation": "a.Remote", "client": "remote" }, local] }),
    )
    .with(&commits[0], PLUGIN, json!({ "content": [{ "name": "intellij.gui", "loading": "REQUIRED" }] }))
    .with(&commits[1], PLUGIN, json!({ "error": true }))
    .with(&commits[1], MEDIA, json!({ "root": "component", "services": [app_service("b.Window", "b.WindowImpl")] }));
  let plan = plan_of(&repo, Some(&index), &range(&repo, &commits[0], &commits[1])).unwrap();
  assert_eq!(
    summary(&plan),
    [(
      GUI,
      "gui",
      RegistrationChange::Changed,
      "App service Some(\"a.Players\") → Some(\"a.Remote\")".to_string(),
      vec!["serviceImplementation"]
    )]
  );
}

/// Since the review the registrations are those between the ends of its scope: of the commits added on top, from the
/// reviewed tip to the tip, never what the reviewer already read
#[test]
fn since_the_review_the_registrations_are_those_of_the_commits_added_on_top() {
  let repo = TestRepo::new();
  repo.create_commit_with_timestamp("Add the plugin", GUI, "<idea-plugin/>\n", Some(1_700_000_000));
  repo.git(&["switch", "-q", "-c", "jane/virtual/auth"]);
  let reviewed = repo.create_commit_with_timestamp("(auth) Add the players", GUI, "<idea-plugin>\n  <!-- players -->\n</idea-plugin>\n", Some(1_700_000_100));
  mark(&repo);
  let tip = repo.create_commit_with_timestamp(
    "(auth) Add the window",
    GUI,
    "<idea-plugin>\n  <!-- players, window -->\n</idea-plugin>\n",
    Some(1_700_000_200),
  );
  let players = app_service("a.Players", "a.PlayersImpl");
  let index =
    Descriptors::default()
      .with(&reviewed, GUI, json!({ "services": [players] }))
      .with(&tip, GUI, json!({ "services": [players, app_service("a.Window", "a.WindowImpl")] }));
  let plan = plan_of(&repo, Some(&index), &branch(&repo, true)).unwrap();
  assert_eq!(
    summary(&plan),
    [(
      GUI,
      "(root)",
      RegistrationChange::Added,
      "App service Some(\"a.Window\") → Some(\"a.WindowImpl\")".to_string(),
      vec![]
    )]
  );
  let mut asked = vec![(reviewed, vec![GUI.to_string()]), (tip, vec![GUI.to_string()])];
  asked.sort();
  assert_eq!(index.asked(), asked);
}

/// The plan sent while the index is asked has no registration, the plan that follows has them: a change of descriptors
/// alone is a query to ask
#[test]
fn the_plan_sent_while_the_index_is_asked_has_no_registration() {
  let (repo, commits) = plugin_repository();
  let index = index_of(&commits);
  let params = range(&repo, &commits[0], &commits[1]);
  let subject = resolve_subject(repo.git_executor(), PREFIX, &params).unwrap();
  let (cache, file_diffs) = (ObjectCache::disabled(), FileDiffs::for_scans());
  let sources = PlanSources {
    cache: &cache,
    file_diffs: &file_diffs,
    index: Some(&index),
  };
  let mut pending = None;
  let plan = review_plan_of(repo.git_executor(), sources, &params, subject, |plan| pending = Some(plan.registrations.len())).unwrap();
  assert_eq!((pending, plan.registrations.len()), (Some(0), 8));
}
