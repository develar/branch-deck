//! Go to Declaration and Find Usages in a C# file from Roslyn's tree (spec/commit/symbol-navigation.spec.md): the name
//! at a place (`Tree::leaf_index`), its qualifier, the kind of the reference, its declaration in the file by the scopes,
//! and the usages of a name, answered by `navigate::{reference_answer, usage_answers}`, as Java's (`java::navigate`).
//!
//! - **A name** is an `IdentifierToken` of the code: not of a documentation comment or a directive. Names compare by
//!   their value (`@count` is `count`).
//! - **Its kind** by the node it is the name of or a part of: the name of a declaration (a type, a member, a parameter,
//!   a type parameter, a local, a local function, a pattern's variable, the variable of a `foreach` or a `catch`, a
//!   range variable of a query, an enum member, a label); a callee (`F()`, `a.F()`, `a?.F()`, with its qualifier); a
//!   member after a qualifier (`a.b`, `a?.b`); a type where C# reads one (`syntax_getters::is_in_type_slot`, the parts of a
//!   qualified name of a type); the names of a namespace and a `using` a value and members after it; else a value.
//! - **Its declaration** by the scopes around it, in one walk of the tree that keeps only the declarations of that
//!   name: a type sees its members, its type parameters and its primary constructor's parameters wherever they are
//!   declared, a member its type parameters and parameters, a block its local functions; a member, an accessor, a local
//!   function, a lambda, a block, a `for`, a `foreach`, a `using`, a `fixed`, a `catch`, a section or an arm of a
//!   `switch` and a query the locals declared before the name (the last one before it, else the first). `goto` goes to
//!   the label of its name in the blocks around it, the innermost first. A member after a qualifier, a named argument
//!   and the implicit `value` of an accessor have none.
//! - **The usages** of a local (declared in a member, an accessor, a lambda, a local function or a top-level statement)
//!   are the names bound to it; those of any other name are its names that no local binds, each with how it uses the
//!   name (`access_of`: `out` and `ref` arguments write).

use crate::csharp::{is_block_scope, is_code, is_type_declaration};
use crate::navigate::{Access, ReferenceKind};
use crate::psi_navigate::{Binding, IsNamed, Reading, Scoping, access_by_kind, is_local_in};
use csharp_syntax::read;
use csharp_syntax::syntax_getters as sg;
use csharp_syntax::{NodeId, NodeKind, TokenKind, Tree, facts};

/// C#'s reading of Roslyn's tree for Go to Declaration and Find Usages (`psi_navigate`)
pub(crate) const NAVIGATION: Reading<csharp_syntax::CSharp, Tree> = Reading {
  parse: |text| crate::csharp::parse(text.as_bytes()),
  psi: |tree| tree,
  is_name: |tree, leaf| tree.token_kind(leaf) == Some(TokenKind::IdentifierToken) && !in_structured_trivia(tree, leaf),
  name: value_of,
  declares,
  kind_in,
  qualifier_text: |tree, qualifier| read::text_without_trivia(tree, qualifier),
  is_named: |tree, node, name| tree.token_kind(node) == Some(TokenKind::IdentifierToken) && value_of(tree, node) == name,
  hoisted,
  scopes: |tree, is_named| Box::new(CSharpScopes { tree, is_named }),
  is_local: |tree, declared| is_local_in(tree, declared, is_code),
  access_of,
  name_start,
};

/// Where the value of a name starts: after the `@` of a verbatim identifier
fn name_start(tree: &Tree, leaf: NodeId) -> usize {
  let start = tree.range(leaf).start;
  if tree.text(leaf).first() == Some(&b'@') { start + 1 } else { start }
}

/// Whether a node lies in a structured trivia: a documentation comment, a directive, tokens the parser skipped
fn in_structured_trivia(tree: &Tree, node: NodeId) -> bool {
  std::iter::successors(tree.parent(node), |&it| tree.parent(it)).any(|it| read::is_structured_trivia(tree, it))
}

/// The value of a name (`count` of `@count`)
fn value_of(tree: &Tree, leaf: NodeId) -> &str {
  read::word(tree, leaf).unwrap_or_default()
}

