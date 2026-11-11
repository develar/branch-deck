//! Go to Declaration and Find Usages in a C# file from Roslyn's tree (openspec/specs/commit/symbol-navigation/spec.md): the name
//! at a place (`Tree::leaf_index`), its qualifier, the kind of the reference, its declaration in the file by the scopes,
//! and the names of one spelling with their bindings, answered by `psi_navigate` and `navigate`, as Java's
//! (`java::navigate`).
//!
//! - **A name** is an `IdentifierToken` of the code: not of a documentation comment or a directive. Names compare by
//!   their value (`@count` is `count`).
//! - **Its kind** by the node it is the name of or a part of: the name of a declaration (a type, a member, a parameter,
//!   a type parameter, a local, a local function, a pattern's variable, the variable of a `foreach` or a `catch`, a
//!   range variable of a query, an enum member, a label); a callee (`F()`, `a.F()`, `a?.F()`, with its qualifier); a
//!   member after a qualifier (`a.b`, `a?.b`), the name of a named argument and the member an initializer, a `with`
//!   or a `Name =` names; a type where C# reads one (`syntax_getters::is_in_type_slot`, the parts of a qualified name
//!   of a type); the names of a namespace and a `using` a value and members after it; else a value.
//! - **Its declaration** by the scopes around it, in one walk of the tree that keeps only the declarations of that
//!   name: a type sees its members, its type parameters and its primary constructor's parameters wherever they are
//!   declared, a member its type parameters and parameters, a block and the top level of a file their local
//!   functions; and each scope the locals declared before the name (the last one before it, else the first). The
//!   scopes of the locals are C#'s (`opens_scope`, `declares_in`, `Scoping::sees`): a member, an accessor, a local
//!   function, a lambda, a block, a loop of any kind, a `using`, a `fixed`, a `catch`, an arm of a `switch`
//!   expression, a query, and a statement that is the body of another without a block; the statements of every
//!   section of a `switch` share its block, a section alone has the variables of its labels, and what the governing
//!   expression of a `switch`, the condition of an `if` and the expression of a `lock` declare is of the statements
//!   around them; the top-level statements of a file share one scope. The collection of a `foreach` is read before
//!   its variables are. In a query the source of a `from`, the value of a `let` and the left key of a `join` are read
//!   before their own variable, the source of a `join` sees no variable of its query, its right key the variable of
//!   the `join` alone, `into` ends the variables before it. A name where C# reads a type is of a type or a type
//!   parameter, never of a variable; but the right side of `x is Max` is a constant where the scopes declare no type
//!   of the name (`Scoping::else_reads`). `goto` goes to the label of its name in the blocks around it, the innermost
//!   first. A member and the implicit `value` of an accessor have none.
//! - **The usages** of a local (declared in a member, an accessor, a lambda, a local function or a top-level statement)
//!   are the names bound to it; those of any other name are its names that no local binds, each with how it uses the
//!   name (`access_of`: `out` and `ref` arguments write).
//! - **What a local is** (`local_of`, openspec/specs/commit/name-bindings/spec.md): a local variable, a variable of a
//!   pattern or of an `out` argument and a range variable of a query a variable, the variable of a `for` and a
//!   `foreach`, the one of a `catch`, the parameter of a lambda or an anonymous method, and the parameter of a method,
//!   a constructor, a local function, an indexer, an operator or a delegate, which a call may name; a local function,
//!   a type parameter, a label and the element of a tuple type are `Other`.
//! - **Its site** (`site_of`): a label and the name of a `goto`; the name of a named argument; a name `nameof` spells;
//!   a member of an anonymous type and an element of a tuple named by the expression (`new { total }`, `(total, 1)`);
//!   a variable of a deconstruction; and a namesake where the name is all of the right side of an `is`, a constant
//!   only while no type of the name is in sight. `value` and `field` are bound without a declaration, in an accessor.
//! - **A file read in part** (`reads_in_part`): one with an error of its parser, and one with an `#if`, whose locals
//!   are bound as one build of the file binds them.

