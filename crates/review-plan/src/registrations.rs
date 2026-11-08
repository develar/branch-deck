//! The registrations of a review (spec/branch/review-plan.spec.md, "Registrations"): how a change plugs into the
//! platform, the services, extension points, listeners, extensions, registry keys, actions and content modules its
//! plugin descriptors add, remove or change.
//!
//! - **The descriptors** are the changed files `dependencies` can read as a plugin descriptor by their path (an `*.xml`
//!   named `plugin.xml`, or in a `META-INF` or `resources` directory, out of `.idea`), read by accio's port of the
//!   platform's reader (`SymbolIndex::descriptors`, `accio parse --descriptor --resolve`), never by a parser here
//!   (decision 12 of docs/design/change-review.md): at the base those the commits do not add, at their first path; at the
//!   head those they do not delete. One process per side, both at once, beside the usages and the tests. A side whose
//!   root is no `idea-plugin` registers nothing; a descriptor the reader refuses on either side has no registration.
//! - **The sides** are the base and the head of the plan; of Since Your Review the two ends of its scope. Since Your
//!   Review whose carry conflicts reads its commits one at a time, which no two revisions hold: it lists none.
//! - **A registration** is known by what identifies it to the platform: a service by its area, its interface (else its
//!   implementation), its client and its OS; an extension point by its area and its qualified name (else its name); a
//!   listener by its area, its topic and its class; an extension of `com.intellij.registryKey` by its key (a registry
//!   key), another by its point and its implementation, else its `id`, else its whole element; an action or a group by
//!   its `id`, else its class, nested in groups too; a content module by its name. The entries of one key pair in their
//!   order; a pair whose other fields differ is `changed`, with the fields it changed; the rest are `added` or `removed`.
//! - **Its classes**: each class it names as accio resolved it in the index at the head (at the base for a removed one):
//!   a declaration, ambiguous, none; unknown without a usable index, whose reason joins `ReviewPlan::index_note`.
//! - **The order**: by the module of the descriptor (`ReviewPlan::modules`), the descriptor in tree order, the kind, then
//!   the order of the head, the removed ones last in the order of the base.

use crate::dependencies::may_be_descriptor;
use crate::fan_in::ask_index;
use crate::files::ChangedFile;
use crate::modules::Modules;
use anyhow::Result;
use file_diffs::accio::{
  Descriptor, DescriptorAction, DescriptorClass, DescriptorContentModule, DescriptorElement, DescriptorExtension, DescriptorExtensionPoint, DescriptorListener, DescriptorService,
  SymbolIndex,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, VecDeque};
use sync_utils::span::in_current_span;
use tracing::debug;

/// The point of the extensions that are registry keys
const REGISTRY_KEY_POINT: &str = "com.intellij.registryKey";

/// How the commits change a registration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum RegistrationChange {
  Added,
  Removed,
  /// A field beside what identifies it differs
  Changed,
}

/// The container a service, an extension point or a listener lives in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum RegistrationArea {
  /// The application: `applicationService`, `applicationListeners`
  App,
  Project,
  Module,
}

impl RegistrationArea {
  /// The area accio names (`app`, `project`, `module`, the three of the platform)
  fn of(name: &str) -> Option<Self> {
    match name {
      "app" => Some(Self::App),
      "project" => Some(Self::Project),
      "module" => Some(Self::Module),
      _ => None,
    }
  }
}

/// When the platform loads a content module (`loading`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ContentLoading {
  Optional,
  Required,
  Embedded,
  OnDemand,
}

impl ContentLoading {
  /// The loading accio names (`OPTIONAL`, `REQUIRED`, `EMBEDDED`, `ON_DEMAND`, the four of the platform)
  fn of(name: &str) -> Option<Self> {
    match name {
      "OPTIONAL" => Some(Self::Optional),
      "REQUIRED" => Some(Self::Required),
      "EMBEDDED" => Some(Self::Embedded),
      "ON_DEMAND" => Some(Self::OnDemand),
      _ => None,
    }
  }
}

