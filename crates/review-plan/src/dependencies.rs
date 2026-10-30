//! The dependency changes of a review (spec/branch/review-plan.spec.md, "Dependency changes"): the entries the
//! reviewed commits add to, remove from or change in the build files, and the build files they add or delete.
//!
//! - **The manifests**, by name: an `*.iml` (its `orderEntry` of type `module`, `library`, or `module-library` with a
//!   named `library`), a plugin XML descriptor (an `*.xml` named `plugin.xml`, or in a `META-INF` or `resources`
//!   directory, and not in `.idea`, whose root is `idea-plugin`: its `depends`, its `dependencies` `plugin` and
//!   `module`, its `content` `module`), a `Cargo.toml` (`dependencies`, `dev-dependencies`, `build-dependencies`, the
//!   same tables of each `target`, and `workspace.dependencies`) and a `package.json` (`dependencies`,
//!   `devDependencies`, `peerDependencies`). Each is read before the first commit that changes it and after the last.
//! - **An entry** has a kind, a name, a version (`Cargo.toml`, `package.json`), a scope and its section. Before and
//!   after, the entries of one kind and name pair first by their section, then in their order. A pair is `Changed`
//!   when its version or its scope differs; the rest are `Added` or `Removed`.
//! - **The module** of a manifest is the name of an `*.iml`, the `id` of a plugin descriptor (else the name of the
//!   file), the `package.name` of a `Cargo.toml` or the `name` of a `package.json`, else the name of its directory. A
//!   `Cargo.toml` or `package.json` at the root of the repository without a name, a workspace, is the module `(root)`:
//!   no crate or package name has parentheses.
//! - **A new manifest** is a `NewModule` entry `Added`, a deleted one a `NewModule` entry `Removed`.
//! - **The state** of a manifest tells `files::mechanical_kind` whether only its dependencies changed: its parsed form
//!   without the entries (the remainder) is the same, and each entry of a pair is the same but for its version and
//!   scope (`Entry::form`). A version bump together with a new feature of a crate is a `Changed` entry, and the
//!   manifest changed beside its dependencies. A manifest that a side does not parse (no text, a syntax error) has no
//!   change and is `Unparsed`.
//! - **The text** comes with the text of the hunks (`manifest_blobs`): from the memory of the diffs, else in the batch
//!   of the summaries.

use crate::files::ChangedFile;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// What a dependency entry names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum DependencyKind {
  /// The manifest itself: a module, crate or package the commits add or delete
  NewModule,
  /// A JPS module (`*.iml` `orderEntry`) or a module a plugin descriptor depends on
  Module,
  /// A library of an `*.iml`
  Library,
  /// A plugin a plugin descriptor depends on (`depends`, `dependencies` `plugin`)
  Plugin,
  /// A content module of a plugin descriptor (`content` `module`)
  Content,
  /// A crate of a `Cargo.toml`
  Crate,
  /// A package of a `package.json`
  Package,
}

/// Where a dependency is used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum DependencyScope {
  /// The code itself: `COMPILE`, `[dependencies]`, `dependencies`, an entry of a plugin descriptor
  Main,
  /// `TEST` of an `*.iml`
  Test,
  /// `[dev-dependencies]`, `devDependencies`
  Dev,
  /// `[build-dependencies]`
  Build,
  /// `RUNTIME` of an `*.iml`
  Runtime,
  /// `PROVIDED` of an `*.iml`
  Provided,
  /// `peerDependencies`
  Peer,
}

/// How the commits change a dependency entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum DependencyChangeKind {
  Added,
  Removed,
  /// Its version or its scope changed
  Changed,
}

/// A dependency entry the commits add, remove or change
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DependencyChange {
  /// The build file, at its last path
  pub manifest: String,
  /// The module of the build file: the name of an `*.iml`, the `id` of a plugin descriptor (else its name), the
  /// `package.name` of a `Cargo.toml` or the `name` of a `package.json` (else the name of its directory, `(root)` at the
  /// root of the repository)
  pub module: String,
  pub kind: DependencyKind,
  /// The module, library, plugin, crate or package; of a `NewModule`, the module
  pub name: String,
  pub change: DependencyChangeKind,
  /// The version before and after the commits, of a `Cargo.toml` or `package.json` entry that has one
  pub version_before: Option<String>,
  pub version_after: Option<String>,
  /// The scope after the commits, of a removal the scope before
  pub scope: DependencyScope,
  /// The scope before the commits, when a `Changed` entry changes it
  pub scope_before: Option<DependencyScope>,
}

