//! The half of Go to Declaration and Find Usages over a psi-syntax tree (Java's PSI, Roslyn's tree of C#, Starlark's
//! PSI) that knows no language (openspec/specs/commit/symbol-navigation/spec.md): the answer of `reference_at` and the
//! names of one spelling with their bindings (`navigate::Names`, of which `navigate` makes the usages and the answers
//! of `names_at`, openspec/specs/commit/name-bindings/spec.md) from a language's reading of its tree (`Reading`: its
//! names, their kinds, the walk of its scopes, what a local is and what a site makes of a name), and the scopes of
//! such a walk (`Frame`, `declaration_in`). What a name is and which scopes a language has are the language's
//! (`java::navigate`, `csharp::navigate`, `starlark::navigate`).

use crate::navigate::{Access, Bound, Leaves, LocalBinding, Names, Position, ReferenceAt, ReferenceKind, Resolved, Site, Wanted, byte_offset, reference_answer, without_bodies};
use psi_builder::language::TreeLanguage;
use psi_builder::{NodeId, Tree};
use std::cell::OnceCell;
use std::ops::Range;

/// A name of a language's walk: its leaf, its kind and the name of the declaration it is bound to
pub(crate) struct Named {
  pub(crate) leaf: NodeId,
  pub(crate) kind: ReferenceKind,
  pub(crate) binding: Option<NodeId>,
}

/// A scope of the walk: its node, the end of its subtree and the names of its declarations of the name, in the order of
/// the text
pub(crate) struct Frame {
  pub(crate) node: NodeId,
  pub(crate) end: u32,
  pub(crate) declared: Vec<NodeId>,
}

/// The declaration of the scopes whose name starts at `start`: in the innermost scope that declares one the name sees
/// (`sees`: the scope and the name of its declaration), the last such before the name, else the first (a member
/// declared after its use)
pub(crate) fn declaration_in<L: TreeLanguage>(frames: &[Frame], start: usize, tree: &Tree<L>, sees: impl Fn(NodeId, NodeId) -> bool) -> Option<NodeId> {
  frames.iter().rev().find_map(|frame| {
    let seen = || frame.declared.iter().copied().filter(|&it| sees(frame.node, it));
    seen().rev().find(|&it| tree.range(it).start <= start).or_else(|| seen().next())
  })
}

/// Whether the name `declared` of a declaration is a local: a node of code that runs (`is_function`) around what it
/// declares
pub(crate) fn is_local_in<L: TreeLanguage>(tree: &Tree<L>, declared: NodeId, is_function: impl Fn(L::Node) -> bool) -> bool {
  let Some(declaration) = tree.parent(declared) else { return false };
  std::iter::successors(tree.parent(declaration), |&it| tree.parent(it)).any(|it| tree.node_kind(it).is_some_and(&is_function))
}

/// What the syntax says of a name: its kind and its qualifier
pub(crate) type Answer = (ReferenceKind, Option<NodeId>);

/// The names of one name of a walk, which a language's scopes take
pub(crate) type IsNamed<'a> = &'a dyn Fn(NodeId) -> bool;

/// The kind of a name in the composite `parent` of the kind `kind`, a language's (`Reading::kind_in`)
type KindIn<L, T> = fn(&T, NodeId, NodeId, <L as TreeLanguage>::Node) -> Answer;

