//! The half of a summary walk that knows no parser: what any walk over any syntax tree (tree-sitter's in `extract.rs`,
//! psi-syntax's in `kotlin/`, `java/` and `starlark/`) collects and how it becomes a `FileSummary`.
//!
//! - `SummaryBuilder`: the names (interned in the order the walk meets them, sorted by `finish`), the package, imports
//!   and import lines, the declarations in source order, the references, the interned shapes, the error bytes, the
//!   receivers whose type `finish` decides, and the names a TypeScript `export { x }` publishes. A walk pushes facts
//!   as it finds them; the rules of what is a declaration or a reference are the walk's.
//! - `Bindings`: the scopes of a walk and the names bound in them (a parameter, a local, a type parameter), so that a
//!   bound name is no reference; a body scope makes the declarations in it locals.
//! - `finish`: the steps after the walk, the same for every parser: a Rust `impl` member to its type, exported
//!   TypeScript declarations public, the implicit `()` of a JVM class without a constructor, the receiver types of
//!   member references, then the names sorted and every id remapped, the references sorted and deduplicated.
//! - Lines are 1-based. `end_line` is the last line of a node as tree-sitter's rows give it: a node that ends with its
//!   line break ends on the line before.

use crate::language::Language;
use crate::summary::{Constructor, DeclKind, Declaration, FileSummary, Import, ImportTarget, LineSpan, MemberKind, Reach, Reference, Role, Shape, Signature, TypeRef};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

/// The last names of the annotations of a JVM function a test framework runs (`Declaration::test`)
pub(crate) const TEST_ANNOTATIONS: &[&str] = &["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate"];

/// The last names of the annotations that mark a JVM declaration as no stable API whatever their qualifier
/// (`Declaration::unstable`): `org.jetbrains.annotations`, Guava and `AndroidX` name them alike
pub(crate) const UNSTABLE_ANNOTATIONS: &[&str] = &["VisibleForTesting", "TestOnly"];

/// The annotations of `API_STATUS` that mark a declaration as no stable API (`Declaration::unstable`)
const API_STATUS_UNSTABLE: &[&str] = &["Internal", "Experimental"];

/// The class whose nested annotations say the status of an API in the code of `JetBrains`
const API_STATUS: &[&str] = &["org", "jetbrains", "annotations", "ApiStatus"];

/// The summary of a file before it is parsed: its size, and `truncated` until a walk is done (`finish` does not clear
/// it; the caller does once the parse succeeded)
pub(crate) fn empty_summary(language: Language, source: &[u8]) -> FileSummary {
  FileSummary {
    language,
    package: None,
    jvm_name: None,
    names: Vec::new(),
    imports: Vec::new(),
    import_lines: Vec::new(),
    declarations: Vec::new(),
    references: Vec::new(),
    shapes: Vec::new(),
    error_bytes: 0,
    total_bytes: u32::try_from(source.len()).unwrap_or(u32::MAX),
    lines: count_lines(source),
    truncated: true,
  }
}

/// The lines of a file: its `\n`s, and one more for a last line without one
pub(crate) fn count_lines(source: &[u8]) -> u32 {
  let newlines = source.iter().fold(0, |count, it| count + usize::from(*it == b'\n'));
  let partial = usize::from(source.last().is_some_and(|it| *it != b'\n'));
  u32::try_from(newlines + partial).unwrap_or(u32::MAX)
}