use crate::csharp::{is_block_scope, is_code, is_type_declaration};
use crate::navigate::{Access, LocalBinding, LocalKind, ReferenceKind, Site};
use crate::psi_navigate::{Binding, IsNamed, Reading, Scoping, access_by_kind, is_local_in, is_within, parent_kind};
use csharp_syntax::read;
use csharp_syntax::syntax_getters as sg;
use csharp_syntax::{NodeId, NodeKind, Severity, TokenKind, Tree, facts};

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
  is_local: |tree, declared| is_local_in(tree, declared, |kind| is_code(kind) || is_initializer(kind)),
  local_of,
  site_of,
  rebinds: |_, _| false,
  // the value a setter takes and the field of a property, in its accessors
  is_implicit: |name| matches!(name, "value" | "field"),
  access_of,
  name_start,
  reads_in_part,
};

/// Whether the scopes read only a part of the code of a file: the parser reported an error (what stands where it gave
/// up is no name of the tree), or the file is compiled by conditions. The text of a branch of `#if` that the parser
/// does not take is no code of the tree, so a use left there, and a name another name would take there, are unseen;
/// and a declaration inside a branch it does take is no declaration where the other is compiled, so the names the
/// scopes bind to it are bound to another there. Either way the bindings of the tree are those of one build of many
fn reads_in_part(tree: &Tree) -> bool {
  let broken = tree.diagnostics().iter().any(|diagnostic| diagnostic.severity() == Severity::Error);
  broken
    || (0..u32::try_from(tree.len()).unwrap_or(u32::MAX)).any(|id| {
      matches!(
        tree.node_kind(NodeId(id)),
        Some(NodeKind::IfDirectiveTrivia | NodeKind::ElifDirectiveTrivia | NodeKind::ElseDirectiveTrivia)
      )
    })
}

/// The node around `node` and its kind, a composite's
fn around(tree: &Tree, node: NodeId) -> Option<(NodeId, NodeKind)> {
  parent_kind(tree, node)
}

/// What a local declares, by its declaration and what it is a part of. A call may name the parameter of anything it
/// calls by a name (a method, a constructor, a local function, an indexer, an operator, a delegate); the parameter of
/// a lambda or an anonymous method is its body's alone, as a call of a delegate names the delegate's
fn local_of(tree: &Tree, declaration: NodeId, kind: NodeKind) -> LocalBinding {
  LocalBinding::of(match kind {
    // `for (var i = 0; …)` declares its variable in the declaration of the `for`
    NodeKind::VariableDeclarator => match around(tree, declaration).and_then(|it| around(tree, it.0)) {
      Some((_, NodeKind::ForStatement)) => LocalKind::LoopVariable,
      _ => LocalKind::Variable,
    },
    NodeKind::SingleVariableDesignation | NodeKind::FromClause | NodeKind::LetClause | NodeKind::JoinClause | NodeKind::JoinIntoClause | NodeKind::QueryContinuation => {
      LocalKind::Variable
    }
    NodeKind::ForEachStatement => LocalKind::LoopVariable,
    NodeKind::CatchDeclaration => LocalKind::CatchParameter,
    NodeKind::Parameter => {
      let owner = match around(tree, declaration) {
        Some((list, NodeKind::ParameterList | NodeKind::BracketedParameterList)) => around(tree, list),
        owner => owner,
      };
      return match owner {
        Some((_, NodeKind::SimpleLambdaExpression | NodeKind::ParenthesizedLambdaExpression | NodeKind::AnonymousMethodExpression)) => LocalBinding::of(LocalKind::LambdaParameter),
        Some(_) => LocalBinding::NAMED_PARAMETER,
        None => LocalBinding::OTHER,
      };
    }
    _ => LocalKind::Other,
  })
}

/// What the site of a name makes of it beside its binding (`Site`)
fn site_of(tree: &Tree, leaf: NodeId, _kind: ReferenceKind, local: Option<(NodeId, LocalBinding)>) -> Site {
  match around(tree, leaf) {
    Some((_, NodeKind::LabeledStatement)) => Site::Label,
    // `var (a, b) = pair`, `(var a, var b) = pair`
    Some((designation, NodeKind::SingleVariableDesignation)) => {
      let destructured = std::iter::successors(around(tree, designation), |it| around(tree, it.0))
        .take_while(|it| {
          matches!(
            it.1,
            NodeKind::ParenthesizedVariableDesignation | NodeKind::DeclarationExpression | NodeKind::Argument | NodeKind::TupleExpression
          )
        })
        .any(|it| matches!(it.1, NodeKind::ParenthesizedVariableDesignation | NodeKind::TupleExpression));
      if destructured { Site::Destructured } else { Site::Plain }
    }
    // `x is total`: the constant, unless a type of the name is declared where no file read here shows it
    Some((name, NodeKind::IdentifierName)) if local.is_some_and(|(_, binding)| binding.kind.is_value()) && is_type_or_constant(tree, name) => Site::Namesake,
    Some((name, NodeKind::IdentifierName)) => name_site(tree, name),
    _ => Site::Plain,
  }
}

