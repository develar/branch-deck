//! Go to Declaration and Find Usages in a TypeScript, TSX or JavaScript file from oxc's tree
//! (openspec/specs/commit/symbol-navigation/spec.md): the name at a place, its qualifier, the kind of the reference, its
//! declaration in the file by the scopes, and the names of one spelling with their bindings (`navigate::Names`), of
//! which `navigate` makes the usages and the answers of `names_at` (openspec/specs/commit/name-bindings/spec.md). One
//! walk of the tree (`Visit`) inside `js::with_program` finds the name at the place, one more per spelling every name
//! of it with its kind and the declaration its scopes bind it to.
//!
//! - **A name** is an identifier of the code: a reference, a binding, the name of a member after its object, of a
//!   member of a class, an interface or an object type, of an enum member, a key of an object literal, a name of an
//!   import or an export, a JSX tag or attribute, a private name (`#count`). A label, the label of a named tuple member
//!   and the parameter of an index signature are none.
//! - **Its kind**: the name of a declaration (a binding, a member of a class, an interface or an object type, an enum
//!   member); a callee (`f()`, `f<T>()`, `new F()`, `` f`…` ``, with its qualifier for a member, `a.f()`); a member
//!   after its object (`a.b`, its qualifier the object as written); a type where a type is read (the last name of
//!   `a.B` with its qualifier); else a value. A name of an import is a value: its declaration is in the module it
//!   names, which the caller finds.
//! - **Its declaration** by the scopes around it, as the language has them: a function's parameters and its `var`s are
//!   seen in all of it, `let`, `const`, a class, a function and an enum in all of their block, a type parameter in its
//!   declaration, what `infer` declares in its conditional type; the last declaration before the name in the innermost
//!   scope that declares one, else the first (a function declared after its use). A function has one variable of a
//!   name: a parameter, a `var` and a function its body declares of one name are that variable, declared by the
//!   first, and a later one is a use of it (a `var` in a `catch` of the name of its parameter writes the parameter).
//!   The signature of a function (the defaults of its parameters, their types) is read before its body: its names
//!   are bound to its parameters, else to a scope around the function, never to what the body declares. The name of
//!   a function expression is seen in the function alone, the members of an enum in its body. Types and values are
//!   apart: a name read as a type, and a namespace before the dot of one, is of no variable, parameter or function.
//!   A bare name is never a member of the class around it, which only `this.` reaches; a member after a qualifier
//!   has none.
//! - **The usages** of a local (a declaration inside a function) are the names bound to it; those of any other name
//!   are its names that no local binds, each with how it uses the name: declared, imported, called, written (the
//!   target of an assignment, a destructuring one too, of `++`, and a later `var` of the variable with a value),
//!   else read.
//! - **What a local is** (`LocalBinding`): a `var`, `let` or `const` a variable, the one of the head of a `for` a loop
//!   variable, the parameter of a `catch`, of an arrow or a function expression, and of a function declaration or a
//!   method; no call names a parameter. The parameter of a signature without a body, a function, a class, a type and
//!   a type parameter are `Other`. A parameter with a modifier (`private total`) declares a property of its class: no
//!   local unless a function is around the class, and one of the names `this.total` spells. A text with a syntax
//!   error and a script that calls `eval` by its name are read in part (`Names::reads_in_part`).
//! - **Its site** (`Site`): `{ total }` of an object, of a pattern and of the target of an assignment names the
//!   property too; any other name a pattern declares or a destructuring assigns is destructured; a variable or a
//!   parameter that is given a function or a class written without a name spells the name that one takes
//!   (`takes_name`); and inside the body of a `with` only what runs tells a variable from a property of the object.

use crate::js;
use crate::js::field_of;
use crate::navigate::{Access, Bound, LocalBinding, LocalKind, Names, Position, ReferenceAt, ReferenceKind, Resolved, Site, Wanted, byte_offset, reference_answer};
use js_syntax::Dialect;
use std::cell::{Cell, OnceCell};
// jscpd:ignore-start: the names of oxc's tree and visitor each walk reads
use js_syntax::oxc_ast::ast::{
  ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression, AssignmentOperator, AssignmentTarget, AssignmentTargetMaybeDefault, AssignmentTargetProperty,
  BindingIdentifier, BindingPattern, BlockStatement, CallExpression, CatchClause, Class, ClassElement, ClassType, ExportAllDeclaration, ExportFromDeclaration,
  ExportNamedDeclaration, Expression, ForInStatement, ForOfStatement, ForStatement, ForStatementInit, ForStatementLeft, FormalParameter, FormalParameterRest, Function,
  FunctionType, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier, JSXAttributeName, JSXElementName, JSXMemberExpression, JSXMemberExpressionObject,
  MethodDefinition, ModuleExportName, NewExpression, ObjectProperty, PrivateFieldExpression, PrivateInExpression, Program, PropertyKey, SimpleAssignmentTarget, StaticBlock,
  StaticMemberExpression, SwitchStatement, TSCallSignatureDeclaration, TSConditionalType, TSConstructSignatureDeclaration, TSConstructorType, TSEnumDeclaration, TSEnumMemberName,
  TSExternalModuleDeclaration, TSFunctionType, TSGlobalDeclaration, TSImportEqualsDeclaration, TSImportType, TSImportTypeQualifier, TSIndexSignature, TSInterfaceDeclaration,
  TSMappedType, TSMethodSignature, TSModuleReference, TSNamespaceDeclaration, TSNamespaceDeclarationBody, TSPropertySignature, TSQualifiedName, TSTypeAliasDeclaration, TSTypeName,
  TSTypeParameter, TSTypePredicate, TSTypePredicateName, TSTypeQuery, TaggedTemplateExpression, UpdateExpression, VariableDeclaration, VariableDeclarationKind, WithStatement,
};
use js_syntax::oxc_ast_visit::{Visit, walk};
use js_syntax::oxc_span::{GetSpan, Span};
use js_syntax::oxc_syntax::scope::ScopeFlags;
// jscpd:ignore-end

/// The name at `position` of `text` and what its syntax says (`navigate::reference_at`)
pub(crate) fn reference_at(dialect: Dialect, text: &str, position: Position) -> Option<ReferenceAt> {
  let offset = u32::try_from(byte_offset(text, position)?).ok()?;
  super::with_program(dialect, text.as_bytes(), |parsed| {
    let program = parsed.program()?;
    let (span, name) = name_at(program, text, offset)?;
    let names = resolve(program, text, Wanted::named(name));
    let found = names.iter().find(|it| it.span == span)?;
    let local = match found.kind {
      ReferenceKind::Declaration | ReferenceKind::Member | ReferenceKind::MemberCall => None,
      _ => found.binding.map(|it| it.start as usize),
    };
    let qualifier = found.qualifier.and_then(|it| text.get(it.start as usize..it.end as usize)).map(ToString::to_string);
    Some(reference_answer(text, span.start as usize, name.to_string(), found.kind, qualifier, local))
  })
  .flatten()
}