/// The kind of a name in the node `parent` of the kind `kind`, which declares none, and its qualifier
fn kind_in(tree: &Tree, _leaf: NodeId, parent: NodeId, kind: NodeKind) -> (ReferenceKind, Option<NodeId>) {
  if !matches!(kind, NodeKind::IdentifierName | NodeKind::GenericName) {
    return (ReferenceKind::Value, None);
  }
  let name = parent;
  let Some(owner) = tree.parent(name) else { return (ReferenceKind::Value, None) };
  match tree.node_kind(owner) {
    Some(NodeKind::SimpleMemberAccessExpression | NodeKind::PointerMemberAccessExpression) if sg::member_access_expression::name(tree, owner) == Some(name) => {
      let qualifier = sg::member_access_expression::expression(tree, owner);
      (if is_callee(tree, owner) { ReferenceKind::MemberCall } else { ReferenceKind::Member }, qualifier)
    }
    Some(NodeKind::MemberBindingExpression) => {
      let qualifier = std::iter::successors(tree.parent(owner), |&it| tree.parent(it))
        .find(|&it| tree.node_kind(it) == Some(NodeKind::ConditionalAccessExpression))
        .and_then(|it| sg::conditional_access_expression::expression(tree, it));
      (if is_callee(tree, owner) { ReferenceKind::MemberCall } else { ReferenceKind::Member }, qualifier)
    }
    Some(NodeKind::QualifiedName | NodeKind::AliasQualifiedName) => {
      let right = sg::qualified_name::right(tree, owner) == Some(name) || sg::alias_qualified_name::name(tree, owner) == Some(name);
      if is_path(tree, owner) {
        (if right { ReferenceKind::Member } else { ReferenceKind::Value }, None)
      } else {
        (ReferenceKind::Type, None)
      }
    }
    // a named argument names a parameter of the callee
    Some(NodeKind::NameColon) => (ReferenceKind::Member, None),
    _ if is_callee(tree, name) => (ReferenceKind::Call, None),
    _ if sg::is_in_type_slot(tree, name) => (ReferenceKind::Type, None),
    _ => (ReferenceKind::Value, None),
  }
}

/// Whether a name is a part of a dotted name that is no type: a namespace's or a `using`'s
fn is_path(tree: &Tree, name: NodeId) -> bool {
  let mut current = name;
  loop {
    let Some(parent) = tree.parent(current) else { return false };
    match tree.node_kind(parent) {
      Some(NodeKind::QualifiedName | NodeKind::AliasQualifiedName) => current = parent,
      Some(NodeKind::NamespaceDeclaration | NodeKind::FileScopedNamespaceDeclaration | NodeKind::UsingDirective) => return true,
      _ => return false,
    }
  }
}

/// Whether `expression` is what an invocation around it calls
fn is_callee(tree: &Tree, expression: NodeId) -> bool {
  tree
    .parent(expression)
    .is_some_and(|call| tree.node_kind(call) == Some(NodeKind::InvocationExpression) && sg::invocation_expression::expression(tree, call) == Some(expression))
}

/// A node whose `IdentifierToken` child is the name it declares
const fn declares(kind: NodeKind) -> bool {
  (is_type_declaration(kind) && !matches!(kind, NodeKind::ExtensionBlockDeclaration))
    || matches!(
      kind,
      NodeKind::DelegateDeclaration
        | NodeKind::MethodDeclaration
        | NodeKind::ConstructorDeclaration
        | NodeKind::DestructorDeclaration
        | NodeKind::PropertyDeclaration
        | NodeKind::EventDeclaration
        | NodeKind::EnumMemberDeclaration
        | NodeKind::VariableDeclarator
        | NodeKind::Parameter
        | NodeKind::TypeParameter
        | NodeKind::LocalFunctionStatement
        | NodeKind::SingleVariableDesignation
        | NodeKind::ForEachStatement
        | NodeKind::CatchDeclaration
        | NodeKind::FromClause
        | NodeKind::LetClause
        | NodeKind::JoinClause
        | NodeKind::JoinIntoClause
        | NodeKind::QueryContinuation
        | NodeKind::LabeledStatement
        | NodeKind::TupleElement
    )
}