/// Whether a name is all of the right side of an `is` (`x is Max`): C# reads a type there when one of the name is in
/// sight, else a constant, a constant pattern
fn is_type_or_constant(tree: &Tree, name: NodeId) -> bool {
  tree.node_kind(name) == Some(NodeKind::IdentifierName)
    && around(tree, name).is_some_and(|(owner, kind)| kind == NodeKind::IsExpression && sg::binary_expression::right(tree, owner) == Some(name))
}

/// The site of an `IdentifierName` by what it is a part of
fn name_site(tree: &Tree, name: NodeId) -> Site {
  let Some((owner, kind)) = around(tree, name) else { return Site::Plain };
  let first = || tree.composite_children(owner).next() == Some(name);
  match kind {
    NodeKind::GotoStatement => return Site::Label,
    NodeKind::NameColon => return Site::NamedArgument,
    // `new { total }`: the member has the name of the expression
    NodeKind::AnonymousObjectMemberDeclarator => return Site::Shorthand,
    // `(total, 1)`: the element has the name of the expression, unless a name is written for it
    NodeKind::Argument if around(tree, owner).is_some_and(|it| it.1 == NodeKind::TupleExpression) && first() => return Site::Shorthand,
    _ => {}
  }
  // `nameof(total)` is the text "total"
  let spelled = std::iter::successors(Some((owner, kind)), |it| around(tree, it.0))
    .take_while(|it| !is_scope(it.1))
    .filter(|it| it.1 == NodeKind::InvocationExpression)
    .any(|it| {
      sg::invocation_expression::expression(tree, it.0)
        .filter(|&callee| tree.node_kind(callee) == Some(NodeKind::IdentifierName) && callee != name)
        .and_then(|callee| sg::identifier_of(tree, callee))
        .is_some_and(|callee| value_of(tree, callee) == "nameof")
    });
  if spelled { Site::Spelled } else { Site::Plain }
}

/// Where the text that writes a name starts: after the `@` of a verbatim identifier, at the token itself where an
/// escape makes the value bytes the source does not hold (`C\u733f`)
fn name_start(tree: &Tree, leaf: NodeId) -> usize {
  read::word_at(tree, leaf).map_or_else(|| tree.range(leaf).start, |(_, at)| at)
}