/// The answer of `read` for the names of a parse of `text` (`navigate::names_at`, `usages_at`, `usages_of`); `None`
/// for a text without a program
pub(crate) fn with_names<R: Send>(dialect: Dialect, text: &str, read: impl FnOnce(&dyn Names) -> R + Send) -> Option<R> {
  super::with_program(dialect, text.as_bytes(), |parsed| {
    let names = ProgramNames {
      program: parsed.program()?,
      text,
      asked: Cell::new(false),
      every: OnceCell::new(),
      // a region a recovery blanked and a syntax error of the parse that made the tree; an early error (strict mode)
      // leaves the tree whole
      broken: parsed.recovered || parsed.diagnostics.iter().any(js_syntax::is_syntax_error),
      evaluates: OnceCell::new(),
    };
    Some(read(&names))
  })
  .flatten()
}

/// A program and its text, for the names of `navigate::Names`
struct ProgramNames<'p, 'a> {
  program: &'p Program<'a>,
  text: &'p str,
  /// A place was asked before
  asked: Cell<bool>,
  /// Every name of the program, by its start: read once, when a second place is asked
  every: OnceCell<Vec<Span>>,
  /// The parser reported a syntax error, or read the program only with a region of the text blanked
  broken: bool,
  /// The program calls `eval` by its name (`evaluates`), read once
  evaluates: OnceCell<bool>,
}

/// Whether a program calls `eval` by its name, a direct `eval`: the text it is given runs in the scope of the call,
/// where it reads and declares the variables of the functions around it by their names
fn evaluates(program: &Program<'_>, text: &str) -> bool {
  struct Calls(bool);
  impl<'a> Visit<'a> for Calls {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
      if matches!(it.callee.get_inner_expression(), Expression::Identifier(callee) if callee.name == "eval") {
        self.0 = true;
      } else {
        walk::walk_call_expression(self, it);
      }
    }
  }
  if !text.contains("eval") {
    return false;
  }
  let mut calls = Calls(false);
  calls.visit_program(program);
  calls.0
}

impl Names for ProgramNames<'_, '_> {
  /// One place is one walk of the program; from the second place on, the names of the program are read once and each
  /// place is looked up among them
  fn name_at(&self, offset: usize) -> Option<(usize, &str)> {
    let offset = u32::try_from(offset).ok()?;
    let span = if self.asked.replace(true) {
      name_among(self.every.get_or_init(|| every_name(self.program, self.text)), offset)?
    } else {
      name_at(self.program, self.text, offset)?.0
    };
    Some((span.start as usize, self.text.get(span.start as usize..span.end as usize)?))
  }

  fn resolve(&self, wanted: &Wanted<'_>) -> Vec<Resolved> {
    let resolved = resolve(self.program, self.text, *wanted).into_iter().map(|it| Resolved {
      start: it.span.start as usize,
      access: it.access,
      site: it.site,
      binding: it.binding.map(|binding| Bound {
        start: binding.start as usize,
        local: binding.local,
      }),
    });
    resolved.collect()
  }

  /// Every name the language binds is written: `arguments` and `undefined` are names of the text
  fn is_implicit(&self, _name: &str) -> bool {
    false
  }

  /// A text with a syntax error (what stands where the parser gave up is no name of the tree), and a script with a
  /// direct `eval`, which names the variables around its call in a text no scope reads
  fn reads_in_part(&self) -> bool {
    self.broken || *self.evaluates.get_or_init(|| evaluates(self.program, self.text))
  }
}

/// The name whose text holds the byte at `offset`, else the one that ends there (`foo|`), and its text
fn name_at<'t>(program: &Program<'_>, text: &'t str, offset: u32) -> Option<(Span, &'t str)> {
  let mut walk = Walk::new(text, Want::At(offset));
  walk.visit_program(program);
  let span = walk.at.or(walk.before)?;
  Some((span, text.get(span.start as usize..span.end as usize)?))
}

/// Every name of a program, by its start
pub(crate) fn every_name(program: &Program<'_>, text: &str) -> Vec<Span> {
  let mut walk = Walk::new(text, Want::Every);
  walk.visit_program(program);
  let mut every = walk.every;
  every.sort_unstable_by_key(|span| (span.start, span.end));
  every.dedup();
  every
}

/// The name among `every` (`every_name`) whose text holds the byte at `offset`, else the one that ends there: the
/// answer of `name_at` without its walk
pub(crate) fn name_among(every: &[Span], offset: u32) -> Option<Span> {
  let after = every.partition_point(|span| span.start <= offset);
  let last = *every.get(after.checked_sub(1)?)?;
  (offset < last.end || (last.start < offset && offset == last.end)).then_some(last)
}

/// Every name the resolution looks for in the order of the text, with its kind, its access, its site and its declaration
fn resolve<'t>(program: &Program<'_>, text: &'t str, wanted: Wanted<'t>) -> Vec<Named> {
  let mut walk = Walk::new(text, Want::Named(wanted));
  walk.visit_program(program);
  let mut names = walk.names;
  names.sort_by_key(|it| it.span.start);
  names.dedup_by_key(|it| it.span);
  names
}

/// Whether an expression is a function or a class written without a name, which takes the name of the variable it is
/// given to as its own `name` (the parentheses and the types around it aside): another name for the variable is
/// another name of the function, which a stack trace, a log and a framework read
fn takes_name(value: &Expression<'_>) -> bool {
  match value.get_inner_expression() {
    Expression::ArrowFunctionExpression(_) => true,
    Expression::FunctionExpression(function) => function.id.is_none(),
    Expression::ClassExpression(class) => class.id.is_none(),
    _ => false,
  }
}

/// What may name a declaration, as the language keeps values and types apart: a value alone (a variable, a parameter,
/// a function), a type alone (an interface, a type alias, a type parameter), or both (a class, an enum, a namespace)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
  Value,
  Type,
  Both,
}

/// The declaration a name is bound to: where its name starts, what it is when a function declares it, what may name
/// it, and whether it is a variable of its function (a parameter, a `var`, a function the body declares), of which a
/// function has one per name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Declared {
  start: u32,
  local: Option<LocalBinding>,
  space: Space,
  of_function: bool,
}

