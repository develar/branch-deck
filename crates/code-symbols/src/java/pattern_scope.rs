//! Where Java sees the variable of a pattern of an `instanceof` (JLS 6.3,
//! openspec/specs/commit/symbol-navigation/spec.md): where the test is known to have matched, which no scope of nodes
//! tells. `if (o instanceof String s) … else …` sees `s` in its first branch alone, `if (!(o instanceof String s))
//! return;` in the statements after it.
//!
//! - **`PatternScope`**, for the navigation (`java::navigate`), which asks of every name whether it sees the
//!   variable: from the test up, the operands after it of an `&&` it is true in and of an `||` it is false in (`!`
//!   swaps, parentheses pass), the operand of a `? :` and the branch of an `if` of its outcome, the body of a `while`
//!   and the update and the body of a `for` it is true in, what a `case` it guards runs; and where a statement
//!   introduces the variable (an `if` whose other branch cannot complete normally, a loop that ends only by its
//!   condition, the label of either), the statements after it in its block, up to the next label of a `switch`. No
//!   other expression or statement passes a variable on.
//! - **`is_pattern_scope`**, for a walk that keeps a stack of scopes (the highlighting, the frames of the
//!   navigation): the nodes that end the pattern variables declared inside them.
//! - **`completes_normally`** and **`jumps_to`** read JLS 14.22 from the syntax: every statement is taken as
//!   reachable, a condition is constant only as the literal `true`, and a `switch` is exhaustive by a `default`, a
//!   pattern or a `null` among its labels.

use super::highlight::is_class;
use crate::psi_navigate::is_within;
use java_syntax::psi;
use java_syntax::{NodeId, NodeKind, TokenKind, Tree};

/// A node whose pattern variables (`s` of `o instanceof String s`) no name after it sees (JLS 6.3.1): an `if`, a
/// `while` or a `do` whose condition introduces none past the statement, a `? :`, an expression statement, a `return`,
/// a `yield`, a `throw`, an `assert` and a field. A pattern variable seen past its statement
/// (`if (!(o instanceof String s)) return;`) belongs to the block around, as a local declared there.
pub(super) fn is_pattern_scope(tree: &Tree, node: NodeId, kind: NodeKind) -> bool {
  match kind {
    NodeKind::IF_STATEMENT | NodeKind::WHILE_STATEMENT | NodeKind::DO_WHILE_STATEMENT => !introduces_past(tree, node),
    NodeKind::CONDITIONAL_EXPRESSION
    | NodeKind::EXPRESSION_STATEMENT
    | NodeKind::RETURN_STATEMENT
    | NodeKind::YIELD_STATEMENT
    | NodeKind::THROW_STATEMENT
    | NodeKind::ASSERT_STATEMENT
    | NodeKind::FIELD => true,
    _ => false,
  }
}

/// Whether an `if`, a `while` or a `do` introduces a pattern variable of its condition to the statements after it
/// (`PatternScope::after`)
fn introduces_past(tree: &Tree, statement: NodeId) -> bool {
  let Some(condition) = child_after(tree, statement, TokenKind::LPARENTH) else {
    return false;
  };
  let after = labeled(tree, statement);
  std::iter::once(condition)
    .chain(tree.descendants(condition))
    .filter(|&it| tree.node_kind(it) == Some(NodeKind::INSTANCE_OF_EXPRESSION) && declares_variable(tree, it))
    .any(|test| PatternScope::of(tree, test).after == Some(after))
}

/// Whether the pattern of an `instanceof` declares a variable
fn declares_variable(tree: &Tree, test: NodeId) -> bool {
  psi::instance_of_expression::get_pattern(tree, test).is_some_and(|pattern| {
    std::iter::once(pattern)
      .chain(tree.descendants(pattern))
      .any(|it| matches!(tree.node_kind(it), Some(NodeKind::PATTERN_VARIABLE | NodeKind::DECONSTRUCTION_PATTERN_VARIABLE)))
  })
}

