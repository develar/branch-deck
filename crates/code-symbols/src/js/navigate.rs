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
//!   scope that declares one, else the first (a function declared after its use). A bare name is never a member of
//!   the class around it, which only `this.` reaches; a member after a qualifier has none.
//! - **The usages** of a local (a declaration inside a function) are the names bound to it; those of any other name
//!   are its names that no local binds, each with how it uses the name: declared, imported, called, written (the
//!   target of an assignment, a destructuring one too, and of `++`), else read.
//! - **What a local is** (`LocalBinding`): a `var`, `let` or `const` a variable, the one of the head of a `for` a loop
//!   variable, the parameter of a `catch`, of an arrow or a function expression, and of a function declaration or a
//!   method; no call names a parameter. A parameter with a modifier (`private total`) declares a property; it, the
//!   parameter of a signature without a body, a function, a class, a type and a type parameter are `Other`.
//! - **Its site** (`Site`): `{ total }` of an object, of a pattern and of the target of an assignment names the
//!   property too; any other name a pattern declares or a destructuring assigns is destructured; a name read as a
//!   type that the scopes bind to a value is a namesake (types and values are apart).

use crate::js;
use crate::js::field_of;
use crate::navigate::{
  Access, Bound, LocalBinding, LocalKind, Names, Position, ReferenceAt, ReferenceKind, Resolved, Site, Wanted, byte_offset, reference_answer, site_by_binding,
};
use js_syntax::Dialect;
// jscpd:ignore-start: the names of oxc's tree and visitor each walk reads
use js_syntax::oxc_ast::ast::{
  ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression, AssignmentTarget, AssignmentTargetMaybeDefault, AssignmentTargetProperty, BindingIdentifier, BindingPattern,
  BlockStatement, CallExpression, CatchClause, Class, ClassElement, ClassType, ExportAllDeclaration, ExportFromDeclaration, ExportNamedDeclaration, Expression, ForInStatement,
  ForOfStatement, ForStatement, ForStatementInit, ForStatementLeft, FormalParameter, FormalParameterRest, Function, FunctionType, IdentifierReference, ImportDeclaration,
  ImportDeclarationSpecifier, JSXAttributeName, JSXElementName, JSXMemberExpression, JSXMemberExpressionObject, MethodDefinition, ModuleExportName, NewExpression, ObjectProperty,
  PrivateFieldExpression, PrivateInExpression, Program, PropertyKey, SimpleAssignmentTarget, StaticBlock, StaticMemberExpression, SwitchStatement, TSCallSignatureDeclaration,
  TSConditionalType, TSConstructSignatureDeclaration, TSConstructorType, TSEnumDeclaration, TSEnumMemberName, TSExternalModuleDeclaration, TSFunctionType, TSGlobalDeclaration,
  TSImportEqualsDeclaration, TSImportType, TSImportTypeQualifier, TSIndexSignature, TSInterfaceDeclaration, TSMappedType, TSMethodSignature, TSModuleReference,
  TSNamespaceDeclaration, TSNamespaceDeclarationBody, TSPropertySignature, TSQualifiedName, TSTypeAliasDeclaration, TSTypeName, TSTypeParameter, TSTypePredicate,
  TSTypePredicateName, TSTypeQuery, TaggedTemplateExpression, UpdateExpression, VariableDeclaration, VariableDeclarationKind,
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
  super::with_program(dialect, text.as_bytes(), |parsed| Some(read(&ProgramNames { program: parsed.program()?, text }))).flatten()
}

/// A program and its text, for the names of `navigate::Names`
struct ProgramNames<'p, 'a> {
  program: &'p Program<'a>,
  text: &'p str,
}

impl Names for ProgramNames<'_, '_> {
  fn name_at(&self, offset: usize) -> Option<(usize, &str)> {
    let (span, name) = name_at(self.program, self.text, u32::try_from(offset).ok()?)?;
    Some((span.start as usize, name))
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
}

/// The name whose text holds the byte at `offset`, else the one that ends there (`foo|`), and its text
fn name_at<'t>(program: &Program<'_>, text: &'t str, offset: u32) -> Option<(Span, &'t str)> {
  let mut walk = Walk::new(text, Want::At(offset));
  walk.visit_program(program);
  let span = walk.at.or(walk.before)?;
  Some((span, text.get(span.start as usize..span.end as usize)?))
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

/// The declaration a name is bound to: where its name starts, and what it is when a function declares it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Declared {
  start: u32,
  local: Option<LocalBinding>,
}