/// A name of the text looked for, with what its syntax says
struct Named {
  span: Span,
  kind: ReferenceKind,
  qualifier: Option<Span>,
  access: Access,
  site: Site,
  /// The name is read as a type or as the namespace of one: no value alone is its declaration
  of_type: bool,
  binding: Option<Declared>,
}

/// What a walk looks for: the name at an offset, every name, or the names of a resolution
#[derive(Clone, Copy)]
enum Want<'t> {
  At(u32),
  Every,
  Named(Wanted<'t>),
}

/// What the binding the walk meets next is of, beside its pattern
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binder {
  /// A `let`, a `const`, a parameter, a class, a type: of the innermost scope
  Scope,
  /// A `var`, of the function around it; with a value, it writes the variable its function has of the name
  Var { assigns: bool },
  /// The name of a function declaration: a variable of the function whose body declares it
  Function,
  /// A parameter with a modifier (`private total`): a property of the class around its constructor
  Property,
}

/// Whether the `var`, `let` or `const` the walk meets next is the head's of a loop
#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
  No,
  /// Of a `for (…; …; …)`
  For,
  /// Of a `for … in` or a `for … of`, which gives its variable a value
  Each,
}

/// A scope of the walk: its declarations of the name, the names in it its declarations may bind, whether it is a
/// function's (its `var`s, its parameters, its declarations locals) and whether it is a `catch`'s
struct Scope {
  function: bool,
  catch: bool,
  declared: Vec<Declared>,
  /// Indices into `Walk::names` of the names waiting for a declaration of this scope or of one around it
  pending: Vec<usize>,
}

/// How a name may be bound: to a declaration of the scopes, or not at all (a member, a key, a name of an import)
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binds {
  Scopes,
  Never,
}

struct Walk<'t> {
  text: &'t str,
  want: Want<'t>,
  at: Option<Span>,
  before: Option<Span>,
  /// Every name met, for `Want::Every`
  every: Vec<Span>,
  names: Vec<Named>,
  scopes: Vec<Scope>,
  /// Inside an import: its names are imports
  importing: bool,
  /// What the bindings the walk meets are of: a `var`'s belong to the function around them
  binder: Binder,
  /// The next `var`, `let` or `const` is the head's of a loop
  head: Head,
  /// The names the walk meets are namespaces of a type (`a.b` of `a.b.C`)
  namespace: bool,
  /// What the parameters of the function the walk is in declare
  parameters: LocalKind,
  /// What a binding the walk meets declares, inside a function: set around a pattern, `Other` elsewhere
  declares: LocalKind,
  /// What the site of the names the walk meets makes of them: set around a pattern or a shorthand, `Plain` elsewhere
  site: Site,
  /// Inside the body of a `with`: a property of its object comes before every name around it
  within_with: bool,
}

impl<'t> Walk<'t> {
  const fn new(text: &'t str, want: Want<'t>) -> Self {
    Self {
      text,
      want,
      at: None,
      before: None,
      every: Vec::new(),
      names: Vec::new(),
      scopes: Vec::new(),
      importing: false,
      binder: Binder::Scope,
      head: Head::No,
      namespace: false,
      parameters: LocalKind::Other,
      declares: LocalKind::Other,
      site: Site::Plain,
      within_with: false,
    }
  }

  /// The pattern `body` walks declares locals of `kind`
  fn declaring(&mut self, kind: LocalKind, body: impl FnOnce(&mut Self)) {
    let saved = std::mem::replace(&mut self.declares, kind);
    body(self);
    self.declares = saved;
  }

  /// The names `body` walks stand at a site of `site`
  fn at_site(&mut self, site: Site, body: impl FnOnce(&mut Self)) {
    let saved = std::mem::replace(&mut self.site, site);
    body(self);
    self.site = saved;
  }

  /// Code inside a pattern or a target (a default, a computed key, the object of a member): its names are of no
  /// pattern, and what it declares is no part of the declaration around it
  fn aside(&mut self, body: impl FnOnce(&mut Self)) {
    let binder = std::mem::replace(&mut self.binder, Binder::Scope);
    self.declaring(LocalKind::Other, |walk| walk.at_site(Site::Plain, body));
    self.binder = binder;
  }

  /// A function-like scope around what `body` walks, whose parameters declare `parameters`
  fn function_scope(&mut self, parameters: LocalKind, body: impl FnOnce(&mut Self)) {
    let saved = std::mem::replace(&mut self.parameters, parameters);
    self.scoped(true, body);
    self.parameters = saved;
  }

