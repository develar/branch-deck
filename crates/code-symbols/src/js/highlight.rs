//! The highlighting of a TypeScript, TSX or JavaScript file from oxc's tokens and tree, after the JavaScript plugin of
//! IntelliJ: what its lexer colours, and what its semantic visitor colours by the declaration of a name, as far as the
//! scopes of the file show it (`JSSemanticHighlightingVisitor`).
//!
//! - **Tokens** are the parse's, in the dialect of the file: a keyword where the parser read one (a contextual word,
//!   `type`, `get`, `as`, `of`, is a name where it names something), `true`, `false`, `null`, `this`, `super` and
//!   `undefined`, a type keyword (`string`, `number`); a number; a string with its escapes, a template literal's text
//!   with its escapes but its `${` and `}`, whose code is code; a regular expression; a comment, a doc comment with its
//!   tags (`program.comments`). A region a recovery of the parse blanked has no tokens and stays plain.
//! - **A variable** the file declares and does not export is local, an exported or imported one global, a name the
//!   file does not declare (`console`, `window`) global too; a parameter has a colour of its own. A function the file
//!   declares is local, an exported or imported one global, as a called name it does not declare (`setTimeout`); a
//!   method is a method, also at a call (`.f(`). An imported name is a function when the file calls it, a class when
//!   it is capitalised, else a variable. A capitalised name the file does not declare (`Math`, `Promise`) stays plain.
//! - **Members**: a field and a property of an interface or an object type (italic when static), an enum member
//!   (static), a member after its object (a method at a call, static after a type name or as a constant's name), a key
//!   of an object literal (a method when its value is a function); a type parameter and its uses; a label.
//! - **A decorator** in its colour, its arguments code; **JSX**: an HTML tag, a component (each part of `Menu.Item`),
//!   an attribute, the brackets of an element.
//!
//! One walk of the tree (`Visit`) marks the names and the brackets, by the start of their token, inside
//! `js::with_program`; one pass over the tokens and the comments in the order of the text paints them.

use crate::highlight::lex::Lex;
use crate::highlight::lines::{Builder, LineRuns};
use crate::highlight::paint::{Paint, comment_paint, paint_leaf};
use crate::highlight::scopes::Scopes;
use crate::highlight::{FIELD, ITALIC, Role, STATIC_FIELD, is_constant_name, is_type_name};
use crate::js;
use crate::js::field_of;
use js_syntax::Dialect;
// jscpd:ignore-start: the names of oxc's tree and visitor each walk reads
use js_syntax::oxc_ast::ast::{
  ArrowFunctionBody, ArrowFunctionExpression, BindingIdentifier, BindingPattern, BlockStatement, CallExpression, CatchClause, Class, ClassElement, Decorator, ExportAllDeclaration,
  ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportFromDeclaration, ExportNamedDeclaration, ExportSpecifier, Expression, ForInStatement, ForOfStatement, ForStatement,
  ForStatementLeft, FormalParameter, FormalParameterRest, Function, FunctionType, IdentifierName, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier,
  JSXAttributeName, JSXClosingElement, JSXElementName, JSXFragment, JSXMemberExpression, JSXMemberExpressionObject, JSXOpeningElement, LabelIdentifier, MethodDefinition,
  ModuleDeclaration, ModuleExportName, ObjectProperty, PrivateFieldExpression, Program, PropertyKey, PropertyKind, Statement, StaticMemberExpression, TSCallSignatureDeclaration,
  TSConstructSignatureDeclaration, TSConstructorType, TSEnumMember, TSEnumMemberName, TSFunctionType, TSImportEqualsDeclaration, TSMappedType, TSMethodSignature,
  TSPropertySignature, TSTypeName, TSTypeParameter, TSTypePredicate, TSTypePredicateName, TSTypeQuery, TaggedTemplateExpression, VariableDeclaration,
};
use js_syntax::oxc_ast_visit::{Visit, walk};
use js_syntax::oxc_parser::{Kind, Token};
use js_syntax::oxc_span::{GetSpan, Span};
use js_syntax::oxc_syntax::scope::ScopeFlags;
use std::collections::{HashMap, HashSet};
// jscpd:ignore-end

const LOCAL_VARIABLE: u32 = Role::LocalVariable.style();
const GLOBAL_VARIABLE: u32 = Role::GlobalVariable.style();
const PARAMETER: u32 = Role::Parameter.style();
const LOCAL_FUNCTION: u32 = Role::FunctionDeclaration.style();
const GLOBAL_FUNCTION: u32 = Role::GlobalFunction.style();
const METHOD: u32 = Role::Method.style();
/// A static method: `DEFAULT_STATIC_METHOD`, the colour of a function declaration in italic
const STATIC_METHOD: u32 = Role::FunctionDeclaration.style() | ITALIC;
/// A class, an interface, an enum: plain
const CLASS: u32 = Role::Foreground.style();
const KEYWORD: u32 = Role::Keyword.style();
const STRING: u32 = Role::String.style();