/// A name of the text looked for, with what its syntax says
struct Named {
  span: Span,
  kind: ReferenceKind,
  qualifier: Option<Span>,
  access: Access,
  site: Site,
  binding: Option<Declared>,
}

/// What a walk looks for: the name at an offset, or the names of a resolution
#[derive(Clone, Copy)]
enum Want<'t> {
  At(u32),
  Named(Wanted<'t>),
}

/// A scope of the walk: its declarations of the name, the names in it its declarations may bind, and whether it is a
/// function's (its `var`s, its parameters, its declarations locals)
struct Scope {
  function: bool,
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
  names: Vec<Named>,
  scopes: Vec<Scope>,
  /// Inside an import: its names are imports
  importing: bool,
  /// The declarations of a `var` belong to the function around them
  var: bool,
  /// The next `var`, `let` or `const` is the head's of a `for`
  loop_head: bool,
  /// What the parameters of the function the walk is in declare
  parameters: LocalKind,
  /// What a binding the walk meets declares, inside a function: set around a pattern, `Other` elsewhere
  declares: LocalKind,
  /// What the site of the names the walk meets makes of them: set around a pattern or a shorthand, `Plain` elsewhere
  site: Site,
}

impl<'t> Walk<'t> {
  const fn new(text: &'t str, want: Want<'t>) -> Self {
    Self {
      text,
      want,
      at: None,
      before: None,
      names: Vec::new(),
      scopes: Vec::new(),
      importing: false,
      var: false,
      loop_head: false,
      parameters: LocalKind::Other,
      declares: LocalKind::Other,
      site: Site::Plain,
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
  /// pattern
  fn aside(&mut self, body: impl FnOnce(&mut Self)) {
    self.declaring(LocalKind::Other, |walk| walk.at_site(Site::Plain, body));
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
      declared: Vec::new(),
      pending: Vec::new(),
    });
  }