  fn text_of(&self, span: Span) -> &'t str {
    self.text.get(span.start as usize..span.end as usize).unwrap_or_default()
  }

  fn push(&mut self, function: bool) {
    self.scopes.push(Scope {
      function,
      catch: false,
      declared: Vec::new(),
      pending: Vec::new(),
    });
  }

  /// The declarations of the innermost scope bind the names waiting in it: each the last declaration before it that
  /// may be what it reads (a type is of no value alone), else the first; the other names wait in the scope around it
  fn settle(&mut self) {
    let Some(scope) = self.scopes.pop() else { return };
    let mut waiting = Vec::new();
    for &index in &scope.pending {
      let named = &mut self.names[index];
      let start = named.span.start;
      let mut seen = scope.declared.iter().filter(|it| !named.of_type || it.space != Space::Value);
      match seen.clone().rev().find(|it| it.start <= start).or_else(|| seen.next()) {
        Some(binding) => named.binding = Some(*binding),
        None => waiting.push(index),
      }
    }
    if let Some(outer) = self.scopes.last_mut() {
      outer.pending.extend(&waiting);
    }
    self.scopes.push(Scope { pending: Vec::new(), ..scope });
  }

  /// Leaves a scope, whose declarations bind the names waiting in it (`settle`)
  fn pop(&mut self) {
    self.settle();
    self.scopes.pop();
  }

  /// Whether a scope at this index or around it is a function's: what it declares is a local
  fn in_function(&self, index: usize) -> bool {
    self.scopes[..=index.min(self.scopes.len().saturating_sub(1))].iter().any(|it| it.function)
  }

  /// A name of the text at `span`, if it is one the walk looks for
  fn name(&mut self, span: Span, kind: ReferenceKind, qualifier: Option<Span>, access: Access, binds: Binds) {
    match self.want {
      Want::At(offset) => {
        if span.start <= offset && offset < span.end {
          self.at.get_or_insert(span);
        } else if span.start < offset && offset == span.end {
          self.before.get_or_insert(span);
        }
      }
      Want::Every => self.every.push(span),
      Want::Named(wanted) => {
        if !wanted.takes(self.text_of(span), span.start as usize) {
          return;
        }
        let access = match kind {
          ReferenceKind::Declaration => Access::Declaration,
          ReferenceKind::Call | ReferenceKind::MemberCall => Access::Call,
          _ if self.importing => Access::Import,
          _ => access,
        };
        let index = self.names.len();
        // in a `with` only what runs tells whether a name is a variable or a property of the object
        let site = if self.within_with && self.site == Site::Plain { Site::Namesake } else { self.site };
        self.names.push(Named {
          span,
          kind,
          qualifier,
          access,
          site,
          of_type: kind == ReferenceKind::Type || self.namespace,
          binding: None,
        });
        if binds == Binds::Scopes
          && let Some(scope) = self.scopes.last_mut()
        {
          scope.pending.push(index);
        }
      }
    }
  }

  /// The name of a declaration, which the scope at `scope` declares (or no scope, `None`: a member), and what may
  /// name it. A function has one variable of a name: where its scope has one already (a parameter, a `var`, a function
  /// it declares), such a declaration is a use of that one, a write when it comes with a value
  fn declaration(&mut self, span: Span, scope: Option<usize>, space: Space) {
    // inside a function a declaration is a local: what the pattern around it declares, a member `Other`. A parameter
    // that declares a property is a member of its class: a local only when a function is around the class, which is
    // around the scope of its constructor
    let innermost = scope.unwrap_or_else(|| self.scopes.len().saturating_sub(1));
    let binder = self.binder;
    let in_function = if binder == Binder::Property {
      innermost.checked_sub(1).is_some_and(|around| self.in_function(around))
    } else {
      self.in_function(innermost)
    };
    let kind = if scope.is_some() { self.declares } else { LocalKind::Other };
    let of_function = scope.is_some_and(|it| self.scopes[it].function)
      && (matches!(binder, Binder::Var { .. } | Binder::Function) || matches!(kind, LocalKind::Parameter | LocalKind::LambdaParameter));
    let declared = Declared {
      start: span.start,
      local: in_function.then(|| LocalBinding::of(kind)),
      space,
      of_function,
    };
    let mut variable = None;
    if let (Want::Named(wanted), Some(scope)) = (self.want, scope)
      && wanted.takes(self.text_of(span), span.start as usize)
    {
      variable = self.scopes[scope].declared.iter().find(|it| of_function && it.of_function).copied();
      if variable.is_none() {
        self.scopes[scope].declared.push(declared);
      }
      // `catch (e) { var e = 1 }` writes the parameter of the `catch`, and declares a variable of the function
      if matches!(binder, Binder::Var { .. }) {
        let caught = self.scopes[scope + 1..].iter().rev().filter(|it| it.catch);
        variable = caught.flat_map(|it| &it.declared).find(|it| it.space == Space::Value).copied().or(variable);
      }
    }
    let index = self.names.len();
    match variable {
      Some(_) => {
        let access = if binder == (Binder::Var { assigns: true }) {
          Access::Write
        } else {
          Access::Declaration
        };
        self.name(span, ReferenceKind::Value, None, access, Binds::Never);
      }
      None => self.name(span, ReferenceKind::Declaration, None, Access::Declaration, Binds::Never),
    }
    // the name of a declaration is bound to itself, or to the variable it declares again
    if self.names.len() > index {
      self.names[index].binding = Some(variable.unwrap_or(declared));
    }
  }

  /// A binding in the innermost scope, a `var`'s in the function's around it
  fn binding(&mut self, identifier: &BindingIdentifier<'_>, space: Space) {
    let innermost = self.scopes.len().saturating_sub(1);
    let scope = if matches!(self.binder, Binder::Var { .. }) {
      self.scopes.iter().rposition(|it| it.function).unwrap_or(0)
    } else {
      innermost
    };
    self.declaration(identifier.span, Some(scope), space);
  }

  /// A use of a name the scopes may bind
  fn value(&mut self, span: Span, kind: ReferenceKind, access: Access) {
    self.name(span, kind, None, access, Binds::Scopes);
  }

  /// A name that no scope binds: a key, the name of an attribute
  fn unbound(&mut self, span: Span, kind: ReferenceKind, qualifier: Option<Span>, access: Access) {
    self.name(span, kind, qualifier, access, Binds::Never);
  }

  /// The callee of a call: a name a call, a member a call of a member
  fn callee(&mut self, callee: &Expression<'_>) {
    match callee {
      Expression::Identifier(it) => self.value(it.span, ReferenceKind::Call, Access::Call),
      Expression::StaticMemberExpression(it) => {
        self.visit_expression(&it.object);
        self.unbound(it.property.span, ReferenceKind::MemberCall, Some(it.object.span()), Access::Call);
      }
      Expression::PrivateFieldExpression(it) => {
        self.visit_expression(&it.object);
        self.unbound(it.field.span, ReferenceKind::MemberCall, Some(it.object.span()), Access::Call);
      }
      Expression::TSInstantiationExpression(it) => {
        self.callee(&it.expression);
        self.visit_ts_type_parameter_instantiation(&it.type_arguments);
      }
      _ => self.visit_expression(callee),
    }
  }

  /// The key of a member: its name the declaration of the member, a computed one code
  fn member_key(&mut self, key: &PropertyKey<'_>, computed: bool, kind: ReferenceKind) {
    match key {
      PropertyKey::StaticIdentifier(it) if !computed => self.unbound(it.span, kind, None, Access::Read),
      PropertyKey::PrivateIdentifier(it) => self.unbound(it.span, kind, None, Access::Read),
      key if computed => self.visit_property_key(key),
      _ => {}
    }
  }

  /// The name of a declaration of the member kind: bound to itself, the declaration of no scope
  fn member_declaration(&mut self, key: &PropertyKey<'_>, computed: bool) {
    match key {
      PropertyKey::StaticIdentifier(it) if !computed => self.declaration(it.span, None, Space::Value),
      PropertyKey::PrivateIdentifier(it) => self.declaration(it.span, None, Space::Value),
      key if computed => self.visit_property_key(key),
      _ => {}
    }
  }

  /// The target of an assignment or of `++`: the name it writes, the object of a member read
  fn target(&mut self, target: &AssignmentTarget<'_>) {
    match target {
      AssignmentTarget::ArrayAssignmentTarget(it) => {
        for element in it.elements.iter().flatten() {
          self.at_site(Site::Destructured, |walk| walk.target_with_default(element));
        }
        if let Some(rest) = &it.rest {
          self.at_site(Site::Destructured, |walk| walk.target(&rest.target));
        }
      }
      AssignmentTarget::ObjectAssignmentTarget(it) => {
        for property in &it.properties {
          match property {
            AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(it) => {
              // `({ total } = box)`: the name it writes is the property it reads
              self.at_site(Site::Shorthand, |walk| walk.value(it.binding.span, ReferenceKind::Value, Access::Write));
              if let Some(init) = &it.init {
                self.aside(|walk| walk.visit_expression(init));
              }
            }
            AssignmentTargetProperty::AssignmentTargetPropertyProperty(it) => {
              self.aside(|walk| walk.pattern_key(&it.name, it.computed));
              self.at_site(Site::Destructured, |walk| walk.target_with_default(&it.binding));
            }
          }
        }
        if let Some(rest) = &it.rest {
          self.at_site(Site::Destructured, |walk| walk.target(&rest.target));
        }
      }
      target => {
        if let Some(simple) = target.as_simple_assignment_target() {
          self.simple_target(simple);
        }
      }
    }
  }

  fn target_with_default(&mut self, target: &AssignmentTargetMaybeDefault<'_>) {
    match target {
      AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(it) => {
        self.target(&it.binding);
        self.aside(|walk| walk.visit_expression(&it.init));
      }
      target => {
        if let Some(target) = target.as_assignment_target() {
          self.target(target);
        }
      }
    }
  }

  fn simple_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
    match target {
      SimpleAssignmentTarget::AssignmentTargetIdentifier(it) => self.value(it.span, ReferenceKind::Value, Access::Write),
      // a member a destructuring assigns is a member: the names of its object are of no pattern
      target => self.aside(|walk| match target {
        SimpleAssignmentTarget::StaticMemberExpression(it) => {
          walk.visit_expression(&it.object);
          walk.unbound(it.property.span, ReferenceKind::Member, Some(it.object.span()), Access::Write);
        }
        SimpleAssignmentTarget::PrivateFieldExpression(it) => {
          walk.visit_expression(&it.object);
          walk.unbound(it.field.span, ReferenceKind::Member, Some(it.object.span()), Access::Write);
        }
        target => walk::walk_simple_assignment_target(walk, target),
      }),
    }
  }

  /// The key of a property of a pattern or of a destructuring target that names another binding (`{ key: name }`): a
  /// name no scope binds, a computed one code
  fn pattern_key(&mut self, key: &PropertyKey<'_>, computed: bool) {
    if computed {
      self.visit_property_key(key);
    } else if let PropertyKey::StaticIdentifier(key) = key {
      self.unbound(key.span, ReferenceKind::Value, None, Access::Read);
    }
  }

  /// A name in a type position: its last name a type, qualified by the names before it, each a value of its own
  fn type_name(&mut self, name: &TSTypeName<'_>, kind: ReferenceKind) {
    match name {
      TSTypeName::IdentifierReference(it) => self.value(it.span, kind, Access::Read),
      TSTypeName::QualifiedName(it) => self.qualified_name(it, kind),
      TSTypeName::ThisExpression(_) => {}
    }
  }

  /// `a.b.C`: `C` of the kind (a member of a value), qualified by `a.b`, whose names are values; before a type they
  /// are namespaces, which no variable is
  fn qualified_name(&mut self, name: &TSQualifiedName<'_>, kind: ReferenceKind) {
    let saved = self.namespace;
    self.namespace = saved || kind == ReferenceKind::Type;
    self.type_name(&name.left, ReferenceKind::Value);
    self.namespace = saved;
    let member = if kind == ReferenceKind::Type { ReferenceKind::Type } else { ReferenceKind::Member };
    self.unbound(name.right.span, member, Some(name.left.span()), Access::Read);
  }

  /// A function: its parameters (which declare `parameters`), type parameters and body in a scope of its own; the own
  /// name of an expression in a scope around that one, as a parameter or a `var` of the name is another variable
  fn function(&mut self, function: &Function<'_>, own_name: bool, parameters: LocalKind) {
    let own_name = function.id.as_ref().filter(|_| own_name);
    let scope = |walk: &mut Self| {
      walk.function_scope(parameters, |walk| {
        js::walk_signature(
          walk,
          function.type_parameters.as_deref(),
          function.this_param.as_deref(),
          &function.params,
          function.return_type.as_deref(),
        );
        walk.settle();
        js::walk_body(walk, function.body.as_deref());
      });
    };
    match own_name {
      Some(id) => self.scoped(true, |walk| {
        walk.binding(id, Space::Value);
        scope(walk);
      }),
      None => scope(self),
    }
  }

  /// A field or an accessor of a class: its name a declaration, its decorators, type and value code
  fn field(&mut self, field: &js::Field<'_, '_>) {
    for decorator in field.decorators {
      self.visit_decorator(decorator);
    }
    self.member_declaration(field.key, field.computed);
    if let Some(annotation) = field.annotation {
      self.visit_ts_type_annotation(annotation);
    }
    if let Some(value) = field.value {
      self.visit_expression(value);
    }
  }

  fn jsx_member(&mut self, member: &JSXMemberExpression<'_>) {
    match &member.object {
      JSXMemberExpressionObject::IdentifierReference(it) => self.value(it.span, ReferenceKind::Value, Access::Read),
      JSXMemberExpressionObject::MemberExpression(it) => self.jsx_member(it),
      JSXMemberExpressionObject::ThisExpression(_) => {}
    }
    self.unbound(member.property.span, ReferenceKind::Member, Some(member.object.span()), Access::Read);
  }

  fn module_export_name(&mut self, name: &ModuleExportName<'_>, binds: Binds) {
    match name {
      ModuleExportName::IdentifierReference(it) => self.name(it.span, ReferenceKind::Value, None, Access::Read, binds),
      ModuleExportName::IdentifierName(it) => self.unbound(it.span, ReferenceKind::Value, None, Access::Read),
      ModuleExportName::StringLiteral(_) => {}
    }
  }

  /// A scope of its own around what `body` walks: a block, a loop
  fn scoped(&mut self, function: bool, body: impl FnOnce(&mut Self)) {
    self.push(function);
    body(self);
    self.pop();
  }
}

