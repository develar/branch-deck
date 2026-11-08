//! The half of Go to Declaration and Find Usages over a psi-syntax tree (Java's PSI, Roslyn's tree of C#) that knows
//! no language (spec/commit/symbol-navigation.spec.md): the answers of `reference_at`, `usages_at` and `usages_of` from
//! a language's reading of its tree (`Navigation`: its names, their kinds, the walk of its scopes), and the scopes of
//! such a walk (`Frame`, `declaration_in`). What a name is and which scopes a language has are the language's
//! (`java::navigate`, `csharp::navigate`).

use crate::navigate::{Access, Position, ReferenceAt, ReferenceKind, Usage, UsagesAt, byte_offset, reference_answer, usage_answers};
use psi_builder::language::TreeLanguage;
use psi_builder::{NodeId, Tree};

/// A name of a language's walk: its leaf, its kind and the name of the declaration it is bound to
pub(crate) struct Named {
  pub(crate) leaf: NodeId,
  pub(crate) kind: ReferenceKind,
  pub(crate) binding: Option<NodeId>,
}

/// A scope of the walk: the end of its subtree and the names of its declarations of the name, in the order of the text
pub(crate) struct Frame {
  pub(crate) end: u32,
  pub(crate) declared: Vec<NodeId>,
}

/// The declaration of the scopes whose name starts at `start`: in the innermost scope that declares one, the last one
/// before it, else the first (a member declared after its use)
pub(crate) fn declaration_in<L: TreeLanguage>(frames: &[Frame], start: usize, tree: &Tree<L>) -> Option<NodeId> {
  let frame = frames.iter().rev().find(|it| !it.declared.is_empty())?;
  frame
    .declared
    .iter()
    .rev()
    .find(|&&it| tree.range(it).start <= start)
    .or_else(|| frame.declared.first())
    .copied()
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
  /// How a name is used
  pub(crate) access_of: fn(&T, NodeId, ReferenceKind) -> Access,
  /// Where the name of a leaf starts: the leaf's start but where its text spells more than the name (C#'s `@`)
  pub(crate) name_start: fn(&T, NodeId) -> usize,
}

/// A language's scopes for `resolve_names`: the walk asks them what a node is to the name it resolves
pub(crate) trait Scoping<L: TreeLanguage> {
  /// Whether the walk skips the subtree of a node: no code (a doc comment, a structured trivia)
  fn skips(&self, node: NodeId) -> bool;

  /// The binding of a name: by the scopes of the walk, or one they don't give (a label, an annotation's element)
  fn binding(&mut self, leaf: NodeId) -> Binding;

  /// The name of the name the walk resolves that a composite declares where the walk meets it (not hoisted)
  fn declared(&self, node: NodeId, kind: L::Node) -> Option<NodeId>;

  /// Whether a composite opens a scope
  fn is_scope(&self, node: NodeId, kind: L::Node) -> bool;

  /// The node whose scope a leaf opens (Java's `{` of an anonymous class)
  fn opens_at(&self, _leaf: NodeId) -> Option<NodeId> {
    None
  }

  /// The walk reaches the node `id`, which it may skip
  fn reached(&mut self, _id: u32) {}

  /// The walk enters a composite
  fn entered(&mut self, _node: NodeId, _kind: L::Node) {}
}