/// What the commits change in a manifest, for `files::mechanical_kind`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManifestState {
  /// A side has no text or does not parse: no dependency change is known
  Unparsed,
  /// Only its dependency entries changed
  DependenciesOnly,
  /// It changed beside its dependency entries, or the commits add or delete it
  Other,
}

/// The dependency changes of the files of a review, and the state of each manifest (by the place of its file)
pub(crate) struct Dependencies {
  pub changes: Vec<DependencyChange>,
  /// `None` for a file that is no manifest, and for a manifest whose dependencies and remainder did not change
  pub states: Vec<Option<ManifestState>>,
}

/// The formats of the manifests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
  Iml,
  PluginXml,
  Cargo,
  Npm,
}

/// The format of the manifest at `path`, by its name; a plugin descriptor is known by its root only
fn format_of(path: &str) -> Option<Format> {
  let name = path.rsplit('/').next().unwrap_or(path);
  let (_, extension) = name.rsplit_once('.')?;
  if extension.eq_ignore_ascii_case("iml") {
    Some(Format::Iml)
  } else if name == "Cargo.toml" {
    Some(Format::Cargo)
  } else if name == "package.json" {
    Some(Format::Npm)
  } else if extension.eq_ignore_ascii_case("xml") && is_descriptor_path(path) {
    Some(Format::PluginXml)
  } else {
    None
  }
}

/// Whether an `*.xml` at `path` can be a plugin descriptor: `plugin.xml`, or in a `META-INF` or `resources` directory,
/// and not in `.idea`
fn is_descriptor_path(path: &str) -> bool {
  let mut directories = path.split('/').rev().skip(1);
  let name = path.rsplit('/').next().unwrap_or(path);
  let in_idea = path.split('/').any(|part| part == ".idea");
  !in_idea && (name == "plugin.xml" || directories.any(|directory| directory == "META-INF" || directory == "resources"))
}