/// A node whose declarations no name outside it sees
const fn is_scope(kind: NodeKind) -> bool {
  is_type_declaration(kind) || is_code(kind) || is_block_scope(kind)
}

/// The scopes of C# for `psi_navigate::resolve`
struct CSharpScopes<'t> {
  tree: &'t Tree,
  is_named: IsNamed<'t>,
}

impl Scoping<csharp_syntax::CSharp> for CSharpScopes<'_> {
  fn skips(&self, node: NodeId) -> bool {
    read::is_structured_trivia(self.tree, node)
  }

  fn binding(&mut self, leaf: NodeId) -> Binding {
    if is_label_use(self.tree, leaf) {
      Binding::Bound(label_of(self.tree, leaf, self.is_named))
    } else {
      Binding::ByScopes
    }
  }

  fn declared(&self, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    let declares = declares(kind) && kind != NodeKind::LabeledStatement && !is_hoisted(self.tree, node, kind);
    sg::identifier_of(self.tree, node).filter(|&it| declares && (self.is_named)(it))
  }

  fn is_scope(&self, _node: NodeId, kind: NodeKind) -> bool {
    is_scope(kind)
  }
}

/// The names of the declarations a scope sees wherever they are: the members of a type (a field's declarators), its
/// type parameters and its primary constructor's parameters; the types of a file or a namespace; the type parameters
/// and parameters of a member, a local function or a lambda; the local functions of a block
fn hoisted(tree: &Tree, scope: NodeId, is_named: &dyn Fn(NodeId) -> bool) -> Vec<NodeId> {
  let mut declared = Vec::new();
  let mut push = |declaration: NodeId| {
    if let Some(name) = sg::identifier_of(tree, declaration).filter(|&it| is_named(it)) {
      declared.push(name);
    }
  };
  if let Some(list) = sg::type_parameter_list_of(tree, scope) {
    sg::type_parameter_list::parameters(tree, list).for_each(&mut push);
  }
  if let Some(list) = sg::parameter_list_of(tree, scope) {
    sg::base_parameter_list::parameters(tree, list).for_each(&mut push);
  }
  if let Some(parameter) = sg::simple_lambda_expression::parameter(tree, scope) {
    push(parameter);
  }
  for child in tree.children(scope) {
    let Some(kind) = tree.node_kind(child) else { continue };
    match kind {
      NodeKind::FieldDeclaration | NodeKind::EventFieldDeclaration => {
        if let Some(declaration) = sg::base_field_declaration::declaration(tree, child) {
          sg::variable_declaration::variables(tree, declaration).for_each(&mut push);
        }
      }
      NodeKind::NamespaceDeclaration | NodeKind::FileScopedNamespaceDeclaration => {
        // the types of a namespace are seen in the file
        for member in sg::base_namespace_declaration::members(tree, child) {
          if tree.node_kind(member).is_some_and(|it| is_type_declaration(it) || it == NodeKind::DelegateDeclaration) {
            push(member);
          }
        }
      }
      kind if is_hoisted(tree, child, kind) => push(child),
      _ => {}
    }
  }
  declared
}

/// Whether a declaration is seen in all of its scope, which collects it on entry (`hoisted`), rather than from where
/// the walk meets it: a member of a type, a type, a local function of a block, a parameter, a type parameter
fn is_hoisted(tree: &Tree, declaration: NodeId, kind: NodeKind) -> bool {
  let parent = tree.parent(declaration).and_then(|it| tree.node_kind(it));
  match kind {
    NodeKind::TypeParameter | NodeKind::Parameter | NodeKind::LocalFunctionStatement => true,
    NodeKind::VariableDeclarator => tree
      .parent(declaration)
      .and_then(|it| tree.parent(it))
      .and_then(|it| tree.node_kind(it))
      .is_some_and(|it| matches!(it, NodeKind::FieldDeclaration | NodeKind::EventFieldDeclaration)),
    NodeKind::MethodDeclaration
    | NodeKind::PropertyDeclaration
    | NodeKind::EventDeclaration
    | NodeKind::EnumMemberDeclaration
    | NodeKind::ConstructorDeclaration
    | NodeKind::DestructorDeclaration
    | NodeKind::DelegateDeclaration => {
      parent.is_some_and(|it| is_type_declaration(it) || matches!(it, NodeKind::CompilationUnit | NodeKind::NamespaceDeclaration | NodeKind::FileScopedNamespaceDeclaration))
    }
    kind if is_type_declaration(kind) => true,
    _ => false,
  }
}

