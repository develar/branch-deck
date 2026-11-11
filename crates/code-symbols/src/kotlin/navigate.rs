//! Go to Declaration and Find Usages in a Kotlin file from its PSI (openspec/specs/commit/symbol-navigation/spec.md): the name
//! at a place (`Tree::leaf_index`), its qualifier, the kind of the reference, its declaration in the file by the
//! scopes, and the names of one spelling with their bindings (`navigate::Names`), which `navigate` makes the usages and
//! the answers of `names_at` of (openspec/specs/commit/name-bindings/spec.md).
//!
//! - **A name** is an `IDENTIFIER` leaf of the code: not one of a doc comment, not a label (but the `Outer` of
//!   `this@Outer`, a type).
//! - **Its declaration by the scopes**: from the name up, the first scope (a function, a lambda, a class or an object,
//!   an accessor, a block, a `for`, a `catch`, a `when` with its subject, the file, and a body between them) that declares the name, without the
//!   scopes inside it but with their own names (a function of a class is declared in the class); in it the last
//!   declaration the name sees, else the first where the order does not count (a member declared after its use).
//!   Code reads in its order: a variable is seen after its own statement (`val name = name.trim()` reads another
//!   `name`), the variable of a `for` after its range, and a name before the declarations of its block is of a scope
//!   around it. The declarations of the name are found in one pass over the leaves and filed under the scopes that
//!   see them (`Declarations`), so the usages of a name cost a pass and a climb per usage, not a walk of the scopes
//!   per name.
//! - **A local** is a declaration inside code (a function, an `init` block, a block or a `catch` of an initializer) or
//!   of a function type, which no other file sees: its usages are those bound to it; the usages of any other name are
//!   those that no local binds. The parameter of a function type, `(event: Event) -> Unit`, no scope sees.
//! - **What a local is** (`LocalBinding`): a `val` or a `var` of a body and the subject of a `when` a variable, the
//!   variable of a `for`, the parameter of a `catch`, of a lambda or of a function without a name, and the parameter of
//!   a function with a name, a constructor or a setter, which a call may name unless a setter declares it; a member of
//!   a local class or object, a `val` of its constructor, a local function, class or type parameter and the parameter
//!   of a function type are `Other`.
//! - **Its site** (`Site`): the name of a named argument, a name after `::` without a receiver, a name called or read
//!   as a type that the scopes bind to a value (a property with `invoke`, or a function and a class of its spelling),
//!   an entry of a destructuring. `it` and `field` are bound without a declaration.

use crate::navigate::{Access, Bound, LocalBinding, LocalKind, Position, ReferenceAt, ReferenceKind, Resolved, Site, Wanted, byte_offset, reference_answer, site_by_binding};
use kotlin_syntax::psi;
use kotlin_syntax::{FileKind, NodeId, NodeKind, TokenKind, Tree};
use std::collections::HashMap;

/// The name at `position` of a Kotlin text of `kind` and what its syntax says (`navigate::reference_at`)
pub(crate) fn reference_at(kind: FileKind, text: &str, position: Position) -> Option<ReferenceAt> {
  with_name_at(kind, text, position, |names, leaf, name, declarations| {
    let (kind, qualifier) = names.kind_of(leaf);
    let local = match kind {
      ReferenceKind::Declaration | ReferenceKind::Member | ReferenceKind::MemberCall => None,
      ReferenceKind::Value | ReferenceKind::Call | ReferenceKind::Type => declarations.binding(names, leaf).map(|it| names.tree.range(it).start),
    };
    reference_answer(text, names.tree.range(leaf).start, name, kind, qualifier, local)
  })
}