/// The blobs of the manifests before and after the commits, each once
pub(crate) fn manifest_blobs(files: &[ChangedFile<'_>]) -> Vec<String> {
  let mut blobs: Vec<String> = files
    .iter()
    .filter(|file| format_of(file.path).is_some())
    .flat_map(|file| [file.before, file.after])
    .flatten()
    .map(str::to_string)
    .collect();
  blobs.sort_unstable();
  blobs.dedup();
  blobs
}

/// A dependency entry of a manifest
#[derive(Debug)]
struct Entry {
  kind: DependencyKind,
  name: String,
  version: Option<String>,
  scope: DependencyScope,
  /// The table or element it is in: entries of one kind and name pair first by it
  section: String,
  /// Its form without its version and scope: a pair whose form differs changed more than its dependency
  form: String,
}

/// A manifest without its entries
#[derive(Debug, PartialEq)]
enum Rest {
  Xml(String),
  Toml(toml::Table),
  Json(serde_json::Value),
}

/// A parsed manifest
#[derive(Debug)]
struct Manifest {
  module: String,
  entries: Vec<Entry>,
  rest: Rest,
}

/// A side of a manifest
#[derive(Debug)]
enum Parsed {
  Manifest(Manifest),
  /// An `*.xml` whose root is no `idea-plugin`
  NotManifest,
  /// No text, or a syntax error
  Unparsed,
}

/// The dependency changes of `files` (`changed_files`, in tree order) and the state of each manifest; `texts` are the
/// texts of their blobs by id
pub(crate) fn dependency_changes(files: &[ChangedFile<'_>], texts: &HashMap<String, String>) -> Dependencies {
  let mut changes = Vec::new();
  let states = files
    .iter()
    .map(|file| format_of(file.path).and_then(|format| diff_manifest(file, format, texts, &mut changes)))
    .collect();
  Dependencies { changes, states }
}

/// The changes of the manifest `file` of `format` into `changes`, and its state
fn diff_manifest(file: &ChangedFile<'_>, format: Format, texts: &HashMap<String, String>, changes: &mut Vec<DependencyChange>) -> Option<ManifestState> {
  let side = |blob: Option<&str>, path: &str| -> Option<Parsed> { Some(texts.get(blob?).map_or(Parsed::Unparsed, |text| parse(format, path, text))) };
  let before = side(file.before, file.first_path);
  let after = side(file.after, file.path);
  let change = |module: &str, kind: DependencyKind, name: &str, change: DependencyChangeKind| DependencyChange {
    manifest: file.path.to_string(),
    module: module.to_string(),
    kind,
    name: name.to_string(),
    change,
    version_before: None,
    version_after: None,
    scope: DependencyScope::Main,
    scope_before: None,
  };
  match (before, after) {
    (Some(Parsed::NotManifest), _) | (_, Some(Parsed::NotManifest)) | (None, None) => None,
    (Some(Parsed::Unparsed), _) | (_, Some(Parsed::Unparsed)) => Some(ManifestState::Unparsed),
    (None, Some(Parsed::Manifest(new))) => {
      changes.push(change(&new.module, DependencyKind::NewModule, &new.module, DependencyChangeKind::Added));
      Some(ManifestState::Other)
    }
    (Some(Parsed::Manifest(old)), None) => {
      changes.push(change(&old.module, DependencyKind::NewModule, &old.module, DependencyChangeKind::Removed));
      Some(ManifestState::Other)
    }
    (Some(Parsed::Manifest(old)), Some(Parsed::Manifest(new))) => {
      let EntryDiff { changes: found, other } = diff_entries(&old.entries, &new.entries);
      let other = other || old.rest != new.rest || old.module != new.module;
      let state = if other {
        Some(ManifestState::Other)
      } else {
        (!found.is_empty()).then_some(ManifestState::DependenciesOnly)
      };
      changes.extend(found.into_iter().map(|entry| DependencyChange {
        version_before: entry.version_before,
        version_after: entry.version_after,
        scope: entry.scope,
        scope_before: entry.scope_before,
        ..change(&new.module, entry.kind, &entry.name, entry.change)
      }));
      state
    }
  }
}

/// A change of an entry, without its manifest
struct EntryChange {
  kind: DependencyKind,
  name: String,
  change: DependencyChangeKind,
  version_before: Option<String>,
  version_after: Option<String>,
  scope: DependencyScope,
  scope_before: Option<DependencyScope>,
}

/// The changes between the entries of a manifest before and after the commits
struct EntryDiff {
  /// By kind, then name
  changes: Vec<EntryChange>,
  /// A pair whose form or table changed beside its version and scope
  other: bool,
}

/// The entries of one kind and name before and after the commits
#[derive(Default)]
struct Sides<'a> {
  before: Vec<&'a Entry>,
  after: Vec<&'a Entry>,
}

/// The changes from `before` to `after`: the entries of one kind and name pair first by their section, then in their
/// order; a pair is `Changed` when its version or scope differs, and changed beside them when its form differs, or its
/// section in the same scope
fn diff_entries(before: &[Entry], after: &[Entry]) -> EntryDiff {
  let mut groups: BTreeMap<(DependencyKind, &str), Sides<'_>> = BTreeMap::new();
  for entry in before {
    groups.entry((entry.kind, &entry.name)).or_default().before.push(entry);
  }
  for entry in after {
    groups.entry((entry.kind, &entry.name)).or_default().after.push(entry);
  }
  let mut diff = EntryDiff {
    changes: Vec::new(),
    other: false,
  };
  for (
    (kind, name),
    Sides {
      before: mut olds,
      after: mut news,
    },
  ) in groups
  {
    let mut pairs: Vec<(&Entry, &Entry)> = Vec::new();
    olds.retain(|old| match news.iter().position(|new| new.section == old.section) {
      Some(index) => {
        pairs.push((old, news.remove(index)));
        false
      }
      None => true,
    });
    let paired = olds.len().min(news.len());
    pairs.extend(olds.drain(..paired).zip(news.drain(..paired)));
    let entry = |change, scope, version_before, version_after, scope_before| EntryChange {
      kind,
      name: name.to_string(),
      change,
      version_before,
      version_after,
      scope,
      scope_before,
    };
    for (old, new) in pairs {
      if old.form != new.form || (old.section != new.section && old.scope == new.scope) {
        diff.other = true;
      }
      if old.version != new.version || old.scope != new.scope {
        let scope_before = (old.scope != new.scope).then_some(old.scope);
        diff
          .changes
          .push(entry(DependencyChangeKind::Changed, new.scope, old.version.clone(), new.version.clone(), scope_before));
      }
    }
    for old in olds {
      diff.changes.push(entry(DependencyChangeKind::Removed, old.scope, old.version.clone(), None, None));
    }
    for new in news {
      diff.changes.push(entry(DependencyChangeKind::Added, new.scope, None, new.version.clone(), None));
    }
  }
  diff
}

/// The manifest of `format` at `path` with `text`
fn parse(format: Format, path: &str, text: &str) -> Parsed {
  match format {
    Format::Iml => parse_iml(path, text),
    Format::PluginXml => parse_plugin(path, text),
    Format::Cargo => parse_cargo(path, text),
    Format::Npm => parse_npm(path, text),
  }
}

/// The name of the file at `path` without its last extension
fn file_stem(path: &str) -> &str {
  let name = path.rsplit('/').next().unwrap_or(path);
  name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// The module of a manifest at the root of the repository without a name
const ROOT_MODULE: &str = "(root)";

/// The name of the directory of `path`; at the root, `ROOT_MODULE`
fn directory_name(path: &str) -> &str {
  path
    .rsplit_once('/')
    .map_or(ROOT_MODULE, |(directory, _)| directory.rsplit('/').next().unwrap_or(directory))
}

/// An element of an XML document, its attributes sorted
#[derive(Debug)]
struct Element {
  name: String,
  attributes: Vec<(String, String)>,
  children: Vec<Node>,
}

#[derive(Debug)]
enum Node {
  Element(Element),
  /// A text, a CDATA section, an entity reference or a comment
  Text(String),
}

impl Element {
  fn attribute(&self, key: &str) -> Option<&str> {
    self.attributes.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
  }

  fn elements(&self) -> impl Iterator<Item = &Self> {
    self.children.iter().filter_map(|node| match node {
      Node::Element(element) => Some(element),
      Node::Text(_) => None,
    })
  }

  /// Its text, trimmed
  fn text(&self) -> String {
    let text: String = self
      .children
      .iter()
      .filter_map(|node| match node {
        Node::Text(text) => Some(text.as_str()),
        Node::Element(_) => None,
      })
      .collect();
    text.trim().to_string()
  }

  /// Its form, the same for the same element whatever the order of its attributes and its whitespace
  fn canonical(&self) -> String {
    self.canonical_without(None)
  }

  /// Its form without its attribute `skipped`
  fn canonical_without(&self, skipped: Option<&str>) -> String {
    let mut out = String::new();
    self.write_canonical(&mut out, skipped);
    out
  }

  fn write_canonical(&self, out: &mut String, skipped: Option<&str>) {
    out.push('<');
    out.push_str(&self.name);
    for (key, value) in self.attributes.iter().filter(|(key, _)| Some(key.as_str()) != skipped) {
      out.push(' ');
      out.push_str(key);
      out.push_str("=\"");
      out.push_str(value);
      out.push('"');
    }
    out.push('>');
    for child in &self.children {
      match child {
        Node::Element(element) => element.write_canonical(out, None),
        Node::Text(text) => out.push_str(text),
      }
    }
    out.push_str("</");
    out.push_str(&self.name);
    out.push('>');
  }
}

/// The element of `start`, with no child
fn element_of(start: &BytesStart<'_>) -> Option<Element> {
  let mut attributes = Vec::new();
  for attribute in start.attributes() {
    let attribute = attribute.ok()?;
    let value = attribute.normalized_value(XmlVersion::Implicit1_0).ok()?;
    attributes.push((attribute.key.as_ref().to_string(), value.into_owned()));
  }
  attributes.sort();
  Some(Element {
    name: start.name().as_ref().to_string(),
    attributes,
    children: Vec::new(),
  })
}

/// The root of the XML document `text`, text trimmed; `None` for a syntax error. The comments and the instructions
/// outside the root are left out
fn parse_xml(text: &str) -> Option<Element> {
  let mut reader = Reader::from_str(text);
  reader.config_mut().trim_text(true);
  let mut stack: Vec<Element> = Vec::new();
  let mut root: Option<Element> = None;
  loop {
    let node = match reader.read_event().ok()? {
      Event::Start(start) => {
        stack.push(element_of(&start)?);
        continue;
      }
      Event::Empty(start) => Node::Element(element_of(&start)?),
      Event::End(_) => Node::Element(stack.pop()?),
      Event::Text(text) => Node::Text(text.xml10_content().into_owned()),
      Event::CData(data) => Node::Text(data.into_inner().into_owned()),
      Event::GeneralRef(reference) => Node::Text(format!("&{};", reference.into_inner())),
      Event::Comment(comment) => Node::Text(format!("<!--{}-->", comment.into_inner())),
      Event::Decl(_) | Event::PI(_) | Event::DocType(_) => continue,
      Event::Eof => break,
    };
    match (stack.last_mut(), node) {
      (Some(parent), node) => parent.children.push(node),
      (None, Node::Element(element)) if root.is_none() => root = Some(element),
      (None, Node::Text(_)) => {}
      (None, Node::Element(_)) => return None,
    }
  }
  if stack.is_empty() { root } else { None }
}

/// The scope of an `orderEntry` by its `scope`
fn iml_scope(scope: Option<&str>) -> DependencyScope {
  match scope {
    Some("TEST") => DependencyScope::Test,
    Some("RUNTIME") => DependencyScope::Runtime,
    Some("PROVIDED") => DependencyScope::Provided,
    _ => DependencyScope::Main,
  }
}

/// The entry of an `orderEntry` of an `*.iml`: a module, a library, or a module library with a name
fn iml_entry(element: &Element) -> Option<Entry> {
  if element.name != "orderEntry" {
    return None;
  }
  let (kind, name) = match element.attribute("type")? {
    "module" => (DependencyKind::Module, element.attribute("module-name")?),
    "library" => (DependencyKind::Library, element.attribute("name")?),
    "module-library" => (DependencyKind::Library, element.elements().find(|child| child.name == "library")?.attribute("name")?),
    _ => return None,
  };
  let scope = element.attribute("scope");
  Some(Entry {
    kind,
    name: name.to_string(),
    version: None,
    scope: iml_scope(scope),
    section: scope.unwrap_or("COMPILE").to_string(),
    form: element.canonical_without(Some("scope")),
  })
}

/// Moves the entries of the elements under `element` into `entries`
fn take_iml_entries(element: &mut Element, entries: &mut Vec<Entry>) {
  element.children.retain_mut(|node| {
    let Node::Element(child) = node else { return true };
    if let Some(entry) = iml_entry(child) {
      entries.push(entry);
      return false;
    }
    take_iml_entries(child, entries);
    true
  });
}

/// An `*.iml`: its module is the name of the file
fn parse_iml(path: &str, text: &str) -> Parsed {
  let Some(mut root) = parse_xml(text).filter(|root| root.name == "module") else {
    return Parsed::Unparsed;
  };
  let mut entries = Vec::new();
  take_iml_entries(&mut root, &mut entries);
  Parsed::Manifest(Manifest {
    module: file_stem(path).to_string(),
    entries,
    rest: Rest::Xml(root.canonical()),
  })
}

/// The entry of a child of the `dependencies` or the `content` (`container`) of a plugin descriptor
fn plugin_entry(container: &str, element: &Element) -> Option<Entry> {
  let (kind, name) = match (container, element.name.as_str()) {
    ("dependencies", "plugin") => (DependencyKind::Plugin, element.attribute("id")?),
    ("dependencies", "module") => (DependencyKind::Module, element.attribute("name")?),
    ("content", "module") => (DependencyKind::Content, element.attribute("name")?),
    _ => return None,
  };
  Some(Entry {
    kind,
    name: name.to_string(),
    version: None,
    scope: DependencyScope::Main,
    section: container.to_string(),
    form: element.canonical(),
  })
}

/// A plugin descriptor: its module is its `id`, else the name of the file; `NotManifest` when its root is no
/// `idea-plugin`. An emptied `dependencies` or `content` without attributes is left out of the remainder
fn parse_plugin(path: &str, text: &str) -> Parsed {
  let Some(mut root) = parse_xml(text) else {
    return Parsed::Unparsed;
  };
  if root.name != "idea-plugin" {
    return Parsed::NotManifest;
  }
  let module = root.elements().find(|child| child.name == "id").map(Element::text).filter(|id| !id.is_empty());
  let module = module.unwrap_or_else(|| file_stem(path).to_string());
  let mut entries = Vec::new();
  for node in &mut root.children {
    let Node::Element(element) = node else { continue };
    if element.name == "dependencies" || element.name == "content" {
      let container = element.name.clone();
      element.children.retain(|node| match node {
        Node::Element(child) => plugin_entry(&container, child).map(|entry| entries.push(entry)).is_none(),
        Node::Text(_) => true,
      });
    }
  }
  root.children.retain(|node| match node {
    Node::Element(element) if element.name == "depends" => {
      entries.push(Entry {
        kind: DependencyKind::Plugin,
        name: element.text(),
        version: None,
        scope: DependencyScope::Main,
        section: "depends".to_string(),
        form: element.canonical(),
      });
      false
    }
    Node::Element(element) => !(matches!(element.name.as_str(), "dependencies" | "content") && element.children.is_empty() && element.attributes.is_empty()),
    Node::Text(_) => true,
  });
  Parsed::Manifest(Manifest {
    module,
    entries,
    rest: Rest::Xml(root.canonical()),
  })
}

/// The dependency tables of a `Cargo.toml` and their scopes
const CARGO_TABLES: [(&str, DependencyScope); 3] = [
  ("dependencies", DependencyScope::Main),
  ("dev-dependencies", DependencyScope::Dev),
  ("build-dependencies", DependencyScope::Build),
];

/// Moves the entries of the table `key` of `table` into `entries`, as `section`
fn take_cargo_entries(table: &mut toml::Table, key: &str, section: &str, scope: DependencyScope, entries: &mut Vec<Entry>) {
  if !matches!(table.get(key), Some(toml::Value::Table(_))) {
    return;
  }
  let Some(toml::Value::Table(dependencies)) = table.remove(key) else { return };
  for (name, value) in dependencies {
    // a plain version and a table of the version alone have the same form
    let (version, form) = match value {
      toml::Value::String(version) => (Some(version), String::new()),
      toml::Value::Table(mut fields) => {
        let version = fields.remove("version").and_then(|version| version.as_str().map(str::to_string));
        let form = if fields.is_empty() { String::new() } else { toml::Value::Table(fields).to_string() };
        (version, form)
      }
      other => (None, other.to_string()),
    };
    entries.push(Entry {
      kind: DependencyKind::Crate,
      name,
      version,
      scope,
      section: section.to_string(),
      form,
    });
  }
}

/// A `Cargo.toml`: its module is its `package.name`, else the name of its directory. A `target` table emptied by the
/// move of its entries is left out of the remainder
fn parse_cargo(path: &str, text: &str) -> Parsed {
  let Ok(mut table) = text.parse::<toml::Table>() else {
    return Parsed::Unparsed;
  };
  let package_name = table.get("package").and_then(|package| package.get("name")).and_then(toml::Value::as_str);
  let module = package_name.map_or_else(|| directory_name(path).to_string(), str::to_string);
  let mut entries = Vec::new();
  for (key, scope) in CARGO_TABLES {
    take_cargo_entries(&mut table, key, key, scope, &mut entries);
  }
  if let Some(toml::Value::Table(targets)) = table.get_mut("target") {
    for (target, value) in targets.iter_mut() {
      let toml::Value::Table(target_table) = value else { continue };
      for (key, scope) in CARGO_TABLES {
        take_cargo_entries(target_table, key, &format!("target.{target}.{key}"), scope, &mut entries);
      }
    }
    targets.retain(|_, value| !matches!(value, toml::Value::Table(target_table) if target_table.is_empty()));
  }
  if matches!(table.get("target"), Some(toml::Value::Table(targets)) if targets.is_empty()) {
    table.remove("target");
  }
  if let Some(toml::Value::Table(workspace)) = table.get_mut("workspace") {
    take_cargo_entries(workspace, "dependencies", "workspace.dependencies", DependencyScope::Main, &mut entries);
  }
  Parsed::Manifest(Manifest {
    module,
    entries,
    rest: Rest::Toml(table),
  })
}

/// The dependency objects of a `package.json` and their scopes
const NPM_TABLES: [(&str, DependencyScope); 3] = [
  ("dependencies", DependencyScope::Main),
  ("devDependencies", DependencyScope::Dev),
  ("peerDependencies", DependencyScope::Peer),
];

/// A `package.json`: its module is its `name`, else the name of its directory
fn parse_npm(path: &str, text: &str) -> Parsed {
  let Ok(serde_json::Value::Object(mut object)) = serde_json::from_str::<serde_json::Value>(text) else {
    return Parsed::Unparsed;
  };
  let module = object.get("name").and_then(serde_json::Value::as_str);
  let module = module.map_or_else(|| directory_name(path).to_string(), str::to_string);
  let mut entries = Vec::new();
  for (key, scope) in NPM_TABLES {
    if !matches!(object.get(key), Some(serde_json::Value::Object(_))) {
      continue;
    }
    let Some(serde_json::Value::Object(dependencies)) = object.remove(key) else { continue };
    for (name, value) in dependencies {
      entries.push(Entry {
        kind: DependencyKind::Package,
        name,
        version: value.as_str().map(str::to_string),
        scope,
        section: key.to_string(),
        form: if value.is_string() { String::new() } else { value.to_string() },
      });
    }
  }
  Parsed::Manifest(Manifest {
    module,
    entries,
    rest: Rest::Json(serde_json::Value::Object(object)),
  })
}