/// Whether a node lies in a structured trivia: a documentation comment, a directive, tokens the parser skipped
fn in_structured_trivia(tree: &Tree, node: NodeId) -> bool {
  std::iter::successors(tree.parent(node), |&it| tree.parent(it)).any(|it| tree.is_trivia(it))
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
    // a named argument names a parameter of the callee; `Name = value` of an anonymous type, an attribute or a `using`
    // the member or the alias it makes: no name of the scopes
    Some(NodeKind::NameColon | NodeKind::NameEquals) => (ReferenceKind::Member, None),
    // `new Box { Total = total }`, `box with { Total = 1 }`: the left side is a member of what is made
    Some(kind)
      if facts::is_assignment_expression(kind.syntax_kind())
        && tree.composite_children(owner).next() == Some(name)
        && around(tree, owner).is_some_and(|it| matches!(it.1, NodeKind::ObjectInitializerExpression | NodeKind::WithInitializerExpression)) =>
    {
      (ReferenceKind::Member, None)
    }
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

/// A node of a kind whose declarations no name outside it sees, as far as its kind tells: where the access of a name
/// and the argument of a `nameof` stop looking around them (`opens_scope` has the scopes of the walk)
const fn is_scope(kind: NodeKind) -> bool {
  is_type_declaration(kind) || is_code(kind) || is_block_scope(kind)
}

/// Whether a node opens a scope of the walk, as C# scopes its locals: a type, code that runs (but a top-level
/// statement: those of a file share its scope), a block, a loop, a `using`, a `fixed`, a `catch`, a section (for the
/// variables of its labels) and an arm of a `switch`, the `switch` (for the statements of its sections), a query and
/// each expression of it that is a lambda's body, the initializer of a field or a property and a body after `=>`,
/// and a statement that is the body of another without a block (`if (c) Use(out var x);`). An `if` and a `lock` open
/// none: what their expressions declare is of the statements around them
fn opens_scope(tree: &Tree, node: NodeId, kind: NodeKind) -> bool {
  is_type_declaration(kind)
    || (is_code(kind) && kind != NodeKind::GlobalStatement)
    || is_block_scope(kind)
    || matches!(kind, NodeKind::SwitchStatement | NodeKind::WhileStatement | NodeKind::DoStatement)
    || is_member_initializer(tree, node, kind)
    || is_query_lambda(tree, node)
    || around(tree, node).is_some_and(|(owner, owner_kind)| embedded_statement(tree, owner, owner_kind) == Some(node))
}

/// A value after `=` and a body after `=>`: code whose declarations are locals, also where a field, a property or an
/// event has it and no member with a body is around
const fn is_initializer(kind: NodeKind) -> bool {
  matches!(kind, NodeKind::EqualsValueClause | NodeKind::ArrowExpressionClause)
}

/// Whether a node is the initializer of a field, an event or a property, or the body of a member after `=>`: what
/// its expression declares ends with it. The value of a local is no scope: what it declares is of the statements
/// around its declaration
fn is_member_initializer(tree: &Tree, node: NodeId, kind: NodeKind) -> bool {
  let owner = |node: NodeId| around(tree, node).map(|it| it.1);
  match kind {
    NodeKind::ArrowExpressionClause => true,
    NodeKind::EqualsValueClause => match around(tree, node) {
      Some((_, NodeKind::PropertyDeclaration)) => true,
      Some((declarator, NodeKind::VariableDeclarator)) => around(tree, declarator)
        .and_then(|it| owner(it.0))
        .is_some_and(|it| matches!(it, NodeKind::FieldDeclaration | NodeKind::EventFieldDeclaration)),
      _ => false,
    },
    _ => false,
  }
}

/// Whether a node is an expression of a query that C# reads as the body of a lambda of its own, so that what it
/// declares ends with it: the source of a `from` after the first, the value of a `let`, the condition of a `where`,
/// a key of a `join` or of an `orderby`, what a `select` or a `group … by` gives. The source of the first `from` and
/// of a `join` is read outside those lambdas (`in_source_of_query`)
fn is_query_lambda(tree: &Tree, node: NodeId) -> bool {
  let Some((clause, kind)) = around(tree, node) else { return false };
  let is = |part: Option<NodeId>| part == Some(node);
  match kind {
    NodeKind::FromClause => around(tree, clause).is_some_and(|it| it.1 == NodeKind::QueryBody) && is(sg::from_clause::expression(tree, clause)),
    NodeKind::LetClause => is(sg::let_clause::expression(tree, clause)),
    NodeKind::WhereClause => is(sg::where_clause::condition(tree, clause)),
    NodeKind::JoinClause => is(sg::join_clause::left_expression(tree, clause)) || is(sg::join_clause::right_expression(tree, clause)),
    NodeKind::AscendingOrdering | NodeKind::DescendingOrdering => is(sg::ordering::expression(tree, clause)),
    NodeKind::SelectClause => is(sg::select_clause::expression(tree, clause)),
    NodeKind::GroupClause => is(sg::group_clause::group_expression(tree, clause)) || is(sg::group_clause::by_expression(tree, clause)),
    _ => false,
  }
}

/// Whether a node of the query `query`, with no scope between them, stands in a source C# reads outside the lambdas
/// of the query: the source of its first `from`, or of a `join`
fn in_source_of_query(tree: &Tree, query: NodeId, node: NodeId) -> bool {
  let mut part = node;
  while let Some((owner, kind)) = around(tree, part) {
    let source = match kind {
      NodeKind::FromClause if tree.parent(owner) == Some(query) => sg::from_clause::expression(tree, owner),
      NodeKind::JoinClause => sg::join_clause::in_expression(tree, owner),
      _ => None,
    };
    if source == Some(part) {
      return true;
    }
    if owner == query {
      return false;
    }
    part = owner;
  }
  false
}

/// The statement that is the body of a statement: a scope of its own when it is no block
fn embedded_statement(tree: &Tree, statement: NodeId, kind: NodeKind) -> Option<NodeId> {
  match kind {
    NodeKind::IfStatement => sg::if_statement::statement(tree, statement),
    NodeKind::ElseClause => sg::else_clause::statement(tree, statement),
    NodeKind::WhileStatement => sg::while_statement::statement(tree, statement),
    NodeKind::DoStatement => sg::do_statement::statement(tree, statement),
    NodeKind::ForStatement => sg::for_statement::statement(tree, statement),
    NodeKind::ForEachStatement | NodeKind::ForEachVariableStatement => sg::common_for_each_statement::statement(tree, statement),
    NodeKind::UsingStatement => sg::using_statement::statement(tree, statement),
    NodeKind::FixedStatement => sg::fixed_statement::statement(tree, statement),
    NodeKind::LockStatement => sg::lock_statement::statement(tree, statement),
    _ => None,
  }
}

/// The scopes around a node, the innermost first
fn scopes_around(tree: &Tree, node: NodeId) -> impl Iterator<Item = (NodeId, NodeKind)> + '_ {
  std::iter::successors(around(tree, node), |it| around(tree, it.0)).filter(|&(scope, kind)| opens_scope(tree, scope, kind))
}