/// The `instanceof` whose pattern holds the declaration of a pattern variable; none for the variable of a `case` or of
/// a `for`
pub(super) fn test_of(tree: &Tree, declaration: NodeId) -> Option<NodeId> {
  let test = std::iter::successors(tree.parent(declaration), |&it| tree.parent(it)).find(|&it| tree.node_kind(it) == Some(NodeKind::INSTANCE_OF_EXPRESSION))?;
  psi::instance_of_expression::get_pattern(tree, test)
    .filter(|&pattern| is_within(tree, declaration, pattern))
    .map(|_| test)
}

/// Where the variables of the pattern of an `instanceof` are in scope
pub(super) struct PatternScope {
  /// The nodes whose names see them: the operands after the test, a branch, a body
  parts: Vec<NodeId>,
  /// The statement of a block that introduces them to the statements after it, up to the next label of a `switch`:
  /// an `if` or a loop (the outermost label of it), or the label of a `case` whose guard holds the test
  pub(super) after: Option<NodeId>,
}

impl PatternScope {
  /// The scope of the variables of the test `test`, an `INSTANCE_OF_EXPRESSION`: from the test up while the node
  /// around tells what the test was (JLS 6.3.1, 6.3.2)
  pub(super) fn of(tree: &Tree, test: NodeId) -> Self {
    let mut scope = Self { parts: Vec::new(), after: None };
    // the variables are introduced when `node` is `when`
    let (mut node, mut when) = (test, true);
    while let Some((parent, kind)) = tree.parent(node).and_then(|it| Some((it, tree.node_kind(it)?))) {
      let rest = || std::iter::successors(tree.next_sibling(node), |&it| tree.next_sibling(it)).filter(|&it| !tree.is_leaf(it));
      let is_condition = || child_after(tree, parent, TokenKind::LPARENTH) == Some(node);
      match kind {
        NodeKind::PARENTH_EXPRESSION => {}
        NodeKind::PREFIX_EXPRESSION if has_token(tree, parent, TokenKind::EXCL) => when = !when,
        // `a && b` when `a` is true, `a || b` when `a` is false: in `b`, and the whole tells the same
        NodeKind::BINARY_EXPRESSION | NodeKind::POLYADIC_EXPRESSION if has_token(tree, parent, if when { TokenKind::ANDAND } else { TokenKind::OROR }) => {
          scope.parts.extend(rest());
        }
        NodeKind::CONDITIONAL_EXPRESSION => {
          if tree.composite_children(parent).next() == Some(node) {
            scope.parts.extend(child_after(tree, parent, if when { TokenKind::QUEST } else { TokenKind::COLON }));
          }
          break;
        }
        NodeKind::IF_STATEMENT if is_condition() => {
          let (then, otherwise) = (child_after(tree, parent, TokenKind::RPARENTH), child_after(tree, parent, TokenKind::ELSE_KEYWORD));
          scope.parts.extend(if when { then } else { otherwise });
          let completes = |branch: Option<NodeId>| branch.is_none_or(|it| completes_normally(tree, it));
          // the branch the variables are not in leaves, and the other does not (JLS 6.3.2.2)
          let introduces = match otherwise {
            None => !when && !completes(then),
            Some(_) if when => completes(then) && !completes(otherwise),
            Some(_) => !completes(then) && completes(otherwise),
          };
          if introduces {
            scope.introduce(tree, parent);
          }
          break;
        }
        NodeKind::WHILE_STATEMENT if is_condition() => {
          if when {
            scope.parts.extend(child_after(tree, parent, TokenKind::RPARENTH));
          } else if !jumps_to(tree, parent, NodeKind::BREAK_STATEMENT) {
            scope.introduce(tree, parent);
          }
          break;
        }
        NodeKind::DO_WHILE_STATEMENT if is_condition() => {
          if !when && !jumps_to(tree, parent, NodeKind::BREAK_STATEMENT) {
            scope.introduce(tree, parent);
          }
          break;
        }
        // an expression that is a child of a `for` is its condition: what it starts with and updates by are statements
        NodeKind::FOR_STATEMENT => {
          if when {
            scope.parts.extend(rest());
          } else if !jumps_to(tree, parent, NodeKind::BREAK_STATEMENT) {
            scope.introduce(tree, parent);
          }
          break;
        }
        // the guard of a `case`: what the rule runs, the statements of the label
        NodeKind::SWITCH_LABELED_RULE if when && is_guard(tree, parent, node) => {
          scope.parts.extend(rest());
          break;
        }
        NodeKind::SWITCH_LABEL_STATEMENT if when && is_guard(tree, parent, node) => {
          scope.after = Some(parent);
          break;
        }
        _ => break,
      }
      node = parent;
    }
    scope
  }