/// The name leaf at `position` of a parse of `text`, its text and the declarations of that name, for `answer`; `None`
/// where no name is
fn with_name_at<T>(kind: FileKind, text: &str, position: Position, answer: impl FnOnce(&Names<'_>, NodeId, String, &Declarations) -> T) -> Option<T> {
  let offset = byte_offset(text, position)?;
  let tree = super::parse(text.as_bytes(), kind);
  let names = Names { tree: &tree, text };
  let leaf = names.name_at(offset)?;
  let name = names.text_of(leaf).to_string();
  let declarations = Declarations::of(&names, &Wanted::named(&name));
  Some(answer(&names, leaf, name, &declarations))
}

/// The answer of `read` for the names of a parse of a Kotlin text of `kind` (`navigate::names_at`, `usages_at`,
/// `usages_of`)
pub(crate) fn with_names<R>(kind: FileKind, text: &str, read: impl FnOnce(&dyn crate::navigate::Names) -> R) -> R {
  let tree = super::parse(text.as_bytes(), kind);
  read(&Names { tree: &tree, text })
}

impl crate::navigate::Names for Names<'_> {
  fn name_at(&self, offset: usize) -> Option<(usize, &str)> {
    let leaf = Names::name_at(self, offset)?;
    Some((self.tree.range(leaf).start, self.text_of(leaf)))
  }

  fn resolve(&self, wanted: &Wanted<'_>) -> Vec<Resolved> {
    let declarations = Declarations::of(self, wanted);
    let resolved = declarations.names.iter().map(|&leaf| {
      let (kind, _) = self.kind_of(leaf);
      let binding = declarations.binding_of(self, leaf, kind).map(|it| Bound {
        start: self.tree.range(it).start,
        local: self.local_of(it),
      });
      Resolved {
        start: self.tree.range(leaf).start,
        access: self.access_of(leaf, kind),
        site: self.site_of(leaf, kind, binding.and_then(|it| it.local)),
        binding,
      }
    });
    resolved.collect()
  }

  /// The parameter of a lambda that declares none, and the field of a property in its accessors
  fn is_implicit(&self, name: &str) -> bool {
    matches!(name, "it" | "field")
  }
}

/// The names of a tree and what its syntax says of each
struct Names<'t> {
  tree: &'t Tree,
  text: &'t str,
}