/// The column of the byte at `offset`, in UTF-16 code units from the start of its line, as a view counts them: the
/// line starts after the last `\n` before it, so a `\r` of a CRLF end is on the line it ends, not the next one. Bytes
/// that are no UTF-8 count as one each, as a lossy read shows them.
///
/// The same arithmetic as `psi_builder::document::column`, which accio's index answers from; the two meet when
/// `psi-builder` stops being optional here, as Rust and TypeScript leave tree-sitter for it
pub(crate) fn column_at(source: &[u8], offset: usize) -> u32 {
  let offset = offset.min(source.len());
  let start = source[..offset].iter().rposition(|&byte| byte == b'\n').map_or(0, |it| it + 1);
  let before = String::from_utf8_lossy(&source[start..offset]);
  u32::try_from(before.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// The last line of a node of `source` that spans `start..end`, given the 1-based line of an offset: a node that ends
/// right after a `\n`, on a later line than it starts, ends on the line before (as `extract.rs` reads tree-sitter's end
/// position at column 0)
pub(crate) fn end_line(line_of: impl Fn(usize) -> u32, source: &[u8], start: usize, end: usize) -> u32 {
  let (first, last) = (line_of(start), line_of(end));
  if end > 0 && source.get(end - 1) == Some(&b'\n') && last > first {
    last - 1
  } else {
    last
  }
}

/// A declaration of `name` with every fact a walk may add left empty (no signature, no supertypes, nothing of
/// overriding or the JVM), its lines and `in_error` too
pub(crate) const fn blank_declaration(name: u32, kind: DeclKind, reach: Reach, member: MemberKind, container: Option<u32>) -> Declaration {
  Declaration {
    name,
    kind,
    container,
    receiver: None,
    reach,
    member,
    signature: None,
    overrides: false,
    overridable: false,
    jvm_static: false,
    jvm_field: false,
    jvm_name: false,
    lateinit: false,
    test: false,
    entry_point: false,
    unstable: false,
    setter: None,
    supertypes: Vec::new(),
    value_type: None,
    value_constructed: false,
    constructors: None,
    header: None,
    start_line: 0,
    end_line: 0,
    name_line: 0,
    name_column: 0,
    in_error: false,
  }
}

/// Whether a JVM function of `name` is a `JUnit` 3 test: `test` and a capital or `_` after it, `no_parameters`, not
/// local, in a class `class` named `…Test`, `…Tests` or `…TestCase` or one whose supertypes (their last names) end in
/// `TestCase`
pub(crate) fn is_junit3_test<'a>(name: &str, no_parameters: bool, local: bool, class: Option<&str>, mut supertypes: impl Iterator<Item = &'a str>) -> bool {
  let named = name
    .strip_prefix("test")
    .and_then(|rest| rest.chars().next())
    .is_some_and(|c| c.is_ascii_uppercase() || c == '_');
  if !named || !no_parameters || local {
    return false;
  }
  class.is_some_and(|it| it.ends_with("Test") || it.ends_with("Tests") || it.ends_with("TestCase")) || supertypes.any(|it| it.ends_with("TestCase"))
}

/// A type of a value (`JvmHooks::value_type`): its path, and whether a constructor call of the initializer gave it
#[derive(Debug, Clone, Default)]
pub(crate) struct Typed {
  pub(crate) path: Vec<u32>,
  pub(crate) constructed: bool,
}

/// A member reference whose receiver may have a type (`Reference::receiver_type`), decided by `finish`
enum TypedReceiver {
  /// A bound name of this type and binding (`Binding::serial`)
  Bound { reference: usize, serial: u32, type_path: Typed },
  /// A property or field `[m]` or `[this, m]`, not enclosed
  Property { reference: usize },
}

/// What a walk found, before `finish` makes it a `FileSummary`
pub(crate) struct SummaryBuilder<'t> {
  name_ids: HashMap<Cow<'t, str>, u32>,
  names: Vec<Cow<'t, str>>,
  pub(crate) package: Option<String>,
  pub(crate) jvm_name: Option<String>,
  pub(crate) imports: Vec<Import>,
  pub(crate) import_lines: Vec<LineSpan>,
  /// In source order; `Declaration::container` indexes here
  pub(crate) declarations: Vec<Declaration>,
  pub(crate) references: Vec<Reference>,
  shapes: Vec<Shape>,
  shape_ids: HashMap<Shape, u32>,
  pub(crate) error_bytes: u32,
  receivers: Vec<TypedReceiver>,
  /// The bindings, and the (container, name) of properties, that a smart cast may give another type
  cast_bindings: HashSet<u32>,
  cast_names: HashSet<(Option<u32>, u32)>,
  /// TypeScript `export { x }`: top-level names whose declarations become public
  exported: Vec<u32>,
}

impl<'t> SummaryBuilder<'t> {
  pub(crate) fn new() -> Self {
    Self {
      name_ids: HashMap::new(),
      names: Vec::new(),
      package: None,
      jvm_name: None,
      imports: Vec::new(),
      import_lines: Vec::new(),
      declarations: Vec::new(),
      references: Vec::new(),
      shapes: Vec::new(),
      shape_ids: HashMap::new(),
      error_bytes: 0,
      receivers: Vec::new(),
      cast_bindings: HashSet::new(),
      cast_names: HashSet::new(),
      exported: Vec::new(),
    }
  }