/// The names a scope sees wherever they are, a language's (`Reading::hoisted`)
type Hoisted<T> = fn(&T, NodeId, IsNamed<'_>) -> Vec<NodeId>;

/// What the site of a name of the kind makes of it beside its binding, a language's (`Reading::site_of`): the name
/// of the declaration of a local and what it declares
type SiteOf<T> = fn(&T, NodeId, ReferenceKind, Option<(NodeId, LocalBinding)>) -> Site;

/// The scopes of a language over the names of one name (`Reading::scopes`)
type MakeScopes<L, T> = for<'a> fn(&'a T, IsNamed<'a>) -> Box<dyn Scoping<L> + 'a>;

/// What a language's scopes say of the binding of a name
pub(crate) enum Binding {
  /// The declaration the scopes of the walk see, if any
  ByScopes,
  /// A binding the scopes don't give: a label, an annotation's element, or none
  Bound(Option<NodeId>),
}

/// A language's reading of its tree for navigation: its parse, its names, their kinds, the walk of its scopes
pub(crate) struct Reading<L: TreeLanguage, T> {
  pub(crate) parse: fn(&str) -> T,
  pub(crate) psi: fn(&T) -> &Tree<L>,
  /// Whether a leaf is a name of the code
  pub(crate) is_name: fn(&T, NodeId) -> bool,
  /// The name a leaf is, as names compare
  pub(crate) name: for<'a> fn(&'a T, NodeId) -> &'a str,
  /// Whether a node of the kind declares the name of its leaf
  pub(crate) declares: fn(L::Node) -> bool,
  /// The kind of a name in the composite `parent` of the kind `kind` that declares none, and its qualifier
  pub(crate) kind_in: KindIn<L, T>,
  /// The text of a qualifier
  pub(crate) qualifier_text: fn(&T, NodeId) -> String,
  /// Whether a leaf is a name of the code of the name `name`
  pub(crate) is_named: fn(&T, NodeId, &str) -> bool,
  /// The names of the declarations the scope `node` sees wherever they are (the root's too), of the names `is_named`
  pub(crate) hoisted: Hoisted<T>,
  /// The language's scopes over the names `is_named`
  pub(crate) scopes: MakeScopes<L, T>,
  /// Whether the name of a declaration is a local
  pub(crate) is_local: fn(&T, NodeId) -> bool,
  /// What a declaration that is a local declares, by its node and the kind of it: a variable, a parameter of some
  /// kind, else `Other`
  pub(crate) local_of: fn(&T, NodeId, L::Node) -> LocalBinding,
  /// What the site of a name of the kind makes of it beside its binding
  pub(crate) site_of: SiteOf<T>,
  /// Whether the name of a declaration may name a variable its scope already has, which the scopes then bind it to: a
  /// name a `def` of Starlark assigns again
  pub(crate) rebinds: fn(&T, NodeId) -> bool,
  /// Whether the language gives a name a meaning no declaration shows
  pub(crate) is_implicit: fn(&str) -> bool,
  /// How a name is used
  pub(crate) access_of: fn(&T, NodeId, ReferenceKind) -> Access,
  /// Where the name of a leaf starts: the leaf's start but where its text spells more than the name (C#'s `@`)
  pub(crate) name_start: fn(&T, NodeId) -> usize,
  /// Whether the scopes read only a part of the code of the tree (`navigate::Names::reads_in_part`): its parser
  /// reported an error, or the language compiles the text by conditions
  pub(crate) reads_in_part: fn(&T) -> bool,
  /// Whether a node is a statement or a member (`statement_of`, `member_of`)
  pub(crate) is_statement: fn(&T, NodeId) -> bool,
  /// Whether a statement or a member declares a type, which holds members: a class, an interface, a namespace
  pub(crate) is_type: fn(&T, NodeId) -> bool,
  /// Whether a node is a member of a type, or the body that holds its members: what the statement of the name of
  /// the type is without
  pub(crate) is_member: fn(&T, NodeId) -> bool,
}

/// The statement of the declared name `leaf`, which is not local (`navigate`, "The statements of a name"): the
/// innermost node around it that `is_statement` (the root where none is), from its first token to its last one,
/// without the children of the nodes from the name up to it that stand after the name and are members of a type
/// (`is_member`)
pub(crate) fn statement_of<L: TreeLanguage>(tree: &Tree<L>, leaf: NodeId, is_statement: impl Fn(NodeId) -> bool, is_member: impl Fn(NodeId) -> bool) -> Vec<Range<usize>> {
  let after = tree.range(leaf).end;
  let mut members = Vec::new();
  let mut statement = leaf;
  while let Some(parent) = tree.parent(statement) {
    statement = parent;
    members.extend(
      tree
        .children(parent)
        .filter(|&child| tree.range(child).start >= after && !tree.is_leaf(child) && is_member(child))
        .map(|child| tree.declaration_range(child)),
    );
    if is_statement(parent) {
      break;
    }
  }
  without_bodies(tree.declaration_range(statement), members)
}

/// The member around the name `leaf` of a local (`navigate`, "The statements of a name"): the outermost node around
/// it that `is_statement` and declares no type (`is_type`), from its first token to its last one; none where no
/// such node is around the name
pub(crate) fn member_of<L: TreeLanguage>(tree: &Tree<L>, leaf: NodeId, is_statement: impl Fn(NodeId) -> bool, is_type: impl Fn(NodeId) -> bool) -> Vec<Range<usize>> {
  let member = std::iter::successors(tree.parent(leaf), |&node| tree.parent(node))
    .filter(|&node| is_statement(node) && !is_type(node))
    .last();
  member.map_or_default(|member| vec![tree.declaration_range(member)])
}