/// What a descriptor registers
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Registered {
  /// `applicationService`, `projectService`, `moduleService`
  #[serde(rename_all = "camelCase")]
  Service {
    area: RegistrationArea,
    interface: Option<String>,
    implementation: Option<String>,
    test_implementation: Option<String>,
    headless_implementation: Option<String>,
    /// `client`: the frontend or the backend of a remote development it serves
    client: Option<String>,
    os: Option<String>,
    preload: Option<String>,
    overrides: bool,
    open: bool,
  },
  /// `extensionPoint`
  #[serde(rename_all = "camelCase")]
  ExtensionPoint {
    area: RegistrationArea,
    name: Option<String>,
    qualified_name: Option<String>,
    interface: Option<String>,
    bean_class: Option<String>,
    dynamic: bool,
  },
  /// A listener of `applicationListeners`, `projectListeners`
  #[serde(rename_all = "camelCase")]
  Listener {
    area: RegistrationArea,
    topic: String,
    class: String,
    os: Option<String>,
  },
  /// An extension of a point other than `com.intellij.registryKey`
  #[serde(rename_all = "camelCase")]
  Extension {
    /// The qualified name of the point
    point: String,
    implementation: Option<String>,
    /// Its attribute `id`
    id: Option<String>,
    /// Its attributes as written, by name
    attributes: BTreeMap<String, String>,
  },
  /// An extension of `com.intellij.registryKey`
  #[serde(rename_all = "camelCase")]
  RegistryKey {
    key: String,
    /// The value it has unless the user sets one: of a key of options the option marked `*` (none when none is), else
    /// its `defaultValue`
    default_value: Option<String>,
    /// The options of a key of options (`[ENABLED|NIGHTLY_ENABLED*|DISABLED]`), without the mark; empty for another
    options: Vec<String>,
    description: Option<String>,
    restart_required: bool,
  },
  /// An action or a group of actions
  #[serde(rename_all = "camelCase")]
  Action {
    id: Option<String>,
    class: Option<String>,
    /// Its attribute `text`; most actions take their text from a bundle
    text: Option<String>,
    group: bool,
  },
  /// A module of the `content` of a plugin
  #[serde(rename_all = "camelCase")]
  ContentModule {
    name: String,
    loading: ContentLoading,
    /// The module of the review of that name, by its place in `ReviewPlan::modules`
    review_module: Option<u32>,
  },
}

/// What a class is to its registration: the field that names it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ClassRole {
  Interface,
  Implementation,
  TestImplementation,
  HeadlessImplementation,
  BeanClass,
  /// The class of a listener or of an action
  Class,
  /// The topic of a listener
  Topic,
}

/// Where accio found a class a registration names, in the index at the side of the registration
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "resolved", rename_all = "camelCase")]
pub enum ClassFound {
  /// One declaration: its file and the line of its name, from 1
  Declared { path: String, line: u32 },
  /// Several declarations of the name
  Ambiguous { count: u32 },
  /// No declaration of the name: a class the side lacks
  NotFound,
}

/// A class a registration names
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RegisteredClass {
  pub role: ClassRole,
  /// Its qualified name, as the descriptor writes it
  pub name: String,
  /// At the head, of a removed registration at the base; `None` without a usable index (`ReviewPlan::index_note`)
  pub found: Option<ClassFound>,
}

/// A registration the commits add, remove or change in a plugin descriptor
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Registration {
  /// The descriptor, at its last path
  pub descriptor: String,
  /// The module of the descriptor, by its place in `ReviewPlan::modules`
  pub module: Option<u32>,
  pub change: RegistrationChange,
  /// What is registered at the head; of a removed one, at the base
  pub registered: Registered,
  /// Of a changed one: what was registered at the base
  pub before: Option<Registered>,
  /// Of a changed one: the fields that differ, by the attribute of the descriptor (`serviceImplementation`,
  /// `defaultValue`, `loading`), a child element by its tag (`<add-to-group>`), in the order of their names
  pub changed: Vec<String>,
  /// The classes it names
  pub classes: Vec<RegisteredClass>,
}

/// A changed file that can be a plugin descriptor, with the paths of its sides
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DescriptorFile {
  /// The path after the commits, of a deleted one before them
  pub path: String,
  /// At the base: its first path; `None` for a file the commits add
  pub before: Option<String>,
  /// At the head; `None` for a file the commits delete
  pub after: Option<String>,
}