impl Names<'_> {
  fn text_of(&self, id: NodeId) -> &str {
    self.text.get(self.tree.range(id)).unwrap_or_default()
  }

  fn kind(&self, id: NodeId) -> Option<NodeKind> {
    self.tree.node_kind(id)
  }

  fn parent_kind(&self, id: NodeId) -> Option<NodeKind> {
    self.tree.parent(id).and_then(|parent| self.tree.node_kind(parent))
  }

  /// The composite children of a node: an expression's operands
  fn parts(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    self.tree.composite_children(id)
  }

  /// The name at `offset`: the character there, else the one before it, as the end of a name is still the name (`foo|`)
  fn name_at(&self, offset: usize) -> Option<NodeId> {
    let index = self.tree.leaf_index();
    let at = |offset: usize| index.at(offset).filter(|&leaf| self.is_name(leaf));
    at(offset).or_else(|| offset.checked_sub(1).and_then(at))
  }

  /// Whether a leaf is a name of the code: an identifier, not of a doc comment, not of a label but `this@Outer`'s
  fn is_name(&self, leaf: NodeId) -> bool {
    if self.tree.token_kind(leaf) != Some(TokenKind::IDENTIFIER) {
      return false;
    }
    match self.parent_kind(leaf) {
      // a doc comment's `[name]` (a composite of a token type, or `KDOC_NAME`)
      None | Some(NodeKind::KDOC_NAME) => false,
      Some(NodeKind::LABEL) => self.is_type_label(leaf),
      Some(_) => true,
    }
  }

  /// The label of `this@Outer` and `super@Outer`: the name of a class
  fn is_type_label(&self, leaf: NodeId) -> bool {
    let qualifier = self.tree.parent(leaf).and_then(|label| self.tree.parent(label));
    qualifier
      .and_then(|it| self.parent_kind(it))
      .is_some_and(|it| matches!(it, NodeKind::THIS_EXPRESSION | NodeKind::SUPER_EXPRESSION))
  }

  /// Whether the leaf is the name a declaration declares
  fn is_declared(&self, leaf: NodeId) -> bool {
    let Some(parent) = self.tree.parent(leaf) else { return false };
    let declares = match self.kind(parent) {
      Some(
        NodeKind::FUN
        | NodeKind::CLASS
        | NodeKind::TYPEALIAS
        | NodeKind::TYPE_PARAMETER
        | NodeKind::PROPERTY
        | NodeKind::VALUE_PARAMETER
        | NodeKind::ENUM_ENTRY
        | NodeKind::DESTRUCTURING_DECLARATION_ENTRY,
      ) => true,
      // the name of a companion object is a type
      Some(NodeKind::OBJECT_DECLARATION) => !psi::kt_object_declaration_is_companion(self.tree, parent),
      _ => false,
    };
    declares && psi::get_name_identifier(self.tree, parent) == Some(leaf)
  }

  /// Inside a `package` or `import` directive, whose names are a path
  fn in_header(&self, leaf: NodeId) -> bool {
    self
      .tree
      .ancestor_or_self(leaf, |it| matches!(self.kind(it), Some(NodeKind::PACKAGE_DIRECTIVE | NodeKind::IMPORT_DIRECTIVE)))
      .is_some()
  }

  /// The kind of a name by the nodes around it, and the qualifier before it
  fn kind_of(&self, leaf: NodeId) -> (ReferenceKind, Option<String>) {
    let tree = self.tree;
    if self.is_declared(leaf) {
      return (ReferenceKind::Declaration, None);
    }
    let Some(parent) = tree.parent(leaf) else { return (ReferenceKind::Value, None) };
    match self.kind(parent) {
      Some(NodeKind::LABEL | NodeKind::IMPORT_ALIAS | NodeKind::OBJECT_DECLARATION) => return (ReferenceKind::Type, None),
      Some(NodeKind::REFERENCE_EXPRESSION) => {}
      _ => return (ReferenceKind::Value, None),
    }
    if self.in_header(leaf) {
      return (ReferenceKind::Value, None);
    }
    let Some(outer) = tree.parent(parent) else { return (ReferenceKind::Value, None) };
    match self.kind(outer) {
      // a type, and the type parameter a `where` constrains
      Some(NodeKind::USER_TYPE | NodeKind::TYPE_CONSTRAINT) => (ReferenceKind::Type, None),
      // `Foo::bar`, `Foo::class`: a name before `::` is a type, the name after a receiver is its member
      Some(NodeKind::CALLABLE_REFERENCE_EXPRESSION | NodeKind::CLASS_LITERAL_EXPRESSION) => match callable_receiver(tree, outer, parent) {
        Receiver::Is => (ReferenceKind::Type, None),
        Receiver::Of(receiver) => (ReferenceKind::Member, Some(self.text_of(receiver).to_string())),
        Receiver::None => (ReferenceKind::Value, None),
      },
      Some(NodeKind::DOT_QUALIFIED_EXPRESSION | NodeKind::SAFE_ACCESS_EXPRESSION) => match self.receiver_of(outer, parent) {
        Some(receiver) => (ReferenceKind::Member, Some(self.text_of(receiver).to_string())),
        None => (ReferenceKind::Value, None),
      },
      Some(NodeKind::CALL_EXPRESSION) if tree.first_child(outer) == Some(parent) => {
        match tree
          .parent(outer)
          .filter(|&it| matches!(self.kind(it), Some(NodeKind::DOT_QUALIFIED_EXPRESSION | NodeKind::SAFE_ACCESS_EXPRESSION)))
        {
          Some(qualified) => match self.receiver_of(qualified, outer) {
            Some(receiver) => (ReferenceKind::MemberCall, Some(self.text_of(receiver).to_string())),
            None => (ReferenceKind::Call, None),
          },
          None => (ReferenceKind::Call, None),
        }
      }
      _ => (ReferenceKind::Value, None),
    }
  }

  /// The receiver of a qualified expression whose selector is `selector`; none when `selector` is the receiver
  fn receiver_of(&self, qualified: NodeId, selector: NodeId) -> Option<NodeId> {
    if psi::kt_qualified_expression_get_selector_expression(self.tree, qualified) != Some(selector) {
      return None;
    }
    psi::kt_qualified_expression_get_receiver_expression(self.tree, qualified)
  }

  /// Whether a declared name is a local, which no other file sees: code around its declaration (a function, not the one
  /// it names, an `init` block, a block, a `catch` or a `when` of a property's initializer), or a function type, whose
  /// parameter names nothing the code sees. The statements of a script are its members
  fn is_local(&self, declared: NodeId) -> bool {
    let tree = self.tree;
    let mut current = tree.parent(declared).and_then(|it| tree.parent(it));
    while let Some(node) = current {
      match self.kind(node) {
        Some(
          NodeKind::FUN
          | NodeKind::FUNCTION_LITERAL
          | NodeKind::SECONDARY_CONSTRUCTOR
          | NodeKind::PROPERTY_ACCESSOR
          | NodeKind::CLASS_INITIALIZER
          | NodeKind::CATCH
          | NodeKind::FOR
          | NodeKind::WHEN
          | NodeKind::FUNCTION_TYPE,
        ) => return true,
        Some(NodeKind::BLOCK) if self.parent_kind(node) != Some(NodeKind::SCRIPT) => return true,
        _ => {}
      }
      current = tree.parent(node);
    }
    false
  }

  /// The byte from which the names of its scope see a declared name: the end of its own statement for a variable of
  /// code, whose initializer reads the name of a scope around it; the end of the range of its `for` for a loop
  /// variable; else its own start
  fn seen_from(&self, declared: NodeId) -> usize {
    let tree = self.tree;
    let around = |kind: NodeKind| tree.ancestor_or_self(declared, |it| self.kind(it) == Some(kind));
    let end = match self.local_of(declared).map(|it| it.kind) {
      Some(LocalKind::Variable) => around(NodeKind::DESTRUCTURING_DECLARATION).or_else(|| tree.parent(declared)),
      Some(LocalKind::LoopVariable) => around(NodeKind::FOR).and_then(|it| tree.children(it).find(|&part| self.kind(part) == Some(NodeKind::LOOP_RANGE))),
      _ => None,
    };
    end.map_or_else(|| tree.range(declared).start, |it| tree.range(it).end)
  }

  /// Whether the declarations of a scope are seen only by what follows them: code (a block but a script's, a body, a
  /// `for`, the subject of a `when`), not the members of a class or a file
  fn reads_in_order(&self, scope: NodeId) -> bool {
    match self.kind(scope) {
      Some(NodeKind::BLOCK) => self.parent_kind(scope) != Some(NodeKind::SCRIPT),
      Some(NodeKind::BODY | NodeKind::THEN | NodeKind::ELSE | NodeKind::FOR | NodeKind::WHEN) => true,
      _ => false,
    }
  }

  /// What a declared name is when code declares it (`is_local`): a variable, a parameter of some kind, else `Other`
  fn local_of(&self, declared: NodeId) -> Option<LocalBinding> {
    if !self.is_local(declared) {
      return None;
    }
    let tree = self.tree;
    let Some(declaration) = tree.parent(declared) else { return Some(LocalBinding::OTHER) };
    Some(match self.kind(declaration) {
      // a property of a local class or object is its member, which a qualifier reaches
      Some(NodeKind::PROPERTY) if self.parent_kind(declaration) == Some(NodeKind::CLASS_BODY) => LocalBinding::OTHER,
      Some(NodeKind::PROPERTY) => LocalBinding::of(LocalKind::Variable),
      Some(NodeKind::VALUE_PARAMETER) => self.parameter_of(declaration),
      // `val (a, b) = pair`, and the entries of a parameter: `for ((key, value) in map)`, `{ (a, b) -> … }`
      Some(NodeKind::DESTRUCTURING_DECLARATION_ENTRY) => match tree.parent(declaration).and_then(|it| tree.parent(it)) {
        Some(parameter) if self.kind(parameter) == Some(NodeKind::VALUE_PARAMETER) => self.parameter_of(parameter),
        _ => LocalBinding::of(LocalKind::Variable),
      },
      _ => LocalBinding::OTHER,
    })
  }

  /// What a parameter is by what it is a parameter of. A call may name the parameter of a function with a name and of
  /// a constructor; a function type takes no named argument, so the parameter of a lambda or of a function without a
  /// name is its body's alone, as the one of a setter
  fn parameter_of(&self, parameter: NodeId) -> LocalBinding {
    let tree = self.tree;
    let Some(parent) = tree.parent(parameter) else { return LocalBinding::OTHER };
    if self.kind(parent) == Some(NodeKind::FOR) {
      return LocalBinding::of(LocalKind::LoopVariable);
    }
    let owner = tree.parent(parent).filter(|_| self.kind(parent) == Some(NodeKind::VALUE_PARAMETER_LIST));
    let named = LocalBinding {
      kind: LocalKind::Parameter,
      named_by_calls: true,
    };
    // a `val` or a `var` of a constructor is a property of its class
    let is_property = || {
      tree
        .children(parameter)
        .any(|it| matches!(tree.token_kind(it), Some(TokenKind::VAL_KEYWORD | TokenKind::VAR_KEYWORD)))
    };
    match owner.and_then(|it| self.kind(it).map(|kind| (it, kind))) {
      Some((_, NodeKind::CATCH)) => LocalBinding::of(LocalKind::CatchParameter),
      Some((_, NodeKind::FUNCTION_LITERAL)) => LocalBinding::of(LocalKind::LambdaParameter),
      Some((function, NodeKind::FUN)) if psi::get_name_identifier(tree, function).is_none() => LocalBinding::of(LocalKind::LambdaParameter),
      Some((_, NodeKind::PRIMARY_CONSTRUCTOR)) if is_property() => LocalBinding::OTHER,
      Some((_, NodeKind::FUN | NodeKind::SECONDARY_CONSTRUCTOR | NodeKind::PRIMARY_CONSTRUCTOR)) => named,
      Some((_, NodeKind::PROPERTY_ACCESSOR)) => LocalBinding::of(LocalKind::Parameter),
      _ => LocalBinding::OTHER,
    }
  }

  /// What the site of a name makes of it beside its binding (`Site`)
  fn site_of(&self, leaf: NodeId, kind: ReferenceKind, binding: Option<LocalBinding>) -> Site {
    let tree = self.tree;
    let Some(parent) = tree.parent(leaf) else { return Site::Plain };
    let syntactic = match (self.kind(parent), tree.parent(parent).and_then(|it| self.kind(it))) {
      (Some(NodeKind::REFERENCE_EXPRESSION), Some(NodeKind::VALUE_ARGUMENT_NAME)) => Site::NamedArgument,
      // `::total` is a function or a property, never a variable or a parameter of a body
      (Some(NodeKind::REFERENCE_EXPRESSION), Some(NodeKind::CALLABLE_REFERENCE_EXPRESSION)) if kind == ReferenceKind::Value => Site::Namesake,
      (Some(NodeKind::DESTRUCTURING_DECLARATION_ENTRY), _) => Site::Destructured,
      _ => Site::Plain,
    };
    // a call of a value is its `invoke` or a function of its spelling, a type is never a value
    site_by_binding(syntactic, matches!(kind, ReferenceKind::Call | ReferenceKind::Type), binding)
  }

  /// How a name is used: declared, imported, called, written (the end of the left side of an assignment), else read
  fn access_of(&self, leaf: NodeId, kind: ReferenceKind) -> Access {
    match kind {
      ReferenceKind::Declaration => return Access::Declaration,
      ReferenceKind::Call | ReferenceKind::MemberCall => return Access::Call,
      _ => {}
    }
    let tree = self.tree;
    let range = tree.range(leaf);
    let mut current = tree.parent(leaf);
    while let Some(node) = current {
      match self.kind(node) {
        Some(NodeKind::IMPORT_DIRECTIVE) => return Access::Import,
        Some(NodeKind::BINARY_EXPRESSION) if self.operation(node).is_some_and(is_assignment) => {
          let left = self.parts(node).next();
          return if left.is_some_and(|it| tree.range(it).end == range.end && tree.range(it).start <= range.start) {
            Access::Write
          } else {
            Access::Read
          };
        }
        Some(NodeKind::POSTFIX_EXPRESSION | NodeKind::PREFIX_EXPRESSION)
          if self.operation(node).is_some_and(|it| matches!(it, TokenKind::PLUSPLUS | TokenKind::MINUSMINUS))
            && self
              .parts(node)
              .find(|&it| self.kind(it) != Some(NodeKind::OPERATION_REFERENCE))
              .is_some_and(|it| tree.range(it).end == range.end) =>
        {
          return Access::Write;
        }
        Some(kind) if is_scope(kind) => return Access::Read,
        _ => {}
      }
      current = tree.parent(node);
    }
    Access::Read
  }

  /// The operator of an operation: the token of its `OPERATION_REFERENCE`
  fn operation(&self, node: NodeId) -> Option<TokenKind> {
    let tree = self.tree;
    let reference = tree.children(node).find(|&child| tree.node_kind(child) == Some(NodeKind::OPERATION_REFERENCE))?;
    tree.first_child(reference).and_then(|it| tree.token_kind(it))
  }
}