/// Every name `name` of the tree in the order of the text, with its kind and its binding by the language's scopes; up
/// to `until` only, when given
pub(crate) fn resolve<L: TreeLanguage, T>(reading: &Reading<L, T>, language_tree: &T, name: &str, until: Option<NodeId>) -> Vec<Named> {
  let tree = (reading.psi)(language_tree);
  let is_named = |node: NodeId| (reading.is_named)(language_tree, node, name);
  let hoisted = |scope: NodeId| (reading.hoisted)(language_tree, scope, &is_named);
  let mut scoping = (reading.scopes)(language_tree, &is_named);
  let count = u32::try_from(tree.len()).unwrap_or(u32::MAX);
  let mut names = Vec::new();
  let mut frames = vec![Frame {
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
          end: tree.subtree_end(scope).0,
          declared: hoisted(scope),
        });
      }
      if is_named(node) {
        let kind = kind_of(reading, language_tree, node).0;
        let binding = match kind {
          ReferenceKind::Declaration => Some(node),
          ReferenceKind::Member | ReferenceKind::MemberCall => None,
          _ => match scoping.binding(node) {
            Binding::Bound(binding) => binding,
            Binding::ByScopes => declaration_in(&frames, tree.range(node).start, tree),
          },
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
      if let Some(declared) = scoping.declared(node, kind)
        && let Some(frame) = frames.last_mut()
      {
        frame.declared.push(declared);
      }
      scoping.entered(node, kind);
      if scoping.is_scope(node, kind) {
        frames.push(Frame {
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
  let local = match kind {
    ReferenceKind::Declaration | ReferenceKind::Member | ReferenceKind::MemberCall => None,
    _ => resolve(reading, &tree, name, Some(leaf)).last().and_then(|it| it.binding),
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

/// The usages of the name at `position` (`navigate::usages_at`): its own of a local, else every name of the text that no
/// local binds
pub(crate) fn usages_at<L: TreeLanguage, T>(reading: &Reading<L, T>, text: &str, position: Position) -> Option<UsagesAt> {
  let (tree, leaf) = parse_with_name_at(reading, text, position)?;
  let name = (reading.name)(&tree, leaf);
  let names = resolve(reading, &tree, name, None);
  let is_local = |binding: NodeId| (reading.is_local)(&tree, binding);
  let local = names.iter().find(|it| it.leaf == leaf).and_then(|it| it.binding).filter(|&it| is_local(it));
  let used = names.iter().filter(|it| match (local, it.binding) {
    (Some(local), binding) => binding == Some(local),
    (None, binding) => binding.is_none_or(|binding| !is_local(binding)),
  });
  let usages = usage_answers(
    text,
    name,
    used.map(|it| ((reading.name_start)(&tree, it.leaf), (reading.access_of)(&tree, it.leaf, it.kind))),
  );
  Some(UsagesAt {
    name: name.to_string(),
    local: local.is_some(),
    usages,
  })
}

/// The usages of `name` in a text that is not the one of the name (`navigate::usages_of`): its names that no local binds
pub(crate) fn usages_of<L: TreeLanguage, T>(reading: &Reading<L, T>, text: &str, name: &str) -> Vec<Usage> {
  // a cheap test first: most files of a review do not have the word
  if !text.contains(name) {
    return Vec::new();
  }
  let tree = (reading.parse)(text);
  let names = resolve(reading, &tree, name, None);
  let used = names.iter().filter(|it| it.binding.is_none_or(|binding| !(reading.is_local)(&tree, binding)));
  usage_answers(
    text,
    name,
    used.map(|it| ((reading.name_start)(&tree, it.leaf), (reading.access_of)(&tree, it.leaf, it.kind))),
  )
}

/// The tree of a text and the leaf of the name at `position` in it: the character at the place, else the one before
/// it, as the end of a name is still the name (`foo|`)
fn parse_with_name_at<L: TreeLanguage, T>(reading: &Reading<L, T>, text: &str, position: Position) -> Option<(T, NodeId)> {
  let offset = byte_offset(text, position)?;
  let tree = (reading.parse)(text);
  let leaf = {
    let leaves = (reading.psi)(&tree).leaf_index();
    let is_name = |leaf: &NodeId| (reading.is_name)(&tree, *leaf);
    leaves
      .at(offset)
      .filter(is_name)
      .or_else(|| offset.checked_sub(1).and_then(|before| leaves.at(before)).filter(is_name))?
  };
  Some((tree, leaf))
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
