//! What the summaries of the two JVM languages share (`kotlin::summary`, `java::summary`), over the ids of
//! `SummaryBuilder` and without a tree: the containers around the walked node, the supertypes their members override,
//! the JVM facts of a declaration, the types of the receivers of member references and the smart casts.
//!
//! The rules both walks follow, besides those of every language (`extract.rs`: locals dropped, a declaration in a body
//! a local):
//!
//! - **Containers** (`Classes`): a class, an object, a function, a property or an accessor is the container of the
//!   declarations and references below it, with what it is to them (`Holder`); a function or an accessor is code,
//!   which sees no parameter of a Kotlin primary constructor.
//! - **Overrides**: a class-like node, local or not, pushes its supertypes onto a stack that its frame pops before its
//!   members are walked, and a function, a Kotlin lambda or anonymous function an empty list, an anonymous class or an
//!   object expression the types it constructs, an enum entry its enum (`Classes::enum_supertypes`), so a member sees
//!   the supertypes of the class it is directly in. A Kotlin `override` or Java `@Override` member is a
//!   `Role::Override` reference per supertype on top of the stack, with its parameter types as a `Shape::Override`.
//! - **Calls**: a call gives the reference of its callee a `Shape::Call` of its arguments. It is `enclosed` when the
//!   stack of supertypes is deeper than at the declaration around it: a Kotlin lambda or anonymous function, an object
//!   expression, an anonymous class or a local declaration lies between them, each of which pushes onto it.
//! - **Constructors**: a class (not an interface, an object, an enum or an annotation) has the constructors its header
//!   declares, those its body adds (a Kotlin secondary constructor, a Java constructor), and without any the implicit
//!   `()`, added when the walk is done (`SummaryBuilder::finish`). Those of a class with a parse error in it, which may
//!   have swallowed one, are unknown. A Kotlin constructor invocation of a supertype and a Java `new` call the type,
//!   with the shape taken before an object expression or an anonymous class pushes its supertypes: they and their
//!   arguments are of the code around it.
//! - **Receivers** (`TypedReceiver`): the receiver of a member reference has a type when it is a bound name declared
//!   with one, or, not enclosed, a property `m` or `this.m` whose type `SummaryBuilder::finish` finds; a smart cast of
//!   the name drops it (`smart_cast`).

use crate::summary::{Constructor, Declaration, Reach, Shape, Signature};
use crate::summary_builder::{Binding, Bindings, SummaryBuilder, Typed, is_junit3_test};

/// What a container is to the declarations directly in it, for their member kind and reach
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Holder {
  Class,
  /// Its Java fields are constants and its members public
  Interface,
  /// A Kotlin object or companion: its members are static
  Object,
  Enum,
  Annotation,
  /// A function, a field or property, an accessor, an enum entry: nothing in it is a member
  Leaf,
}

/// The JVM facts of a declaration, which a walk reads before it declares it
#[derive(Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a declaration")]
pub(crate) struct JvmFields {
  pub(crate) signature: Option<Signature>,
  pub(crate) overrides: bool,
  pub(crate) overridable: bool,
  pub(crate) jvm_static: bool,
  pub(crate) jvm_field: bool,
  pub(crate) jvm_name: bool,
  pub(crate) lateinit: bool,
  pub(crate) test: bool,
  pub(crate) entry_point: bool,
  pub(crate) unstable: bool,
  pub(crate) setter: Option<Reach>,
  pub(crate) supertypes: Vec<Vec<u32>>,
  pub(crate) value_type: Option<Typed>,
  pub(crate) constructors: Option<Vec<Constructor>>,
}

