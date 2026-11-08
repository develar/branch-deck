//! The summary of a TypeScript, TSX or JavaScript file from oxc's tree (`FileSummary`), by the rules that the head of
//! `extract.rs` states for every language (locals dropped): one recursive walk (`Visit`) of the tree inside
//! `js::with_program`, whose stack holds it.
//!
//! - **Declarations**: a class, an interface, an enum, a type alias (their type parameters seen in all of them), a
//!   namespace or an ambient module (`declare module "m"`, by its name when it reads as an identifier; `namespace a.b.c`
//!   is one named `c`), a function and its overload or `declare` signatures, a variable declarator's names at the top
//!   level or in a namespace (an arrow or function value makes a `Function`, an `UPPER_CASE` `const` a `Constant`); a
//!   member of a class (a method, a field, an accessor: `get`/`set` are values; no constructor), of an interface, of an
//!   object type a type alias holds, an enum's members; a constructor's parameter property (`private readonly x`) is a
//!   field of its class. A member is named by an identifier, a private name, a string without escapes that reads as an
//!   identifier, or a computed identifier (`[k]`); other computed keys, numbers and object-literal members name none.
//! - **Reach**: `export` makes a top-level declaration public, and so does `export { name }` below it (`finish`); a
//!   member of a namespace is private to it unless exported; a class member is private for `private` and a private
//!   name, else public (`protected` too).
//! - **Imports**: `import` and `import x = require("m")` are `Import`s with their module specifier and `Binding`
//!   declarations, a named one also an `Import` reference to the name it imports; a re-export (`export … from`) is an
//!   exported `Import` and its names are public bindings, and no import line.
//! - **Scopes**: a function, an arrow, a method, a block, a loop, a `catch`, a `switch` and a class expression's body
//!   are local: what they declare binds its name and declares nothing. `let`, `const`, `class`, `function` and `enum`
//!   are visible in their whole block, `var` in its whole function (hoisting), parameters and the names of a pattern
//!   after their pattern, type parameters, a mapped type's key and `infer` in the scope around them. A namespace body
//!   is no scope.
//! - **References**: an identifier of the code (a type in a type position, the callee of a call, `new` or a tagged
//!   template a call), the name of a member access after its qualifier (`a.b.c`; `this` and `super` are segments, a
//!   qualifier whose first name is bound or that is no path is empty), a capitalised JSX tag (a type). Not read:
//!   labels, keys, JSX attributes, `undefined`, a lowercase JSX tag, the names of an index signature or a type
//!   predicate.
//! - **Headers** (`Declaration::header`): the tokens of a function, a field or a variable before its body or
//!   initializer, the `const` of its statement before a declarator, without decorators, comments, accessibility and
//!   `override` modifiers, `declare` before a statement and `;`; parameters, type parameters and their defaults kept.
//! - **Errors**: the bytes a recovery blanked and the labels of the parse's diagnostics (`js::Errors`); a declaration
//!   with one between its start and its name is `in_error`. A file no recovery saves has every byte in error and no
//!   facts.

use crate::js;
use crate::js::{Errors, Source, doc_lines, field_of, tokens_in, with_program};
use crate::summary::{DeclKind, FileSummary, Import, ImportTarget, LineSpan, MemberKind, Reach, Role};
use crate::summary_builder::{Bindings, SummaryBuilder, blank_declaration};
use js_syntax::Dialect;
use js_syntax::oxc_ast::ast::{
  Argument, ArrowFunctionBody, ArrowFunctionExpression, BindingIdentifier, BindingPattern, BlockStatement, CallExpression, CatchClause, Class, ClassElement, ClassType,
  Declaration, Decorator, ExportAllDeclaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportFromDeclaration, ExportNamedDeclaration, Expression, ForInStatement,
  ForOfStatement, ForStatement, ForStatementLeft, FormalParameter, FormalParameterRest, FormalParameters, Function, FunctionType, IdentifierReference, ImportDeclaration,
  ImportDeclarationSpecifier, JSXClosingElement, JSXElementName, JSXMemberExpression, JSXMemberExpressionObject, JSXOpeningElement, LabeledStatement, MethodDefinition,
  MethodDefinitionKind, ModuleDeclaration, ModuleExportName, NewExpression, ObjectProperty, PrivateFieldExpression, Program, PropertyKey, PropertyKind, Statement,
  StaticMemberExpression, StringLiteral, SwitchStatement, TSAccessibility, TSConditionalType, TSConstructorType, TSEnumDeclaration, TSEnumMemberName, TSExternalModuleDeclaration,
  TSFunctionType, TSImportEqualsDeclaration, TSImportType, TSImportTypeQualifier, TSIndexSignature, TSInferType, TSInterfaceBody, TSInterfaceDeclaration, TSMappedType,
  TSModuleReference, TSNamespaceDeclaration, TSNamespaceDeclarationBody, TSSignature, TSType, TSTypeAliasDeclaration, TSTypeLiteral, TSTypeName, TSTypeParameter,
  TSTypeParameterInstantiation, TSTypePredicate, TSTypeQuery, TSTypeQueryExprName, TaggedTemplateExpression, VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};
use js_syntax::oxc_ast_visit::{Visit, walk};
use js_syntax::oxc_parser::{Kind, Token};
use js_syntax::oxc_span::{GetSpan, Span};
use js_syntax::oxc_syntax::scope::ScopeFlags;

/// Fills `summary` (`summary_builder::empty_summary` of the file, within the size limits) with the facts of `source`
/// read as `dialect`; whether it was read, else it stays `truncated`: the file nests deeper than the parse reads
pub(crate) fn summarize(dialect: Dialect, source: &[u8], summary: &mut FileSummary) -> bool {
  with_program(dialect, source, |parsed| {
    let Some(program) = parsed.program() else {
      // no recovery saved the file: nothing of it is read
      summary.error_bytes = summary.total_bytes;
      return;
    };
    let text = Source::new(source);
    let errors = Errors::of(parsed, source);
    let mut walk = Walk {
      source: &text,
      tokens: &parsed.tokens,
      docs: doc_lines(program, &text),
      errors: &errors,
      out: SummaryBuilder::new(),
      bindings: Bindings::default(),
      containers: Vec::new(),
    };
    walk.visit_program(program);
    walk.out.error_bytes = errors.bytes;
    walk.out.finish(summary, None);
  })
  .is_some()
}

/// What a container is to the declarations in it, for their member kind and reach
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
  Class,
  Interface,
  Enum,
  /// A type alias, whose object types declare their members
  Alias,
  Module,
  /// A function, a value, a member: nothing in it is a member
  Leaf,
}

struct Container {
  decl: u32,
  holder: Holder,
}

/// The statement around a declaration: an `export` makes it public, and its start (or a variable statement's, for
/// its first declarator) is where the declaration starts
#[derive(Debug, Clone, Copy, Default)]
struct Wrap {
  exported: bool,
  start: Option<u32>,
}

/// What a declaration is, before it is declared
struct Decl<'s> {
  /// The node, from its first token (after its decorators) to its last
  span: Span,
  name: Span,
  text: &'s str,
  kind: DeclKind,
  reach: Reach,
  member: MemberKind,
  header: Option<String>,
  /// The other offsets the declaration may start at: its statement, its first decorator
  starts: [Option<u32>; 2],
  /// The doc comments above it start it: not those above an import, whose bindings start with its statement
  documented: bool,
}