/// The changed files of `changed` that can be plugin descriptors, in tree order
pub(crate) fn changed_descriptors(changed: &[ChangedFile<'_>]) -> Vec<DescriptorFile> {
  changed
    .iter()
    .filter(|file| may_be_descriptor(file.path) || may_be_descriptor(file.first_path))
    .filter(|file| file.before.is_some() || file.after.is_some())
    .map(|file| DescriptorFile {
      path: file.path.to_string(),
      before: file.before.map(|_| file.first_path.to_string()),
      after: file.after.map(|_| file.path.to_string()),
    })
    .collect()
}

/// What the registrations of a review read beside the index
pub(crate) struct RegistrationsInput<'a> {
  pub repository_path: &'a str,
  /// The revisions of the base and the head; `None` when no two revisions hold them
  pub sides: Option<(&'a str, &'a str)>,
  pub files: &'a [DescriptorFile],
  pub changed: &'a [ChangedFile<'a>],
  pub modules: &'a Modules,
}

/// The registrations the commits add, remove or change, each side asked of `index` in one process, both at once; the
/// answer says why there are none (no accio, a failed query) or why their classes are unknown (no usable index), `None`
/// when accio answered all. Fails only when the index was canceled
pub(crate) fn registrations_of(index: Option<&dyn SymbolIndex>, input: &RegistrationsInput<'_>) -> Result<(Vec<Registration>, Option<String>)> {
  let Some((base, head)) = input.sides.filter(|_| !input.files.is_empty()) else {
    return Ok((Vec::new(), None));
  };
  let base_paths: Vec<String> = input.files.iter().filter_map(|file| file.before.clone()).collect();
  let head_paths: Vec<String> = input.files.iter().filter_map(|file| file.after.clone()).collect();
  let ask = |paths: &[String], revision: &str| -> Result<Result<Vec<Descriptor>, String>> {
    if paths.is_empty() {
      return Ok(Ok(Vec::new()));
    }
    ask_index(index, |lenient| lenient.descriptors(input.repository_path, paths, revision))
  };
  let (before, after) = std::thread::scope(|scope| {
    let before = scope.spawn(in_current_span(|| ask(&base_paths, base)));
    let after = ask(&head_paths, head);
    (before.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the descriptors panicked"))), after)
  });
  let (before, after) = match (before?, after?) {
    (Ok(before), Ok(after)) => (before, after),
    (Err(note), _) | (_, Err(note)) => return Ok((Vec::new(), Some(note))),
  };
  let note = before
    .iter()
    .chain(&after)
    .filter(|descriptor| descriptor.is_read())
    .find_map(|descriptor| descriptor.note.clone());
  let (mut before, mut after) = (before.into_iter(), after.into_iter());
  let mut found = Vec::new();
  for (place, file) in input.files.iter().enumerate() {
    let base_side = file.before.as_ref().and_then(|_| before.next());
    let head_side = file.after.as_ref().and_then(|_| after.next());
    let module = input.modules.of_path(input.changed, &file.path);
    for registration in descriptor_registrations(file, base_side.as_ref(), head_side.as_ref(), module, input.modules) {
      found.push((place, registration));
    }
  }
  found.sort_by_key(|(place, registration)| (registration.module.unwrap_or(u32::MAX), *place, kind_rank(&registration.registered)));
  debug!(descriptors = input.files.len(), registrations = found.len(), "the registrations of the descriptors");
  Ok((found.into_iter().map(|(_, registration)| registration).collect(), note))
}

/// The place of a kind in the order of the registrations of a descriptor
const fn kind_rank(registered: &Registered) -> u8 {
  match registered {
    Registered::Service { .. } => 0,
    Registered::ExtensionPoint { .. } => 1,
    Registered::Listener { .. } => 2,
    Registered::Extension { .. } => 3,
    Registered::RegistryKey { .. } => 4,
    Registered::Action { .. } => 5,
    Registered::ContentModule { .. } => 6,
  }
}

/// A registration of one side: what identifies it, what it is, the other fields a change compares, and the classes it
/// names by accio's role of each
struct Entry {
  key: String,
  registered: Registered,
  fields: BTreeMap<String, String>,
  classes: Vec<(ClassRole, &'static str, String)>,
}

/// The registrations of one descriptor between its sides: `None` where the side has no file
fn descriptor_registrations(file: &DescriptorFile, before: Option<&Descriptor>, after: Option<&Descriptor>, module: Option<u32>, modules: &Modules) -> Vec<Registration> {
  // a descriptor the reader refuses has no known registration
  if [before, after].into_iter().flatten().any(|side| side.error) {
    return Vec::new();
  }
  let entries = |side: Option<&Descriptor>| side.filter(|side| side.is_read()).map_or_else(Vec::new, |side| entries_of(side, modules));
  let (old, new) = (entries(before), entries(after));
  let (old_classes, new_classes) = (classes_of(before), classes_of(after));
  let registration = |change: RegistrationChange, entry: Entry, before: Option<Registered>, changed: Vec<String>| {
    let classes = if change == RegistrationChange::Removed {
      old_classes.as_ref()
    } else {
      new_classes.as_ref()
    };
    Registration {
      descriptor: file.path.clone(),
      module,
      change,
      registered: entry.registered,
      before,
      changed,
      classes: entry
        .classes
        .into_iter()
        .map(|(role, accio_role, name)| RegisteredClass {
          found: classes.map(|classes| classes.get(&(accio_role, name.as_str())).map_or(ClassFound::NotFound, |class| found(class))),
          role,
          name,
        })
        .collect(),
    }
  };
  // the entries of the base by their key, in their order
  let mut unpaired: HashMap<String, VecDeque<usize>> = HashMap::new();
  for (place, entry) in old.iter().enumerate() {
    unpaired.entry(entry.key.clone()).or_default().push_back(place);
  }
  let mut old: Vec<Option<Entry>> = old.into_iter().map(Some).collect();
  let mut registrations = Vec::new();
  for entry in new {
    let paired = unpaired.get_mut(&entry.key).and_then(VecDeque::pop_front).and_then(|place| old[place].take());
    match paired {
      None => registrations.push(registration(RegistrationChange::Added, entry, None, Vec::new())),
      Some(previous) => {
        let changed = changed_fields(&previous.fields, &entry.fields);
        if !changed.is_empty() {
          registrations.push(registration(RegistrationChange::Changed, entry, Some(previous.registered), changed));
        }
      }
    }
  }
  registrations.extend(old.into_iter().flatten().map(|entry| registration(RegistrationChange::Removed, entry, None, Vec::new())));
  registrations
}

/// The classes `side` names by accio's role and their name; `None` without a side or an answer of the index
fn classes_of(side: Option<&Descriptor>) -> Option<HashMap<(&str, &str), &DescriptorClass>> {
  side?
    .classes
    .as_ref()
    .map(|classes| classes.iter().map(|class| ((class.role.as_str(), class.name.as_str()), class)).collect())
}

/// What accio answered of a class
fn found(class: &DescriptorClass) -> ClassFound {
  match (class.resolved.as_str(), &class.path, class.line) {
    ("exact", Some(path), Some(line)) => ClassFound::Declared { path: path.clone(), line },
    // ambiguous is two declarations or more, and accio always counts them
    ("ambiguous", _, _) => ClassFound::Ambiguous { count: class.count.unwrap_or(2) },
    _ => ClassFound::NotFound,
  }
}

/// The names of the fields that differ between two entries of one key, in the order of their names
fn changed_fields(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> Vec<String> {
  let mut names: Vec<String> = before
    .iter()
    .filter(|(name, value)| after.get(*name) != Some(*value))
    .map(|(name, _)| name.clone())
    .chain(after.keys().filter(|name| !before.contains_key(*name)).cloned())
    .collect();
  names.sort_unstable();
  names
}

/// The registrations of a descriptor a side reads, by kind
fn entries_of(descriptor: &Descriptor, modules: &Modules) -> Vec<Entry> {
  let mut entries: Vec<Entry> = descriptor.services.iter().filter_map(service).collect();
  entries.extend(descriptor.extension_points.iter().filter_map(extension_point));
  entries.extend(descriptor.listeners.iter().filter_map(listener));
  entries.extend(descriptor.extensions.iter().map(extension));
  for action in &descriptor.actions {
    actions(action, &mut entries);
  }
  entries.extend(descriptor.content.iter().filter_map(|module| content_module(module, modules)));
  entries
}

/// A flag as a field
const fn flag(value: bool) -> &'static str {
  if value { "true" } else { "false" }
}

/// The classes of `names` that are there, each by its role and accio's
fn named(names: &[(ClassRole, &'static str, Option<&str>)]) -> Vec<(ClassRole, &'static str, String)> {
  names
    .iter()
    .filter_map(|(role, accio_role, name)| Some((*role, *accio_role, (*name)?.to_string())))
    .collect()
}

/// A field of `fields` for each value that is there
fn fields(values: &[(&str, Option<&str>)]) -> BTreeMap<String, String> {
  values.iter().filter_map(|(name, value)| Some(((*name).to_string(), (*value)?.to_string()))).collect()
}

fn service(service: &DescriptorService) -> Option<Entry> {
  let area = RegistrationArea::of(&service.area)?;
  let identity = service.interface.as_deref().or(service.implementation.as_deref()).unwrap_or_default();
  Some(Entry {
    key: format!(
      "service\t{}\t{identity}\t{}\t{}",
      service.area,
      service.client.as_deref().unwrap_or_default(),
      service.os.as_deref().unwrap_or_default()
    ),
    fields: fields(&[
      ("serviceImplementation", service.implementation.as_deref()),
      ("testServiceImplementation", service.test.as_deref()),
      ("headlessImplementation", service.headless.as_deref()),
      ("configurationSchemaKey", service.schema_key.as_deref()),
      ("preload", service.preload.as_deref()),
      ("overrides", Some(flag(service.overrides))),
      ("open", Some(flag(service.open))),
    ]),
    classes: named(&[
      (ClassRole::Interface, "services.interface", service.interface.as_deref()),
      (ClassRole::Implementation, "services.implementation", service.implementation.as_deref()),
      (ClassRole::TestImplementation, "services.test", service.test.as_deref()),
      (ClassRole::HeadlessImplementation, "services.headless", service.headless.as_deref()),
    ]),
    registered: Registered::Service {
      area,
      interface: service.interface.clone(),
      implementation: service.implementation.clone(),
      test_implementation: service.test.clone(),
      headless_implementation: service.headless.clone(),
      client: service.client.clone(),
      os: service.os.clone(),
      preload: service.preload.clone(),
      overrides: service.overrides,
      open: service.open,
    },
  })
}

fn extension_point(point: &DescriptorExtensionPoint) -> Option<Entry> {
  let area = RegistrationArea::of(&point.area)?;
  let identity = point.qualified_name.as_deref().or(point.name.as_deref()).unwrap_or_default();
  Some(Entry {
    key: format!("extensionPoint\t{}\t{identity}", point.area),
    fields: fields(&[
      ("interface", point.interface.as_deref()),
      ("beanClass", point.bean_class.as_deref()),
      ("dynamic", Some(flag(point.dynamic))),
    ]),
    classes: named(&[
      (ClassRole::Interface, "extensionPoints.interface", point.interface.as_deref()),
      (ClassRole::BeanClass, "extensionPoints.beanClass", point.bean_class.as_deref()),
    ]),
    registered: Registered::ExtensionPoint {
      area,
      name: point.name.clone(),
      qualified_name: point.qualified_name.clone(),
      interface: point.interface.clone(),
      bean_class: point.bean_class.clone(),
      dynamic: point.dynamic,
    },
  })
}

fn listener(listener: &DescriptorListener) -> Option<Entry> {
  let area = RegistrationArea::of(&listener.area)?;
  Some(Entry {
    key: format!("listener\t{}\t{}\t{}", listener.area, listener.topic, listener.class),
    fields: fields(&[
      ("activeInTestMode", Some(flag(listener.test_mode))),
      ("activeInHeadlessMode", Some(flag(listener.headless_mode))),
      ("os", listener.os.as_deref()),
    ]),
    classes: vec![
      (ClassRole::Class, "listeners.class", listener.class.clone()),
      (ClassRole::Topic, "listeners.topic", listener.topic.clone()),
    ],
    registered: Registered::Listener {
      area,
      topic: listener.topic.clone(),
      class: listener.class.clone(),
      os: listener.os.clone(),
    },
  })
}

/// An extension, a registry key one of `com.intellij.registryKey` with a key
fn extension(extension: &DescriptorExtension) -> Entry {
  let attributes = extension.element.as_ref().map_or_default(|element| element.attributes.clone());
  let mut compared = attributes.clone();
  if let Some(element) = &extension.element {
    compared.extend(child_fields(element, false));
  }
  let classes = extension
    .implementation
    .iter()
    .map(|implementation| (ClassRole::Implementation, "extensions.implementation", implementation.clone()))
    .collect();
  if extension.point == REGISTRY_KEY_POINT
    && let Some(key) = attributes.get("key")
  {
    let (default_value, options) = registry_default(attributes.get("defaultValue").map(String::as_str));
    return Entry {
      key: format!("registryKey\t{key}"),
      fields: compared,
      classes,
      registered: Registered::RegistryKey {
        key: key.clone(),
        default_value,
        options,
        description: attributes.get("description").cloned(),
        restart_required: attributes.get("restartRequired").is_some_and(|value| value == "true"),
      },
    };
  }
  let identity = extension.implementation.as_ref().map_or_else(
    || {
      extension
        .order_id
        .as_ref()
        .map_or_else(|| format!("element\t{}", extension.element.as_ref().map_or_default(canonical)), |id| format!("id\t{id}"))
    },
    |implementation| format!("implementation\t{implementation}"),
  );
  Entry {
    key: format!("extension\t{}\t{identity}", extension.point),
    fields: compared,
    classes,
    registered: Registered::Extension {
      point: extension.point.clone(),
      implementation: extension.implementation.clone(),
      id: extension.order_id.clone(),
      attributes,
    },
  }
}

/// The value a registry key has by default and its options: of `[A|B*|C]` the option marked `*` and every option, else
/// the value as written and none
fn registry_default(value: Option<&str>) -> (Option<String>, Vec<String>) {
  let Some(value) = value else { return (None, Vec::new()) };
  let Some(list) = value.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) else {
    return (Some(value.to_string()), Vec::new());
  };
  let mut selected = None;
  let options = list
    .split('|')
    .map(|option| match option.strip_suffix('*') {
      Some(marked) => {
        selected.get_or_insert_with(|| marked.to_string());
        marked.to_string()
      }
      None => option.to_string(),
    })
    .collect();
  (selected, options)
}

/// The actions and groups of an entry of `<actions>`, a group's nested ones after it
fn actions(action: &DescriptorAction, entries: &mut Vec<Entry>) {
  if let Some(element) = &action.element {
    action_elements(element, entries);
  }
}

/// `element` when it is an action or a group, then the actions and groups below it
fn action_elements(element: &DescriptorElement, entries: &mut Vec<Entry>) {
  let group = element.name == "group";
  if group || element.name == "action" {
    let (id, class) = (element.attributes.get("id"), element.attributes.get("class"));
    if let Some(identity) = id.map(|id| format!("id\t{id}")).or_else(|| class.map(|class| format!("class\t{class}"))) {
      let mut compared = element.attributes.clone();
      compared.extend(child_fields(element, true));
      entries.push(Entry {
        key: format!("action\t{identity}"),
        fields: compared,
        classes: class.iter().map(|class| (ClassRole::Class, "actions.class", (*class).clone())).collect(),
        registered: Registered::Action {
          id: id.cloned(),
          class: class.cloned(),
          text: element.attributes.get("text").cloned(),
          group,
        },
      });
    }
  }
  for child in &element.children {
    action_elements(child, entries);
  }
}

fn content_module(module: &DescriptorContentModule, modules: &Modules) -> Option<Entry> {
  let loading = ContentLoading::of(&module.loading)?;
  Some(Entry {
    key: format!("contentModule\t{}", module.name),
    fields: fields(&[("loading", Some(module.loading.as_str()))]),
    classes: Vec::new(),
    registered: Registered::ContentModule {
      name: module.name.clone(),
      loading,
      review_module: modules
        .shapes
        .iter()
        .position(|shape| shape.name == module.name)
        .and_then(|place| u32::try_from(place).ok()),
    },
  })
}

/// The children of `element` as fields, by their element name: the children of one name in their order. Of an action
/// or a group (`actions`), its nested actions and groups are registrations of their own, no field of it
fn child_fields(element: &DescriptorElement, actions: bool) -> BTreeMap<String, String> {
  let mut fields: BTreeMap<String, String> = BTreeMap::new();
  for child in &element.children {
    if actions && (child.name == "action" || child.name == "group") {
      continue;
    }
    // a name no attribute has: an attribute is a word of XML, `<` is none
    fields.entry(format!("<{}>", child.name)).or_default().push_str(&canonical(child));
  }
  fields
}

/// `element` as one text, the same for the same element
fn canonical(element: &DescriptorElement) -> String {
  let attributes: Vec<String> = element.attributes.iter().map(|(name, value)| format!("{name}={value:?}")).collect();
  let children: String = element.children.iter().map(canonical).collect();
  format!("<{} {}>{children}</>", element.name, attributes.join(" "))
}