/// The highlighting of `text` read as `dialect`; `None` when no tree came of it: it nests deeper than the parse reads, or
/// no recovery saved it, and the view colours it otherwise
pub(crate) fn highlight(dialect: Dialect, text: &str) -> Option<LineRuns> {
  code_tokens::js::with_program(dialect, text.as_bytes(), |parsed| {
    let program = parsed.program()?;
    let mut marks = Marks {
      text,
      styles: HashMap::new(),
      regions: Vec::new(),
      scopes: Scopes::default(),
      called: called_names(program, text),
      exported: false,
      next_scope: 0,
    };
    marks.visit_program(program);
    let mut out = LineRuns::builder(text);
    paint(&mut out, text, &parsed.tokens, program, &marks);
    Some(out.finish())
  })
  .flatten()
}

/// Paints the tokens and the comments in the order of the text: a name by its mark, else a token by its kind
fn paint(out: &mut Builder<'_>, text: &str, tokens: &[Token], program: &Program<'_>, marks: &Marks<'_>) {
  let mut comments = program.comments.iter().map(|it| it.span).peekable();
  let hashbang = program.hashbang.as_ref().map(|it| it.span);
  if let Some(span) = hashbang {
    paint_leaf(out, span.start as usize, slice(text, span), Paint::Leaf(Role::Comment.style()), 0);
  }
  // the end of the last token: the text between it and the next is in a decorator only when both are
  let mut previous = 0;
  for token in tokens {
    while let Some(comment) = comments.next_if(|it| it.start < token.start()) {
      paint_leaf(out, comment.start as usize, slice(text, comment), comment_paint(slice(text, comment)), 0);
      previous = comment.end;
    }
    let span = token.span();
    let around = marks.region(span.start);
    out.push(span.start as usize, if marks.region(previous) == around { around } else { 0 });
    previous = span.end;
    let style = marks.styles.get(&span.start).copied();
    let paint = match (style, token.kind()) {
      (Some(style), _) => Paint::Leaf(style),
      // the name of a decorator is the decorator's, keywords of its path included (`@module.Getter`)
      (None, _) if around != 0 => Paint::Leaf(around),
      (None, Kind::Str) => Paint::Lexed(STRING, Lex::JsEscapes),
      (None, Kind::NoSubstitutionTemplate | Kind::TemplateHead | Kind::TemplateMiddle | Kind::TemplateTail) => {
        template(out, span.start as usize, slice(text, span), token.kind(), around);
        continue;
      }
      (None, Kind::RegExp) => Paint::Leaf(Role::Regexp.style()),
      (None, kind) if kind.is_number() => Paint::Leaf(Role::Number.style()),
      (None, kind) if kind.is_any_keyword() || matches!(kind, Kind::True | Kind::False | Kind::Null | Kind::This | Kind::Super) => Paint::Leaf(KEYWORD),
      (None, _) => Paint::Inherit,
    };
    paint_leaf(out, span.start as usize, slice(text, span), paint, around);
  }
  for comment in comments {
    paint_leaf(out, comment.start as usize, slice(text, comment), comment_paint(slice(text, comment)), 0);
  }
}

/// A piece of a template literal: its text a string with its escapes, its `${` and `}` code around it
fn template(out: &mut Builder<'_>, start: usize, text: &str, kind: Kind, around: u32) {
  let opens = usize::from(matches!(kind, Kind::TemplateMiddle | Kind::TemplateTail));
  let closes = if matches!(kind, Kind::TemplateHead | Kind::TemplateMiddle) { 2 } else { 0 };
  let inner = opens..text.len().saturating_sub(closes).max(opens);
  out.push(start, around);
  out.push(start + inner.start, around);
  paint_leaf(out, start + inner.start, &text[inner], Paint::Lexed(STRING, Lex::JsEscapes), around);
  out.push(start + text.len(), around);
}

fn slice(text: &str, span: Span) -> &str {
  text.get(span.start as usize..span.end as usize).unwrap_or_default()
}

/// The names the file calls (`f(…)`), for the colour of an import
fn called_names(program: &Program<'_>, text: &str) -> HashSet<String> {
  struct Called<'t> {
    text: &'t str,
    names: HashSet<String>,
  }
  impl<'a> Visit<'a> for Called<'_> {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
      if let Expression::Identifier(callee) = &it.callee {
        self.names.insert(slice(self.text, callee.span).to_string());
      }
      walk::walk_call_expression(self, it);
    }
  }
  let mut called = Called { text, names: HashSet::new() };
  called.visit_program(program);
  called.names
}