  /// The id of a name, interned at its first use
  pub(crate) fn intern(&mut self, name: &'t str) -> u32 {
    if let Some(id) = self.name_ids.get(name) {
      return *id;
    }
    let id = u32::try_from(self.names.len()).unwrap_or(u32::MAX);
    self.names.push(Cow::Borrowed(name));
    self.name_ids.insert(Cow::Borrowed(name), id);
    id
  }

  /// The id of an owned, decoded name; `intern` keeps source-backed names borrowed
  pub(crate) fn intern_owned(&mut self, name: String) -> u32 {
    if let Some(id) = self.name_ids.get(name.as_str()) {
      return *id;
    }
    let id = u32::try_from(self.names.len()).unwrap_or(u32::MAX);
    self.names.push(Cow::Owned(name.clone()));
    self.name_ids.insert(Cow::Owned(name), id);
    id
  }

  /// The ids of a path of names
  pub(crate) fn ids(&mut self, path: &[&'t str]) -> Vec<u32> {
    path.iter().map(|it| self.intern(it)).collect()
  }

  /// The id of a name already interned
  pub(crate) fn name_id(&self, name: &str) -> Option<u32> {
    self.name_ids.get(name).copied()
  }

  /// The name of an id; empty for an unknown one
  pub(crate) fn name(&self, id: u32) -> &'t str {
    self.names.get(id as usize).map_or("", |name| match name {
      Cow::Borrowed(name) => *name,
      Cow::Owned(_) => "",
    })
  }

  /// The text of an id, including an owned decoded name
  pub(crate) fn name_text(&self, id: u32) -> &str {
    self.names.get(id as usize).map_or("", |name| name.as_ref())
  }

  /// The id of a shape, one per distinct shape
  pub(crate) fn intern_shape(&mut self, shape: Shape) -> u32 {
    if let Some(id) = self.shape_ids.get(&shape) {
      return *id;
    }
    let id = u32::try_from(self.shapes.len()).unwrap_or(u32::MAX);
    self.shapes.push(shape.clone());
    self.shape_ids.insert(shape, id);
    id
  }

  /// Adds a declaration; its id, which `Declaration::container` and `Reference::container` take
  pub(crate) fn add_declaration(&mut self, declaration: Declaration) -> u32 {
    let id = u32::try_from(self.declarations.len()).unwrap_or(u32::MAX);
    self.declarations.push(declaration);
    id
  }

  /// Adds a reference as it is (the walk has decided it is one: not bound, not reserved); its index, for
  /// `bound_receiver` and `property_receiver`
  #[expect(clippy::too_many_arguments, reason = "the fields of a reference that a walk decides")]
  pub(crate) fn add_reference(
    &mut self,
    name: u32,
    line: u32,
    role: Role,
    qualifier: Option<Vec<u32>>,
    container: Option<u32>,
    shape: Option<u32>,
    coroutine: Option<bool>,
  ) -> usize {
    self.references.push(Reference {
      name,
      line,
      role,
      qualifier,
      container,
      shape,
      receiver_type: None,
      receiver_constructed: false,
      coroutine,
    });
    self.references.len() - 1
  }

  /// The receiver of the member reference at `reference` is a bound name of this type, unless a smart cast of its
  /// binding (`cast_binding`) may change it
  pub(crate) fn bound_receiver(&mut self, reference: usize, serial: u32, type_path: Typed) {
    self.receivers.push(TypedReceiver::Bound { reference, serial, type_path });
  }

  /// The receiver of the member reference at `reference` is a property or field `[m]` or `[this, m]` (its qualifier)
  /// of a class around it, whose declared type `finish` finds (`Reference::receiver_type`)
  pub(crate) fn property_receiver(&mut self, reference: usize) {
    self.receivers.push(TypedReceiver::Property { reference });
  }

  /// A smart cast may give the binding `serial` another type in its scope
  pub(crate) fn cast_binding(&mut self, serial: u32) {
    self.cast_bindings.insert(serial);
  }