/// A language's scopes for `resolve_names`: the walk asks them what a node is to the name it resolves
pub(crate) trait Scoping<L: TreeLanguage> {
  /// Whether the walk skips the subtree of a node: no code (a doc comment, a structured trivia)
  fn skips(&self, node: NodeId) -> bool;

  /// The binding of a name: by the scopes of the walk, or one they don't give (a label, an annotation's element)
  fn binding(&mut self, leaf: NodeId) -> Binding;

  /// The name of the name the walk resolves that a composite declares where the walk meets it (not hoisted)
  fn declared(&self, node: NodeId, kind: L::Node) -> Option<NodeId>;

  /// The scope around a composite that declares a name where the walk meets it, when it is not the innermost: the
  /// `switch` of a statement of its section
  fn declares_in(&self, _node: NodeId) -> Option<NodeId> {
    None
  }

  /// Whether a composite opens a scope
  fn is_scope(&self, node: NodeId, kind: L::Node) -> bool;

  /// Whether the name `leaf` of the kind sees the declaration `declared` of the scope `scope` around it: not from a
  /// part of the scope's node that the language reads outside it (the collection of a `foreach`, the default of a
  /// parameter of Starlark), and not a declaration of another kind of thing than the name reads (a variable for a type)
  fn sees(&self, _scope: NodeId, _declared: NodeId, _leaf: NodeId, _kind: ReferenceKind) -> bool {
    true
  }

  /// The kind a name of the kind is read as where the scopes declare nothing of that kind for it: a constant for the
  /// type of C#'s `x is Max`
  fn else_reads(&self, _leaf: NodeId, _kind: ReferenceKind) -> Option<ReferenceKind> {
    None
  }

  /// The node whose scope a leaf opens (Java's `{` of an anonymous class)
  fn opens_at(&self, _leaf: NodeId) -> Option<NodeId> {
    None
  }

  /// The walk reaches the node `id`, which it may skip
  fn reached(&mut self, _id: u32) {}

  /// The walk enters a composite
  fn entered(&mut self, _node: NodeId, _kind: L::Node) {}
}

/// Every name the resolution looks for in the order of the text, with its kind and its binding by the language's
/// scopes; up to `until` only, when given
pub(crate) fn resolve<L: TreeLanguage, T>(reading: &Reading<L, T>, language_tree: &T, wanted: &Wanted<'_>, until: Option<NodeId>) -> Vec<Named> {
  let tree = (reading.psi)(language_tree);
  let is_named = |node: NodeId| {
    (reading.is_named)(language_tree, node, wanted.name)
      || (!wanted.also.is_empty() && tree.is_leaf(node) && (reading.is_name)(language_tree, node) && wanted.starts_at((reading.name_start)(language_tree, node)))
  };
  let hoisted = |scope: NodeId| (reading.hoisted)(language_tree, scope, &is_named);
  let mut scoping = (reading.scopes)(language_tree, &is_named);
  let count = u32::try_from(tree.len()).unwrap_or(u32::MAX);
  let mut names = Vec::new();
  let mut frames = vec![Frame {
    node: tree.root(),
    end: count,
    declared: hoisted(tree.root()),
  }];
  let mut id = 0;
  while id < count {
    let node = NodeId(id);
    while frames.len() > 1 && frames.last().is_some_and(|it| it.end <= id) {
      frames.pop();
    }
    scoping.reached(id);
    if scoping.skips(node) {
      id = tree.subtree_end(node).0;
      continue;
    }
    if tree.is_leaf(node) {
      if let Some(scope) = scoping.opens_at(node) {
        frames.push(Frame {
          node: scope,
          end: tree.subtree_end(scope).0,
          declared: hoisted(scope),
        });
      }
      if is_named(node) {
        let syntactic = kind_of(reading, language_tree, node).0;
        // a binding the scopes of the walk don't give: a label, an annotation's element
        let given = match syntactic {
          ReferenceKind::Declaration | ReferenceKind::Member | ReferenceKind::MemberCall => None,
          _ => match scoping.binding(node) {
            Binding::Bound(binding) => Some(binding),
            Binding::ByScopes => None,
          },
        };
        let by_scopes = |kind: ReferenceKind| declaration_in(&frames, tree.range(node).start, tree, |scope, declared| scoping.sees(scope, declared, node, kind));
        let (kind, binding) = match syntactic {
          // a name its scope declares again is a use of the variable the scope has: its first declaration
          ReferenceKind::Declaration if (reading.rebinds)(language_tree, node) => match by_scopes(ReferenceKind::Value).filter(|&it| it != node) {
            Some(first) => (ReferenceKind::Value, Some(first)),
            None => (syntactic, Some(node)),
          },
          ReferenceKind::Declaration => (syntactic, Some(node)),
          ReferenceKind::Member | ReferenceKind::MemberCall => (syntactic, None),
          _ => (
            syntactic,
            given.unwrap_or_else(|| by_scopes(syntactic).or_else(|| scoping.else_reads(node, syntactic).and_then(by_scopes))),
          ),
        };
        names.push(Named { leaf: node, kind, binding });
        if until == Some(node) {
          break;
        }
      }
      id += 1;
      continue;
    }
    if let Some(kind) = tree.node_kind(node) {
      if let Some(declared) = scoping.declared(node, kind) {
        let scope = scoping.declares_in(node);
        let frame = scope.and_then(|scope| frames.iter().rposition(|it| it.node == scope)).unwrap_or(frames.len() - 1);
        frames[frame].declared.push(declared);
      }
      scoping.entered(node, kind);
      if scoping.is_scope(node, kind) {
        frames.push(Frame {
          node,
          end: tree.subtree_end(node).0,
          declared: hoisted(node),
        });
      }
    }
    id += 1;
  }
  names
}