/// The styles of the names of the tree by the start of their token, and the regions a decorator colours
struct Marks<'t> {
  text: &'t str,
  styles: HashMap<u32, u32>,
  /// The decorators, `@` to the arguments of their call, in their colour
  regions: Vec<Span>,
  scopes: Scopes,
  /// The names the file calls
  called: HashSet<String>,
  /// The declaration being walked is exported
  exported: bool,
  next_scope: usize,
}

impl Marks<'_> {
  /// The style around a token at `start`: a decorator's
  fn region(&self, start: u32) -> u32 {
    if self.regions.iter().any(|it| it.start <= start && start < it.end) {
      Role::Annotation.style()
    } else {
      0
    }
  }

  fn mark(&mut self, span: Span, style: u32) {
    self.styles.insert(span.start, style);
  }

  /// A name no rule colours: plain, never a keyword, as a contextual word that names something (`{ as: x }`, `z.infer`)
  fn plain(&mut self, span: Span) {
    self.styles.entry(span.start).or_insert(0);
  }

  fn text_of(&self, span: Span) -> &str {
    slice(self.text, span)
  }

  fn push(&mut self) -> usize {
    self.next_scope += 1;
    self.scopes.push_local(self.next_scope);
    self.next_scope
  }

  fn pop(&mut self, scope: usize) {
    self.scopes.pop(scope);
  }

  /// A name by its declaration in the scopes; one the file does not declare global, a capitalised one a class
  fn name(&mut self, span: Span, callee: bool) {
    let name = self.text_of(span);
    let style = if name == "undefined" {
      KEYWORD
    } else if let Some(style) = self.scopes.local(name) {
      style
    } else if is_type_name(name) {
      CLASS
    } else if callee {
      GLOBAL_FUNCTION
    } else {
      GLOBAL_VARIABLE
    };
    self.mark(span, style);
  }

  /// Binds the names of a pattern in `style` (a variable, a parameter) and colours them so; its default values are code
  fn bind_pattern(&mut self, pattern: &BindingPattern<'_>, style: u32) {
    match pattern {
      BindingPattern::BindingIdentifier(it) => {
        let name = self.text_of(it.span).to_string();
        self.scopes.bind_styled(&name, style);
        self.mark(it.span, style);
      }
      BindingPattern::AssignmentPattern(it) => {
        self.bind_pattern(&it.left, style);
        self.visit_expression(&it.right);
      }
      BindingPattern::ObjectPattern(it) => {
        for property in &it.properties {
          if property.computed {
            self.visit_property_key(&property.key);
          } else if let PropertyKey::StaticIdentifier(key) = &property.key {
            self.plain(key.span);
          }
          self.bind_pattern(&property.value, style);
        }
        if let Some(rest) = &it.rest {
          self.bind_pattern(&rest.argument, style);
        }
      }
      BindingPattern::ArrayPattern(it) => {
        for element in it.elements.iter().flatten() {
          self.bind_pattern(element, style);
        }
        if let Some(rest) = &it.rest {
          self.bind_pattern(&rest.argument, style);
        }
      }
    }
  }

  /// The declarations of a file or a block, visible in all of it: its functions and classes, and of a file its variables
  /// and imports too, which a function declared above them uses
  fn bind_declarations(&mut self, statements: &[Statement<'_>], file: bool) {
    for statement in statements {
      let (declaration, exported) = match statement {
        Statement::ExportNamedDeclaration(_) | Statement::ExportFromDeclaration(_) | Statement::ExportAllDeclaration(_) => continue,
        Statement::ExportDeclaration(it) => (Some(&it.declaration), true),
        Statement::ExportDefaultDeclaration(it) => {
          match &it.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => self.bind_function(function, true),
            ExportDefaultDeclarationKind::ClassDeclaration(class) => self.bind_class(class),
            _ => {}
          }
          continue;
        }
        Statement::ImportDeclaration(it) if file => {
          self.bind_imports(it);
          continue;
        }
        Statement::TSImportEqualsDeclaration(it) if file => {
          let name = self.text_of(it.id.span).to_string();
          let style = self.import_style(&name);
          self.scopes.bind_styled(&name, style);
          continue;
        }
        statement => (statement.as_declaration(), false),
      };
      match declaration {
        Some(js_syntax::oxc_ast::ast::Declaration::FunctionDeclaration(function)) => self.bind_function(function, exported),
        Some(js_syntax::oxc_ast::ast::Declaration::ClassDeclaration(class)) => self.bind_class(class),
        Some(js_syntax::oxc_ast::ast::Declaration::TSEnumDeclaration(enumeration)) => {
          let name = self.text_of(enumeration.id.span).to_string();
          self.scopes.bind_styled(&name, CLASS);
        }
        Some(js_syntax::oxc_ast::ast::Declaration::VariableDeclaration(variables)) if file => {
          for declarator in &variables.declarations {
            self.bind_names(&declarator.id, if exported { GLOBAL_VARIABLE } else { LOCAL_VARIABLE });
          }
        }
        _ => {}
      }
    }
  }

  fn bind_function(&mut self, function: &Function<'_>, exported: bool) {
    if function.r#type == FunctionType::FunctionDeclaration
      && let Some(id) = &function.id
    {
      let name = self.text_of(id.span).to_string();
      self.scopes.bind_styled(&name, if exported { GLOBAL_FUNCTION } else { LOCAL_FUNCTION });
    }
  }

  fn bind_class(&mut self, class: &Class<'_>) {
    if let Some(id) = &class.id {
      let name = self.text_of(id.span).to_string();
      self.scopes.bind_styled(&name, CLASS);
    }
  }

  /// Binds the names of a pattern without colouring them: the declarations a scope sees before the walk reaches them
  fn bind_names(&mut self, pattern: &BindingPattern<'_>, style: u32) {
    let names: Vec<String> = pattern_spans(pattern).into_iter().map(|it| self.text_of(it).to_string()).collect();
    for name in names {
      self.scopes.bind_styled(&name, style);
    }
  }

  /// The names an import binds: a function when the file calls it, a class when it is capitalised, else a variable
  fn bind_imports(&mut self, import: &ImportDeclaration<'_>) {
    for specifier in import.specifiers.iter().flatten() {
      let local = match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(it) => it.local.span,
        ImportDeclarationSpecifier::ImportDefaultSpecifier(it) => it.local.span,
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(it) => it.local.span,
      };
      let name = self.text_of(local).to_string();
      let style = self.import_style(&name);
      self.scopes.bind_styled(&name, style);
    }
  }

  /// The style of a name an import binds: a function when the file calls it, a class when it is capitalised, else a
  /// variable
  fn import_style(&self, name: &str) -> u32 {
    if self.called.contains(name) {
      GLOBAL_FUNCTION
    } else if is_type_name(name) {
      CLASS
    } else {
      GLOBAL_VARIABLE
    }
  }

  /// A function: a scope of its parameters, a scope of its body that sees its functions and classes
  fn function_scope(&mut self, function: &Function<'_>) {
    let scope = self.push();
    js::walk_signature(
      self,
      function.type_parameters.as_deref(),
      function.this_param.as_deref(),
      &function.params,
      function.return_type.as_deref(),
    );
    if let Some(body) = &function.body {
      self.body(&body.statements);
    }
    self.pop(scope);
  }

  /// The body of a function: a scope that sees its functions and classes
  fn body(&mut self, statements: &[Statement<'_>]) {
    let scope = self.push();
    self.bind_declarations(statements, false);
    for statement in statements {
      self.visit_statement(statement);
    }
    self.pop(scope);
  }

  /// The name of a member after its object: a method at a call, static after a type name or as a constant's name
  fn member(&mut self, object: &Expression<'_>, name: Span, callee: bool) {
    self.visit_expression(object);
    let style = if callee {
      METHOD
    } else {
      let on_type = matches!(object, Expression::Identifier(it) if is_type_name(self.text_of(it.span)));
      if on_type || is_constant_name(self.text_of(name)) { STATIC_FIELD } else { FIELD }
    };
    self.mark(name, style);
  }

  /// The callee of a call: a name by its declaration, a member a method
  fn callee(&mut self, callee: &Expression<'_>) {
    match callee {
      Expression::Identifier(it) => self.name(it.span, true),
      Expression::StaticMemberExpression(it) => self.member(&it.object, it.property.span, true),
      Expression::PrivateFieldExpression(it) => self.member(&it.object, it.field.span, true),
      callee => self.visit_expression(callee),
    }
  }

  /// The name of a key of a member in `style`; a computed key is code
  fn key(&mut self, key: &PropertyKey<'_>, computed: bool, style: u32) {
    match key {
      PropertyKey::StaticIdentifier(it) if !computed => self.mark(it.span, style),
      PropertyKey::PrivateIdentifier(it) => self.mark(it.span, style),
      // a quoted name of a member is the member's (`"data-testid"?: string`)
      PropertyKey::StringLiteral(it) if !computed => self.mark(it.span, style),
      key if computed => self.visit_property_key(key),
      _ => {}
    }
  }

  /// A JSX tag: an HTML tag (`div`), else a component (`Button`, each part of `Menu.Item`)
  fn tag(&mut self, name: &JSXElementName<'_>) {
    match name {
      JSXElementName::Identifier(it) => self.tag_name(it.span),
      JSXElementName::IdentifierReference(it) => self.tag_name(it.span),
      JSXElementName::MemberExpression(it) => self.tag_member(it),
      JSXElementName::NamespacedName(it) => {
        self.mark(it.namespace.span, Role::Tag.style());
        self.mark(it.name.span, Role::Tag.style());
      }
      JSXElementName::ThisExpression(_) => {}
    }
  }

  /// A field or an accessor of a class: its name a field (italic when static), its decorators, type and value code
  fn field(&mut self, field: &js::Field<'_, '_>) {
    for decorator in field.decorators {
      self.visit_decorator(decorator);
    }
    self.key(field.key, field.computed, if field.is_static { STATIC_FIELD } else { FIELD });
    if let Some(annotation) = field.annotation {
      self.visit_ts_type_annotation(annotation);
    }
    if let Some(value) = field.value {
      self.visit_expression(value);
    }
  }

  /// A tag of one name: a component when capitalised, else an HTML tag
  fn tag_name(&mut self, span: Span) {
    let component = self.text_of(span).starts_with(|c: char| c.is_ascii_uppercase());
    self.mark(span, if component { Role::ComponentTag.style() } else { Role::Tag.style() });
  }

  fn tag_member(&mut self, member: &JSXMemberExpression<'_>) {
    match &member.object {
      JSXMemberExpressionObject::IdentifierReference(it) => self.mark(it.span, Role::ComponentTag.style()),
      JSXMemberExpressionObject::MemberExpression(it) => self.tag_member(it),
      JSXMemberExpressionObject::ThisExpression(_) => {}
    }
    self.mark(member.property.span, Role::ComponentTag.style());
  }

  /// The brackets of an element in `span` (`<`, `>`, `</`, `/>`), outside its name and its attributes
  fn brackets(&mut self, span: Span, inside: Span) {
    let text = self.text;
    let bytes = text.as_bytes();
    for offset in (span.start..inside.start).chain(inside.end..span.end) {
      if matches!(bytes.get(offset as usize), Some(b'<' | b'>' | b'/')) {
        self.styles.insert(offset, Role::TagBracket.style());
      }
    }
  }
}