  /// A smart cast may give the property `name` another type in the code of `container`
  pub(crate) fn cast_name(&mut self, container: Option<u32>, name: u32) {
    self.cast_names.insert((container, name));
  }

  /// TypeScript `export { x }`: a top-level declaration of `x` private to the file becomes public
  pub(crate) fn export_name(&mut self, name: &'t str) {
    let id = self.intern(name);
    self.exported.push(id);
  }

  /// `import a.b.C as D`: the `Import` of `name` under `path`; the walk adds the `Import` reference to it
  pub(crate) fn add_import_name(&mut self, path: &[&'t str], name: &'t str, alias: Option<&'t str>, exported: bool, line: u32) {
    let path = self.ids(path);
    let target = self.intern(name);
    let alias = alias.map(|it| self.intern(it));
    self.imports.push(Import {
      path,
      target: ImportTarget::Name(target),
      alias,
      exported,
      specifier: None,
      line,
    });
  }

  /// An import a walk built itself: a TypeScript or Starlark one, whose module is a `specifier`
  pub(crate) fn add_import(&mut self, import: Import) {
    self.imports.push(import);
  }

  /// `import a.b.*`
  pub(crate) fn add_import_all(&mut self, path: &[&'t str], exported: bool, line: u32) {
    let path = self.ids(path);
    self.imports.push(Import {
      path,
      target: ImportTarget::All,
      alias: None,
      exported,
      specifier: None,
      line,
    });
  }

  /// Adds a constructor to the type `decl` (a Kotlin secondary constructor, a Java constructor); a `canonical` one,
  /// the canonical constructor of a Java record declared again or compact, replaces the one of its header, the first.
  /// Any other is added, also beside one of the same types as the summary has them (`Stamp(java.util.Date)` and
  /// `Stamp(java.sql.Date)` of the same last names, `T` and `List<T>` of any type), which sync-core then can't tell
  /// apart. Nothing for a type whose constructors are unknown or that has none (an enum)
  pub(crate) fn add_constructor(&mut self, decl: u32, constructor: Constructor, canonical: bool) {
    let Some(constructors) = self.declarations.get_mut(decl as usize).and_then(|it| it.constructors.as_mut()) else {
      return;
    };
    match constructors.first_mut().filter(|_| canonical) {
      Some(header) => *header = constructor,
      None => constructors.push(constructor),
    }
  }

  /// Whether an annotation of this path, as written, marks a declaration as no stable API (`Declaration::unstable`):
  /// one of `UNSTABLE_ANNOTATIONS` by its last name; `Internal` or `Experimental` of `ApiStatus` qualified by
  /// `ApiStatus` or by its full name, or bare when an import of the file brings it in from
  /// `org.jetbrains.annotations.ApiStatus` (by its name, an alias of it, or `ApiStatus.*`), so that another `Internal`
  /// is none
  pub(crate) fn unstable_annotation(&self, path: &[&str]) -> bool {
    let Some((last, qualifier)) = path.split_last() else { return false };
    if UNSTABLE_ANNOTATIONS.contains(last) {
      return true;
    }
    if !qualifier.is_empty() {
      return API_STATUS_UNSTABLE.contains(last) && (qualifier == ["ApiStatus"] || qualifier == API_STATUS);
    }
    self.imports.iter().any(|import| {
      let from_api_status = import.path.len() == API_STATUS.len() && import.path.iter().zip(API_STATUS).all(|(id, segment)| self.name(*id) == *segment);
      from_api_status
        && match import.target {
          ImportTarget::Name(target) => API_STATUS_UNSTABLE.contains(&self.name(target)) && self.name(import.alias.unwrap_or(target)) == *last,
          ImportTarget::All => API_STATUS_UNSTABLE.contains(last),
          ImportTarget::Default | ImportTarget::Namespace => false,
        }
    })
  }

  /// Whether a parameter type is not nullable and of these last names and array dimensions (`[String]` and 1 of Java
  /// `String[]`)
  pub(crate) fn type_is(&self, type_ref: Option<&TypeRef>, names: &[&str], arrays: u8) -> bool {
    type_ref.is_some_and(|it| !it.nullable && it.arrays == arrays && it.names.len() == names.len() && it.names.iter().zip(names).all(|(id, name)| self.name(*id) == *name))
  }

  /// The summary of the walk into `summary`, made by `empty_summary`; the reach of the implicit `()` of a JVM class
  /// without a constructor, given the reach of the class (`None` in other languages)
  pub(crate) fn finish(mut self, summary: &mut FileSummary, implicit_constructor: Option<fn(Reach) -> Reach>) {
    if summary.language == Language::Rust {
      self.resolve_impl_members();
    }
    self.publish_exported();
    if let Some(reach_of) = implicit_constructor {
      self.add_implicit_constructors(reach_of);
    }
    self.receiver_types();
    let mut order: Vec<u32> = (0..u32::try_from(self.names.len()).unwrap_or(u32::MAX)).collect();
    order.sort_unstable_by(|left, right| self.name_text(*left).cmp(self.name_text(*right)));
    let mut remap = vec![0u32; order.len()];
    for (new, old) in order.iter().enumerate() {
      remap[*old as usize] = u32::try_from(new).unwrap_or(u32::MAX);
    }
    let map = |id: &mut u32| *id = remap[*id as usize];
    let map_type = |type_ref: &mut Option<TypeRef>| type_ref.iter_mut().flat_map(|it| &mut it.names).for_each(map);
    for import in &mut self.imports {
      import.path.iter_mut().for_each(map);
      if let ImportTarget::Name(name) = &mut import.target {
        map(name);
      }
      import.alias.iter_mut().for_each(map);
    }
    for declaration in &mut self.declarations {
      map(&mut declaration.name);
      declaration.receiver.iter_mut().for_each(map);
      let constructors = declaration.constructors.iter_mut().flatten().map(|it| &mut it.signature);
      for parameter in declaration.signature.iter_mut().chain(constructors).flat_map(|it| &mut it.parameters) {
        map(&mut parameter.name);
        map_type(&mut parameter.type_ref);
      }
      declaration.supertypes.iter_mut().flatten().for_each(map);
      declaration.value_type.iter_mut().flatten().for_each(map);
    }
    for shape in &mut self.shapes {
      match shape {
        Shape::Override { receiver, parameters, .. } => {
          receiver.iter_mut().for_each(map);
          parameters.iter_mut().for_each(map_type);
        }
        Shape::Call { arguments, .. } => arguments.iter_mut().filter_map(|(name, _)| name.as_mut()).for_each(map),
      }
    }
    for reference in &mut self.references {
      map(&mut reference.name);
      reference.qualifier.iter_mut().flatten().for_each(map);
      reference.receiver_type.iter_mut().flatten().for_each(map);
    }
    self.references.sort_unstable();
    self.references.dedup();
    summary.names = order.iter().map(|it| self.name_text(*it).to_string()).collect();
    summary.package = self.package;
    summary.jvm_name = self.jvm_name;
    summary.imports = self.imports;
    summary.import_lines = self.import_lines;
    summary.declarations = self.declarations;
    summary.references = self.references;
    summary.shapes = self.shapes;
    summary.error_bytes = self.error_bytes;
  }

  /// The types of the receivers of member references (`Reference::receiver_type`): a bound name's, unless a smart
  /// cast may change it; a property's or a field's of the class around the reference that declares it, the innermost
  /// first, not past a class with a supertype (whose `m` comes first), a nested class that isn't inner or a companion
  /// (whose code sees no `m` of the class around it) or an extension function or property of another receiver (whose
  /// members come first; `this` is that receiver), and for `this.m` only of the innermost class, with no extension
  /// between
  fn receiver_types(&mut self) {
    let this = self.name_ids.get("this").copied();
    // (class, name) -> the property or field of that name it declares
    let properties: HashMap<(u32, u32), &Declaration> = self
      .declarations
      .iter()
      .filter(|declaration| declaration.kind == DeclKind::Value)
      .filter_map(|declaration| Some(((declaration.container?, declaration.name), declaration)))
      .collect();
    let mut types = Vec::new();
    for receiver in &self.receivers {
      types.push(match receiver {
        TypedReceiver::Bound { reference, serial, type_path } => (*reference, (!self.cast_bindings.contains(serial)).then(|| type_path.clone())),
        TypedReceiver::Property { reference } => (*reference, self.property_type(&self.references[*reference], this, &properties)),
      });
    }
    for (index, type_path) in types {
      let reference = &mut self.references[index];
      reference.receiver_constructed = type_path.as_ref().is_some_and(|it| it.constructed);
      reference.receiver_type = type_path.map(|it| it.path);
    }
  }

  fn property_type(&self, reference: &Reference, this: Option<u32>, properties: &HashMap<(u32, u32), &Declaration>) -> Option<Typed> {
    let (own, name) = match reference.qualifier.as_deref()? {
      [name] => (false, *name),
      [first, name] if Some(*first) == this => (true, *name),
      _ => return None,
    };
    if self.cast_names.contains(&(reference.container, name)) {
      return None;
    }
    let at = |index: u32| self.declarations.get(index as usize).map(|declaration| (index, declaration));
    let around = std::iter::successors(reference.container.and_then(at), |(_, declaration)| declaration.container.and_then(at));
    // the receiver of an extension function or property around the use, whose `this` and members come first
    let mut extension = None;
    for (index, declaration) in around {
      if !matches!(declaration.kind, DeclKind::Type | DeclKind::EnumEntry) {
        if let Some(receiver) = declaration.receiver.filter(|_| extension.is_none()) {
          extension = Some(receiver);
        }
        continue;
      }
      if (own && extension.is_some()) || extension.is_some_and(|receiver| receiver != declaration.name) {
        return None;
      }
      if let Some(property) = properties.get(&(index, name)) {
        return property.value_type.clone().map(|path| Typed {
          path,
          constructed: property.value_constructed,
        });
      }
      // an inherited `m` comes first; the code of a nested class, a companion or an enum entry sees no `m` around it
      if own || declaration.kind == DeclKind::EnumEntry || !declaration.supertypes.is_empty() || (declaration.member == MemberKind::Static && declaration.container.is_some()) {
        return None;
      }
    }
    None
  }

  /// A member of a Rust `impl` belongs to the declaration of its type when this file has it
  fn resolve_impl_members(&mut self) {
    let types: HashMap<(u32, Option<u32>), u32> = self
      .declarations
      .iter()
      .enumerate()
      .filter(|(_, it)| it.kind == DeclKind::Type)
      .filter_map(|(index, it)| Some(((it.name, it.container), u32::try_from(index).ok()?)))
      .collect();
    for declaration in &mut self.declarations {
      if let Some(receiver) = declaration.receiver
        && let Some(owner) = types.get(&(receiver, declaration.container))
      {
        declaration.container = Some(*owner);
      }
    }
  }

  fn publish_exported(&mut self) {
    for declaration in &mut self.declarations {
      if declaration.container.is_none() && declaration.reach == Reach::File && self.exported.contains(&declaration.name) {
        declaration.reach = Reach::Public;
      }
    }
  }

  /// A class that declares no constructor has the implicit `()`
  fn add_implicit_constructors(&mut self, reach_of: fn(Reach) -> Reach) {
    for declaration in &mut self.declarations {
      if let Some(constructors) = declaration.constructors.as_mut().filter(|it| it.is_empty()) {
        constructors.push(Constructor {
          signature: Signature { parameters: Vec::new() },
          reach: reach_of(declaration.reach),
        });
      }
    }
  }
}

/// What the parents decided about their children before the walk reaches them (a declaration's name, the callee of a
/// call), by node, each with the index of the frame that decided it: a mark lasts while that frame does
pub(crate) struct Marks<K, M>(Vec<(K, M, usize)>);

impl<K, M> Default for Marks<K, M> {
  fn default() -> Self {
    Self(Vec::new())
  }
}

impl<K: Copy + PartialEq, M> Marks<K, M> {
  /// Marks `node`, a child of the node of the frame at index `frame`
  pub(crate) fn set(&mut self, node: K, mark: M, frame: usize) {
    self.0.push((node, mark, frame));
  }

  /// The mark of `node`, the one set last, which it no longer has
  pub(crate) fn take(&mut self, node: K) -> Option<M> {
    let index = self.0.iter().rposition(|it| it.0 == node)?;
    Some(self.0.remove(index).1)
  }

  pub(crate) fn contains(&self, node: K) -> bool {
    self.0.iter().any(|it| it.0 == node)
  }

  /// The walk leaves its frame at index `frame`: the marks it and the frames below it set are gone
  pub(crate) fn leave_frame(&mut self, frame: usize) {
    while self.0.last().is_some_and(|it| it.2 >= frame) {
      self.0.pop();
    }
  }
}

/// A scope of `Bindings`
struct Scope {
  bound_start: usize,
  body: bool,
}

/// A name bound in a scope, with the type it was declared with (`Declaration::value_type`)
pub(crate) struct Binding {
  pub(crate) name: u32,
  pub(crate) type_path: Option<Typed>,
  /// A parameter of a Kotlin primary constructor, which the initializers of the class see but not its functions and
  /// accessors: the depth of the walk's containers it was bound at
  pub(crate) initializers: Option<usize>,
  /// Unique per binding of the walk, for the smart casts that drop its type (`SummaryBuilder::cast_binding`)
  pub(crate) serial: u32,
}

/// The scopes of a walk and the names bound in them: a bound name is no reference while its scope lasts
#[derive(Default)]
pub(crate) struct Bindings {
  scopes: Vec<Scope>,
  bound: Vec<Binding>,
  counts: HashMap<u32, u32>,
  serials: u32,
  body_scopes: u32,
  /// (frame index, name, type): the names `bind_later` binds when the walk leaves that frame
  pending: Vec<(usize, u32, Option<Typed>)>,
}

impl Bindings {
  /// Opens a scope; a `body` one (a function, a lambda, a block of code) makes the declarations in it locals
  pub(crate) fn push(&mut self, body: bool) {
    self.scopes.push(Scope {
      bound_start: self.bound.len(),
      body,
    });
    if body {
      self.body_scopes += 1;
    }
  }

  /// Closes the innermost scope and unbinds what it bound
  pub(crate) fn pop(&mut self) {
    let Some(scope) = self.scopes.pop() else { return };
    if scope.body {
      self.body_scopes -= 1;
    }
    for Binding { name, .. } in self.bound.drain(scope.bound_start..) {
      if let Some(count) = self.counts.get_mut(&name) {
        *count -= 1;
        if *count == 0 {
          self.counts.remove(&name);
        }
      }
    }
  }

  /// Binds `name` with its declared type in the innermost scope; nothing outside every scope (the top level)
  pub(crate) fn bind(&mut self, name: u32, type_path: Option<Typed>) {
    if self.scopes.is_empty() {
      return;
    }
    self.serials += 1;
    self.bound.push(Binding {
      name,
      type_path,
      initializers: None,
      serial: self.serials,
    });
    *self.counts.entry(name).or_insert(0) += 1;
  }

  pub(crate) fn is_bound(&self, name: u32) -> bool {
    self.counts.contains_key(&name)
  }

  /// The innermost binding of `name`
  pub(crate) fn binding(&self, name: u32) -> Option<&Binding> {
    self.bound.iter().rev().find(|it| it.name == name)
  }

  /// Binds `name` when the walk leaves its frame at index `frame` (`bind_left`): a local `val x = x`, the names of a
  /// pattern, which the code they are declared in does not see
  pub(crate) fn bind_later(&mut self, frame: usize, name: u32, type_path: Option<Typed>) {
    self.pending.push((frame, name, type_path));
  }

  /// The walk leaves its frame at index `frame`: closes the `scopes` it opened, then binds the names of `bind_later`
  /// that wait for it (`bind_left`)
  pub(crate) fn leave_frame(&mut self, frame: usize, scopes: u8) {
    for _ in 0..scopes {
      self.pop();
    }
    self.bind_left(frame);
  }

  /// Binds the names of `bind_later` whose frame, at index `frame` or deeper, the walk leaves, in the scope it is in
  pub(crate) fn bind_left(&mut self, frame: usize) {
    while self.pending.last().is_some_and(|(at, ..)| *at >= frame) {
      if let Some((_, name, type_path)) = self.pending.pop() {
        self.bind(name, type_path);
      }
    }
  }

  /// The binding made last
  pub(crate) fn last_mut(&mut self) -> Option<&mut Binding> {
    self.bound.last_mut()
  }

  /// Whether a body scope is open: a declaration here is a local
  pub(crate) const fn is_local(&self) -> bool {
    self.body_scopes > 0
  }
}
