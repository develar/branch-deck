//! The summary of a Starlark file from its PSI, as the Bazel plugin's structure view lists it: the top-level `def`s,
//! the names a top-level `=` assigns and, in a file whose top-level calls declare targets (`BUILD`, `BUILD.bazel`,
//! `MODULE.bazel`), its targets — a call with a literal `name`, which `starlark_syntax::psi::rule_target` decides, the
//! one place that rule lives for every consumer of the tree.
//!
//! - **A `load` is an import per name it binds**, the label as written its `specifier`, as a TypeScript module string
//!   is one: `trial_check` turns either into the files it may mean, by the same step.
//! - **A label in a string is no reference** (`"//pkg:x"`, `":x"`): a reference here is a name the tree uses, and a
//!   label needs a package model the summary does not have (spec/commit/symbol-dependencies.spec.md).
//! - **One pass, no walk state but the `def` around a node**: Starlark has no nested containers, no supertypes and no
//!   types, so `psi_walk`'s frames would hold nothing.

use crate::psi_tree::error_bytes;
use crate::summary::{Argument, DeclKind, Declaration, FileSummary, Import, ImportTarget, LineSpan, MemberKind, Parameter, Reach, Role, Shape, Signature};
use crate::summary_builder::{SummaryBuilder, blank_declaration, column_at, end_line};
use psi_builder::document::Lines;
use starlark_syntax::{FileKind, NodeId, NodeKind as N, TokenKind as T, Tree, psi};
use std::collections::HashSet;

/// Fills `summary` (`summary_builder::empty_summary` of the file, its size within the limits) with the facts of the
/// Starlark file `source`, read as `kind`; whether it was read, else it stays `truncated`
pub(crate) fn summarize(source: &[u8], kind: FileKind, summary: &mut FileSummary) -> bool {
  let tree = super::parse(source, kind);
  let mut walk = Walk::new(&tree, kind);
  walk.run();
  walk.out.finish(summary, None);
  true
}

/// The `def` the walk is inside: where its subtree ends, its declaration and the names its body binds
struct Function<'t> {
  end: u32,
  declaration: u32,
  bound: HashSet<&'t str>,
}

struct Walk<'t> {
  tree: &'t Tree,
  text: &'t [u8],
  lines: Lines,
  out: SummaryBuilder<'t>,
  /// A top-level call declares a target here (`FileKind::top_level_calls_declare`)
  targets: bool,
  function: Option<Function<'t>>,
}

impl<'t> Walk<'t> {
  fn new(tree: &'t Tree, kind: FileKind) -> Self {
    let text = tree.source();
    Self {
      tree,
      text,
      lines: Lines::new(text),
      out: SummaryBuilder::new(),
      targets: kind.top_level_calls_declare(),
      function: None,
    }
  }

  fn run(&mut self) {
    self.out.error_bytes = u32::try_from(error_bytes(self.tree)).unwrap_or(u32::MAX);
    let root = self.tree.root();
    for node in self.tree.descendants(root) {
      if self.function.as_ref().is_some_and(|it| node.0 >= it.end) {
        self.function = None;
      }
      match self.tree.node_kind(node) {
        Some(N::FUNCTION_DECLARATION) if self.tree.parent(node) == Some(root) => self.function(node),
        Some(N::TARGET_EXPRESSION) => self.assigned(node, root),
        Some(N::LOAD_STATEMENT) if psi::load_is_top_level(self.tree, node) => self.load(node),
        Some(N::CALL_EXPRESSION) if self.targets => self.target(node),
        Some(N::REFERENCE_EXPRESSION) if !self.is_qualifier(node) => self.reference(node),
        _ => {}
      }
    }
  }

  /// A top-level `def`: a function with the parameters of its list, and the scope of the names its body binds
  fn function(&mut self, node: NodeId) {
    let Some(name_node) = psi::direct_identifier(self.tree, node) else { return };
    let Some(name) = self.name_of(name_node) else { return };
    let signature = self.signature(node);
    let mut declaration = self.blank(name, DeclKind::Function);
    declaration.signature = Some(signature);
    self.fill(&mut declaration, node, self.tree.range(name_node).start);
    let declaration = self.out.add_declaration(declaration);
    self.function = Some(Function {
      end: self.tree.subtree_end(node).0,
      declaration,
      bound: self.bound_names(node),
    });
  }

  /// A name a top-level `=` assigns (not `+=`): a constant in capitals, a value otherwise. A name assigned in a `def`
  /// or by a loop is bound, not declared
  fn assigned(&mut self, node: NodeId, root: NodeId) {
    let top_level = psi::assignment_of_target(self.tree, node)
      .is_some_and(|assignment| self.tree.parent(assignment) == Some(root) && self.tree.node_kind(assignment) == Some(N::ASSIGNMENT_STATEMENT));
    if !top_level {
      return;
    }
    let Some(name_node) = psi::direct_identifier(self.tree, node) else { return };
    let Some(name) = self.name_of(name_node) else { return };
    let kind = if is_upper_case(name) { DeclKind::Constant } else { DeclKind::Value };
    let mut declaration = self.blank(name, kind);
    self.fill(&mut declaration, node, self.tree.range(name_node).start);
    self.out.add_declaration(declaration);
  }