struct Walk<'w, 's> {
  source: &'w Source<'s>,
  tokens: &'w [Token],
  /// (the start of the token a doc comment is attached to, the first line of the doc comments), sorted
  docs: Vec<(u32, u32)>,
  errors: &'w Errors,
  out: SummaryBuilder<'s>,
  bindings: Bindings,
  containers: Vec<Container>,
}

impl<'s> Walk<'_, 's> {
  fn text(&self, span: Span) -> Option<&'s str> {
    self.source.text(span)
  }

  const fn is_local(&self) -> bool {
    self.bindings.is_local()
  }

  fn holder(&self) -> Option<Holder> {
    self.containers.last().map(|it| it.holder)
  }

  fn container(&self) -> Option<u32> {
    self.containers.last().map(|it| it.decl)
  }

  /// The member kind and reach of a declaration in the innermost container
  fn member_and_reach(&self, exported: bool, is_static: bool, private: bool) -> (MemberKind, Reach) {
    match self.holder() {
      None => (MemberKind::TopLevel, if exported { Reach::Public } else { Reach::File }),
      Some(Holder::Module) => (MemberKind::TopLevel, if exported { Reach::Public } else { Reach::Container }),
      Some(Holder::Class) => (
        if is_static { MemberKind::Static } else { MemberKind::Instance },
        if private { Reach::Container } else { Reach::Public },
      ),
      Some(_) => (MemberKind::Instance, Reach::Public),
    }
  }

  /// The first line of a declaration that starts at `start` or at one of `starts`, with the doc comments above any of them
  fn start_line(&self, start: u32, starts: &[Option<u32>]) -> u32 {
    std::iter::once(start)
      .chain(starts.iter().flatten().copied())
      .flat_map(|offset| {
        let doc = self.docs.binary_search_by_key(&offset, |it| it.0).ok().map(|index| self.docs[index].1);
        std::iter::once(self.source.line(offset)).chain(doc)
      })
      .min()
      .unwrap_or(1)
  }

  fn declare(&mut self, decl: Decl<'s>) -> u32 {
    let name = self.out.intern(decl.text);
    let mut declaration = blank_declaration(name, decl.kind, decl.reach, decl.member, self.container());
    declaration.header = decl.header;
    declaration.start_line = if decl.documented {
      self.start_line(decl.span.start, &decl.starts)
    } else {
      self.source.line(decl.span.start)
    };
    declaration.end_line = self.source.end_line(decl.span);
    declaration.name_line = self.source.line(decl.name.start);
    declaration.name_column = self.source.column(decl.name.start);
    declaration.in_error = self
      .errors
      .within(decl.span.start.min(decl.starts.iter().flatten().copied().min().unwrap_or(u32::MAX)), decl.name.start);
    self.out.add_declaration(declaration)
  }

  /// Declares `decl` and makes it the container of what `body` walks
  fn within(&mut self, decl: Option<Decl<'s>>, holder: Holder, body: impl FnOnce(&mut Self)) {
    match decl {
      Some(decl) => {
        let id = self.declare(decl);
        self.containers.push(Container { decl: id, holder });
        body(self);
        self.containers.pop();
      }
      None => body(self),
    }
  }

  fn bind(&mut self, name: &'s str) {
    let id = self.out.intern(name);
    self.bindings.bind(id, None);
  }

  fn bind_identifier(&mut self, identifier: &BindingIdentifier<'_>) {
    if let Some(text) = self.text(identifier.span) {
      self.bind(text);
    }
  }

  /// A reference to `name` at `offset`, but to a bare `undefined` (a value of the language, which no file declares) or
  /// a bare name a scope binds
  fn reference(&mut self, name: &'s str, offset: u32, role: Role, qualifier: Option<Vec<u32>>) {
    if name.is_empty() || (qualifier.is_none() && name == "undefined") {
      return;
    }
    let id = self.out.intern(name);
    if qualifier.is_none() && self.bindings.is_bound(id) {
      return;
    }
    let line = self.source.line(offset);
    let container = self.container();
    self.out.add_reference(id, line, role, qualifier, container, None, None);
  }

  fn reference_at(&mut self, span: Span, role: Role, qualifier: Option<Vec<u32>>) {
    if let Some(text) = self.text(span) {
      self.reference(text, span.start, role, qualifier);
    }
  }

  /// The qualifier of a path of names: their ids, or empty when the first is bound (a local) or there is no path
  fn qualifier(&mut self, path: Option<Vec<&'s str>>) -> Vec<u32> {
    path.map_or_else(Vec::new, |path| self.bindings.qualifier(self.out.ids(&path)))
  }

  /// `a.b.c` as `[a, b, c]`: names, `this`, `super`, members of them, else `None`
  fn path_of(&self, expression: &Expression<'_>) -> Option<Vec<&'s str>> {
    let mut reversed = Vec::new();
    let mut current = expression;
    loop {
      match current {
        Expression::Identifier(it) if &*it.name != "undefined" => {
          reversed.push(self.text(it.span)?);
          break;
        }
        Expression::ThisExpression(_) => {
          reversed.push("this");
          break;
        }
        Expression::Super(_) => {
          reversed.push("super");
          break;
        }
        Expression::StaticMemberExpression(it) => {
          reversed.push(self.text(it.property.span)?);
          current = &it.object;
        }
        Expression::PrivateFieldExpression(it) => {
          reversed.push(self.text(it.field.span)?);
          current = &it.object;
        }
        Expression::TSInstantiationExpression(it) => current = &it.expression,
        _ => return None,
      }
    }
    reversed.reverse();
    Some(reversed)
  }

  fn type_name_path(&self, name: &TSTypeName<'_>) -> Option<Vec<&'s str>> {
    match name {
      TSTypeName::IdentifierReference(it) => Some(vec![self.text(it.span)?]),
      TSTypeName::ThisExpression(_) => Some(vec!["this"]),
      TSTypeName::QualifiedName(it) => {
        let mut path = self.type_name_path(&it.left)?;
        path.push(self.text(it.right.span)?);
        Some(path)
      }
    }
  }

  /// A name in a type position (`role` a type) or of `typeof` (a value): its last name with the role, qualified by the
  /// names before it, each a value of its own
  fn type_name(&mut self, name: &TSTypeName<'_>, role: Role) {
    match name {
      // `as const` is a type reference of the name `const` to oxc
      TSTypeName::IdentifierReference(it) if &*it.name == "const" => {}
      TSTypeName::IdentifierReference(it) => self.reference_at(it.span, role, None),
      TSTypeName::QualifiedName(it) => {
        let qualifier = self.type_name_path(&it.left);
        let qualifier = self.qualifier(qualifier);
        self.reference_at(it.right.span, role, Some(qualifier));
        self.type_name(&it.left, Role::Value);
      }
      TSTypeName::ThisExpression(_) => {}
    }
  }

  /// `import("m").A.B`: the names after the module, each qualified by nothing the file names, the last with `role`
  fn import_type(&mut self, import: &TSImportType<'_>, role: Role) {
    let mut qualifier = import.qualifier.as_ref();
    let mut role = role;
    while let Some(name) = qualifier {
      match name {
        TSImportTypeQualifier::Identifier(it) => {
          self.reference_at(it.span, role, Some(Vec::new()));
          qualifier = None;
        }
        TSImportTypeQualifier::QualifiedName(it) => {
          self.reference_at(it.right.span, role, Some(Vec::new()));
          qualifier = Some(&it.left);
        }
      }
      role = Role::Value;
    }
    if let Some(arguments) = &import.type_arguments {
      self.visit_ts_type_parameter_instantiation(arguments);
    }
  }

  /// `object.name` with `role`: `name` qualified by the path of `object`, which is walked
  fn member(&mut self, object: &Expression<'_>, name: Span, role: Role) {
    let qualifier = self.path_of(object);
    let qualifier = self.qualifier(qualifier);
    self.reference_at(name, role, Some(qualifier));
    self.visit_expression(object);
  }

  /// A call or a `new`: its callee a call, its type arguments and its arguments
  fn call(&mut self, callee: &Expression<'_>, type_arguments: Option<&TSTypeParameterInstantiation<'_>>, arguments: &[Argument<'_>]) {
    self.callee(callee, Role::Call);
    js::walk_arguments(self, type_arguments, arguments);
  }

  /// `for (left of right) body`, `for (left in right) body`: a declaration of `left` binds in the loop
  fn for_each(&mut self, left: &ForStatementLeft<'_>, right: &Expression<'_>, body: &Statement<'_>) {
    self.visit_expression(right);
    self.bindings.push(true);
    self.visit_for_statement_left(left);
    self.visit_statement(body);
    self.bindings.pop();
  }

  /// The callee of a call, `new` or a tagged template: a name or a member gets `role`, through `f<T>`
  fn callee(&mut self, callee: &Expression<'_>, role: Role) {
    match callee {
      Expression::Identifier(it) => self.reference_at(it.span, role, None),
      Expression::StaticMemberExpression(it) => self.member(&it.object, it.property.span, role),
      Expression::PrivateFieldExpression(it) => self.member(&it.object, it.field.span, role),
      Expression::TSInstantiationExpression(it) => {
        self.callee(&it.expression, role);
        self.visit_ts_type_parameter_instantiation(&it.type_arguments);
      }
      _ => self.visit_expression(callee),
    }
  }

  /// The name a member key declares: an identifier, a private name, a string without escapes that reads as an
  /// identifier (its content), a computed identifier
  fn key_name(&self, key: &PropertyKey<'_>, computed: bool) -> Option<(Span, &'s str)> {
    let span = match key {
      PropertyKey::StaticIdentifier(it) => it.span,
      PropertyKey::PrivateIdentifier(it) => it.span,
      PropertyKey::Identifier(it) if computed => it.span,
      PropertyKey::StringLiteral(it) if !computed && it.span.size() > 2 => Span::new(it.span.start + 1, it.span.end - 1),
      _ => return None,
    };
    let text = self.text(span)?;
    let identifier = text.chars().next().is_some_and(|it| it.is_alphabetic() || it == '_' || it == '$' || it == '#') && !text.contains('\\');
    identifier.then_some((span, text))
  }

  /// Walks the code of a key: a computed one but a computed identifier that names the member
  fn key_code(&mut self, key: &PropertyKey<'_>, computed: bool) {
    if computed && !matches!(key, PropertyKey::Identifier(_)) {
      self.visit_property_key(key);
    }
  }

  /// The header of a declaration (`Declaration::header`): the tokens of `pieces` in order, without those in
  /// `skipped` (decorators), the accessibility and `override` modifiers before `modifiers_end`, `;`, and a last `=` or
  /// separator
  fn header(&self, pieces: &[(u32, u32)], skipped: &[Span], modifiers_end: u32) -> String {
    let mut tokens: Vec<Token> = Vec::new();
    for &(start, end) in pieces {
      tokens.extend(tokens_in(self.tokens, start, end).iter().filter(|token| {
        let kind = token.kind();
        !skipped.iter().any(|it| it.start <= token.start() && token.end() <= it.end)
          && kind != Kind::Semicolon
          && !(token.start() < modifiers_end && matches!(kind, Kind::Private | Kind::Protected | Kind::Public | Kind::Override))
      }));
    }
    if tokens.last().is_some_and(|it| matches!(it.kind(), Kind::Eq | Kind::Comma)) {
      tokens.pop();
    }
    let texts: Vec<&str> = tokens.iter().filter_map(|it| self.text(it.span())).filter(|it| !it.is_empty()).collect();
    texts.join(" ")
  }

  /// The offset after a `declare` that opens the statement at `start`
  fn after_declare(&self, start: u32) -> u32 {
    match tokens_in(self.tokens, start, start.saturating_add(8)).first() {
      Some(token) if token.kind() == Kind::Declare && token.start() == start => tokens_in(self.tokens, token.end(), u32::MAX).first().map_or_else(|| token.end(), Token::start),
      _ => start,
    }
  }

  // ---- statements and declarations

  /// Binds what a block's statements declare in all of it: `let`, `const`, a class, a function, an enum
  fn hoist_block(&mut self, statements: &[Statement<'_>]) {
    for statement in statements {
      match statement {
        Statement::VariableDeclaration(it) if it.kind != VariableDeclarationKind::Var => {
          for declarator in &it.declarations {
            for name in pattern_names(&declarator.id) {
              self.bind_identifier(name);
            }
          }
        }
        Statement::FunctionDeclaration(it) if it.r#type == FunctionType::FunctionDeclaration => {
          if let Some(id) = &it.id {
            self.bind_identifier(id);
          }
        }
        Statement::ClassDeclaration(it) => {
          if let Some(id) = &it.id {
            self.bind_identifier(id);
          }
        }
        Statement::TSEnumDeclaration(it) => self.bind_identifier(&it.id),
        _ => {}
      }
    }
  }

  /// Binds the `var`s of a function body, not those of the functions and classes in it
  fn hoist_vars(&mut self, statements: &[Statement<'_>]) {
    let mut vars = Vars { names: Vec::new() };
    for statement in statements {
      vars.visit_statement(statement);
    }
    for span in vars.names {
      if let Some(text) = self.text(span) {
        self.bind(text);
      }
    }
  }

  fn declaration(&mut self, declaration: &Declaration<'_>, wrap: Wrap) {
    match declaration {
      Declaration::VariableDeclaration(it) => self.variables(it, wrap),
      Declaration::FunctionDeclaration(it) => self.function_declaration(it, wrap),
      Declaration::ClassDeclaration(it) => self.class(it, wrap),
      Declaration::TSTypeAliasDeclaration(it) => self.type_alias(it, wrap),
      Declaration::TSInterfaceDeclaration(it) => self.interface(it, wrap),
      Declaration::TSEnumDeclaration(it) => self.enumeration(it, wrap),
      Declaration::TSExternalModuleDeclaration(it) => self.external_module(it, wrap),
      Declaration::TSNamespaceDeclaration(it) => self.namespace(it, wrap),
      Declaration::TSGlobalDeclaration(it) => {
        // `declare global { … }` is a block: what it declares is local to it
        self.bindings.push(true);
        self.hoist_block(&it.body.body);
        for statement in &it.body.body {
          self.visit_statement(statement);
        }
        self.bindings.pop();
      }
      Declaration::TSImportEqualsDeclaration(it) => self.import_equals(it),
    }
  }

  /// A declaration of a type or a module named `id`, in `wrap`; `None` when it is a local, whose name it binds
  fn type_decl(&mut self, span: Span, id: Span, kind: DeclKind, wrap: Wrap, starts: Option<u32>) -> Option<Decl<'s>> {
    let text = self.text(id)?;
    if self.is_local() {
      self.bind(text);
      return None;
    }
    let (member, reach) = self.member_and_reach(wrap.exported, false, false);
    Some(Decl {
      span,
      name: id,
      text,
      kind,
      reach,
      member,
      header: None,
      starts: [wrap.start, starts],
      documented: true,
    })
  }

  fn variables(&mut self, variables: &VariableDeclaration<'_>, wrap: Wrap) {
    let Some(first) = variables.declarations.first() else { return };
    let statement_start = self.after_declare(variables.span.start);
    let prefix = (statement_start, first.span.start);
    for (index, declarator) in variables.declarations.iter().enumerate() {
      if self.is_local() {
        self.pattern_code(&declarator.id);
        for name in pattern_names(&declarator.id) {
          self.bind_identifier(name);
        }
        self.declarator_rest(declarator);
        continue;
      }
      self.pattern_code(&declarator.id);
      let names = pattern_names(&declarator.id);
      if names.is_empty() || !matches!(self.holder(), None | Some(Holder::Module)) {
        self.declarator_rest(declarator);
        continue;
      }
      let kind = declarator_kind(self, variables.kind, declarator);
      let end = declarator.init.as_ref().map_or(declarator.span.end, |it| it.span().start);
      let header = self.header(&[prefix, (declarator.span.start, end)], &[], 0);
      let (member, reach) = self.member_and_reach(wrap.exported, false, false);
      let starts = if index == 0 { [wrap.start, Some(variables.span.start)] } else { [None, None] };
      let mut first_id = None;
      for name in names {
        let Some(text) = self.text(name.span) else { continue };
        let id = self.declare(Decl {
          span: declarator.span,
          name: name.span,
          text,
          kind,
          reach,
          member,
          header: Some(header.clone()),
          starts,
          documented: true,
        });
        first_id.get_or_insert(id);
      }
      match first_id {
        Some(id) => {
          self.containers.push(Container { decl: id, holder: Holder::Leaf });
          self.declarator_rest(declarator);
          self.containers.pop();
        }
        None => self.declarator_rest(declarator),
      }
    }
  }

  fn declarator_rest(&mut self, declarator: &VariableDeclarator<'_>) {
    if let Some(annotation) = &declarator.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
    if let Some(init) = &declarator.init {
      self.visit_expression(init);
    }
  }

  /// The code of a binding pattern: its default values and computed keys; its names are the caller's
  fn pattern_code(&mut self, pattern: &BindingPattern<'_>) {
    match pattern {
      BindingPattern::BindingIdentifier(_) => {}
      BindingPattern::AssignmentPattern(it) => {
        self.pattern_code(&it.left);
        self.visit_expression(&it.right);
      }
      BindingPattern::ObjectPattern(it) => {
        for property in &it.properties {
          if property.computed {
            self.visit_property_key(&property.key);
          }
          self.pattern_code(&property.value);
        }
        if let Some(rest) = &it.rest {
          self.pattern_code(&rest.argument);
        }
      }
      BindingPattern::ArrayPattern(it) => {
        for element in it.elements.iter().flatten() {
          self.pattern_code(element);
        }
        if let Some(rest) = &it.rest {
          self.pattern_code(&rest.argument);
        }
      }
    }
  }

  /// A pattern whose names are bound after its code
  fn bind_pattern(&mut self, pattern: &BindingPattern<'_>) {
    self.pattern_code(pattern);
    for name in pattern_names(pattern) {
      self.bind_identifier(name);
    }
  }

  fn function_declaration(&mut self, function: &Function<'_>, wrap: Wrap) {
    let decl = function.id.as_ref().and_then(|id| {
      let text = self.text(id.span)?;
      if self.is_local() {
        self.bind(text);
        return None;
      }
      let start = self.after_declare(function.span.start);
      let end = function.body.as_ref().map_or(function.span.end, |it| it.span.start);
      let header = self.header(&[(start, end)], &parameter_decorators(&function.params), 0);
      let (member, reach) = self.member_and_reach(wrap.exported, false, false);
      Some(Decl {
        span: function.span,
        name: id.span,
        text,
        kind: DeclKind::Function,
        reach,
        member,
        header: Some(header),
        starts: [wrap.start, None],
        documented: true,
      })
    });
    self.within(decl, Holder::Leaf, |walk| walk.function_scope(function, false));
  }

  /// The scope of a function: its own name for an expression, its type parameters, parameters and body
  fn function_scope(&mut self, function: &Function<'_>, own_name: bool) {
    self.bindings.push(true);
    if own_name && let Some(id) = &function.id {
      self.bind_identifier(id);
    }
    if let Some(body) = &function.body {
      self.hoist_vars(&body.statements);
      self.hoist_block(&body.statements);
    }
    js::walk_function(self, function);
    self.bindings.pop();
  }

  fn class(&mut self, class: &Class<'_>, wrap: Wrap) {
    let Some(id) = class.id.as_ref().filter(|_| class.r#type == ClassType::ClassDeclaration) else {
      // a class expression, an anonymous default export: its body is local
      self.bindings.push(true);
      if let Some(id) = &class.id {
        self.bind_identifier(id);
      }
      self.class_rest(class);
      self.bindings.pop();
      return;
    };
    let decl = self.type_decl(class.span, id.span, DeclKind::Type, wrap, class.decorators.first().map(|it| it.span.start));
    self.within(decl, Holder::Class, |walk| {
      walk.bindings.push(false);
      walk.class_rest(class);
      walk.bindings.pop();
    });
  }

  /// The decorators of a class, its type parameters, its supertypes and its members, in the scope and the container of
  /// the class
  fn class_rest(&mut self, class: &Class<'_>) {
    for decorator in &class.decorators {
      self.visit_decorator(decorator);
    }
    if let Some(parameters) = &class.type_parameters {
      self.visit_ts_type_parameter_declaration(parameters);
    }
    if let Some(heritage) = &class.heritage {
      self.visit_expression(&heritage.expression);
      if let Some(arguments) = &heritage.type_arguments {
        self.visit_ts_type_parameter_instantiation(arguments);
      }
    }
    for implemented in &class.implements {
      self.visit_ts_class_implements(implemented);
    }
    for element in &class.body.body {
      self.class_element(element);
    }
  }

  fn class_element(&mut self, element: &ClassElement<'_>) {
    match element {
      ClassElement::MethodDefinition(it) => self.method(it),
      ClassElement::PropertyDefinition(it) => self.field(&field_of!(it)),
      ClassElement::AccessorProperty(it) => self.field(&field_of!(it)),
      ClassElement::StaticBlock(it) => {
        self.bindings.push(true);
        self.hoist_block(&it.body);
        for statement in &it.body {
          self.visit_statement(statement);
        }
        self.bindings.pop();
      }
      ClassElement::TSIndexSignature(it) => self.index_signature(it),
    }
  }

  /// A member of a class, the span of its node without its decorators
  #[expect(clippy::too_many_arguments, reason = "the parts of a member that its declaration reads, of three node types")]
  fn member_decl(&self, key: &PropertyKey<'_>, computed: bool, span: Span, decorators: &[Decorator<'_>], kind: DeclKind, is_static: bool, private: bool) -> Option<Decl<'s>> {
    if self.is_local() || self.holder() != Some(Holder::Class) {
      return None;
    }
    let (name, text) = self.key_name(key, computed)?;
    let start = decorators.last().map_or(span.start, |it| first_token_after(self.tokens, it.span.end).unwrap_or(span.start));
    let (member, reach) = self.member_and_reach(false, is_static, private);
    Some(Decl {
      span: Span::new(start, span.end),
      name,
      text,
      kind,
      reach,
      member,
      header: None,
      starts: [decorators.first().map(|it| it.span.start), None],
      documented: true,
    })
  }

  fn method(&mut self, method: &MethodDefinition<'_>) {
    let private = method.accessibility == Some(TSAccessibility::Private) || matches!(method.key, PropertyKey::PrivateIdentifier(_));
    let kind = match method.kind {
      MethodDefinitionKind::Constructor => None,
      MethodDefinitionKind::Get | MethodDefinitionKind::Set => Some(DeclKind::Value),
      MethodDefinitionKind::Method => Some(DeclKind::Function),
    };
    let decl = kind
      .and_then(|kind| self.member_decl(&method.key, method.computed, method.span, &method.decorators, kind, method.r#static, private))
      .map(|mut decl| {
        let end = method.value.body.as_ref().map_or(method.span.end, |it| it.span.start);
        let mut skipped = parameter_decorators(&method.value.params);
        skipped.extend(method.decorators.iter().map(|it| it.span));
        decl.header = Some(self.header(&[(decl.span.start, end)], &skipped, method.key.span().start));
        decl
      });
    self.within(decl, Holder::Leaf, |walk| {
      for decorator in &method.decorators {
        walk.visit_decorator(decorator);
      }
      walk.key_code(&method.key, method.computed);
      walk.function_scope(&method.value, false);
    });
  }

  fn field(&mut self, field: &js::Field<'_, '_>) {
    let decl = self
      .member_decl(field.key, field.computed, field.span, field.decorators, DeclKind::Value, field.is_static, field.private)
      .map(|mut decl| {
        let end = field.value.map_or(field.span.end, |it| it.span().start);
        let skipped: Vec<Span> = field.decorators.iter().map(|it| it.span).collect();
        decl.header = Some(self.header(&[(decl.span.start, end)], &skipped, field.key.span().start));
        decl
      });
    self.within(decl, Holder::Leaf, |walk| {
      for decorator in field.decorators {
        walk.visit_decorator(decorator);
      }
      walk.key_code(field.key, field.computed);
      if let Some(annotation) = field.annotation {
        walk.visit_ts_type_annotation(annotation);
      }
      if let Some(value) = field.value {
        walk.visit_expression(value);
      }
    });
  }

  /// `[key: string]: T`: its name is none, its types are walked
  fn index_signature(&mut self, signature: &TSIndexSignature<'_>) {
    self.visit_ts_type_annotation(&signature.parameter.type_annotation);
    self.visit_ts_type_annotation(&signature.type_annotation);
  }

  fn interface(&mut self, interface: &TSInterfaceDeclaration<'_>, wrap: Wrap) {
    let decl = self.type_decl(interface.span, interface.id.span, DeclKind::Type, wrap, None);
    self.within(decl, Holder::Interface, |walk| {
      walk.bindings.push(false);
      if let Some(parameters) = &interface.type_parameters {
        walk.visit_ts_type_parameter_declaration(parameters);
      }
      for heritage in &interface.extends {
        walk.visit_ts_interface_heritage(heritage);
      }
      let declares = !walk.is_local();
      for signature in &interface.body.body {
        walk.signature(signature, declares);
      }
      walk.bindings.pop();
    });
  }

  /// A member of an interface or an object type; `declares` in an interface and in an object type of a type alias
  fn signature(&mut self, signature: &TSSignature<'_>, declares: bool) {
    match signature {
      TSSignature::TSPropertySignature(it) => {
        self.key_code(&it.key, it.computed);
        let decl = self.signature_decl(&it.key, it.computed, it.span, DeclKind::Value, declares);
        self.within(decl, Holder::Leaf, |walk| {
          if let Some(annotation) = &it.type_annotation {
            walk.visit_ts_type_annotation(annotation);
          }
        });
      }
      TSSignature::TSMethodSignature(it) => {
        self.key_code(&it.key, it.computed);
        let decl = self.signature_decl(&it.key, it.computed, it.span, DeclKind::Function, declares);
        self.within(decl, Holder::Leaf, |walk| {
          walk.bindings.push(true);
          js::walk_signature(walk, it.type_parameters.as_deref(), it.this_param.as_deref(), &it.params, it.return_type.as_deref());
          walk.bindings.pop();
        });
      }
      TSSignature::TSIndexSignature(it) => self.index_signature(it),
      TSSignature::TSCallSignatureDeclaration(it) => {
        self.bindings.push(false);
        walk::walk_ts_call_signature_declaration(self, it);
        self.bindings.pop();
      }
      TSSignature::TSConstructSignatureDeclaration(it) => {
        self.bindings.push(false);
        walk::walk_ts_construct_signature_declaration(self, it);
        self.bindings.pop();
      }
    }
  }

  fn signature_decl(&self, key: &PropertyKey<'_>, computed: bool, span: Span, kind: DeclKind, declares: bool) -> Option<Decl<'s>> {
    if !declares || self.is_local() {
      return None;
    }
    let (name, text) = self.key_name(key, computed)?;
    let header = self.header(&[(span.start, span.end)], &[], 0);
    Some(Decl {
      span,
      name,
      text,
      kind,
      reach: Reach::Public,
      member: MemberKind::Instance,
      header: Some(header),
      starts: [None, None],
      documented: true,
    })
  }

  fn type_alias(&mut self, alias: &TSTypeAliasDeclaration<'_>, wrap: Wrap) {
    let decl = self.type_decl(alias.span, alias.id.span, DeclKind::TypeAlias, wrap, None);
    self.within(decl, Holder::Alias, |walk| {
      walk.bindings.push(false);
      if let Some(parameters) = &alias.type_parameters {
        walk.visit_ts_type_parameter_declaration(parameters);
      }
      walk.visit_ts_type(&alias.type_annotation);
      walk.bindings.pop();
    });
  }

  fn enumeration(&mut self, enumeration: &TSEnumDeclaration<'_>, wrap: Wrap) {
    let decl = self.type_decl(enumeration.span, enumeration.id.span, DeclKind::Type, wrap, None);
    let declares = decl.is_some();
    self.within(decl, Holder::Enum, |walk| {
      for member in &enumeration.body.members {
        if declares
          && let TSEnumMemberName::Identifier(name) = &member.id
          && let Some(text) = walk.text(name.span)
        {
          walk.declare(Decl {
            span: member.span,
            name: name.span,
            text,
            kind: DeclKind::EnumEntry,
            reach: Reach::Public,
            member: MemberKind::Static,
            header: None,
            starts: [None, None],
            documented: true,
          });
        }
        if let Some(initializer) = &member.initializer {
          walk.visit_expression(initializer);
        }
      }
    });
  }

  /// `namespace a.b.c { … }`: one module named `c`, whose body is no scope
  fn namespace(&mut self, namespace: &TSNamespaceDeclaration<'_>, wrap: Wrap) {
    let mut innermost = namespace;
    while let TSNamespaceDeclarationBody::TSNamespaceDeclaration(inner) = &innermost.body {
      innermost = inner;
    }
    let decl = self.type_decl(namespace.span, innermost.id.span, DeclKind::Module, wrap, None);
    self.within(decl, Holder::Module, |walk| {
      if let TSNamespaceDeclarationBody::TSModuleBlock(block) = &innermost.body {
        for statement in &block.body {
          walk.visit_statement(statement);
        }
      }
    });
  }

  /// `declare module "m" { … }`: a module named by the content of its string when it reads as an identifier
  fn external_module(&mut self, module: &TSExternalModuleDeclaration<'_>, wrap: Wrap) {
    let content = Span::new(module.id.span.start + 1, module.id.span.end.saturating_sub(1).max(module.id.span.start + 1));
    let named = self
      .text(content)
      .is_some_and(|it| it.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$' || c == '#'));
    let decl = if named {
      self.type_decl(module.span, content, DeclKind::Module, wrap, None)
    } else {
      None
    };
    self.within(decl, Holder::Module, |walk| {
      if let Some(block) = &module.body {
        for statement in &block.body {
          walk.visit_statement(statement);
        }
      }
    });
  }

  /// The lines of an import statement
  fn import_statement(&mut self, span: Span) {
    self.out.import_lines.push(LineSpan {
      start: self.source.line(span.start),
      end: self.source.end_line(span),
    });
  }

  /// The content of a module string: `./m` of `"./m"`
  fn specifier(&self, string: &StringLiteral<'_>) -> String {
    let content = Span::new(string.span.start + 1, string.span.end.saturating_sub(1).max(string.span.start + 1));
    self.text(content).unwrap_or_default().to_string()
  }

  /// A `Binding` declaration of a name an import brings in, over the statement `statement`; a local import binds it
  fn binding(&mut self, statement: Span, name: Span, text: &'s str, reach: Reach) {
    if self.is_local() {
      self.bind(text);
      return;
    }
    let member = if self.container().is_some() { MemberKind::Static } else { MemberKind::TopLevel };
    self.declare(Decl {
      span: statement,
      name,
      text,
      kind: DeclKind::Binding,
      reach,
      member,
      header: None,
      starts: [None, None],
      documented: false,
    });
  }

  /// A default or namespace import: the module under a local name
  fn bound_import(&mut self, statement: Span, target: ImportTarget, alias: Span, specifier: String, exported: bool) {
    let Some(text) = self.text(alias) else { return };
    let alias_id = self.out.intern(text);
    self.out.add_import(Import {
      path: Vec::new(),
      target,
      alias: Some(alias_id),
      exported,
      specifier: Some(specifier),
      line: self.source.line(alias.start),
    });
    self.binding(statement, alias, text, if exported { Reach::Public } else { Reach::File });
  }

  /// `a` or `a as b` of an import or a re-export: the name imported, at `name`, under the local name at `alias`
  fn named_import(&mut self, statement: Span, name: Span, alias: Option<Span>, specifier: &str, exported: bool) {
    let Some(text) = self.text(name) else { return };
    let alias_text = alias.and_then(|it| self.text(it));
    let target = self.out.intern(text);
    let alias_id = alias_text.map(|it| self.out.intern(it));
    self.out.add_import(Import {
      path: Vec::new(),
      target: ImportTarget::Name(target),
      alias: alias_id,
      exported,
      specifier: Some(specifier.to_string()),
      line: self.source.line(name.start),
    });
    self.reference(text, name.start, Role::Import, None);
    let (bound, bound_text) = alias.zip(alias_text).unwrap_or((name, text));
    self.binding(statement, bound, bound_text, if exported { Reach::Public } else { Reach::File });
  }

  /// The span of the name a module export name writes: an identifier, or the content of a string
  fn export_name_span(name: &ModuleExportName<'_>) -> Span {
    match name {
      ModuleExportName::IdentifierName(it) => it.span,
      ModuleExportName::IdentifierReference(it) => it.span,
      ModuleExportName::StringLiteral(it) => Span::new(it.span.start + 1, it.span.end.saturating_sub(1).max(it.span.start + 1)),
    }
  }

  fn import(&mut self, import: &ImportDeclaration<'_>) {
    self.import_statement(import.span);
    let Some(specifiers) = &import.specifiers else { return };
    let specifier = self.specifier(&import.source);
    for item in specifiers {
      match item {
        ImportDeclarationSpecifier::ImportDefaultSpecifier(it) => self.bound_import(import.span, ImportTarget::Default, it.local.span, specifier.clone(), false),
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(it) => self.bound_import(import.span, ImportTarget::Namespace, it.local.span, specifier.clone(), false),
        ImportDeclarationSpecifier::ImportSpecifier(it) => {
          let name = Self::export_name_span(&it.imported);
          let alias = (it.local.span != it.imported.span()).then_some(it.local.span);
          self.named_import(import.span, name, alias, &specifier, false);
        }
      }
    }
  }

  /// `import x = require("m")` is a namespace import; `import x = a.b` names `a.b`
  fn import_equals(&mut self, import: &TSImportEqualsDeclaration<'_>) {
    match &import.module_reference {
      TSModuleReference::ExternalModuleReference(it) => {
        self.import_statement(import.span);
        let specifier = self.specifier(&it.expression);
        self.bound_import(import.span, ImportTarget::Namespace, import.id.span, specifier, false);
      }
      TSModuleReference::IdentifierReference(it) => self.reference_at(it.span, Role::Value, None),
      TSModuleReference::QualifiedName(it) => {
        let qualifier = self.type_name_path(&it.left);
        let qualifier = self.qualifier(qualifier);
        self.reference_at(it.right.span, Role::Value, Some(qualifier));
        self.type_name(&it.left, Role::Value);
      }
    }
  }

  /// `export { a, b as c }` publishes local names: a reference to each, an alias a public binding
  fn export_local(&mut self, export: &ExportNamedDeclaration<'_>) {
    for item in &export.specifiers {
      let local = Self::export_name_span(&item.local);
      let exported = Self::export_name_span(&item.exported);
      self.reference_at(local, Role::Value, None);
      if exported == local {
        if let Some(text) = self.text(local) {
          self.out.export_name(text);
        }
      } else if let Some(text) = self.text(exported) {
        self.binding(export.span, exported, text, Reach::Public);
      }
    }
  }

  /// `export { a, b as c } from "m"`: exported imports and public bindings
  fn export_from(&mut self, export: &ExportFromDeclaration<'_>) {
    let specifier = self.specifier(&export.source);
    for item in &export.specifiers {
      let local = Self::export_name_span(&item.local);
      let exported = Self::export_name_span(&item.exported);
      self.named_import(export.span, local, (exported != local).then_some(exported), &specifier, true);
    }
  }

  fn export_all(&mut self, export: &ExportAllDeclaration<'_>) {
    let specifier = self.specifier(&export.source);
    match &export.exported {
      Some(name) => self.bound_import(export.span, ImportTarget::Namespace, Self::export_name_span(name), specifier, true),
      None => self.out.add_import(Import {
        path: Vec::new(),
        target: ImportTarget::All,
        alias: None,
        exported: true,
        specifier: Some(specifier),
        line: self.source.line(export.span.start),
      }),
    }
  }

  fn export_default(&mut self, export: &ExportDefaultDeclaration<'_>) {
    let wrap = Wrap {
      exported: true,
      start: Some(export.span.start),
    };
    match &export.declaration {
      ExportDefaultDeclarationKind::FunctionDeclaration(it) => self.function_declaration(it, wrap),
      ExportDefaultDeclarationKind::ClassDeclaration(it) => self.class(it, wrap),
      ExportDefaultDeclarationKind::TSInterfaceDeclaration(it) => self.interface(it, wrap),
      kind => {
        if let Some(expression) = kind.as_expression() {
          self.visit_expression(expression);
        }
      }
    }
  }

  /// A JSX element's name: a capitalised one a type, a member's last name a type qualified by the others
  fn jsx_name(&mut self, name: &JSXElementName<'_>) {
    match name {
      JSXElementName::IdentifierReference(it) => self.jsx_tag(it.span),
      JSXElementName::Identifier(it) => self.jsx_tag(it.span),
      JSXElementName::MemberExpression(it) => self.jsx_member(it, Role::Type),
      JSXElementName::NamespacedName(_) | JSXElementName::ThisExpression(_) => {}
    }
  }

  /// A JSX tag of one name: a component when capitalised, else an HTML tag, which names nothing
  fn jsx_tag(&mut self, name: Span) {
    if self.text(name).is_some_and(|text| text.starts_with(|c: char| c.is_ascii_uppercase())) {
      self.reference_at(name, Role::Type, None);
    }
  }

  fn jsx_member(&mut self, member: &JSXMemberExpression<'_>, role: Role) {
    let qualifier = self.jsx_path(&member.object);
    let qualifier = self.qualifier(qualifier);
    self.reference_at(member.property.span, role, Some(qualifier));
    match &member.object {
      JSXMemberExpressionObject::IdentifierReference(it) => self.reference_at(it.span, Role::Value, None),
      JSXMemberExpressionObject::MemberExpression(it) => self.jsx_member(it, Role::Value),
      JSXMemberExpressionObject::ThisExpression(_) => {}
    }
  }

  fn jsx_path(&self, object: &JSXMemberExpressionObject<'_>) -> Option<Vec<&'s str>> {
    match object {
      JSXMemberExpressionObject::IdentifierReference(it) => Some(vec![self.text(it.span)?]),
      JSXMemberExpressionObject::ThisExpression(_) => Some(vec!["this"]),
      JSXMemberExpressionObject::MemberExpression(it) => {
        let mut path = self.jsx_path(&it.object)?;
        path.push(self.text(it.property.span)?);
        Some(path)
      }
    }
  }
}

impl<'a> Visit<'a> for Walk<'_, '_> {
  fn visit_program(&mut self, program: &Program<'a>) {
    for statement in &program.body {
      self.visit_statement(statement);
    }
  }

  fn visit_declaration(&mut self, declaration: &Declaration<'a>) {
    self.declaration(declaration, Wrap::default());
  }

  fn visit_module_declaration(&mut self, declaration: &ModuleDeclaration<'a>) {
    match declaration {
      ModuleDeclaration::ImportDeclaration(it) => self.import(it),
      ModuleDeclaration::ExportDeclaration(it) => self.declaration(
        &it.declaration,
        Wrap {
          exported: true,
          start: Some(it.span.start),
        },
      ),
      ModuleDeclaration::ExportNamedDeclaration(it) => self.export_local(it),
      ModuleDeclaration::ExportFromDeclaration(it) => self.export_from(it),
      ModuleDeclaration::ExportDefaultDeclaration(it) => self.export_default(it),
      ModuleDeclaration::ExportAllDeclaration(it) => self.export_all(it),
      ModuleDeclaration::TSExportAssignment(it) => self.visit_expression(&it.expression),
      ModuleDeclaration::TSNamespaceExportDeclaration(_) => {}
    }
  }

  fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
    self.bindings.push(true);
    self.hoist_block(&block.body);
    for statement in &block.body {
      self.visit_statement(statement);
    }
    self.bindings.pop();
  }

  fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
    self.bindings.push(true);
    walk::walk_for_statement(self, it);
    self.bindings.pop();
  }

  fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
    self.for_each(&it.left, &it.right, &it.body);
  }

  fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
    self.for_each(&it.left, &it.right, &it.body);
  }

  fn visit_switch_statement(&mut self, it: &SwitchStatement<'a>) {
    self.visit_expression(&it.discriminant);
    self.bindings.push(true);
    for case in &it.cases {
      self.visit_switch_case(case);
    }
    self.bindings.pop();
  }

  fn visit_catch_clause(&mut self, it: &CatchClause<'a>) {
    self.bindings.push(true);
    if let Some(parameter) = &it.param {
      self.bind_pattern(&parameter.pattern);
      if let Some(annotation) = &parameter.type_annotation {
        self.visit_ts_type_annotation(annotation);
      }
    }
    self.visit_block_statement(&it.body);
    self.bindings.pop();
  }

  fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
    self.variables(it, Wrap::default());
  }

  fn visit_function(&mut self, it: &Function<'a>, _: ScopeFlags) {
    match it.r#type {
      FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction => self.function_declaration(it, Wrap::default()),
      FunctionType::FunctionExpression | FunctionType::TSEmptyBodyFunctionExpression => self.function_scope(it, true),
    }
  }

  fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
    self.bindings.push(true);
    let statements = match &it.body {
      ArrowFunctionBody::FunctionBody(body) => Some(&body.statements),
      _ => None,
    };
    if let Some(statements) = statements {
      self.hoist_vars(statements);
      self.hoist_block(statements);
    }
    if let Some(parameters) = &it.type_parameters {
      self.visit_ts_type_parameter_declaration(parameters);
    }
    self.visit_formal_parameters(&it.params);
    if let Some(annotation) = &it.return_type {
      self.visit_ts_type_annotation(annotation);
    }
    match &it.body {
      ArrowFunctionBody::FunctionBody(body) => {
        for statement in &body.statements {
          self.visit_statement(statement);
        }
      }
      body => {
        if let Some(expression) = body.as_expression() {
          self.visit_expression(expression);
        }
      }
    }
    self.bindings.pop();
  }

  fn visit_class(&mut self, it: &Class<'a>) {
    self.class(it, Wrap::default());
  }

  /// A parameter; `constructor(private readonly x: X)` also declares the field `x` of its class
  fn visit_formal_parameter(&mut self, it: &FormalParameter<'a>) {
    for decorator in &it.decorators {
      self.visit_decorator(decorator);
    }
    let property = it.accessibility.is_some() || it.readonly;
    match &it.pattern {
      BindingPattern::BindingIdentifier(name) if property && self.holder() == Some(Holder::Class) => {
        if let Some(text) = self.text(name.span) {
          let start = it
            .decorators
            .last()
            .map_or(it.span.start, |decorator| first_token_after(self.tokens, decorator.span.end).unwrap_or(it.span.start));
          let end = it.initializer.as_ref().map_or(it.span.end, |value| value.span().start);
          let header = self.header(&[(start, end)], &[], name.span.start);
          let reach = if it.accessibility == Some(TSAccessibility::Private) {
            Reach::Container
          } else {
            Reach::Public
          };
          self.declare(Decl {
            span: Span::new(start, it.span.end),
            name: name.span,
            text,
            kind: DeclKind::Value,
            reach,
            member: MemberKind::Instance,
            header: Some(header),
            starts: [None, None],
            documented: true,
          });
          self.bind(text);
        }
      }
      pattern => self.bind_pattern(pattern),
    }
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
    if let Some(initializer) = &it.initializer {
      self.visit_expression(initializer);
    }
  }

  fn visit_formal_parameter_rest(&mut self, it: &FormalParameterRest<'a>) {
    for decorator in &it.decorators {
      self.visit_decorator(decorator);
    }
    self.bind_pattern(&it.rest.argument);
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
  }

  fn visit_binding_pattern(&mut self, it: &BindingPattern<'a>) {
    // a pattern reached by the default walk (a `for … of` head, a local declarator) binds its names
    self.bind_pattern(it);
  }

  fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
    self.reference_at(it.span, Role::Value, None);
  }

  fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
    self.member(&it.object, it.property.span, Role::Value);
  }

  fn visit_private_field_expression(&mut self, it: &PrivateFieldExpression<'a>) {
    self.member(&it.object, it.field.span, Role::Value);
  }

  fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
    self.call(&it.callee, it.type_arguments.as_deref(), &it.arguments);
  }

  fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
    self.call(&it.callee, it.type_arguments.as_deref(), &it.arguments);
  }

  fn visit_tagged_template_expression(&mut self, it: &TaggedTemplateExpression<'a>) {
    self.callee(&it.tag, Role::Call);
    if let Some(arguments) = &it.type_arguments {
      self.visit_ts_type_parameter_instantiation(arguments);
    }
    self.visit_template_literal(&it.quasi);
  }

  /// An object literal's member declares nothing; a method's value is a function
  fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
    if it.method || it.kind != PropertyKind::Init {
      self.key_code(&it.key, it.computed);
    } else if it.computed {
      self.visit_property_key(&it.key);
    }
    self.visit_expression(&it.value);
  }

  fn visit_ts_type_literal(&mut self, it: &TSTypeLiteral<'a>) {
    let declares = self.holder() == Some(Holder::Alias) && !self.is_local();
    for member in &it.members {
      self.signature(member, declares);
    }
  }

  fn visit_ts_interface_body(&mut self, it: &TSInterfaceBody<'a>) {
    for member in &it.body {
      self.signature(member, false);
    }
  }

  fn visit_ts_type_name(&mut self, it: &TSTypeName<'a>) {
    self.type_name(it, Role::Type);
  }

  fn visit_ts_type_query(&mut self, it: &TSTypeQuery<'a>) {
    match &it.expr_name {
      TSTypeQueryExprName::TSImportType(import) => self.import_type(import, Role::Value),
      name => {
        if let Some(name) = name.as_ts_type_name() {
          self.type_name(name, Role::Value);
        }
      }
    }
    if let Some(arguments) = &it.type_arguments {
      self.visit_ts_type_parameter_instantiation(arguments);
    }
  }

  fn visit_ts_import_type(&mut self, it: &TSImportType<'a>) {
    self.import_type(it, Role::Type);
  }

  fn visit_ts_module_reference(&mut self, it: &TSModuleReference<'a>) {
    if let TSModuleReference::IdentifierReference(name) = it {
      self.reference_at(name.span, Role::Value, None);
    }
  }

  fn visit_ts_type_parameter(&mut self, it: &TSTypeParameter<'a>) {
    self.bind_identifier(&it.name);
    js::walk_type_parameter_bounds(self, it);
  }

  fn visit_ts_mapped_type(&mut self, it: &TSMappedType<'a>) {
    self.bind_identifier(&it.key);
    self.visit_ts_type(&it.constraint);
    if let Some(name) = &it.name_type {
      self.visit_ts_type(name);
    }
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type(annotation);
    }
  }

  /// `A extends infer B ? B : C`: what `infer` declares is seen in the whole conditional type
  fn visit_ts_conditional_type(&mut self, it: &TSConditionalType<'a>) {
    self.bindings.push(false);
    self.visit_ts_type(&it.check_type);
    let mut inferred = Inferred { names: Vec::new() };
    inferred.visit_ts_type(&it.extends_type);
    for span in inferred.names {
      if let Some(text) = self.text(span) {
        self.bind(text);
      }
    }
    self.visit_ts_type(&it.extends_type);
    self.visit_ts_type(&it.true_type);
    self.visit_ts_type(&it.false_type);
    self.bindings.pop();
  }

  fn visit_ts_function_type(&mut self, it: &TSFunctionType<'a>) {
    self.bindings.push(false);
    walk::walk_ts_function_type(self, it);
    self.bindings.pop();
  }

  fn visit_ts_constructor_type(&mut self, it: &TSConstructorType<'a>) {
    self.bindings.push(false);
    walk::walk_ts_constructor_type(self, it);
    self.bindings.pop();
  }

  fn visit_ts_index_signature(&mut self, it: &TSIndexSignature<'a>) {
    self.index_signature(it);
  }

  fn visit_ts_type_predicate(&mut self, it: &TSTypePredicate<'a>) {
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
  }

  fn visit_jsx_opening_element(&mut self, it: &JSXOpeningElement<'a>) {
    self.jsx_name(&it.name);
    if let Some(arguments) = &it.type_arguments {
      self.visit_ts_type_parameter_instantiation(arguments);
    }
    for attribute in &it.attributes {
      self.visit_jsx_attribute_item(attribute);
    }
  }

  fn visit_jsx_closing_element(&mut self, it: &JSXClosingElement<'a>) {
    self.jsx_name(&it.name);
  }

  fn visit_labeled_statement(&mut self, it: &LabeledStatement<'a>) {
    self.visit_statement(&it.body);
  }
}