  /// The statements after `statement` see the variables, where it is a statement of a block (JLS 6.3.2.1, 6.3.2.6),
  /// under its labels or not (6.3.2.7)
  fn introduce(&mut self, tree: &Tree, statement: NodeId) {
    let statement = labeled(tree, statement);
    let in_block = tree.parent(statement).is_some_and(|it| tree.node_kind(it) == Some(NodeKind::CODE_BLOCK));
    self.after = in_block.then_some(statement);
  }

  /// Whether the name `leaf` sees the variables
  pub(super) fn holds(&self, tree: &Tree, leaf: NodeId) -> bool {
    self.parts.iter().any(|&part| is_within(tree, leaf, part)) || self.after.is_some_and(|after| follows(tree, after, leaf))
  }
}

/// Whether `leaf` stands in a statement after `statement` in its block, before the next label of a `switch`
fn follows(tree: &Tree, statement: NodeId, leaf: NodeId) -> bool {
  let block = tree.parent(statement);
  let Some(at) = std::iter::successors(Some(leaf), |&it| tree.parent(it)).find(|&it| tree.parent(it) == block) else {
    return false;
  };
  for sibling in std::iter::successors(tree.next_sibling(statement), |&it| tree.next_sibling(it)) {
    if tree.node_kind(sibling) == Some(NodeKind::SWITCH_LABEL_STATEMENT) {
      return false;
    }
    if sibling == at {
      return true;
    }
  }
  false
}

/// Whether `expression` is the guard of the label or the rule `label`: its expression after `when`
fn is_guard(tree: &Tree, label: NodeId, expression: NodeId) -> bool {
  child_after(tree, label, TokenKind::WHEN_KEYWORD) == Some(expression)
}

/// A statement with the labels right around it: the outermost `LABELED_STATEMENT` of it, else itself
fn labeled(tree: &Tree, statement: NodeId) -> NodeId {
  let label = |it: &NodeId| tree.parent(*it).filter(|&it| tree.node_kind(it) == Some(NodeKind::LABELED_STATEMENT));
  std::iter::successors(Some(statement), label).last().unwrap_or(statement)
}