/// The name at `position` of a text and what its syntax says (`navigate::reference_at`)
pub(crate) fn reference_at<L: TreeLanguage, T>(reading: &Reading<L, T>, text: &str, position: Position) -> Option<ReferenceAt> {
  let (tree, leaf) = parse_with_name_at(reading, text, position)?;
  let psi = (reading.psi)(&tree);
  let name = (reading.name)(&tree, leaf);
  let (kind, qualifier) = kind_of(reading, &tree, leaf);
  let resolved = || resolve(reading, &tree, &Wanted::named(name), Some(leaf)).pop();
  let (kind, local) = match kind {
    // the name of a declaration that names a variable its scope already has is a use of it
    ReferenceKind::Declaration if (reading.rebinds)(&tree, leaf) => resolved()
      .filter(|it| it.kind != ReferenceKind::Declaration)
      .map_or((kind, None), |it| (it.kind, it.binding)),
    ReferenceKind::Declaration | ReferenceKind::Member | ReferenceKind::MemberCall => (kind, None),
    _ => (kind, resolved().and_then(|it| it.binding)),
  };
  Some(reference_answer(
    text,
    psi.range(leaf).start,
    psi.utf8_text(leaf).unwrap_or(name).to_string(),
    kind,
    qualifier.map(|it| (reading.qualifier_text)(&tree, it)),
    // where the name of the declaration starts, not its leaf: the text may spell more (C#'s `@`, a Starlark `load`'s quotes)
    local.map(|it| (reading.name_start)(&tree, it)),
  ))
}

/// The answer of `read` for the names of a parse of `text` in a language's reading (`navigate::names_at`, `usages_at`,
/// `usages_of`)
pub(crate) fn with_names<L: TreeLanguage, T, R>(reading: &Reading<L, T>, text: &str, read: impl FnOnce(&dyn Names) -> R) -> R {
  read(&TreeNames {
    reading,
    tree: (reading.parse)(text),
    leaves: Leaves::default(),
    in_part: OnceCell::new(),
  })
}

/// A tree in a language's reading, for the names of `navigate::Names`
struct TreeNames<'r, L: TreeLanguage, T> {
  reading: &'r Reading<L, T>,
  tree: T,
  leaves: Leaves,
  /// `Reading::reads_in_part` of the tree, read once
  in_part: OnceCell<bool>,
}