  /// A target of a BUILD file: the contents of its literal `name`, the rule it calls (`jps_test`,
  /// `native.cc_library`) its `value_type`, which the summary prints as `<jps_test>`
  fn target(&mut self, call: NodeId) {
    let Some(target) = psi::rule_target(self.tree, call) else { return };
    let name = self.out.intern_owned(target.name);
    let mut declaration = blank_declaration(name, DeclKind::Target, Reach::Public, MemberKind::TopLevel, None);
    if let Some(rule) = target.rule {
      declaration.value_type = Some(rule.split('.').map(|segment| self.out.intern_owned(segment.to_owned())).collect());
    }
    self.fill(&mut declaration, call, target.name_at);
    self.out.add_declaration(declaration);
  }

  /// A top-level `load`: its lines, and an import per name it binds, the label as written its specifier
  fn load(&mut self, node: NodeId) {
    let range = self.tree.range(node);
    self.out.import_lines.push(LineSpan {
      start: self.lines.line(range.start),
      end: end_line(|offset| self.lines.line(offset), self.text, range.start, range.end),
    });
    let Some((label, _)) = psi::load_label(self.tree, node) else { return };
    for value in psi::loaded_values(self.tree, node) {
      let Some((remote, remote_range)) = psi::string_value(self.tree, value) else { continue };
      let local = psi::direct_identifier(self.tree, value).and_then(|it| self.name_of(it));
      let line = self.lines.line(self.tree.range(value).start);
      let target = self.out.intern_owned(remote.clone());
      let alias = local.filter(|it| *it != remote).map(|it| self.out.intern(it));
      self.out.add_import(Import {
        path: Vec::new(),
        target: ImportTarget::Name(target),
        alias,
        exported: false,
        specifier: Some(label.clone()),
        line,
      });
      self.out.add_reference(target, self.lines.line(remote_range.start), Role::Import, None, None, None, None);
    }
  }

  /// A name the code reads: the callee of a call, else a value; a name bound in the `def` around it is none
  fn reference(&mut self, node: NodeId) {
    let Some(name_node) = psi::direct_identifier(self.tree, node) else { return };
    let Some(name) = self.name_of(name_node) else { return };
    let segments = self.qualifier(node);
    if segments.is_none() && self.function.as_ref().is_some_and(|it| it.bound.contains(name)) {
      return;
    }
    let qualifier = segments.map(|segments| self.out.ids(&segments));
    let call = self.tree.parent(node).filter(|&parent| self.is_callee(parent, node));
    let shape = call.and_then(|call| self.call_shape(call));
    let role = if call.is_some() { Role::Call } else { Role::Value };
    let name = self.out.intern(name);
    let container = self.function.as_ref().map(|it| it.declaration);
    self
      .out
      .add_reference(name, self.lines.line(self.tree.range(name_node).start), role, qualifier, container, shape, None);
  }