impl JvmFields {
  /// The declaration `blank` (`summary_builder::blank_declaration`) with these facts
  pub(crate) fn declaration(self, blank: Declaration) -> Declaration {
    Declaration {
      signature: self.signature,
      overrides: self.overrides,
      overridable: self.overridable,
      jvm_static: self.jvm_static,
      jvm_field: self.jvm_field,
      jvm_name: self.jvm_name,
      lateinit: self.lateinit,
      test: self.test,
      entry_point: self.entry_point,
      unstable: self.unstable,
      setter: self.setter,
      supertypes: self.supertypes,
      value_constructed: self.value_type.as_ref().is_some_and(|it| it.constructed),
      value_type: self.value_type.map(|it| it.path),
      constructors: self.constructors,
      ..blank
    }
  }
}

struct Container {
  /// `None` for an accessor outside a property
  decl: Option<u32>,
  holder: Holder,
  /// A function or an accessor, whose code sees no parameter of a primary constructor
  code: bool,
  /// The depth of `Classes::supertypes` when it was entered, its own entry included
  supertypes: usize,
}

/// The containers around the walked node and the supertypes whose members the declarations at each level override
#[derive(Default)]
pub(crate) struct Classes {
  containers: Vec<Container>,
  supertypes: Vec<Vec<Vec<u32>>>,
}

impl Classes {
  /// Makes the declaration the container of what is below the current node; `code` for a function or an accessor
  pub(crate) fn enter(&mut self, decl: Option<u32>, holder: Holder, code: bool) {
    let supertypes = self.supertypes.len();
    self.containers.push(Container { decl, holder, code, supertypes });
  }

  pub(crate) fn leave(&mut self) {
    self.containers.pop();
  }

  /// Makes `supertypes` those whose members the declarations directly below the current node override, until
  /// `pop_supertypes`
  pub(crate) fn push_supertypes(&mut self, supertypes: Vec<Vec<u32>>) {
    self.supertypes.push(supertypes);
  }

  pub(crate) fn pop_supertypes(&mut self) {
    self.supertypes.pop();
  }

  /// The innermost declaration around the current node
  pub(crate) fn container(&self) -> Option<u32> {
    self.containers.iter().rev().find_map(|it| it.decl)
  }

  /// The declaration of the innermost container, `None` for an accessor outside a property
  pub(crate) fn innermost(&self) -> Option<u32> {
    self.containers.last().and_then(|it| it.decl)
  }

  pub(crate) fn holder(&self) -> Option<Holder> {
    self.containers.last().map(|it| it.holder)
  }

  /// The number of containers around the current node, for `Binding::initializers`
  pub(crate) const fn depth(&self) -> usize {
    self.containers.len()
  }

  /// Whether a function or an accessor lies between the current node and the depth `depth`
  pub(crate) fn in_code_below(&self, depth: usize) -> bool {
    self.containers.iter().skip(depth).any(|it| it.code)
  }

  /// Whether a lambda, an anonymous function, an object expression, an anonymous class or a local declaration lies
  /// between the current node and the declaration around it: each pushes onto the supertypes, and the declaration's
  /// own entry (a type's supertypes, a function's empty list) is pushed before it is entered
  pub(crate) fn enclosed(&self) -> bool {
    let level = self.containers.iter().rev().find(|it| it.decl.is_some()).map_or(0, |it| it.supertypes);
    self.supertypes.len() > level
  }

  /// The supertypes a member directly in the class around the current node overrides; none outside a class
  pub(crate) fn overridden(&self) -> Option<Vec<Vec<u32>>> {
    self.supertypes.last().filter(|it| !it.is_empty()).cloned()
  }

  /// The supertypes of the body of an enum entry or constant, the current node: the path of the enum around it, its
  /// containers and its name; none outside an enum
  pub(crate) fn enum_supertypes(&self, out: &SummaryBuilder<'_>) -> Vec<Vec<u32>> {
    if self.holder() != Some(Holder::Enum) {
      return Vec::new();
    }
    let mut path = Vec::new();
    let mut current = self.container();
    while let Some(declaration) = current.and_then(|it| out.declarations.get(it as usize)) {
      path.push(declaration.name);
      current = declaration.container;
    }
    path.reverse();
    vec![path]
  }