/// The names of the tree a resolution looks for (those of one spelling), and the declarations among them filed under
/// the scopes that see them
struct Declarations {
  /// Every name looked for, in the order of the text
  names: Vec<NodeId>,
  /// For each scope (or a body) that declares the name, its declarations in the order of the text
  by_scope: HashMap<u32, Vec<NodeId>>,
}

impl Declarations {
  fn of(names: &Names<'_>, wanted: &Wanted<'_>) -> Self {
    let tree = names.tree;
    let mut found = Self {
      names: Vec::new(),
      by_scope: HashMap::new(),
    };
    for leaf in tree.tokens() {
      if tree.token_kind(leaf) != Some(TokenKind::IDENTIFIER) || !wanted.takes(names.text_of(leaf), tree.range(leaf).start) || !names.is_name(leaf) {
        continue;
      }
      found.names.push(leaf);
      if !names.is_declared(leaf) {
        continue;
      }
      // the scopes that see the declaration: from the node around the declaring one up to the first scope, which stops
      // the scopes above it from seeing inside it
      let mut current = tree.parent(leaf).and_then(|it| tree.parent(it));
      while let Some(node) = current {
        let kind = tree.node_kind(node);
        // the parameter of a function type, `(event: Event) -> Unit`, names nothing a scope sees
        if kind == Some(NodeKind::FUNCTION_TYPE) {
          break;
        }
        let scope = kind.is_some_and(is_scope);
        if scope || tree.parent(node).is_none() || kind.is_some_and(is_body) {
          found.by_scope.entry(node.0).or_default().push(leaf);
        }
        if scope {
          break;
        }
        current = tree.parent(node);
      }
    }
    found
  }