impl<L: TreeLanguage, T> Names for TreeNames<'_, L, T> {
  fn name_at(&self, offset: usize) -> Option<(usize, &str)> {
    let leaf = name_leaf_at(self.reading, &self.tree, &self.leaves, offset)?;
    Some(((self.reading.name_start)(&self.tree, leaf), (self.reading.name)(&self.tree, leaf)))
  }

  fn resolve(&self, wanted: &Wanted<'_>) -> Vec<Resolved> {
    let (reading, tree) = (self.reading, &self.tree);
    let resolved = resolve(reading, tree, wanted, None).into_iter().map(|it| {
      let local_of = |declared: NodeId| parent_kind((reading.psi)(tree), declared).map_or(LocalBinding::OTHER, |(declaration, kind)| (reading.local_of)(tree, declaration, kind));
      let binding = it.binding.map(|declared| Bound {
        start: (reading.name_start)(tree, declared),
        local: (reading.is_local)(tree, declared).then(|| local_of(declared)),
      });
      let local = it.binding.zip(binding.and_then(|it| it.local));
      Resolved {
        start: (reading.name_start)(tree, it.leaf),
        access: (reading.access_of)(tree, it.leaf, it.kind),
        site: (reading.site_of)(tree, it.leaf, it.kind, local),
        binding,
      }
    });
    resolved.collect()
  }

  fn is_implicit(&self, name: &str) -> bool {
    (self.reading.is_implicit)(name)
  }

  fn reads_in_part(&self) -> bool {
    *self.in_part.get_or_init(|| (self.reading.reads_in_part)(&self.tree))
  }

  fn statements_of(&self, starts: &[usize], local: bool) -> Vec<Vec<Range<usize>>> {
    let (reading, tree) = (self.reading, &self.tree);
    let (psi, is_statement) = ((reading.psi)(tree), |node: NodeId| (reading.is_statement)(tree, node));
    let statement = |&start: &usize| {
      name_leaf_at(reading, tree, &self.leaves, start).map_or_default(|leaf| {
        if local {
          member_of(psi, leaf, is_statement, |node| (reading.is_type)(tree, node))
        } else {
          statement_of(psi, leaf, is_statement, |node| (reading.is_member)(tree, node))
        }
      })
    };
    starts.iter().map(statement).collect()
  }
}

/// The tree of a text and the leaf of the name at `position` in it
fn parse_with_name_at<L: TreeLanguage, T>(reading: &Reading<L, T>, text: &str, position: Position) -> Option<(T, NodeId)> {
  let offset = byte_offset(text, position)?;
  let tree = (reading.parse)(text);
  let leaf = name_leaf_at(reading, &tree, &Leaves::default(), offset)?;
  Some((tree, leaf))
}

/// The leaf of the name at the byte `offset` of a tree: the character at the place, else the one before it, as the end
/// of a name is still the name (`foo|`)
fn name_leaf_at<L: TreeLanguage, T>(reading: &Reading<L, T>, tree: &T, leaves: &Leaves, offset: usize) -> Option<NodeId> {
  let psi = (reading.psi)(tree);
  let is_name = |leaf: &NodeId| (reading.is_name)(tree, *leaf);
  leaves
    .at(psi, offset)
    .filter(is_name)
    .or_else(|| offset.checked_sub(1).and_then(|before| leaves.at(psi, before)).filter(is_name))
}

/// The kind of a name by the node around it, and its qualifier: of a declaration's name, else as the language reads the
/// node it is a part of (`Reading::kind_in`)
pub(crate) fn kind_of<L: TreeLanguage, T>(reading: &Reading<L, T>, tree: &T, leaf: NodeId) -> Answer {
  match parent_kind((reading.psi)(tree), leaf) {
    Some((_, kind)) if (reading.declares)(kind) => (ReferenceKind::Declaration, None),
    Some((parent, kind)) => (reading.kind_in)(tree, leaf, parent, kind),
    None => (ReferenceKind::Value, None),
  }
}

/// The parent of a leaf and its kind, a composite's
pub(crate) fn parent_kind<L: TreeLanguage>(tree: &Tree<L>, leaf: NodeId) -> Option<(NodeId, L::Node)> {
  let parent = tree.parent(leaf)?;
  Some((parent, tree.node_kind(parent)?))
}

/// The access a name has by its kind alone: a declaration, a call; `None` where its place decides
pub(crate) const fn access_by_kind(kind: ReferenceKind) -> Option<Access> {
  match kind {
    ReferenceKind::Declaration => Some(Access::Declaration),
    ReferenceKind::Call | ReferenceKind::MemberCall => Some(Access::Call),
    _ => None,
  }
}

/// Whether `node` is `part` or lies inside it
pub(crate) fn is_within<L: TreeLanguage>(tree: &Tree<L>, node: NodeId, part: NodeId) -> bool {
  part.0 <= node.0 && node.0 < tree.subtree_end(part).0
}
