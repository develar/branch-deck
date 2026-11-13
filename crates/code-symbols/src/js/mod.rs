//! TypeScript, TSX and JavaScript through psi-syntax's `js-syntax`: oxc's parser, the parse accio reads these languages
//! with too (accio decision `2026-11-08-rust-and-js-parse-entries`), whose tree is oxc's typed arena AST
//! (`Function`, `Class`, `VariableDeclarator`, `IdentifierReference`, `StaticMemberExpression`, …) and whose tokens
//! the parse keeps on request. `with_program` is the one call into the parser; `summary` makes the `FileSummary` by the
//! parser-neutral `summary_builder`, as the psi languages do.
//!
//! - **Three dialects** (`Dialect`): `.ts`, `.mts`, `.cts` are TypeScript without JSX, where `<T>x` is a type
//!   assertion; `.tsx` is TSX; `.js`, `.mjs`, `.cjs`, `.jsx` are JavaScript with JSX. The summary takes the dialect from
//!   the file's path (`dialect_of_path`), so its cache key carries it (`language::summary_key`).
//! - **The text** the parser reads is the file's bytes, a byte that is no UTF-8 a space at its own offset
//!   (`js_syntax::parse_text`); every span of the tree is an offset into the file, and names are cut from the file's
//!   own bytes (`Source::text`), where such a byte makes the name none.
//! - **Recovery**: a parse oxc gives up on is parsed again with the region of its error blanked, up to sixteen times
//!   (`js_syntax::recover`); the blanked bytes are the errors of the file (`Errors`), with the labels of the
//!   syntax errors of the parse that made the tree (`js_syntax::is_syntax_error`: an early error is none). A file no recovery saves has no tree: every byte is an error.
//! - **Bounded by the stack**: a parse and every walk of its tree run inside `with_program`, on the calling thread when
//!   the file's nesting fits `psi_builder::stack::STACK`, else on a thread of the stack it needs; past
//!   `js_syntax`'s limit there is no tree and the summary is `truncated`.

pub(crate) mod highlight;
pub(crate) mod navigate;
pub(crate) mod summary;

#[cfg(test)]
mod highlight_test;
#[cfg(test)]
mod navigate_test;
#[cfg(test)]
pub(crate) mod summary_test;

use js_syntax::oxc_ast::ast::{
  Argument, BindingPattern, Decorator, Expression, FormalParameterRest, FormalParameters, Function, FunctionBody, JSXOpeningElement, Program, PropertyKey, TSMappedType,
  TSThisParameter, TSTypeAnnotation, TSTypeName, TSTypeParameter, TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeQuery, TSTypeQueryExprName,
  TaggedTemplateExpression,
};
use js_syntax::oxc_ast_visit::Visit;
use js_syntax::oxc_parser::Token;
use js_syntax::oxc_span::Span;
use js_syntax::{Dialect, Parsed};

/// The dialect of a file by its path: its extension's (`Dialect::of_extension`), TSX for a path without one of them,
/// the superset that reads every file of the other two
pub(crate) fn dialect_of_path(path: &str) -> Dialect {
  let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  file_name
    .rsplit_once('.')
    .and_then(|(_, extension)| Dialect::of_extension(extension))
    .unwrap_or(Dialect::Tsx)
}

/// The bytes of a file and where its lines start: what a walk cuts names from and numbers lines by (1-based, a line
/// ends at `\n`, as the other parsers count them)
pub(crate) struct Source<'s> {
  pub(crate) bytes: &'s [u8],
  newlines: Vec<u32>,
}

impl<'s> Source<'s> {
  pub(crate) fn new(bytes: &'s [u8]) -> Self {
    let newlines = bytes
      .iter()
      .enumerate()
      .filter(|(_, byte)| **byte == b'\n')
      .map(|(index, _)| u32::try_from(index).unwrap_or(u32::MAX))
      .collect();
    Self { bytes, newlines }
  }

  /// The 1-based line of the byte at `offset`
  pub(crate) fn line(&self, offset: u32) -> u32 {
    u32::try_from(self.newlines.partition_point(|&it| it < offset) + 1).unwrap_or(u32::MAX)
  }

  /// The last line of a node at `span`: a node that ends with its line break ends on the line before
  pub(crate) fn end_line(&self, span: Span) -> u32 {
    crate::summary_builder::end_line(|it| self.line(u32::try_from(it).unwrap_or(u32::MAX)), self.bytes, span.start as usize, span.end as usize)
  }

  /// The UTF-16 column of the byte at `offset` in its line
  pub(crate) fn column(&self, offset: u32) -> u32 {
    psi_builder::document::column(self.bytes, offset as usize)
  }

  /// The text of `span` as the file has it; `None` when it is no UTF-8
  pub(crate) fn text(&self, span: Span) -> Option<&'s str> {
    std::str::from_utf8(self.bytes.get(span.start as usize..span.end as usize)?).ok()
  }
}