  /// The declaration a name is bound to by the scopes: the name itself when it is the name of a declaration; none for a
  /// member after a qualifier, which the scopes do not bind
  fn binding_of(&self, names: &Names<'_>, leaf: NodeId, kind: ReferenceKind) -> Option<NodeId> {
    match kind {
      ReferenceKind::Declaration => Some(leaf),
      ReferenceKind::Value | ReferenceKind::Call | ReferenceKind::Type => self.binding(names, leaf),
      ReferenceKind::Member | ReferenceKind::MemberCall => None,
    }
  }

  /// The declaration of the name in the scopes around `leaf`: in the nearest scope that declares one the name sees,
  /// the last such before the name; else the first where the order of the scope does not count (a member declared
  /// after its use). A scope of code that declares the name only later is no scope of the name
  fn binding(&self, names: &Names<'_>, leaf: NodeId) -> Option<NodeId> {
    let tree = names.tree;
    let start = tree.range(leaf).start;
    let mut current = tree.parent(leaf);
    while let Some(node) = current {
      if let Some(found) = self.by_scope.get(&node.0) {
        let seen = found.iter().rev().find(|&&it| names.seen_from(it) <= start);
        if let Some(declared) = seen.or_else(|| found.first().filter(|_| !names.reads_in_order(node))) {
          return Some(*declared);
        }
      }
      current = tree.parent(node);
    }
    None
  }
}