impl<'a> Visit<'a> for Marks<'_> {
  fn visit_program(&mut self, program: &Program<'a>) {
    let scope = self.push();
    self.bind_declarations(&program.body, true);
    for statement in &program.body {
      self.visit_statement(statement);
    }
    self.pop(scope);
  }

  fn visit_block_statement(&mut self, it: &BlockStatement<'a>) {
    let scope = self.push();
    self.bind_declarations(&it.body, false);
    for statement in &it.body {
      self.visit_statement(statement);
    }
    self.pop(scope);
  }

  fn visit_module_declaration(&mut self, it: &ModuleDeclaration<'a>) {
    match it {
      ModuleDeclaration::ExportDeclaration(export) => {
        self.exported = true;
        self.visit_declaration(&export.declaration);
        self.exported = false;
      }
      ModuleDeclaration::ExportDefaultDeclaration(export) => self.visit_export_default_declaration(export),
      module => walk::walk_module_declaration(self, module),
    }
  }

  fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'a>) {
    self.exported = true;
    walk::walk_export_default_declaration(self, it);
    self.exported = false;
  }

  fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
    for specifier in it.specifiers.iter().flatten() {
      match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
          let imported = specifier.imported.span();
          if !matches!(specifier.imported, ModuleExportName::StringLiteral(_)) {
            self.name(imported, false);
          }
          if specifier.local.span != imported {
            self.name(specifier.local.span, false);
          }
        }
        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => self.name(specifier.local.span, false),
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => self.name(specifier.local.span, false),
      }
    }
  }

  fn visit_function(&mut self, it: &Function<'a>, _: ScopeFlags) {
    if matches!(it.r#type, FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction)
      && let Some(id) = &it.id
    {
      self.mark(id.span, if self.exported { GLOBAL_FUNCTION } else { LOCAL_FUNCTION });
    }
    self.exported = false;
    if it.r#type == FunctionType::FunctionExpression
      && let Some(id) = &it.id
    {
      self.name(id.span, false);
    }
    self.function_scope(it);
  }

  fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
    let scope = self.push();
    js::walk_signature(self, it.type_parameters.as_deref(), None, &it.params, it.return_type.as_deref());
    match &it.body {
      ArrowFunctionBody::FunctionBody(body) => self.body(&body.statements),
      body => {
        if let Some(expression) = body.as_expression() {
          self.visit_expression(expression);
        }
      }
    }
    self.pop(scope);
  }

  fn visit_class(&mut self, it: &Class<'a>) {
    self.exported = false;
    for decorator in &it.decorators {
      self.visit_decorator(decorator);
    }
    if let Some(parameters) = &it.type_parameters {
      self.visit_ts_type_parameter_declaration(parameters);
    }
    if let Some(heritage) = &it.heritage {
      self.visit_expression(&heritage.expression);
      if let Some(arguments) = &heritage.type_arguments {
        self.visit_ts_type_parameter_instantiation(arguments);
      }
    }
    for implemented in &it.implements {
      self.visit_ts_class_implements(implemented);
    }
    for element in &it.body.body {
      match element {
        ClassElement::MethodDefinition(method) => self.visit_method_definition(method),
        ClassElement::PropertyDefinition(property) => self.field(&field_of!(property)),
        ClassElement::AccessorProperty(property) => self.field(&field_of!(property)),
        element => self.visit_class_element(element),
      }
    }
  }

  fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
    for decorator in &it.decorators {
      self.visit_decorator(decorator);
    }
    self.key(&it.key, it.computed, if it.r#static { STATIC_METHOD } else { METHOD });
    self.function_scope(&it.value);
  }

  fn visit_ts_method_signature(&mut self, it: &TSMethodSignature<'a>) {
    self.key(&it.key, it.computed, METHOD);
    let scope = self.push();
    js::walk_signature(self, it.type_parameters.as_deref(), it.this_param.as_deref(), &it.params, it.return_type.as_deref());
    self.pop(scope);
  }

  fn visit_ts_property_signature(&mut self, it: &TSPropertySignature<'a>) {
    self.key(&it.key, it.computed, FIELD);
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
  }

  fn visit_ts_enum_member(&mut self, it: &TSEnumMember<'a>) {
    if let TSEnumMemberName::Identifier(name) = &it.id {
      self.mark(name.span, STATIC_FIELD);
    }
    if let Some(initializer) = &it.initializer {
      self.visit_expression(initializer);
    }
  }

  fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
    if it.shorthand {
      self.visit_expression(&it.value);
      return;
    }
    let function = it.method || matches!(it.value, Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_));
    // a string key is a string, but of a method (`get "name"() {}`)
    if it.method || it.kind != PropertyKind::Init || !matches!(it.key, PropertyKey::StringLiteral(_)) {
      self.key(&it.key, it.computed, if function { METHOD } else { FIELD });
    }
    self.visit_expression(&it.value);
  }

  fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
    let exported = std::mem::take(&mut self.exported);
    for declarator in &it.declarations {
      self.bind_pattern(&declarator.id, if exported { GLOBAL_VARIABLE } else { LOCAL_VARIABLE });
      if let Some(annotation) = &declarator.type_annotation {
        self.visit_ts_type_annotation(annotation);
      }
      if let Some(init) = &declarator.init {
        self.visit_expression(init);
      }
    }
  }

  fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
    let scope = self.push();
    walk::walk_for_statement(self, it);
    self.pop(scope);
  }

  // jscpd:ignore-start: the two loops over a collection are one rule in each walk (`for_each`)
  fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
    self.for_each(&it.left, &it.right, &it.body);
  }

  fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
    self.for_each(&it.left, &it.right, &it.body);
  }
  // jscpd:ignore-end

  fn visit_catch_clause(&mut self, it: &CatchClause<'a>) {
    let scope = self.push();
    if let Some(parameter) = &it.param {
      self.bind_pattern(&parameter.pattern, PARAMETER);
      if let Some(annotation) = &parameter.type_annotation {
        self.visit_ts_type_annotation(annotation);
      }
    }
    self.visit_block_statement(&it.body);
    self.pop(scope);
  }

  fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
    self.name(it.span, false);
  }

  fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
    self.member(&it.object, it.property.span, false);
  }

  fn visit_private_field_expression(&mut self, it: &PrivateFieldExpression<'a>) {
    self.member(&it.object, it.field.span, false);
  }

  fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
    self.callee(&it.callee);
    if let Some(arguments) = &it.type_arguments {
      self.visit_ts_type_parameter_instantiation(arguments);
    }
    for argument in &it.arguments {
      self.visit_argument(argument);
    }
  }

  fn visit_tagged_template_expression(&mut self, it: &TaggedTemplateExpression<'a>) {
    self.callee(&it.tag);
    js::walk_tagged_template(self, it);
  }

  /// A decorator in its colour, from its `@` to the arguments of its call, which are code
  fn visit_decorator(&mut self, it: &Decorator<'a>) {
    let (callee_end, arguments) = match &it.expression {
      Expression::CallExpression(call) => (call.callee.span().end, Some(call)),
      expression => (expression.span().end, None),
    };
    self.regions.push(Span::new(it.span.start, callee_end));
    if let Some(call) = arguments {
      for argument in &call.arguments {
        self.visit_argument(argument);
      }
    }
  }

  /// `{ [K in Keys]: T }`: its key a type parameter of it
  fn visit_ts_mapped_type(&mut self, it: &TSMappedType<'a>) {
    let scope = self.push();
    let name = self.text_of(it.key.span).to_string();
    self.scopes.bind_type_parameter(&name);
    self.mark(it.key.span, Role::TypeParameter.style());
    js::walk_mapped_type_types(self, it);
    self.pop(scope);
  }

  fn visit_ts_type_parameter(&mut self, it: &TSTypeParameter<'a>) {
    let name = self.text_of(it.name.span).to_string();
    self.scopes.bind_type_parameter(&name);
    self.mark(it.name.span, Role::TypeParameter.style());
    if let Some(constraint) = &it.constraint {
      self.visit_ts_type(constraint);
    }
    if let Some(default) = &it.default {
      self.visit_ts_type(default);
    }
  }

  /// A type: a type parameter in scope in its colour, any other plain; the names before the last of `a.B` names
  fn visit_ts_type_name(&mut self, it: &TSTypeName<'a>) {
    match it {
      TSTypeName::IdentifierReference(name) => {
        if self.scopes.is_type_parameter(self.text_of(name.span)) {
          self.mark(name.span, Role::TypeParameter.style());
        }
      }
      TSTypeName::QualifiedName(name) => {
        if let TSTypeName::IdentifierReference(first) = &name.left {
          self.name(first.span, false);
        } else {
          self.visit_ts_type_name(&name.left);
        }
        self.plain(name.right.span);
      }
      TSTypeName::ThisExpression(_) => {}
    }
  }

  fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'a>) {
    self.export_specifiers(&it.specifiers);
  }

  fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
    if let Some(exported) = &it.exported {
      self.export_name(exported);
    }
  }

  fn visit_ts_import_equals_declaration(&mut self, it: &TSImportEqualsDeclaration<'a>) {
    self.name(it.id.span, false);
    self.visit_ts_module_reference(&it.module_reference);
  }

  /// A parameter of a signature or a function type: a parameter in its scope
  fn visit_formal_parameter(&mut self, it: &FormalParameter<'a>) {
    js::walk_decorators(self, &it.decorators);
    self.bind_pattern(&it.pattern, PARAMETER);
    js::walk_parameter_type(self, it.type_annotation.as_deref(), it.initializer.as_deref());
  }

  fn visit_formal_parameter_rest(&mut self, it: &FormalParameterRest<'a>) {
    js::walk_rest_parameter(self, it, |walk, pattern| walk.bind_pattern(pattern, PARAMETER));
  }

  fn visit_ts_function_type(&mut self, it: &TSFunctionType<'a>) {
    let scope = self.push();
    walk::walk_ts_function_type(self, it);
    self.pop(scope);
  }

  fn visit_ts_constructor_type(&mut self, it: &TSConstructorType<'a>) {
    let scope = self.push();
    walk::walk_ts_constructor_type(self, it);
    self.pop(scope);
  }

  fn visit_ts_call_signature_declaration(&mut self, it: &TSCallSignatureDeclaration<'a>) {
    let scope = self.push();
    walk::walk_ts_call_signature_declaration(self, it);
    self.pop(scope);
  }

  fn visit_ts_construct_signature_declaration(&mut self, it: &TSConstructSignatureDeclaration<'a>) {
    let scope = self.push();
    walk::walk_ts_construct_signature_declaration(self, it);
    self.pop(scope);
  }

  /// `<>` and `</>`: the brackets of an element
  fn visit_jsx_fragment(&mut self, it: &JSXFragment<'a>) {
    let opening = it.opening_fragment.span;
    let closing = it.closing_fragment.span;
    self.brackets(opening, Span::new(opening.end, opening.end));
    self.brackets(closing, Span::new(closing.end, closing.end));
    for child in &it.children {
      self.visit_jsx_child(child);
    }
  }

  /// `typeof a.b`: names of the code, by their declarations, a member after its object
  fn visit_ts_type_query(&mut self, it: &TSTypeQuery<'a>) {
    js::walk_type_query(self, it, Self::query_name);
  }

  fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
    self.plain(it.span);
  }

  /// A declared name no rule colours (a namespace's, `namespace assert`): plain, never a keyword
  fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
    self.plain(it.span);
  }

  /// `x is T`: the parameter `x` by its declaration
  fn visit_ts_type_predicate(&mut self, it: &TSTypePredicate<'a>) {
    if let TSTypePredicateName::Identifier(name) = &it.parameter_name {
      self.name(name.span, false);
    }
    if let Some(annotation) = &it.type_annotation {
      self.visit_ts_type_annotation(annotation);
    }
  }

  fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
    self.export_specifiers(&it.specifiers);
  }

  fn visit_label_identifier(&mut self, it: &LabelIdentifier<'a>) {
    self.mark(it.span, Role::DefaultLabel.style());
  }

  fn visit_jsx_opening_element(&mut self, it: &JSXOpeningElement<'a>) {
    self.tag(&it.name);
    let inside_end = it
      .attributes
      .last()
      .map(|attribute| attribute.span().end)
      .or_else(|| it.type_arguments.as_ref().map(|arguments| arguments.span.end))
      .unwrap_or_else(|| it.name.span().end);
    self.brackets(it.span, Span::new(it.name.span().start, inside_end));
    js::walk_jsx_attributes(self, it);
  }

  fn visit_jsx_closing_element(&mut self, it: &JSXClosingElement<'a>) {
    self.tag(&it.name);
    self.brackets(it.span, it.name.span());
  }

  fn visit_jsx_attribute_name(&mut self, it: &JSXAttributeName<'a>) {
    match it {
      JSXAttributeName::Identifier(name) => self.mark(name.span, Role::Attribute.style()),
      JSXAttributeName::NamespacedName(name) => {
        self.mark(name.namespace.span, Role::Attribute.style());
        self.mark(name.name.span, Role::Attribute.style());
      }
    }
  }
}