/// The errors of a parse: the regions a recovery blanked and the labels of the syntax errors of the parse that made the
/// tree (`js_syntax::is_syntax_error`, never an early error), sorted, and the bytes they cover
/// (`FileSummary::error_bytes`), a zero-width label one
pub(crate) struct Errors {
  pub(crate) ranges: Vec<(u32, u32)>,
  pub(crate) bytes: u32,
}

impl Errors {
  pub(crate) fn of(parsed: &Parsed<'_>, source: &[u8]) -> Self {
    let mut ranges = Vec::new();
    if parsed.recovered {
      // the text of the last parse is the file's with the regions of the earlier errors blanked
      let read = js_syntax::parse_text(source);
      let blanked = parsed.program.source_text.as_bytes();
      let mut start = None;
      for (index, (left, right)) in read.as_bytes().iter().zip(blanked).enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        match (left == right, start) {
          (false, None) => start = Some(index),
          (true, Some(from)) => {
            ranges.push((from, index));
            start = None;
          }
          _ => {}
        }
      }
      if let Some(from) = start {
        ranges.push((from, u32::try_from(blanked.len()).unwrap_or(u32::MAX)));
      }
    }
    // an early error (a modifier where it may not stand, strict mode) leaves the tree whole: no error of the file, as
    // accio reads it too
    for diagnostic in parsed.diagnostics.iter().filter(|it| js_syntax::is_syntax_error(it)) {
      for label in &diagnostic.labels {
        ranges.push((label.offset(), label.offset().saturating_add(label.len())));
      }
    }
    ranges.sort_unstable();
    // the bytes under any range once, a zero-width one as one byte
    let mut bytes = 0u32;
    let mut covered = 0u32;
    for &(start, end) in &ranges {
      let end = end.max(start + 1);
      let from = start.max(covered);
      if end > from {
        bytes = bytes.saturating_add(end - from);
        covered = end;
      }
    }
    Self { ranges, bytes }
  }

  /// Whether an error lies in `start..end`, a zero-width one at its offset
  pub(crate) fn within(&self, start: u32, end: u32) -> bool {
    let first = self.ranges.partition_point(|it| it.0 < start);
    // a range that starts before `start` and reaches into it
    self.ranges[..first].iter().any(|it| it.1 > start) || self.ranges.get(first).is_some_and(|it| it.0 < end)
  }
}

/// The tokens of a parse in `start..end`, in source order
pub(crate) fn tokens_in(tokens: &[Token], start: u32, end: u32) -> &[Token] {
  let from = tokens.partition_point(|it| it.start() < start);
  let to = tokens.partition_point(|it| it.start() < end).max(from);
  &tokens[from..to]
}

/// The doc comments of a program by the start of the token each is attached to, with the first line of the doc
/// comments above that token: a `/**` comment, not `/**/` (a `///` one is a directive of the compiler)
pub(crate) fn doc_lines(program: &Program<'_>, source: &Source<'_>) -> Vec<(u32, u32)> {
  let mut docs: Vec<(u32, u32)> = program
    .comments
    .iter()
    .filter(|it| it.is_leading())
    .filter(|it| source.text(it.span).is_some_and(|text| text.starts_with("/**") && !text.starts_with("/**/")))
    .map(|it| (it.attached_to, source.line(it.span.start)))
    .collect();
  docs.sort_unstable();
  docs.dedup_by_key(|it| it.0);
  docs
}

/// A field or an accessor of a class, two node types of one rule (`field_of`)
pub(crate) struct Field<'n, 'a> {
  pub(crate) decorators: &'n [Decorator<'a>],
  pub(crate) key: &'n PropertyKey<'a>,
  pub(crate) computed: bool,
  pub(crate) span: Span,
  pub(crate) is_static: bool,
  pub(crate) private: bool,
  pub(crate) annotation: Option<&'n TSTypeAnnotation<'a>>,
  pub(crate) value: Option<&'n Expression<'a>>,
}

/// The `Field` of a `PropertyDefinition` or an `AccessorProperty`, whose parts have the same names
macro_rules! field_of {
  ($it:expr) => {
    $crate::js::Field {
      decorators: &$it.decorators,
      key: &$it.key,
      computed: $it.computed,
      span: $it.span,
      is_static: $it.r#static,
      private: $it.accessibility == Some(js_syntax::oxc_ast::ast::TSAccessibility::Private) || matches!($it.key, js_syntax::oxc_ast::ast::PropertyKey::PrivateIdentifier(_)),
      annotation: $it.type_annotation.as_deref(),
      value: $it.value.as_ref(),
    }
  };
}
pub(crate) use field_of;

/// The parts of a function or a signature after its name: its type parameters, `this`, its parameters, its return type
pub(crate) fn walk_signature<'a>(
  visitor: &mut impl Visit<'a>,
  type_parameters: Option<&TSTypeParameterDeclaration<'a>>,
  this: Option<&TSThisParameter<'a>>,
  parameters: &FormalParameters<'a>,
  return_type: Option<&TSTypeAnnotation<'a>>,
) {
  if let Some(type_parameters) = type_parameters {
    visitor.visit_ts_type_parameter_declaration(type_parameters);
  }
  if let Some(this) = this {
    visitor.visit_ts_this_parameter(this);
  }
  visitor.visit_formal_parameters(parameters);
  if let Some(return_type) = return_type {
    visitor.visit_ts_type_annotation(return_type);
  }
}