/// Whether a statement can complete normally (JLS 14.22), as far as its syntax tells: not a `return`, a `throw`, a
/// `break`, a `continue` or a `yield`; a block and a `synchronized` by their last statement; an `if` with an `else`
/// by either branch; a labeled statement by its statement or a `break` to it; a `try` by its block or a `catch`,
/// unless its `finally` cannot; a loop of the condition `true` or of none only by a `break` of it; a `do` by its
/// body or a `continue` of it too; an exhaustive `switch` by its last statement, a rule that is an expression or a
/// block that can, or a `break` of it. Every other statement can
fn completes_normally(tree: &Tree, statement: NodeId) -> bool {
  let mut pending = vec![statement];
  while let Some(statement) = pending.pop() {
    let Some(kind) = tree.node_kind(statement) else { return true };
    let breaks = || jumps_to(tree, statement, NodeKind::BREAK_STATEMENT);
    match kind {
      NodeKind::RETURN_STATEMENT | NodeKind::THROW_STATEMENT | NodeKind::BREAK_STATEMENT | NodeKind::CONTINUE_STATEMENT | NodeKind::YIELD_STATEMENT => {}
      NodeKind::BLOCK_STATEMENT | NodeKind::SYNCHRONIZED_STATEMENT | NodeKind::CODE_BLOCK => {
        let block = if kind == NodeKind::CODE_BLOCK {
          Some(statement)
        } else {
          tree.first_child_of_kind(statement, NodeKind::CODE_BLOCK)
        };
        match block.and_then(|it| statements(tree, it).last()) {
          Some(last) => pending.push(last),
          None => return true,
        }
      }
      NodeKind::IF_STATEMENT => match (child_after(tree, statement, TokenKind::RPARENTH), child_after(tree, statement, TokenKind::ELSE_KEYWORD)) {
        (Some(then), Some(otherwise)) => pending.extend([then, otherwise]),
        _ => return true,
      },
      NodeKind::LABELED_STATEMENT => match statements(tree, statement).last() {
        Some(inner) if !breaks() => pending.push(inner),
        _ => return true,
      },
      NodeKind::TRY_STATEMENT => {
        // a `finally` that cannot complete ends the statement whatever its blocks do; the depth of this call is the
        // nesting of such blocks, not of the code
        let finally = child_after(tree, statement, TokenKind::FINALLY_KEYWORD);
        if finally.is_none_or(|it| completes_normally(tree, it)) {
          let catches = tree.children(statement).filter(|&it| tree.node_kind(it) == Some(NodeKind::CATCH_SECTION));
          pending.extend(tree.first_child_of_kind(statement, NodeKind::CODE_BLOCK));
          pending.extend(catches.filter_map(|it| tree.first_child_of_kind(it, NodeKind::CODE_BLOCK)));
        }
      }
      NodeKind::WHILE_STATEMENT => {
        if !child_after(tree, statement, TokenKind::LPARENTH).is_some_and(|it| is_true(tree, it)) || breaks() {
          return true;
        }
      }
      NodeKind::FOR_STATEMENT => {
        // the condition is the second node before the `;` of the `for`, after what it starts with
        let condition = tree
          .children(statement)
          .take_while(|&it| tree.token_kind(it) != Some(TokenKind::SEMICOLON))
          .filter(|&it| !tree.is_leaf(it))
          .nth(1);
        if condition.is_some_and(|it| !is_true(tree, it)) || breaks() {
          return true;
        }
      }
      NodeKind::DO_WHILE_STATEMENT => {
        if breaks() {
          return true;
        }
        if !child_after(tree, statement, TokenKind::LPARENTH).is_some_and(|it| is_true(tree, it)) {
          match child_after(tree, statement, TokenKind::DO_KEYWORD) {
            Some(body) if !jumps_to(tree, statement, NodeKind::CONTINUE_STATEMENT) => pending.push(body),
            _ => return true,
          }
        }
      }
      NodeKind::SWITCH_STATEMENT => {
        let parts: Vec<NodeId> = tree
          .first_child_of_kind(statement, NodeKind::CODE_BLOCK)
          .map_or_else(Vec::new, |it| statements(tree, it).collect());
        if !parts.iter().any(|&it| is_exhaustive_label(tree, it)) || breaks() {
          return true;
        }
        let rules = parts.iter().filter(|&&it| tree.node_kind(it) == Some(NodeKind::SWITCH_LABELED_RULE));
        let mut bodies = rules.filter_map(|&it| statements(tree, it).last()).peekable();
        if bodies.peek().is_some() {
          // a rule that is an expression completes; a block may, a `throw` does not
          for body in bodies {
            if tree.node_kind(body) == Some(NodeKind::EXPRESSION_STATEMENT) {
              return true;
            }
            pending.push(body);
          }
        } else {
          match parts.last() {
            Some(&last) if tree.node_kind(last) != Some(NodeKind::SWITCH_LABEL_STATEMENT) => pending.push(last),
            _ => return true,
          }
        }
      }
      _ => return true,
    }
  }
  false
}

/// The nodes of a block, a labeled statement or a rule that are no token and no comment: its statements
fn statements(tree: &Tree, block: NodeId) -> impl Iterator<Item = NodeId> + '_ {
  tree.children(block).filter(|&it| !tree.is_leaf(it) && !tree.is_doc_comment(it))
}