  /// Whether a function of `name` and `signature` is a `JUnit` 3 test of the class around it (`is_junit3_test`)
  pub(crate) fn junit3_test(&self, out: &SummaryBuilder<'_>, local: bool, name: Option<&str>, signature: Option<&Signature>) -> bool {
    let Some(name) = name else { return false };
    let class = self.container().and_then(|id| out.declarations.get(id as usize)).map(|it| out.name(it.name));
    let supertypes = self.supertypes.last().into_iter().flatten().filter_map(|path| path.last()).map(|&name| out.name(name));
    is_junit3_test(name, signature.is_some_and(|it| it.parameters.is_empty()), local, class, supertypes)
  }
}

/// The interned `Shape::Override` of a member that overrides, with the types of its parameters
pub(crate) fn override_shape(out: &mut SummaryBuilder<'_>, function: bool, receiver: Option<u32>, signature: Option<&Signature>) -> u32 {
  let parameters = signature.map_or_else(Vec::new, |it| it.parameters.iter().map(|parameter| parameter.type_ref.clone()).collect());
  out.intern_shape(Shape::Override { function, receiver, parameters })
}

/// The name a type of `path` gives the `TypeRef` of a parameter, its last; `None` when its first is a type parameter,
/// one of the function's (`type_parameters`) or one bound around it (of the class), whose type matches any
pub(crate) fn type_name<'t>(out: &mut SummaryBuilder<'t>, bindings: &Bindings, path: &[&'t str], type_parameters: &[&str]) -> Option<u32> {
  let (first, last) = (path.first()?, path.last()?);
  if type_parameters.contains(first) || out.name_id(first).is_some_and(|it| bindings.is_bound(it)) {
    return None;
  }
  Some(out.intern(last))
}

/// A member reference whose receiver may have a type (`Reference::receiver_type`), decided when the walk is done
pub(crate) enum TypedReceiver {
  /// A bound name of this type and binding
  Bound { serial: u32, type_path: Typed },
  /// A property or field `[m]` or `[this, m]`, not enclosed
  Property,
}

impl TypedReceiver {
  /// The receiver that is a name of this binding: of its declared type, if it has one
  pub(crate) fn bound(binding: &Binding) -> Option<Self> {
    Some(Self::Bound {
      serial: binding.serial,
      type_path: binding.type_path.clone()?,
    })
  }

  /// The receiver that `qualifier` names when it is no bound name: a property `[m]` or `[this, m]`, not `enclosed`
  pub(crate) fn property(out: &SummaryBuilder<'_>, qualifier: &[u32], enclosed: bool) -> Option<Self> {
    let this = out.name_id("this");
    let property = match qualifier {
      [_] => true,
      [first, _] => Some(*first) == this,
      _ => false,
    };
    (property && !enclosed).then_some(Self::Property)
  }

  /// Gives the reference at `reference` of `out` this receiver
  pub(crate) fn give(self, out: &mut SummaryBuilder<'_>, reference: usize) {
    match self {
      Self::Bound { serial, type_path } => out.bound_receiver(reference, serial, type_path),
      Self::Property => out.property_receiver(reference),
    }
  }
}

/// The operand `path` of a Kotlin `is` or `as` or a Java `instanceof` or cast: a smart cast may give a name of it, a
/// local or a property (`m`, `this.m`), another type in its scope or function, which the walk can't follow
pub(crate) fn smart_cast(out: &mut SummaryBuilder<'_>, bindings: &Bindings, container: Option<u32>, path: &[&str]) {
  let name = match path {
    [name] | ["this", name] => *name,
    _ => return,
  };
  let Some(id) = out.name_id(name) else { return };
  if path.len() == 1
    && let Some(serial) = bindings.binding(id).map(|it| it.serial)
  {
    out.cast_binding(serial);
    return;
  }
  out.cast_name(container, id);
}