/// The label a `goto` jumps to: a labeled statement of its name in a statement list around the `goto` (a block, a
/// section of a `switch`), the innermost first, as C# scopes a label to its block, before or after the `goto`; none
/// past the member around it
fn label_of(tree: &Tree, goto_name: NodeId, is_named: &dyn Fn(NodeId) -> bool) -> Option<NodeId> {
  for around in std::iter::successors(tree.parent(goto_name), |&it| tree.parent(it)) {
    let Some(kind) = tree.node_kind(around) else { continue };
    if matches!(kind, NodeKind::Block | NodeKind::SwitchSection) {
      // a statement of the list, and the statements its labels label (`a: b: …`)
      for statement in tree.children(around) {
        let mut labeled = Some(statement).filter(|&it| tree.node_kind(it) == Some(NodeKind::LabeledStatement));
        while let Some(label) = labeled {
          if let Some(name) = sg::labeled_statement::identifier(tree, label).filter(|&it| is_named(it)) {
            return Some(name);
          }
          labeled = sg::labeled_statement::statement(tree, label).filter(|&it| tree.node_kind(it) == Some(NodeKind::LabeledStatement));
        }
      }
    }
    if is_code(kind) {
      return None;
    }
  }
  None
}

/// The name of a `goto`: its label
fn is_label_use(tree: &Tree, leaf: NodeId) -> bool {
  tree
    .parent(leaf)
    .and_then(|it| tree.parent(it))
    .and_then(|it| tree.node_kind(it))
    .is_some_and(|it| it == NodeKind::GotoStatement)
}

/// How a name is used: declared, imported, called, written (the left side of an assignment, the operand of `++` or
/// `--`, an `out` or `ref` argument), else read
fn access_of(tree: &Tree, leaf: NodeId, kind: ReferenceKind) -> Access {
  if let Some(access) = access_by_kind(kind) {
    return access;
  }
  let range = tree.range(leaf);
  let ends_here = |node: Option<NodeId>| node.is_some_and(|it| own_end(tree, it) == range.end);
  for node in std::iter::successors(tree.parent(leaf), |&it| tree.parent(it)) {
    let Some(kind) = tree.node_kind(node) else { continue };
    match kind {
      NodeKind::UsingDirective => return Access::Import,
      kind if facts::is_assignment_expression(kind.syntax_kind()) => {
        let left = sg::assignment_expression::left(tree, node);
        return if ends_here(left) && left.is_some_and(|it| tree.range(it).start <= range.start) {
          Access::Write
        } else {
          Access::Read
        };
      }
      NodeKind::PreIncrementExpression | NodeKind::PreDecrementExpression => {
        if ends_here(sg::prefix_unary_expression::operand(tree, node)) {
          return Access::Write;
        }
      }
      NodeKind::PostIncrementExpression | NodeKind::PostDecrementExpression => {
        if ends_here(sg::postfix_unary_expression::operand(tree, node)) {
          return Access::Write;
        }
      }
      NodeKind::Argument => {
        let writes = sg::argument::ref_kind_keyword(tree, node).is_some_and(|it| matches!(tree.token_kind(it), Some(TokenKind::OutKeyword | TokenKind::RefKeyword)));
        return if writes && ends_here(sg::argument::expression(tree, node)) {
          Access::Write
        } else {
          Access::Read
        };
      }
      kind if is_scope(kind) => return Access::Read,
      _ => {}
    }
  }
  Access::Read
}

/// Where a node ends without the trivia after it
fn own_end(tree: &Tree, node: NodeId) -> usize {
  tree.own_end(node, |it| read::is_trivia(tree, it), |_| false)
}

#[cfg(test)]
#[path = "navigate_test.rs"]
mod tests;