impl<'a> Visit<'a> for Walk<'_> {
  fn visit_program(&mut self, program: &Program<'a>) {
    self.push(false);
    for statement in &program.body {
      self.visit_statement(statement);
    }
    self.pop();
  }

  fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
    self.value(it.span, ReferenceKind::Value, Access::Read);
  }

  fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
    if self.importing {
      self.unbound(it.span, ReferenceKind::Value, None, Access::Import);
    } else {
      self.binding(it, Space::Value);
    }
  }

  fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
    self.visit_expression(&it.object);
    self.unbound(it.property.span, ReferenceKind::Member, Some(it.object.span()), Access::Read);
  }

  fn visit_private_field_expression(&mut self, it: &PrivateFieldExpression<'a>) {
    self.visit_expression(&it.object);
    self.unbound(it.field.span, ReferenceKind::Member, Some(it.object.span()), Access::Read);
  }

  fn visit_private_in_expression(&mut self, it: &PrivateInExpression<'a>) {
    self.unbound(it.left.span, ReferenceKind::Value, None, Access::Read);
    self.visit_expression(&it.right);
  }

  fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
    self.callee(&it.callee);
    js::walk_arguments(self, it.type_arguments.as_deref(), &it.arguments);
  }

  fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
    self.callee(&it.callee);
    js::walk_arguments(self, it.type_arguments.as_deref(), &it.arguments);
  }

  fn visit_tagged_template_expression(&mut self, it: &TaggedTemplateExpression<'a>) {
    self.callee(&it.tag);
    js::walk_tagged_template(self, it);
  }

  fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
    // `total = () => 1`: the function takes the name of the variable it is given to
    let names = matches!(it.left, AssignmentTarget::AssignmentTargetIdentifier(_))
      && matches!(
        it.operator,
        AssignmentOperator::Assign | AssignmentOperator::LogicalOr | AssignmentOperator::LogicalAnd | AssignmentOperator::LogicalNullish
      )
      && takes_name(&it.right);
    self.at_site(if names { Site::Spelled } else { self.site }, |walk| walk.target(&it.left));
    self.visit_expression(&it.right);
  }

  fn visit_with_statement(&mut self, it: &WithStatement<'a>) {
    self.visit_expression(&it.object);
    let around = std::mem::replace(&mut self.within_with, true);
    self.visit_statement(&it.body);
    self.within_with = around;
  }

  fn visit_update_expression(&mut self, it: &UpdateExpression<'a>) {
    self.simple_target(&it.argument);
  }

  fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
    if it.shorthand {
      // `{ name }`: one name, the value, which names the property too
      match &it.value {
        Expression::Identifier(name) => self.at_site(Site::Shorthand, |walk| walk.visit_identifier_reference(name)),
        value => self.visit_expression(value),
      }
      return;
    }
    self.member_key(&it.key, it.computed, ReferenceKind::Value);
    self.visit_expression(&it.value);
  }

  fn visit_block_statement(&mut self, it: &BlockStatement<'a>) {
    self.scoped(false, |walk| walk::walk_block_statement(walk, it));
  }

  fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
    self.scoped(false, |walk| walk::walk_for_statement(walk, it));
  }

  fn visit_for_statement_init(&mut self, it: &ForStatementInit<'a>) {
    if matches!(it, ForStatementInit::VariableDeclaration(_)) {
      self.head = Head::For;
    }
    walk::walk_for_statement_init(self, it);
  }

  fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
    self.scoped(false, |walk| walk::walk_for_in_statement(walk, it));
  }

  fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
    self.scoped(false, |walk| walk::walk_for_of_statement(walk, it));
  }

  fn visit_for_statement_left(&mut self, it: &ForStatementLeft<'a>) {
    if matches!(it, ForStatementLeft::VariableDeclaration(_)) {
      self.head = Head::Each;
    }
    walk::walk_for_statement_left(self, it);
  }

  fn visit_switch_statement(&mut self, it: &SwitchStatement<'a>) {
    self.visit_expression(&it.discriminant);
    self.scoped(false, |walk| {
      for case in &it.cases {
        walk.visit_switch_case(case);
      }
    });
  }

  fn visit_catch_clause(&mut self, it: &CatchClause<'a>) {
    self.scoped(false, |walk| {
      if let Some(scope) = walk.scopes.last_mut() {
        scope.catch = true;
      }
      if let Some(parameter) = &it.param {
        walk.declaring(LocalKind::CatchParameter, |walk| walk.visit_binding_pattern(&parameter.pattern));
        js::walk_parameter_type(walk, parameter.type_annotation.as_deref(), None);
      }
      walk.visit_block_statement(&it.body);
    });
  }

  fn visit_formal_parameter_rest(&mut self, it: &FormalParameterRest<'a>) {
    js::walk_rest_parameter(self, it, |walk, pattern| walk.declaring(walk.parameters, |walk| walk.visit_binding_pattern(pattern)));
  }

  fn visit_formal_parameter(&mut self, it: &FormalParameter<'a>) {
    js::walk_decorators(self, &it.decorators);
    // a parameter with a modifier (`private total`) declares a property of its class, which `this.total` names
    let property = it.accessibility.is_some() || it.readonly || it.r#override;
    let kind = if property { LocalKind::Other } else { self.parameters };
    // `(total = () => 1)`: the function takes the name of the parameter it is the default of
    let names = matches!(it.pattern, BindingPattern::BindingIdentifier(_)) && it.initializer.as_deref().is_some_and(takes_name);
    let around = self.binder;
    if property {
      self.binder = Binder::Property;
    }
    let site = if names { Site::Spelled } else { self.site };
    self.declaring(kind, |walk| walk.at_site(site, |walk| walk.visit_binding_pattern(&it.pattern)));
    self.binder = around;
    js::walk_parameter_type(self, it.type_annotation.as_deref(), it.initializer.as_deref());
  }

  fn visit_static_block(&mut self, it: &StaticBlock<'a>) {
    self.function_scope(LocalKind::Other, |walk| walk::walk_static_block(walk, it));
  }

  fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
    // only the declaration that is the head of a loop declares its variables
    let head = std::mem::replace(&mut self.head, Head::No);
    let kind = if head == Head::No { LocalKind::Variable } else { LocalKind::LoopVariable };
    for declarator in &it.declarations {
      let binder = if it.kind == VariableDeclarationKind::Var {
        Binder::Var {
          assigns: head == Head::Each || declarator.init.is_some(),
        }
      } else {
        Binder::Scope
      };
      let around = std::mem::replace(&mut self.binder, binder);
      // `const total = () => 1`: the function takes the name of the variable it is the value of
      let names = matches!(declarator.id, BindingPattern::BindingIdentifier(_)) && declarator.init.as_ref().is_some_and(takes_name);
      let site = if names { Site::Spelled } else { self.site };
      self.declaring(kind, |walk| walk.at_site(site, |walk| walk.visit_binding_pattern(&declarator.id)));
      self.binder = around;
      if let Some(annotation) = &declarator.type_annotation {
        self.visit_ts_type_annotation(annotation);
      }
      if let Some(init) = &declarator.init {
        self.visit_expression(init);
      }
    }
  }

  fn visit_binding_pattern(&mut self, it: &BindingPattern<'a>) {
    match it {
      BindingPattern::BindingIdentifier(identifier) => self.visit_binding_identifier(identifier),
      BindingPattern::ObjectPattern(pattern) => {
        for property in &pattern.properties {
          if !property.shorthand {
            self.aside(|walk| walk.pattern_key(&property.key, property.computed));
          }
          // `{ total }` names the property it reads; `{ key: total }` takes a value apart
          let site = if property.shorthand { Site::Shorthand } else { Site::Destructured };
          self.at_site(site, |walk| walk.visit_binding_pattern(&property.value));
        }
        if let Some(rest) = &pattern.rest {
          self.at_site(Site::Destructured, |walk| walk.visit_binding_pattern(&rest.argument));
        }
      }
      BindingPattern::ArrayPattern(pattern) => {
        self.at_site(Site::Destructured, |walk| {
          for element in pattern.elements.iter().flatten() {
            walk.visit_binding_pattern(element);
          }
          if let Some(rest) = &pattern.rest {
            walk.visit_binding_pattern(&rest.argument);
          }
        });
      }
      BindingPattern::AssignmentPattern(pattern) => {
        self.visit_binding_pattern(&pattern.left);
        self.aside(|walk| walk.visit_expression(&pattern.right));
      }
    }
  }

  fn visit_function(&mut self, it: &Function<'a>, _: ScopeFlags) {
    match it.r#type {
      FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction => {
        if let Some(id) = &it.id {
          let around = std::mem::replace(&mut self.binder, Binder::Function);
          self.binding(id, Space::Value);
          self.binder = around;
        }
        self.function(it, false, LocalKind::Parameter);
      }
      FunctionType::FunctionExpression | FunctionType::TSEmptyBodyFunctionExpression => self.function(it, true, LocalKind::LambdaParameter),
    }
  }

  fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
    self.function_scope(LocalKind::LambdaParameter, |walk| {
      if let Some(parameters) = &it.type_parameters {
        walk.visit_ts_type_parameter_declaration(parameters);
      }
      walk.visit_formal_parameters(&it.params);
      if let Some(annotation) = &it.return_type {
        walk.visit_ts_type_annotation(annotation);
      }
      walk.settle();
      match &it.body {
        ArrowFunctionBody::FunctionBody(body) => {
          for statement in &body.statements {
            walk.visit_statement(statement);
          }
        }
        body => {
          if let Some(expression) = body.as_expression() {
            walk.visit_expression(expression);
          }
        }
      }
    });
  }

  fn visit_class(&mut self, it: &Class<'a>) {
    for decorator in &it.decorators {
      self.visit_decorator(decorator);
    }
    let declaration = it.r#type == ClassType::ClassDeclaration;
    if declaration && let Some(id) = &it.id {
      self.binding(id, Space::Both);
    }
    self.scoped(false, |walk| {
      if !declaration && let Some(id) = &it.id {
        walk.binding(id, Space::Both);
      }
      if let Some(parameters) = &it.type_parameters {
        walk.visit_ts_type_parameter_declaration(parameters);
      }
      if let Some(heritage) = &it.heritage {
        walk.visit_expression(&heritage.expression);
        if let Some(arguments) = &heritage.type_arguments {
          walk.visit_ts_type_parameter_instantiation(arguments);
        }
      }
      for implemented in &it.implements {
        walk.visit_ts_class_implements(implemented);
      }
      for element in &it.body.body {
        match element {
          ClassElement::MethodDefinition(method) => walk.visit_method_definition(method),
          ClassElement::PropertyDefinition(property) => walk.field(&field_of!(property)),
          ClassElement::AccessorProperty(property) => walk.field(&field_of!(property)),
          element => walk.visit_class_element(element),
        }
      }
    });
  }

  fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
    for decorator in &it.decorators {
      self.visit_decorator(decorator);
    }
    self.member_declaration(&it.key, it.computed);
    self.function(&it.value, false, LocalKind::Parameter);
  }

  fn visit_ts_index_signature(&mut self, it: &TSIndexSignature<'a>) {
    self.visit_ts_type_annotation(&it.parameter.type_annotation);
    self.visit_ts_type_annotation(&it.type_annotation);
  }

  fn visit_ts_property_signature(&mut self, it: &TSPropertySignature<'a>) {
    self.member_declaration(&it.key, it.computed);
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
  }

  fn visit_ts_method_signature(&mut self, it: &TSMethodSignature<'a>) {
    self.member_declaration(&it.key, it.computed);
    self.function_scope(LocalKind::Other, |walk| {
      js::walk_signature(walk, it.type_parameters.as_deref(), it.this_param.as_deref(), &it.params, it.return_type.as_deref());
    });
  }

  fn visit_ts_call_signature_declaration(&mut self, it: &TSCallSignatureDeclaration<'a>) {
    self.function_scope(LocalKind::Other, |walk| walk::walk_ts_call_signature_declaration(walk, it));
  }

  fn visit_ts_construct_signature_declaration(&mut self, it: &TSConstructSignatureDeclaration<'a>) {
    self.function_scope(LocalKind::Other, |walk| walk::walk_ts_construct_signature_declaration(walk, it));
  }

  fn visit_ts_function_type(&mut self, it: &TSFunctionType<'a>) {
    self.function_scope(LocalKind::Other, |walk| walk::walk_ts_function_type(walk, it));
  }

  fn visit_ts_constructor_type(&mut self, it: &TSConstructorType<'a>) {
    self.function_scope(LocalKind::Other, |walk| walk::walk_ts_constructor_type(walk, it));
  }

  fn visit_ts_interface_declaration(&mut self, it: &TSInterfaceDeclaration<'a>) {
    self.binding(&it.id, Space::Type);
    self.scoped(false, |walk| {
      if let Some(parameters) = &it.type_parameters {
        walk.visit_ts_type_parameter_declaration(parameters);
      }
      for heritage in &it.extends {
        walk.visit_ts_interface_heritage(heritage);
      }
      walk.visit_ts_interface_body(&it.body);
    });
  }

  fn visit_ts_type_alias_declaration(&mut self, it: &TSTypeAliasDeclaration<'a>) {
    self.binding(&it.id, Space::Type);
    self.scoped(false, |walk| {
      if let Some(parameters) = &it.type_parameters {
        walk.visit_ts_type_parameter_declaration(parameters);
      }
      walk.visit_ts_type(&it.type_annotation);
    });
  }

  /// The members of an enum are seen by their bare names in its body alone (`B = A * 2`)
  fn visit_ts_enum_declaration(&mut self, it: &TSEnumDeclaration<'a>) {
    self.binding(&it.id, Space::Both);
    self.scoped(false, |walk| {
      let body = walk.scopes.len().saturating_sub(1);
      for member in &it.body.members {
        if let TSEnumMemberName::Identifier(name) = &member.id {
          walk.declaration(name.span, Some(body), Space::Value);
        }
        if let Some(initializer) = &member.initializer {
          walk.visit_expression(initializer);
        }
      }
    });
  }

  fn visit_ts_namespace_declaration(&mut self, it: &TSNamespaceDeclaration<'a>) {
    self.binding(&it.id, Space::Both);
    self.scoped(false, |walk| match &it.body {
      TSNamespaceDeclarationBody::TSNamespaceDeclaration(inner) => walk.visit_ts_namespace_declaration(inner),
      TSNamespaceDeclarationBody::TSModuleBlock(block) => walk.visit_ts_module_block(block),
    });
  }

  fn visit_ts_external_module_declaration(&mut self, it: &TSExternalModuleDeclaration<'a>) {
    self.scoped(false, |walk| walk::walk_ts_external_module_declaration(walk, it));
  }

  fn visit_ts_global_declaration(&mut self, it: &TSGlobalDeclaration<'a>) {
    self.scoped(false, |walk| walk::walk_ts_global_declaration(walk, it));
  }

  fn visit_ts_type_parameter(&mut self, it: &TSTypeParameter<'a>) {
    self.binding(&it.name, Space::Type);
    js::walk_type_parameter_bounds(self, it);
  }

  fn visit_ts_mapped_type(&mut self, it: &TSMappedType<'a>) {
    self.scoped(false, |walk| walk::walk_ts_mapped_type(walk, it));
  }

  /// What `infer` declares is seen in the whole conditional type
  fn visit_ts_conditional_type(&mut self, it: &TSConditionalType<'a>) {
    self.scoped(false, |walk| walk::walk_ts_conditional_type(walk, it));
  }

  fn visit_ts_type_name(&mut self, it: &TSTypeName<'a>) {
    self.type_name(it, ReferenceKind::Type);
  }

  fn visit_ts_type_query(&mut self, it: &TSTypeQuery<'a>) {
    js::walk_type_query(self, it, |walk, name| walk.type_name(name, ReferenceKind::Value));
  }

  fn visit_ts_type_predicate(&mut self, it: &TSTypePredicate<'a>) {
    if let TSTypePredicateName::Identifier(name) = &it.parameter_name {
      self.value(name.span, ReferenceKind::Value, Access::Read);
    }
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
  }

  fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
    self.importing = true;
    for specifier in it.specifiers.iter().flatten() {
      match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(it) => {
          self.module_export_name(&it.imported, Binds::Never);
          // `{ a as b }` names two, `{ a }` one
          if it.local.span != it.imported.span() {
            self.visit_binding_identifier(&it.local);
          }
        }
        ImportDeclarationSpecifier::ImportDefaultSpecifier(it) => self.visit_binding_identifier(&it.local),
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(it) => self.visit_binding_identifier(&it.local),
      }
    }
    self.importing = false;
  }

  fn visit_ts_import_equals_declaration(&mut self, it: &TSImportEqualsDeclaration<'a>) {
    self.importing = true;
    self.visit_binding_identifier(&it.id);
    self.importing = false;
    match &it.module_reference {
      TSModuleReference::IdentifierReference(name) => self.value(name.span, ReferenceKind::Value, Access::Read),
      TSModuleReference::QualifiedName(name) => self.qualified_name(name, ReferenceKind::Value),
      TSModuleReference::ExternalModuleReference(_) => {}
    }
  }

  fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'a>) {
    for specifier in &it.specifiers {
      self.module_export_name(&specifier.local, Binds::Scopes);
      if specifier.exported.span() != specifier.local.span() {
        self.module_export_name(&specifier.exported, Binds::Never);
      }
    }
  }

  fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
    if let Some(exported) = &it.exported {
      self.module_export_name(exported, Binds::Never);
    }
  }

  /// `import("m").A.B`: the names after the module, members of what it names
  fn visit_ts_import_type(&mut self, it: &TSImportType<'a>) {
    let mut qualifier = it.qualifier.as_ref();
    while let Some(name) = qualifier {
      let (span, left) = match name {
        TSImportTypeQualifier::Identifier(name) => (name.span, None),
        TSImportTypeQualifier::QualifiedName(name) => (name.right.span, Some(&name.left)),
      };
      // the qualifier is the text before the name and its dot: `import("m").A`
      let object = Span::new(it.span.start, span.start.saturating_sub(1).max(it.span.start));
      self.unbound(span, ReferenceKind::Member, Some(object), Access::Read);
      qualifier = left;
    }
    if let Some(arguments) = &it.type_arguments {
      self.visit_ts_type_parameter_instantiation(arguments);
    }
  }

  fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
    for specifier in &it.specifiers {
      self.module_export_name(&specifier.local, Binds::Never);
      if specifier.exported.span() != specifier.local.span() {
        self.module_export_name(&specifier.exported, Binds::Never);
      }
    }
  }

  fn visit_jsx_element_name(&mut self, it: &JSXElementName<'a>) {
    match it {
      JSXElementName::Identifier(name) => self.unbound(name.span, ReferenceKind::Value, None, Access::Read),
      JSXElementName::IdentifierReference(name) => self.value(name.span, ReferenceKind::Value, Access::Read),
      JSXElementName::MemberExpression(member) => self.jsx_member(member),
      JSXElementName::NamespacedName(_) | JSXElementName::ThisExpression(_) => {}
    }
  }

  fn visit_jsx_attribute_name(&mut self, it: &JSXAttributeName<'a>) {
    if let JSXAttributeName::Identifier(name) = it {
      self.unbound(name.span, ReferenceKind::Value, None, Access::Read);
    }
  }
}