/// The names a binding pattern binds, in source order, without its keys and default values
fn pattern_names<'p, 'a>(pattern: &'p BindingPattern<'a>) -> Vec<&'p BindingIdentifier<'a>> {
  let mut names = Vec::new();
  let mut stack = vec![pattern];
  while let Some(pattern) = stack.pop() {
    match pattern {
      BindingPattern::BindingIdentifier(it) => names.push(&**it),
      BindingPattern::AssignmentPattern(it) => stack.push(&it.left),
      BindingPattern::ObjectPattern(it) => {
        let start = stack.len();
        stack.extend(it.properties.iter().map(|property| &property.value));
        stack.extend(it.rest.iter().map(|rest| &rest.argument));
        stack[start..].reverse();
      }
      BindingPattern::ArrayPattern(it) => {
        let start = stack.len();
        stack.extend(it.elements.iter().flatten());
        stack.extend(it.rest.iter().map(|rest| &rest.argument));
        stack[start..].reverse();
      }
    }
  }
  names
}

/// The decorators of a function's parameters, which its header leaves out
fn parameter_decorators(parameters: &FormalParameters<'_>) -> Vec<Span> {
  parameters
    .items
    .iter()
    .flat_map(|it| it.decorators.iter().map(|decorator| decorator.span))
    .chain(parameters.rest.iter().flat_map(|it| it.decorators.iter().map(|decorator| decorator.span)))
    .collect()
}

