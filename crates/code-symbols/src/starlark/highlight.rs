//! The highlighting of a Starlark file from its PSI, as the Bazel plugin colours it without resolving names: the
//! lexer's colours (`StarlarkSyntaxHighlighter`: keywords, strings, numbers, comments) and the semantic ones of a leaf
//! by its parent (`starlarkSemanticHighlightingColor`): a named argument's name and its `=`, the name of a `def`, and
//! the callee of a top-level call of a BUILD or MODULE file, a declaration. One pass over the leaves of the tree, which
//! tile the text.

use crate::highlight::lines::LineRuns;
use crate::highlight::{COMMENT, Role};
use starlark_syntax::{NodeId, NodeKind, TokenKind, Tree};

/// The highlighting of a Starlark text (its size within `highlight::MAX_BYTES`); `top_level_calls_declare`: a BUILD or
/// MODULE file (`Grammar::StarlarkBuild`). Any bytes parse, so it always has one
pub(crate) fn highlight(text: &str, top_level_calls_declare: bool) -> LineRuns {
  let tree = super::parse(text.as_bytes());
  let mut out = LineRuns::builder(text);
  let len = u32::try_from(tree.len()).unwrap_or(u32::MAX);
  for leaf in (0..len).map(NodeId).filter(|&id| tree.is_leaf(id)) {
    let range = tree.range(leaf);
    out.push(range.start, 0);
    out.push(range.end, style(&tree, leaf, top_level_calls_declare));
  }
  out.finish()
}

/// The style of a leaf: its token's colour, or the semantic one its parent gives it
fn style(tree: &Tree, leaf: NodeId, top_level_calls_declare: bool) -> u32 {
  let Some(token) = tree.token_kind(leaf) else { return 0 };
  let parent = tree.parent(leaf).and_then(|parent| tree.node_kind(parent).map(|kind| (parent, kind)));
  match token {
    TokenKind::IDENTIFIER => match parent {
      Some((_, NodeKind::NAMED_ARGUMENT_EXPRESSION)) => Role::NamedArgument.style(),
      Some((_, NodeKind::FUNCTION_DECLARATION)) => Role::FunctionDeclaration.style(),
      Some((reference, NodeKind::REFERENCE_EXPRESSION)) if top_level_calls_declare && is_top_level_callee(tree, reference) => Role::FunctionDeclaration.style(),
      _ => 0,
    },
    TokenKind::EQ if matches!(parent, Some((_, NodeKind::NAMED_ARGUMENT_EXPRESSION))) => Role::NamedArgument.style(),
    TokenKind::COMMENT => COMMENT,
    TokenKind::STRING | TokenKind::BYTES => Role::String.style(),
    TokenKind::INT | TokenKind::FLOAT => Role::Number.style(),
    // `StarlarkSyntaxHighlighter`'s keywords: not every word the lexer reserves (`class`, `while`), which it leaves plain
    TokenKind::DEF_KEYWORD
    | TokenKind::LAMBDA_KEYWORD
    | TokenKind::IF_KEYWORD
    | TokenKind::ELIF_KEYWORD
    | TokenKind::ELSE_KEYWORD
    | TokenKind::FOR_KEYWORD
    | TokenKind::IN_KEYWORD
    | TokenKind::RETURN_KEYWORD
    | TokenKind::BREAK_KEYWORD
    | TokenKind::CONTINUE_KEYWORD
    | TokenKind::PASS_KEYWORD
    | TokenKind::LOAD_KEYWORD
    | TokenKind::FROM_KEYWORD
    | TokenKind::OR_KEYWORD
    | TokenKind::AND_KEYWORD
    | TokenKind::NOT_KEYWORD
    | TokenKind::NONE_KEYWORD
    | TokenKind::TRUE_KEYWORD
    | TokenKind::FALSE_KEYWORD => Role::Keyword.style(),
    _ => 0,
  }
}

/// Is the reference the callee of a top-level call statement (`isBazelFileTopLevelCall`: the call's parent is an
/// expression statement of the file)
fn is_top_level_callee(tree: &Tree, reference: NodeId) -> bool {
  let Some(call) = tree.parent(reference).filter(|&call| tree.node_kind(call) == Some(NodeKind::CALL_EXPRESSION)) else {
    return false;
  };
  let callee = tree.children(call).find(|&child| tree.token_kind(child).is_none());
  callee == Some(reference)
    && tree
      .parent(call)
      .is_some_and(|statement| tree.node_kind(statement) == Some(NodeKind::EXPRESSION_STATEMENT) && tree.parent(statement) == Some(tree.root()))
}