/// A node where C# reads a type: a name there is of a type or a type parameter
const fn declares_type(kind: NodeKind) -> bool {
  is_type_declaration(kind) || matches!(kind, NodeKind::TypeParameter | NodeKind::DelegateDeclaration)
}

/// Whether the name `leaf` sees the range variable `declared` of the query `query`, as C# scopes the clauses: the
/// source of a `from`, the value of a `let` and the left key of a `join` are read before their own variable, the
/// source of a `join` sees no variable of its query (C# reads it outside the query's lambdas), its right key the
/// variable of the `join` alone, the variable of a `join … into` ends with its clause, and an `into` ends every
/// variable before it
fn query_sees(tree: &Tree, query: NodeId, declared: NodeId, leaf: NodeId) -> bool {
  let Some((clause, kind)) = around(tree, declared) else { return true };
  let within = |part: Option<NodeId>| part.is_some_and(|it| is_within(tree, leaf, it));
  let own = match kind {
    NodeKind::FromClause | NodeKind::LetClause => !within(Some(clause)),
    NodeKind::JoinClause if within(Some(clause)) => within(sg::join_clause::right_expression(tree, clause)),
    NodeKind::JoinClause => sg::join_clause::into(tree, clause).is_none(),
    NodeKind::JoinIntoClause => !within(tree.parent(clause)),
    _ => true,
  };
  if !own {
    return false;
  }
  // where the name stands in this query: the nodes from it up to the query, from the query inside it that holds the
  // name when there is one, whose clauses are its own
  let path: Vec<NodeId> = std::iter::successors(tree.parent(leaf), |&it| tree.parent(it)).take_while(|&it| it != query).collect();
  let from = path.iter().rposition(|&it| tree.node_kind(it) == Some(NodeKind::QueryExpression)).unwrap_or(0);
  let ended = path[from..].iter().enumerate().any(|(index, &node)| match tree.node_kind(node) {
    Some(NodeKind::QueryContinuation) => !is_within(tree, clause, node),
    Some(NodeKind::JoinClause) => (from + index).checked_sub(1).is_some_and(|below| {
      let part = Some(path[below]);
      sg::join_clause::in_expression(tree, node) == part || (node != clause && sg::join_clause::right_expression(tree, node) == part)
    }),
    _ => false,
  });
  !ended
}

/// The scopes of C# for `psi_navigate::resolve`
struct CSharpScopes<'t> {
  tree: &'t Tree,
  is_named: IsNamed<'t>,
}