/// A function after its name: its signature and its body
pub(crate) fn walk_function<'a>(visitor: &mut impl Visit<'a>, function: &Function<'a>) {
  walk_signature(
    visitor,
    function.type_parameters.as_deref(),
    function.this_param.as_deref(),
    &function.params,
    function.return_type.as_deref(),
  );
  walk_body(visitor, function.body.as_deref());
}

/// The statements of a function's body
pub(crate) fn walk_body<'a>(visitor: &mut impl Visit<'a>, body: Option<&FunctionBody<'a>>) {
  for statement in body.iter().flat_map(|it| &it.statements) {
    visitor.visit_statement(statement);
  }
}

/// The type arguments and the arguments of a call or a `new`, after its callee
pub(crate) fn walk_arguments<'a>(visitor: &mut impl Visit<'a>, type_arguments: Option<&TSTypeParameterInstantiation<'a>>, arguments: &[Argument<'a>]) {
  if let Some(type_arguments) = type_arguments {
    visitor.visit_ts_type_parameter_instantiation(type_arguments);
  }
  for argument in arguments {
    visitor.visit_argument(argument);
  }
}

/// The constraint and the default of a type parameter, after its name
pub(crate) fn walk_type_parameter_bounds<'a>(visitor: &mut impl Visit<'a>, parameter: &TSTypeParameter<'a>) {
  if let Some(constraint) = &parameter.constraint {
    visitor.visit_ts_type(constraint);
  }
  if let Some(default) = &parameter.default {
    visitor.visit_ts_type(default);
  }
}

/// The type arguments and the text of a tagged template, after its tag
pub(crate) fn walk_tagged_template<'a>(visitor: &mut impl Visit<'a>, template: &TaggedTemplateExpression<'a>) {
  if let Some(type_arguments) = &template.type_arguments {
    visitor.visit_ts_type_parameter_instantiation(type_arguments);
  }
  visitor.visit_template_literal(&template.quasi);
}

/// The types of a mapped type, after its key
pub(crate) fn walk_mapped_type_types<'a>(visitor: &mut impl Visit<'a>, mapped: &TSMappedType<'a>) {
  visitor.visit_ts_type(&mapped.constraint);
  if let Some(name) = &mapped.name_type {
    visitor.visit_ts_type(name);
  }
  if let Some(annotation) = &mapped.type_annotation {
    visitor.visit_ts_type(annotation);
  }
}

/// `typeof x`: its name by `name`, an import type and the type arguments walked
pub(crate) fn walk_type_query<'a, V: Visit<'a>>(visitor: &mut V, query: &TSTypeQuery<'a>, name: impl FnOnce(&mut V, &TSTypeName<'a>)) {
  match &query.expr_name {
    TSTypeQueryExprName::TSImportType(import) => visitor.visit_ts_import_type(import),
    expression => {
      if let Some(type_name) = expression.as_ts_type_name() {
        name(visitor, type_name);
      }
    }
  }
  if let Some(type_arguments) = &query.type_arguments {
    visitor.visit_ts_type_parameter_instantiation(type_arguments);
  }
}

/// The type and the default of a parameter, after its pattern
pub(crate) fn walk_parameter_type<'a>(visitor: &mut impl Visit<'a>, annotation: Option<&TSTypeAnnotation<'a>>, initializer: Option<&Expression<'a>>) {
  if let Some(annotation) = annotation {
    visitor.visit_ts_type_annotation(annotation);
  }
  if let Some(initializer) = initializer {
    visitor.visit_expression(initializer);
  }
}

/// A rest parameter: its decorators, its pattern by `bind`, its type
pub(crate) fn walk_rest_parameter<'a, V: Visit<'a>>(visitor: &mut V, rest: &FormalParameterRest<'a>, bind: impl FnOnce(&mut V, &BindingPattern<'a>)) {
  walk_decorators(visitor, &rest.decorators);
  bind(visitor, &rest.rest.argument);
  walk_parameter_type(visitor, rest.type_annotation.as_deref(), None);
}

/// The decorators of a parameter, before its pattern
pub(crate) fn walk_decorators<'a>(visitor: &mut impl Visit<'a>, decorators: &[Decorator<'a>]) {
  for decorator in decorators {
    visitor.visit_decorator(decorator);
  }
}

/// The type arguments and the attributes of a JSX element, after its name
pub(crate) fn walk_jsx_attributes<'a>(visitor: &mut impl Visit<'a>, element: &JSXOpeningElement<'a>) {
  if let Some(type_arguments) = &element.type_arguments {
    visitor.visit_ts_type_parameter_instantiation(type_arguments);
  }
  for attribute in &element.attributes {
    visitor.visit_jsx_attribute_item(attribute);
  }
}
