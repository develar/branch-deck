//! The highlighting of a Kotlin file from its PSI, as IntelliJ colours it without resolving names
//! (`KotlinHighlightingColors`): the tokens from the tree's leaves (soft keywords, `?.`, `?:` and `!!` are exact only
//! there), the roles of names from the nodes around them, painted by `highlight::paint::paint_leaf`.
//!
//! - **One preorder pass** over the ids with an explicit stack of the open nodes (`Frame`): a node with a style paints
//!   the leaves under it in it (a string, an annotation), a node may mark a leaf below it before the pass reaches it
//!   (the name of a function, of a property, the selector of `a.b`), and a few nodes are painted whole (a doc comment
//!   with its tags, a label, a use-site target).
//! - **Scopes** (`highlight::scopes`): the file and each class or object are a class scope with their properties; a
//!   function, an accessor, a lambda, a block, a `for`, a `catch` and a `when` a local one. So an unqualified name is
//!   a field only where no parameter or local of that name is in between; a local `var` is underlined at every use, the
//!   implicit `it` of a lambda bold, the backing `field` of an accessor bold.

use crate::highlight::lex::Lex;
use crate::highlight::lines::{Builder, LineRuns};
use crate::highlight::paint::{Paint, comment_paint, paint_leaf};
use crate::highlight::scopes::Scopes;
use crate::highlight::{BOLD, COMMENT, FIELD, Role, STATIC_FIELD, UNDERLINE, is_constant_name};
use crate::kotlin::navigate::{Receiver, callable_receiver};
use kotlin_syntax::{FileKind, NodeId, NodeKind, TokenKind, Tree};
use std::collections::HashMap;
use std::ops::Range;

/// The highlighting of a Kotlin text of `kind` (its size within `highlight::MAX_BYTES`): any bytes parse, so it always
/// has one
pub(crate) fn highlight(kind: FileKind, text: &str) -> LineRuns {
  let tree = super::parse(text.as_bytes(), kind);
  let mut out = LineRuns::builder(text);
  Painter {
    tree: &tree,
    text,
    scopes: Scopes::default(),
    marks: HashMap::new(),
    frames: Vec::new(),
  }
  .paint(&mut out);
  out.finish()
}

/// An open node of the pass
struct Frame {
  node: NodeId,
  /// The end of its subtree: the ids below it are `node..end`
  end: u32,
  /// The style of the text under it
  style: u32,
  /// Inside a `package` or `import` directive, whose names are a path, not references
  header: bool,
}

struct Painter<'t> {
  tree: &'t Tree,
  text: &'t str,
  scopes: Scopes,
  /// The styles that nodes gave the nodes below them before the pass reached them
  marks: HashMap<u32, u32>,
  frames: Vec<Frame>,
}

impl<'t> Painter<'t> {
  fn paint(&mut self, out: &mut Builder<'_>) {
    let tree = self.tree;
    let len = u32::try_from(tree.len()).unwrap_or(u32::MAX);
    let mut next = 0u32;
    while next < len {
      let id = NodeId(next);
      while self.frames.last().is_some_and(|frame| frame.end <= next) {
        self.close();
      }
      let around = self.frames.last().map_or(0, |frame| frame.style);
      let range = tree.range(id);
      if tree.is_leaf(id) {
        let paint = self.marks.remove(&next).map_or_else(|| self.leaf(id), Paint::Leaf);
        self.paint_range(out, range, paint, around);
        next += 1;
        continue;
      }
      let paint = self.marks.remove(&next).map_or_else(|| self.node(id), Paint::Leaf);
      match paint {
        Paint::Inherit | Paint::Style(_) => {
          let style = if let Paint::Style(style) = paint { style } else { around };
          let header = self.in_header() || matches!(tree.node_kind(id), Some(NodeKind::PACKAGE_DIRECTIVE | NodeKind::IMPORT_DIRECTIVE));
          self.frames.push(Frame {
            node: id,
            end: tree.subtree_end(id).0,
            style,
            header,
          });
          next += 1;
        }
        Paint::Opaque | Paint::Leaf(_) | Paint::Lexed(..) => {
          self.paint_range(out, range, paint, around);
          self.scopes.pop(next as usize);
          next = tree.subtree_end(id).0;
        }
      }
    }
    while !self.frames.is_empty() {
      self.close();
    }
  }

  fn close(&mut self) {
    if let Some(frame) = self.frames.pop() {
      self.scopes.pop(frame.node.0 as usize);
    }
  }