/// Where a name stands in a callable reference or a class literal
pub(super) enum Receiver {
  /// It is the receiver, before `::`
  Is,
  /// It is after `::`, a member of this receiver
  Of(NodeId),
  /// It is after `::` with no receiver (`::foo`)
  None,
}

/// Where `part` stands in the callable reference or class literal `reference` (`Foo::bar`, `::bar`, `Foo::class`)
pub(super) fn callable_receiver(tree: &Tree, reference: NodeId, part: NodeId) -> Receiver {
  let Some(colons) = tree.children(reference).find(|&it| tree.token_kind(it) == Some(TokenKind::COLONCOLON)) else {
    return Receiver::None;
  };
  if tree.range(part).start < tree.range(colons).start {
    return Receiver::Is;
  }
  tree
    .children(reference)
    .find(|&it| !tree.is_leaf(it) && tree.node_kind(it).is_some() && tree.range(it).end <= tree.range(colons).start)
    .map_or(Receiver::None, Receiver::Of)
}

/// The nodes that open a scope: a name declared inside is not visible outside
const fn is_scope(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::FUN
      | NodeKind::FUNCTION_LITERAL
      | NodeKind::CLASS
      | NodeKind::OBJECT_DECLARATION
      | NodeKind::SECONDARY_CONSTRUCTOR
      | NodeKind::PROPERTY_ACCESSOR
      | NodeKind::FOR
      | NodeKind::CATCH
      | NodeKind::WHEN
      | NodeKind::BLOCK
  )
}

/// A body between scopes, which sees what its scope declares before it
const fn is_body(kind: NodeKind) -> bool {
  matches!(kind, NodeKind::CLASS_BODY | NodeKind::BODY | NodeKind::THEN | NodeKind::ELSE)
}

const fn is_assignment(token: TokenKind) -> bool {
  matches!(
    token,
    TokenKind::EQ | TokenKind::PLUSEQ | TokenKind::MINUSEQ | TokenKind::MULTEQ | TokenKind::DIVEQ | TokenKind::PERCEQ
  )
}

#[cfg(test)]
#[path = "navigate_test.rs"]
mod tests;