/// The start of the first token at or after `offset`
fn first_token_after(tokens: &[Token], offset: u32) -> Option<u32> {
  tokens_in(tokens, offset, u32::MAX).first().map(Token::start)
}

/// `const f = () => …` is a function, `const MAX_SIZE = 10` a constant
fn declarator_kind(walk: &Walk<'_, '_>, kind: VariableDeclarationKind, declarator: &VariableDeclarator<'_>) -> DeclKind {
  if matches!(declarator.init, Some(Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_))) {
    return DeclKind::Function;
  }
  let constant = kind == VariableDeclarationKind::Const
    && matches!(&declarator.id, BindingPattern::BindingIdentifier(it) if walk.text(it.span).is_some_and(|name| {
      name.chars().any(|c| c.is_ascii_uppercase()) && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }));
  if constant { DeclKind::Constant } else { DeclKind::Value }
}

/// The names `infer` declares in the `extends` type of a conditional type, not in the conditional types inside it
struct Inferred {
  names: Vec<Span>,
}

impl<'a> Visit<'a> for Inferred {
  fn visit_ts_infer_type(&mut self, it: &TSInferType<'a>) {
    self.names.push(it.type_parameter.name.span);
  }

  fn visit_ts_conditional_type(&mut self, _: &TSConditionalType<'a>) {}
}

/// The `var` names of a function body, not those of the functions and classes in it
struct Vars {
  names: Vec<Span>,
}

impl<'a> Visit<'a> for Vars {
  fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
    if it.kind == VariableDeclarationKind::Var {
      for declarator in &it.declarations {
        self.names.extend(pattern_names(&declarator.id).iter().map(|name| name.span));
      }
    }
    walk::walk_variable_declaration(self, it);
  }

  fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}

  fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}

  fn visit_class(&mut self, _: &Class<'a>) {}

  fn visit_expression(&mut self, _: &Expression<'a>) {}

  fn visit_ts_type(&mut self, _: &TSType<'a>) {}
}