  fn in_header(&self) -> bool {
    self.frames.last().is_some_and(|frame| frame.header)
  }

  fn paint_range(&self, out: &mut Builder<'_>, range: Range<usize>, paint: Paint, around: u32) {
    if let Some(text) = self.text.get(range.clone()) {
      paint_leaf(out, range.start, text, paint, around);
    } else {
      // a range that ends inside a character (a bad character's byte): painted whole
      let style = match paint {
        Paint::Inherit | Paint::Opaque => around,
        Paint::Style(style) | Paint::Leaf(style) | Paint::Lexed(style, _) => style,
      };
      out.push(range.start, around);
      out.push(range.end, style);
    }
  }

  fn text_of(&self, id: NodeId) -> &'t str {
    self.text.get(self.tree.range(id)).unwrap_or_default()
  }

  fn mark(&mut self, id: NodeId, style: u32) {
    self.marks.insert(id.0, style);
  }

  fn child_token(&self, id: NodeId, kind: TokenKind) -> Option<NodeId> {
    child_token(self.tree, id, kind)
  }

  fn child_node(&self, id: NodeId, kind: NodeKind) -> Option<NodeId> {
    self.tree.children(id).find(|&child| self.tree.node_kind(child) == Some(kind))
  }

  fn parent_kind(&self, id: NodeId) -> Option<NodeKind> {
    self.tree.parent(id).and_then(|parent| self.tree.node_kind(parent))
  }

  /// The paint of a composite node, with what it does to the scopes and the marks
  fn node(&mut self, id: NodeId) -> Paint {
    let tree = self.tree;
    let Some(kind) = tree.node_kind(id) else {
      // a doc comment: a composite of the token `DOC_COMMENT`, painted whole with its tags
      return comment_paint(self.text_of(id));
    };
    match kind {
      NodeKind::KT_FILE => {
        let members = self.members(id, STATIC_FIELD);
        self.scopes.push_class(id.0 as usize, members);
      }
      NodeKind::CLASS => {
        let members = self.members(id, FIELD);
        self.scopes.push_class(id.0 as usize, members);
      }
      NodeKind::OBJECT_DECLARATION => {
        // the object of an object expression has instance properties, any other object static ones
        let style = if self.parent_kind(id) == Some(NodeKind::OBJECT_LITERAL) { FIELD } else { STATIC_FIELD };
        let members = self.members(id, style);
        self.scopes.push_class(id.0 as usize, members);
      }
      NodeKind::FUN => {
        self.scopes.push_local(id.0 as usize);
        if let Some(name) = self.child_token(id, TokenKind::IDENTIFIER) {
          self.mark(name, Role::FunctionDeclaration.style());
        }
      }
      NodeKind::SECONDARY_CONSTRUCTOR | NodeKind::PROPERTY_ACCESSOR | NodeKind::CLASS_INITIALIZER | NodeKind::FOR | NodeKind::CATCH | NodeKind::WHEN => {
        self.scopes.push_local(id.0 as usize);
      }
      // the statements of a script are its members, not locals
      NodeKind::BLOCK if self.parent_kind(id) != Some(NodeKind::SCRIPT) => self.scopes.push_local(id.0 as usize),
      NodeKind::FUNCTION_LITERAL => {
        self.scopes.push_local(id.0 as usize);
        // the implicit `it` is bold (`KOTLIN_CLOSURE_DEFAULT_PARAMETER`)
        if self.child_node(id, NodeKind::VALUE_PARAMETER_LIST).is_none() {
          self.scopes.bind_styled("it", BOLD);
        }
      }
      NodeKind::PROPERTY | NodeKind::DESTRUCTURING_DECLARATION => self.property(id),
      NodeKind::VALUE_PARAMETER => self.parameter(id),
      NodeKind::VALUE_ARGUMENT => {
        // a named argument: `name = value`
        if let Some(name) = self.child_node(id, NodeKind::VALUE_ARGUMENT_NAME) {
          self.mark(name, Role::NamedArgument.style());
          if let Some(equals) = self.child_token(id, TokenKind::EQ) {
            self.mark(equals, Role::NamedArgument.style());
          }
        }
      }
      NodeKind::TYPE_PARAMETER => {
        if let Some(name) = self.child_token(id, TokenKind::IDENTIFIER) {
          self.scopes.bind_type_parameter(self.text_of(name));
          self.mark(name, Role::TypeParameter.style());
        }
      }
      // `a.b`, `a?.b`: a selector that is not called is a property
      NodeKind::DOT_QUALIFIED_EXPRESSION | NodeKind::SAFE_ACCESS_EXPRESSION if !self.in_header() => {
        if let Some(selector) = tree.children(id).filter(|&child| !tree.is_leaf(child)).nth(1)
          && tree.node_kind(selector) == Some(NodeKind::REFERENCE_EXPRESSION)
          && let Some(name) = self.child_token(selector, TokenKind::IDENTIFIER)
        {
          let style = if is_constant_name(self.text_of(name)) { STATIC_FIELD } else { FIELD };
          self.mark(name, style);
        }
      }
      NodeKind::STRING_TEMPLATE => return Paint::Style(Role::String.style()),
      // the code of a template is code
      NodeKind::SHORT_STRING_TEMPLATE_ENTRY | NodeKind::LONG_STRING_TEMPLATE_ENTRY => return Paint::Style(Role::Foreground.style()),
      NodeKind::ANNOTATION_ENTRY | NodeKind::ANNOTATION => return Paint::Style(Role::Annotation.style()),
      NodeKind::ANNOTATION_TARGET => return Paint::Leaf(Role::Annotation.style()),
      // the arguments of an annotation are code
      NodeKind::VALUE_ARGUMENT_LIST if self.parent_kind(id) == Some(NodeKind::ANNOTATION_ENTRY) => return Paint::Style(Role::Foreground.style()),
      // `loop@`, `@loop` of `break@loop`, `@Outer` of `this@Outer`
      NodeKind::LABEL => return Paint::Leaf(Role::Label.style()),
      _ => {}
    }
    Paint::Inherit
  }

  /// A property or a destructuring declaration: a member's names in its style, a local's bound, a local `var`
  /// underlined wherever it is used, as IntelliJ's mutable variables
  fn property(&mut self, id: NodeId) {
    let tree = self.tree;
    let parent = self.parent_kind(id);
    // the names of a destructured parameter of a lambda or a loop
    if parent == Some(NodeKind::VALUE_PARAMETER) {
      for name in self.declared_names(id) {
        self.scopes.bind(self.text_of(name));
      }
      return;
    }
    let local = match parent {
      Some(NodeKind::BLOCK) => tree.parent(id).and_then(|block| self.parent_kind(block)) != Some(NodeKind::SCRIPT),
      Some(NodeKind::THEN | NodeKind::ELSE | NodeKind::BODY | NodeKind::WHEN | NodeKind::WHEN_ENTRY) => true,
      _ => false,
    };
    let style = if local {
      None
    } else if self.scopes.in_class_body() && !self.in_object(id) && !self.is_const(id) {
      Some(FIELD)
    } else {
      Some(STATIC_FIELD)
    };
    let mutable = self.child_token(id, TokenKind::VAR_KEYWORD).is_some();
    for name in self.declared_names(id) {
      if let Some(style) = style {
        self.mark(name, style);
      } else if mutable {
        self.scopes.bind_styled(self.text_of(name), UNDERLINE);
        self.mark(name, UNDERLINE);
      } else {
        self.scopes.bind(self.text_of(name));
      }
    }
  }

  /// A parameter: of a primary constructor with `val` or `var`, a property; of a function, an accessor, a lambda, a
  /// loop or a `catch`, bound in its scope; of a function type, `(size: Int) -> Unit`, nothing the code sees
  fn parameter(&mut self, id: NodeId) {
    let tree = self.tree;
    let Some(name) = self.child_token(id, TokenKind::IDENTIFIER) else { return };
    match tree.parent(id).and_then(|list| self.parent_kind(list)) {
      Some(NodeKind::PRIMARY_CONSTRUCTOR) => {
        if is_property_parameter(tree, id) {
          self.mark(name, FIELD);
        }
      }
      Some(NodeKind::FUNCTION_TYPE) => {}
      _ => self.scopes.bind(self.text_of(name)),
    }
  }

  /// The names a property or a destructuring declaration declares
  fn declared_names(&self, id: NodeId) -> Vec<NodeId> {
    let tree = self.tree;
    if tree.node_kind(id) == Some(NodeKind::DESTRUCTURING_DECLARATION) {
      tree
        .children(id)
        .filter(|&entry| tree.node_kind(entry) == Some(NodeKind::DESTRUCTURING_DECLARATION_ENTRY))
        .filter_map(|entry| self.child_token(entry, TokenKind::IDENTIFIER))
        .collect()
    } else {
      self.child_token(id, TokenKind::IDENTIFIER).into_iter().collect()
    }
  }

  fn is_const(&self, property: NodeId) -> bool {
    self
      .child_node(property, NodeKind::MODIFIER_LIST)
      .is_some_and(|modifiers| self.child_token(modifiers, TokenKind::CONST_KEYWORD).is_some())
  }

  /// A property of an object, a companion object or a file: no instance property
  fn in_object(&self, id: NodeId) -> bool {
    let tree = self.tree;
    let mut current = tree.parent(id);
    while let Some(node) = current {
      match tree.node_kind(node) {
        Some(NodeKind::CLASS) => return false,
        Some(NodeKind::OBJECT_DECLARATION) => return self.parent_kind(node) != Some(NodeKind::OBJECT_LITERAL),
        _ => {}
      }
      current = tree.parent(node);
    }
    true
  }

  /// The properties of a class with their styles: those of its body and the `val`/`var` parameters of its primary
  /// constructor; of a file, its top-level properties (of a script, those of its block). A `const val` and an enum
  /// entry are static, and the members of a companion object are visible in its class without a qualifier
  fn members(&self, class: NodeId, style: u32) -> Vec<(String, u32)> {
    let tree = self.tree;
    let mut names = Vec::new();
    let add_property = |names: &mut Vec<(String, u32)>, property: NodeId| {
      let style = if self.is_const(property) { STATIC_FIELD } else { style };
      names.extend(self.declared_names(property).into_iter().map(|name| (self.text_of(name).to_string(), style)));
    };
    for child in tree.children(class) {
      match tree.node_kind(child) {
        Some(NodeKind::PRIMARY_CONSTRUCTOR) => {
          let parameters = self.child_node(child, NodeKind::VALUE_PARAMETER_LIST);
          for parameter in parameters.into_iter().flat_map(|it| tree.children(it)) {
            if tree.node_kind(parameter) == Some(NodeKind::VALUE_PARAMETER)
              && is_property_parameter(tree, parameter)
              && let Some(name) = self.child_token(parameter, TokenKind::IDENTIFIER)
            {
              names.push((self.text_of(name).to_string(), style));
            }
          }
        }
        Some(NodeKind::CLASS_BODY) => {
          for member in tree.children(child) {
            match tree.node_kind(member) {
              Some(NodeKind::PROPERTY | NodeKind::DESTRUCTURING_DECLARATION) => add_property(&mut names, member),
              Some(NodeKind::ENUM_ENTRY) => names.extend(self.child_token(member, TokenKind::IDENTIFIER).map(|name| (self.text_of(name).to_string(), STATIC_FIELD))),
              Some(NodeKind::OBJECT_DECLARATION) if self.is_companion(member) => names.extend(self.members(member, STATIC_FIELD)),
              _ => {}
            }
          }
        }
        Some(NodeKind::PROPERTY | NodeKind::DESTRUCTURING_DECLARATION) => add_property(&mut names, child),
        Some(NodeKind::SCRIPT) => {
          for block in tree.children(child).filter(|&it| tree.node_kind(it) == Some(NodeKind::BLOCK)) {
            for member in tree.children(block) {
              if matches!(tree.node_kind(member), Some(NodeKind::PROPERTY | NodeKind::DESTRUCTURING_DECLARATION)) {
                add_property(&mut names, member);
              }
            }
          }
        }
        _ => {}
      }
    }
    names
  }

  fn is_companion(&self, object: NodeId) -> bool {
    self
      .child_node(object, NodeKind::MODIFIER_LIST)
      .is_some_and(|modifiers| self.child_token(modifiers, TokenKind::COMPANION_KEYWORD).is_some())
  }

  /// The paint of a leaf that no node marked
  fn leaf(&self, id: NodeId) -> Paint {
    let Some(token) = self.tree.token_kind(id) else { return Paint::Inherit };
    match token {
      TokenKind::EOL_COMMENT | TokenKind::SHEBANG_COMMENT => Paint::Leaf(COMMENT),
      TokenKind::BLOCK_COMMENT | TokenKind::DOC_COMMENT => comment_paint(self.text_of(id)),
      TokenKind::CHARACTER_LITERAL => Paint::Lexed(Role::String.style(), Lex::Escapes),
      // the text of a string, also where a parse error holds its tokens without a template around them
      TokenKind::OPEN_QUOTE | TokenKind::CLOSING_QUOTE | TokenKind::REGULAR_STRING_PART | TokenKind::DANGLING_NEWLINE => Paint::Leaf(Role::String.style()),
      // an escape, and the `$`, `${` and `}` of a template (`KOTLIN_STRING_ESCAPE`)
      TokenKind::ESCAPE_SEQUENCE | TokenKind::SHORT_TEMPLATE_ENTRY_START | TokenKind::LONG_TEMPLATE_ENTRY_START | TokenKind::LONG_TEMPLATE_ENTRY_END => {
        Paint::Leaf(Role::Escape.style())
      }
      TokenKind::INTEGER_LITERAL | TokenKind::FLOAT_LITERAL => Paint::Leaf(Role::Number.style()),
      // the braces and the arrow of a lambda are bold (`KOTLIN_FUNCTION_LITERAL_BRACES_AND_ARROW`)
      TokenKind::LBRACE | TokenKind::RBRACE | TokenKind::ARROW if self.parent_kind(id) == Some(NodeKind::FUNCTION_LITERAL) => Paint::Leaf(BOLD),
      TokenKind::IDENTIFIER => self.identifier(id),
      token if token.is_keyword() => Paint::Leaf(Role::Keyword.style()),
      _ => Paint::Inherit,
    }
  }

  /// A name that no declaration marked: a type's in a type, else by the scopes
  fn identifier(&self, id: NodeId) -> Paint {
    let tree = self.tree;
    if self.in_header() {
      return Paint::Opaque;
    }
    let Some(parent) = tree.parent(id) else { return Paint::Opaque };
    match tree.node_kind(parent) {
      Some(NodeKind::REFERENCE_EXPRESSION) => {
        let Some(grandparent) = tree.parent(parent) else { return Paint::Opaque };
        match tree.node_kind(grandparent) {
          // a type, and the type parameter a `where` constrains
          Some(NodeKind::USER_TYPE | NodeKind::TYPE_CONSTRAINT) => self.type_name(id),
          // the callee of a call
          Some(NodeKind::CALL_EXPRESSION) if tree.first_child(grandparent) == Some(parent) => Paint::Opaque,
          // `Foo::bar`, `Foo::class`: a name before `::` is a type, the name after a receiver is a member of it, which
          // only resolving tells a property from a function
          Some(NodeKind::CALLABLE_REFERENCE_EXPRESSION | NodeKind::CLASS_LITERAL_EXPRESSION) => match callable_receiver(tree, grandparent, parent) {
            Receiver::Is => self.type_name(id),
            Receiver::Of(_) => Paint::Opaque,
            Receiver::None => self.name_style(id),
          },
          _ => self.name_style(id),
        }
      }
      // the function of an infix call, an alias of an import, a parameter that no `val` makes a property
      Some(NodeKind::OPERATION_REFERENCE | NodeKind::IMPORT_ALIAS | NodeKind::VALUE_PARAMETER) => Paint::Opaque,
      Some(NodeKind::CLASS | NodeKind::OBJECT_DECLARATION | NodeKind::TYPEALIAS) => self.type_name(id),
      _ => self.name_style(id),
    }
  }

  /// A type's name: a type parameter in scope, else plain
  fn type_name(&self, id: NodeId) -> Paint {
    if self.scopes.is_type_parameter(self.text_of(id)) {
      Paint::Leaf(Role::TypeParameter.style())
    } else {
      Paint::Opaque
    }
  }

  /// An unqualified name: a property of an enclosing class with no local of that name in between is a field, the
  /// backing `field` of an accessor too; anything else is plain
  fn name_style(&self, id: NodeId) -> Paint {
    let name = self.text_of(id);
    if let Some(style) = self.scopes.local(name) {
      return if style == 0 { Paint::Opaque } else { Paint::Leaf(style) };
    }
    // the backing field of an accessor is bold text (`KOTLIN_BACKING_FIELD_VARIABLE`)
    if name == "field" && self.tree.ancestor_or_self(id, |it| self.tree.node_kind(it) == Some(NodeKind::PROPERTY_ACCESSOR)).is_some() {
      return Paint::Leaf(BOLD);
    }
    self.scopes.member(name).map_or(Paint::Opaque, Paint::Leaf)
  }
}

/// The first child leaf of a token kind
fn child_token(tree: &Tree, id: NodeId, kind: TokenKind) -> Option<NodeId> {
  tree.children(id).find(|&child| tree.token_kind(child) == Some(kind))
}

/// A parameter of a primary constructor that is a property: `val` or `var`
fn is_property_parameter(tree: &Tree, parameter: NodeId) -> bool {
  child_token(tree, parameter, TokenKind::VAL_KEYWORD).is_some() || child_token(tree, parameter, TokenKind::VAR_KEYWORD).is_some()
}

#[cfg(test)]
#[path = "highlight_test.rs"]
mod tests;