  /// Leaves a scope: its declarations bind the names waiting in it, the last before each name else the first; the other
  /// names wait in the scope around it
  fn pop(&mut self) {
    let Some(scope) = self.scopes.pop() else { return };
    if scope.declared.is_empty() {
      if let Some(outer) = self.scopes.last_mut() {
        outer.pending.extend(scope.pending);
      }
      return;
    }
    for index in scope.pending {
      let named = &mut self.names[index];
      let start = named.span.start;
      let binding = scope.declared.iter().rev().find(|it| it.start <= start).or_else(|| scope.declared.first()).copied();
      named.binding = binding;
      // a type is never a value: `total` of `let x: total` is a type of that spelling
      named.site = site_by_binding(named.site, named.kind == ReferenceKind::Type, binding.and_then(|it| it.local));
    }
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
        self.names.push(Named {
          span,
          kind,
          qualifier,
          access,
          site: self.site,
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

  /// The name of a declaration, which the scope at `scope` declares (or no scope, `None`: a member)
  fn declaration(&mut self, span: Span, scope: Option<usize>) {
    // inside a function a declaration is a local: what the pattern around it declares, a member `Other`
    let in_function = self.in_function(scope.unwrap_or_else(|| self.scopes.len().saturating_sub(1)));
    let kind = if scope.is_some() { self.declares } else { LocalKind::Other };
    let declared = Declared {
      start: span.start,
      local: in_function.then(|| LocalBinding::of(kind)),
    };
    if let (Want::Named(wanted), Some(scope)) = (self.want, scope)
      && wanted.takes(self.text_of(span), span.start as usize)
    {
      self.scopes[scope].declared.push(declared);
    }
    let index = self.names.len();
    self.name(span, ReferenceKind::Declaration, None, Access::Declaration, Binds::Never);
    // the name of a declaration is bound to itself
    if self.names.len() > index {
      self.names[index].binding = Some(declared);
    }
  }

  /// A binding in the innermost scope, a `var`'s in the function's around it
  fn binding(&mut self, identifier: &BindingIdentifier<'_>) {
    let innermost = self.scopes.len().saturating_sub(1);
    let scope = if self.var {
      self.scopes.iter().rposition(|it| it.function).unwrap_or(0)
    } else {
      innermost
    };
    self.declaration(identifier.span, Some(scope));
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
      PropertyKey::StaticIdentifier(it) if !computed => self.declaration(it.span, None),
      PropertyKey::PrivateIdentifier(it) => self.declaration(it.span, None),
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

  /// `a.b.C`: `C` of the kind (a member of a value), qualified by `a.b`, whose names are values
  fn qualified_name(&mut self, name: &TSQualifiedName<'_>, kind: ReferenceKind) {
    self.type_name(&name.left, ReferenceKind::Value);
    let member = if kind == ReferenceKind::Type { ReferenceKind::Type } else { ReferenceKind::Member };
    self.unbound(name.right.span, member, Some(name.left.span()), Access::Read);
  }

  /// A function: its parameters (which declare `parameters`), type parameters and body in a scope of its own, its own
  /// name too for an expression
  fn function(&mut self, function: &Function<'_>, own_name: bool, parameters: LocalKind) {
    self.function_scope(parameters, |walk| {
      if own_name && let Some(id) = &function.id {
        walk.binding(id);
      }
      js::walk_function(walk, function);
    });
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
      self.binding(it);
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
    self.target(&it.left);
    self.visit_expression(&it.right);
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
    self.loop_head = matches!(it, ForStatementInit::VariableDeclaration(_));
    walk::walk_for_statement_init(self, it);
  }

  fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
    self.scoped(false, |walk| walk::walk_for_in_statement(walk, it));
  }

  fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
    self.scoped(false, |walk| walk::walk_for_of_statement(walk, it));
  }

  fn visit_for_statement_left(&mut self, it: &ForStatementLeft<'a>) {
    self.loop_head = matches!(it, ForStatementLeft::VariableDeclaration(_));
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
    // a parameter with a modifier (`private total`) declares a property of its class
    let kind = if it.accessibility.is_some() || it.readonly || it.r#override {
      LocalKind::Other
    } else {
      self.parameters
    };
    self.declaring(kind, |walk| walk.visit_binding_pattern(&it.pattern));
    js::walk_parameter_type(self, it.type_annotation.as_deref(), it.initializer.as_deref());
  }

  fn visit_static_block(&mut self, it: &StaticBlock<'a>) {
    self.function_scope(LocalKind::Other, |walk| walk::walk_static_block(walk, it));
  }

  fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
    // only the declaration that is the head of a `for` declares its variables
    let kind = if std::mem::take(&mut self.loop_head) {
      LocalKind::LoopVariable
    } else {
      LocalKind::Variable
    };
    for declarator in &it.declarations {
      self.var = it.kind == VariableDeclarationKind::Var;
      self.declaring(kind, |walk| walk.visit_binding_pattern(&declarator.id));
      self.var = false;
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
          self.binding(id);
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
      self.binding(id);
    }
    self.scoped(false, |walk| {
      if !declaration && let Some(id) = &it.id {
        walk.binding(id);
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
    self.binding(&it.id);
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
    self.binding(&it.id);
    self.scoped(false, |walk| {
      if let Some(parameters) = &it.type_parameters {
        walk.visit_ts_type_parameter_declaration(parameters);
      }
      walk.visit_ts_type(&it.type_annotation);
    });
  }

  fn visit_ts_enum_declaration(&mut self, it: &TSEnumDeclaration<'a>) {
    self.binding(&it.id);
    for member in &it.body.members {
      if let TSEnumMemberName::Identifier(name) = &member.id {
        self.declaration(name.span, None);
      }
      if let Some(initializer) = &member.initializer {
        self.visit_expression(initializer);
      }
    }
  }

  fn visit_ts_namespace_declaration(&mut self, it: &TSNamespaceDeclaration<'a>) {
    self.binding(&it.id);
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
    self.binding(&it.name);
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