/// Whether a condition is the literal `true`, in parentheses or not
fn is_true(tree: &Tree, condition: NodeId) -> bool {
  let mut expression = condition;
  while tree.node_kind(expression) == Some(NodeKind::PARENTH_EXPRESSION) {
    match tree.composite_children(expression).next() {
      Some(inner) => expression = inner,
      None => return false,
    }
  }
  tree.node_kind(expression) == Some(NodeKind::LITERAL_EXPRESSION) && has_token(tree, expression, TokenKind::TRUE_KEYWORD)
}

/// Whether a label or a rule of a `switch` makes it exhaustive, as the language demands of a `switch` with it: a
/// `default`, a pattern or a `null`
fn is_exhaustive_label(tree: &Tree, label: NodeId) -> bool {
  if !matches!(tree.node_kind(label), Some(NodeKind::SWITCH_LABEL_STATEMENT | NodeKind::SWITCH_LABELED_RULE)) {
    return false;
  }
  let elements = tree.first_child_of_kind(label, NodeKind::CASE_LABEL_ELEMENT_LIST);
  has_token(tree, label, TokenKind::DEFAULT_KEYWORD)
    || elements.is_some_and(|list| {
      tree.composite_children(list).any(|it| match tree.node_kind(it) {
        Some(NodeKind::DEFAULT_CASE_LABEL_ELEMENT | NodeKind::TYPE_TEST_PATTERN | NodeKind::DECONSTRUCTION_PATTERN) => true,
        Some(NodeKind::LITERAL_EXPRESSION) => has_token(tree, it, TokenKind::NULL_KEYWORD),
        _ => false,
      })
    })
}

/// Whether a `break` or a `continue` (`jump`) inside a statement is of that statement: one without a label whose
/// innermost loop, or `switch` for a `break`, is the statement, or one with a label that stands right around the
/// statement. A lambda and a class inside it have their own
fn jumps_to(tree: &Tree, target: NodeId, jump: NodeKind) -> bool {
  let is_labeled = |it: &NodeId| tree.node_kind(*it) == Some(NodeKind::LABELED_STATEMENT);
  let name = |it: NodeId| tree.children(it).find(|&it| tree.token_kind(it) == Some(TokenKind::IDENTIFIER)).map(|it| tree.text(it));
  let innermost = Some(target).filter(is_labeled).or_else(|| tree.parent(target).filter(is_labeled));
  let labels: Vec<&[u8]> = std::iter::successors(innermost, |&it| tree.parent(it).filter(is_labeled)).filter_map(name).collect();
  let takes = |kind: NodeKind| {
    matches!(
      kind,
      NodeKind::WHILE_STATEMENT | NodeKind::DO_WHILE_STATEMENT | NodeKind::FOR_STATEMENT | NodeKind::FOREACH_STATEMENT | NodeKind::FOREACH_PATTERN_STATEMENT
    ) || kind == NodeKind::SWITCH_EXPRESSION
      || (jump == NodeKind::BREAK_STATEMENT && kind == NodeKind::SWITCH_STATEMENT)
  };
  tree
    .descendants_pruned(target, |it| tree.node_kind(it).is_some_and(|kind| is_class(kind) || kind == NodeKind::LAMBDA_EXPRESSION))
    .filter(|&it| tree.node_kind(it) == Some(jump))
    .any(|it| match name(it) {
      Some(label) => labels.contains(&label),
      None => std::iter::successors(tree.parent(it), |&it| tree.parent(it)).find(|&it| tree.node_kind(it).is_some_and(takes)) == Some(target),
    })
}

/// The first composite child of a node after its child token `token`: the condition after `(`, the branch after `)`
/// or `else`
fn child_after(tree: &Tree, node: NodeId, token: TokenKind) -> Option<NodeId> {
  tree.children(node).skip_while(|&it| tree.token_kind(it) != Some(token)).find(|&it| !tree.is_leaf(it))
}

fn has_token(tree: &Tree, node: NodeId, token: TokenKind) -> bool {
  tree.children(node).any(|it| tree.token_kind(it) == Some(token))
}