impl Scoping<csharp_syntax::CSharp> for CSharpScopes<'_> {
  fn skips(&self, node: NodeId) -> bool {
    self.tree.is_trivia(node)
  }

  fn binding(&mut self, leaf: NodeId) -> Binding {
    if is_label_use(self.tree, leaf) {
      Binding::Bound(label_of(self.tree, leaf, self.is_named))
    } else {
      Binding::ByScopes
    }
  }

  fn declared(&self, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    // a `foreach` declares its variable in its own scope, not in the block around it (`hoisted`)
    let declares = declares(kind) && !matches!(kind, NodeKind::LabeledStatement | NodeKind::ForEachStatement) && !is_hoisted(self.tree, node, kind);
    sg::identifier_of(self.tree, node).filter(|&it| declares && (self.is_named)(it))
  }

  /// A statement of a section of a `switch` declares in the block of the `switch`, which its sections share; the
  /// expression a `switch` is over, and the source of the first `from` or of a `join` of a query, in the scope
  /// around the `switch` or the query, the file's where no other is
  fn declares_in(&self, node: NodeId) -> Option<NodeId> {
    let tree = self.tree;
    let mut scopes = scopes_around(tree, node);
    let innermost = scopes.next()?;
    let mut outer = || scopes.next().map_or_else(|| tree.root(), |it| it.0);
    match innermost {
      (section, NodeKind::SwitchSection) => {
        let in_label = sg::switch_section::labels(tree, section).any(|it| is_within(tree, node, it));
        tree.parent(section).filter(|_| !in_label)
      }
      (switch, NodeKind::SwitchStatement) => {
        let governs = sg::switch_statement::expression(tree, switch).is_some_and(|it| is_within(tree, node, it));
        governs.then(&mut outer)
      }
      (query, NodeKind::QueryExpression) => in_source_of_query(tree, query, node).then(&mut outer),
      _ => None,
    }
  }

  fn is_scope(&self, node: NodeId, kind: NodeKind) -> bool {
    opens_scope(self.tree, node, kind)
  }

  /// The right side of `x is Max` is a constant where the scopes declare no type of the name
  fn else_reads(&self, leaf: NodeId, kind: ReferenceKind) -> Option<ReferenceKind> {
    let name = self.tree.parent(leaf)?;
    (kind == ReferenceKind::Type && is_type_or_constant(self.tree, name)).then_some(ReferenceKind::Value)
  }

  /// A name where C# reads a type is of a type or a type parameter. The collection of a `foreach` is read before its
  /// variables are, a query reads its clauses in their order (`query_sees`), and the body of a `for` sees nothing its
  /// incrementors declare
  fn sees(&self, scope: NodeId, declared: NodeId, leaf: NodeId, kind: ReferenceKind) -> bool {
    let tree = self.tree;
    if kind == ReferenceKind::Type && !around(tree, declared).is_some_and(|it| declares_type(it.1)) {
      return false;
    }
    match tree.node_kind(scope) {
      // what an incrementor of a `for` declares is seen by its incrementors alone, not by its body
      Some(NodeKind::ForStatement) => {
        let in_incrementor = |node: NodeId| sg::for_statement::incrementors(tree, scope).any(|it| is_within(tree, node, it));
        !in_incrementor(declared) || in_incrementor(leaf)
      }
      Some(NodeKind::ForEachStatement | NodeKind::ForEachVariableStatement) => {
        let collection = sg::common_for_each_statement::expression(tree, scope);
        // what the collection itself declares (`Next(out var count)`) is seen in it
        !collection.is_some_and(|it| is_within(tree, leaf, it) && !is_within(tree, declared, it))
      }
      Some(NodeKind::QueryExpression) => query_sees(tree, scope, declared, leaf),
      _ => true,
    }
  }
}

/// The names of the declarations a scope sees wherever they are: the members of a type (a field's declarators), its
/// type parameters and its primary constructor's parameters; the types of a file or a namespace; the type parameters
/// and parameters of a member, a local function or a lambda; the local functions of a block, of the top-level
/// statements of a file and of the sections of a `switch`; the variable of a `foreach`, which is its own scope
fn hoisted(tree: &Tree, scope: NodeId, is_named: &dyn Fn(NodeId) -> bool) -> Vec<NodeId> {
  let mut declared = Vec::new();
  let mut push = |declaration: NodeId| {
    if let Some(name) = sg::identifier_of(tree, declaration).filter(|&it| is_named(it)) {
      declared.push(name);
    }
  };
  if tree.node_kind(scope) == Some(NodeKind::ForEachStatement) {
    push(scope);
  }
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
      // the statements of a file and of the sections of a `switch` share a scope
      NodeKind::GlobalStatement => sg::global_statement::statement(tree, child)
        .filter(|&it| tree.node_kind(it) == Some(NodeKind::LocalFunctionStatement))
        .into_iter()
        .for_each(&mut push),
      NodeKind::SwitchSection if tree.node_kind(scope) == Some(NodeKind::SwitchStatement) => sg::switch_section::statements(tree, child)
        .filter(|&it| tree.node_kind(it) == Some(NodeKind::LocalFunctionStatement))
        .for_each(&mut push),
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
  tree.declaration_end(node)
}

#[cfg(test)]
#[path = "navigate_test.rs"]
mod tests;