  /// The names before the last one of `a.b.c`, as `Reference::qualifier` takes them: `None` for a bare name, empty
  /// when the receiver is an expression (`foo().bar`) or a name bound in the `def` around it
  fn qualifier(&self, node: NodeId) -> Option<Vec<&'t str>> {
    let receiver = psi::first_node_child(self.tree, node)?;
    let mut segments = Vec::new();
    let mut current = Some(receiver);
    while let Some(node) = current {
      if self.tree.node_kind(node) != Some(N::REFERENCE_EXPRESSION) {
        return Some(Vec::new());
      }
      let Some(name) = psi::direct_identifier(self.tree, node).and_then(|it| self.name_of(it)) else {
        return Some(Vec::new());
      };
      segments.push(name);
      current = psi::first_node_child(self.tree, node);
    }
    segments.reverse();
    if self.function.as_ref().is_some_and(|it| segments.first().is_some_and(|head| it.bound.contains(head))) {
      return Some(Vec::new());
    }
    Some(segments)
  }

  /// The arguments of a call, interned as one shape: a named one with its name, `spread` for `*args` and `**kwargs`
  fn call_shape(&mut self, call: NodeId) -> Option<u32> {
    let list = psi::first_child(self.tree, call, |child| self.tree.node_kind(child) == Some(N::ARGUMENT_LIST))?;
    let mut arguments = Vec::new();
    let mut spread = false;
    for argument in self.tree.children(list) {
      match self.tree.node_kind(argument) {
        Some(N::ARGUMENT_EXPRESSION) => arguments.push((None, self.argument_kind(argument))),
        Some(N::NAMED_ARGUMENT_EXPRESSION) => {
          let name = psi::direct_identifier(self.tree, argument).and_then(|it| self.name_of(it)).map(|it| self.out.intern(it));
          arguments.push((name, self.argument_kind(argument)));
        }
        Some(N::STAR_ARGUMENT_EXPRESSION | N::DOUBLE_STAR_EXPRESSION) => spread = true,
        _ => {}
      }
    }
    Some(self.out.intern_shape(Shape::Call {
      arguments,
      spread,
      enclosed: false,
    }))
  }

  /// What an argument is, as far as its syntax tells
  fn argument_kind(&self, argument: NodeId) -> Argument {
    let value = self.tree.children(argument).filter(|&child| self.tree.token_kind(child).is_none()).last();
    match value.and_then(|it| self.tree.node_kind(it)) {
      Some(N::STRING_LITERAL_EXPRESSION) => Argument::Text,
      Some(N::INTEGER_LITERAL_EXPRESSION) => Argument::Number,
      Some(N::TRUE_LITERAL_EXPRESSION | N::FALSE_LITERAL_EXPRESSION) => Argument::Boolean,
      Some(N::LAMBDA_EXPRESSION) => Argument::Lambda,
      _ => Argument::Expression,
    }
  }

  /// The parameters of a `def`, in order: `optional` for one with a default, `vararg` for `*args` and `**kwargs`
  fn signature(&mut self, function: NodeId) -> Signature {
    let mut parameters = Vec::new();
    let list = psi::first_child(self.tree, function, |child| self.tree.node_kind(child) == Some(N::PARAMETER_LIST));
    for parameter in list.into_iter().flat_map(|list| self.tree.children(list)) {
      let (optional, vararg) = match self.tree.node_kind(parameter) {
        Some(N::MANDATORY_PARAMETER) => (false, false),
        Some(N::OPTIONAL_PARAMETER) => (true, false),
        Some(N::VARIADIC_PARAMETER | N::KEYWORD_VARIADIC_PARAMETER) => (false, true),
        _ => continue,
      };
      let Some(name) = psi::direct_identifier(self.tree, parameter).and_then(|it| self.name_of(it)) else {
        continue;
      };
      let name = self.out.intern(name);
      parameters.push(Parameter {
        name,
        type_ref: None,
        optional,
        vararg,
      });
    }
    Signature { parameters }
  }

  /// The names a `def` binds: its parameters, the names its body assigns or loops over, and the `def`s in it
  fn bound_names(&self, function: NodeId) -> HashSet<&'t str> {
    let mut names = HashSet::new();
    for node in self.tree.descendants(function) {
      let binds = matches!(
        self.tree.node_kind(node),
        Some(
          N::FUNCTION_DECLARATION
            | N::MANDATORY_PARAMETER
            | N::OPTIONAL_PARAMETER
            | N::VARIADIC_PARAMETER
            | N::KEYWORD_VARIADIC_PARAMETER
            | N::TARGET_EXPRESSION
            | N::NAMED_LOAD_VALUE
        )
      );
      if binds && let Some(name) = psi::direct_identifier(self.tree, node).and_then(|it| self.name_of(it)) {
        names.insert(name);
      }
    }
    names
  }

  fn blank(&mut self, name: &'t str, kind: DeclKind) -> Declaration {
    let id = self.out.intern(name);
    // Bazel loads no name that starts with `_`: it is private to its file
    let reach = if name.starts_with('_') { Reach::File } else { Reach::Public };
    blank_declaration(id, kind, reach, MemberKind::TopLevel, None)
  }

  /// The lines of a declaration, where its name starts and whether the parser recovered around it
  fn fill(&self, declaration: &mut Declaration, node: NodeId, name_at: usize) {
    let range = self.tree.range(node);
    declaration.start_line = self.lines.line(range.start);
    declaration.end_line = end_line(|offset| self.lines.line(offset), self.text, range.start, range.end);
    declaration.name_line = self.lines.line(name_at);
    declaration.name_column = column_at(self.text, name_at);
    declaration.in_error = self.tree.ancestor_or_self(node, |it| self.tree.node_kind(it) == Some(N::ERROR_ELEMENT)).is_some();
  }

  /// Is `node` the callee of the call `parent`, the first element of it
  fn is_callee(&self, parent: NodeId, node: NodeId) -> bool {
    self.tree.node_kind(parent) == Some(N::CALL_EXPRESSION) && psi::first_node_child(self.tree, parent) == Some(node)
  }

  /// Is `node` the receiver of a reference around it (`a` of `a.b`), which that reference takes as its qualifier
  fn is_qualifier(&self, node: NodeId) -> bool {
    self.tree.parent(node).and_then(|parent| self.tree.node_kind(parent)) == Some(N::REFERENCE_EXPRESSION)
  }

  fn name_of(&self, token: NodeId) -> Option<&'t str> {
    (self.tree.token_kind(token) == Some(T::IDENTIFIER)).then(|| std::str::from_utf8(self.tree.text(token)).ok())?
  }
}

/// Whether a name is written in capitals, the convention of a Starlark constant: `MAX_SIZE`, not `Label` or `rules`
fn is_upper_case(name: &str) -> bool {
  name.chars().any(char::is_alphabetic) && !name.chars().any(char::is_lowercase)
}