impl Marks<'_> {
  /// `for (left of right) body`: a scope of the loop
  fn for_each(&mut self, left: &ForStatementLeft<'_>, right: &Expression<'_>, body: &Statement<'_>) {
    let scope = self.push();
    self.for_left(left);
    self.visit_expression(right);
    self.visit_statement(body);
    self.pop(scope);
  }

  /// The names of `export { a, b as c }`, by their declarations
  fn export_specifiers(&mut self, specifiers: &[ExportSpecifier<'_>]) {
    for specifier in specifiers {
      self.export_name(&specifier.local);
      if specifier.exported.span() != specifier.local.span() {
        self.export_name(&specifier.exported);
      }
    }
  }

  /// A name of `typeof`: the first by its declaration, the others members
  fn query_name(&mut self, name: &TSTypeName<'_>) {
    match name {
      TSTypeName::IdentifierReference(it) => self.name(it.span, false),
      TSTypeName::QualifiedName(it) => {
        self.query_name(&it.left);
        let on_type = matches!(&it.left, TSTypeName::IdentifierReference(left) if is_type_name(self.text_of(left.span)));
        let style = if on_type || is_constant_name(self.text_of(it.right.span)) {
          STATIC_FIELD
        } else {
          FIELD
        };
        self.mark(it.right.span, style);
      }
      TSTypeName::ThisExpression(_) => {}
    }
  }

  /// A name of an export: by its declaration in the scopes
  fn export_name(&mut self, name: &ModuleExportName<'_>) {
    match name {
      ModuleExportName::IdentifierName(it) => self.name(it.span, false),
      ModuleExportName::IdentifierReference(it) => self.name(it.span, false),
      ModuleExportName::StringLiteral(_) => {}
    }
  }

  /// `for (left of …)`: a declaration's names are local variables, an assignment's names by their declarations
  fn for_left(&mut self, left: &ForStatementLeft<'_>) {
    match left {
      ForStatementLeft::VariableDeclaration(declaration) => {
        for declarator in &declaration.declarations {
          self.bind_pattern(&declarator.id, LOCAL_VARIABLE);
        }
      }
      ForStatementLeft::AssignmentTargetIdentifier(it) => {
        let name = self.text_of(it.span).to_string();
        self.scopes.bind_styled(&name, LOCAL_VARIABLE);
        self.mark(it.span, LOCAL_VARIABLE);
      }
      left => {
        if let Some(target) = left.as_assignment_target() {
          self.visit_assignment_target(target);
        }
      }
    }
  }
}

/// The spans of the names a binding pattern binds
fn pattern_spans(pattern: &BindingPattern<'_>) -> Vec<Span> {
  match pattern {
    BindingPattern::BindingIdentifier(it) => vec![it.span],
    BindingPattern::AssignmentPattern(it) => pattern_spans(&it.left),
    BindingPattern::ObjectPattern(it) => it
      .properties
      .iter()
      .flat_map(|property| pattern_spans(&property.value))
      .chain(it.rest.iter().flat_map(|rest| pattern_spans(&rest.argument)))
      .collect(),
    BindingPattern::ArrayPattern(it) => it
      .elements
      .iter()
      .flatten()
      .flat_map(pattern_spans)
      .chain(it.rest.iter().flat_map(|rest| pattern_spans(&rest.argument)))
      .collect(),
  }
}
