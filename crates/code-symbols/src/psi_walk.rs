//! The half of a summary walk over a psi-syntax tree (Java's PSI, Roslyn's tree of C#) that knows no language: one
//! scan over the preorder ids of the tree with an explicit stack of frames (`run`), so a file nested as deep as the
//! parser allows takes no stack per level, and the state every such walk keeps (`WalkCore`): the text and its lines,
//! the builder of the summary, the scopes of the bound names, the containers and the stack of supertypes, the frames.
//! What a node is to the summary (a declaration, a reference, a scope) is the language's walk's (`java::summary`,
//! `csharp::summary`); its frames carry what it keeps of a node (`Frame::extra`).

use crate::jvm::{Classes, Holder, JvmFields, TypedReceiver};
use crate::summary::{DeclKind, MemberKind, Reach, Role};
use crate::summary_builder::{Bindings, SummaryBuilder, Typed, blank_declaration, end_line};
use psi_builder::document::Lines;
use psi_builder::language::TreeLanguage;
use psi_builder::{NodeId, Tree};
use std::collections::HashMap;
use std::ops::Range;

/// Where a declaration lies: its first and last line and the line of its name
pub(crate) struct Span {
  pub(crate) start: u32,
  pub(crate) end: u32,
  pub(crate) name: u32,
}

/// A composite node the walk is inside, until the end of its subtree: what it opened, and what the language's walk
/// keeps of it
pub(crate) struct Frame<E> {
  /// The end of the node's subtree: the frame is left at this id
  pub(crate) end: u32,
  pub(crate) scopes: u8,
  pub(crate) container: bool,
  /// The entries this node pushed onto the supertypes of `WalkCore::classes`
  pub(crate) supertypes: u8,
  pub(crate) extra: E,
}

/// The state of a walk that no language decides: of a tree of the language `L`, with the frames' extra `E` and the
/// marks `M` a parent leaves on a node below it
pub(crate) struct WalkCore<'t, L: TreeLanguage, E, M> {
  pub(crate) tree: &'t Tree<L>,
  pub(crate) source: &'t [u8],
  pub(crate) lines: Lines,
  pub(crate) out: SummaryBuilder<'t>,
  pub(crate) bindings: Bindings,
  pub(crate) frames: Vec<Frame<E>>,
  /// The containers and the supertypes around the node
  pub(crate) classes: Classes,
  /// What a parent decided about a node below it before the walk reaches it, by its id
  pub(crate) marks: HashMap<u32, M>,
}

impl<'t, L: TreeLanguage, E: Default, M> WalkCore<'t, L, E, M> {
  pub(crate) fn new(tree: &'t Tree<L>) -> Self {
    let source = tree.source();
    Self {
      tree,
      source,
      lines: Lines::new(source),
      out: SummaryBuilder::new(),
      bindings: Bindings::default(),
      frames: Vec::new(),
      classes: Classes::default(),
      marks: HashMap::new(),
    }
  }

  /// The frame of the current node
  pub(crate) fn frame(&mut self) -> &mut Frame<E> {
    let index = self.frames.len() - 1;
    &mut self.frames[index]
  }

  /// Leaves the innermost frame: closes its scopes, its container and its supertypes; what the language's walk kept of
  /// it, to undo
  fn pop_frame(&mut self) -> Option<E> {
    let frame = self.frames.pop()?;
    let index = self.frames.len();
    self.bindings.leave_frame(index, frame.scopes);
    if frame.container {
      self.classes.leave();
    }
    for _ in 0..frame.supertypes {
      self.classes.pop_supertypes();
    }
    Some(frame.extra)
  }

  pub(crate) fn push_scope(&mut self, body: bool) {
    self.bindings.push(body);
    self.frame().scopes += 1;
  }

  pub(crate) fn push_supertypes(&mut self, supertypes: Vec<Vec<u32>>) {
    self.classes.push_supertypes(supertypes);
    self.frame().supertypes += 1;
  }

  /// Makes the declaration the container of what is below the current node
  pub(crate) fn enter_container(&mut self, decl: u32, holder: Holder) {
    self.classes.enter(Some(decl), holder, false);
    self.frame().container = true;
  }

  pub(crate) fn container(&self) -> Option<u32> {
    self.classes.container()
  }

  pub(crate) fn holder(&self) -> Option<Holder> {
    self.classes.holder()
  }

  /// The 1-based line of a byte of the text
  pub(crate) fn line(&self, offset: usize) -> u32 {
    self.lines.line(offset)
  }

  /// The last line of the text from `start` to `end`, a final line break on the line before
  pub(crate) fn end_line(&self, start: usize, end: usize) -> u32 {
    end_line(|offset| self.lines.line(offset), self.source, start, end)
  }

  /// The text of a range, `None` when it is not UTF-8
  pub(crate) fn text(&self, range: Range<usize>) -> Option<&'t str> {
    std::str::from_utf8(self.source.get(range)?).ok()
  }

  /// The text of a node as written, `None` when it is not UTF-8
  pub(crate) fn text_of(&self, id: NodeId) -> Option<&'t str> {
    self.text(self.tree.range(id))
  }

  /// The line a node starts on
  pub(crate) fn line_at(&self, id: NodeId) -> u32 {
    self.line(self.tree.range(id).start)
  }

  /// The span of a declaration from its own range, its name's start and the start of a doc comment above it
  pub(crate) fn span(&self, own: Range<usize>, name: usize, doc: Option<usize>) -> Span {
    let start = self.line(own.start);
    Span {
      start: doc.map_or(start, |it| start.min(self.line(it))),
      end: self.end_line(own.start, own.end),
      name: self.line(name),
    }
  }

  /// Adds a declaration of the name `name` in the current container, with its facts and lines
  pub(crate) fn declare(
    &mut self,
    name: &'t str,
    (kind, reach, member): (DeclKind, Reach, MemberKind),
    span: &Span,
    fields: JvmFields,
    header: Option<String>,
    in_error: bool,
  ) -> u32 {
    let name = self.out.intern(name);
    let mut declaration = fields.declaration(blank_declaration(name, kind, reach, member, self.container()));
    declaration.header = header;
    declaration.start_line = span.start;
    declaration.end_line = span.end;
    declaration.name_line = span.name;
    declaration.in_error = in_error;
    self.out.add_declaration(declaration)
  }

  /// A reference, unless it is a bare name bound around it; the language's walk drops the names that are no reference
  pub(crate) fn reference(&mut self, name: &'t str, line: u32, role: Role, qualifier: Option<Vec<u32>>, shape: Option<u32>, coroutine: Option<bool>) {
    let id = self.out.intern(name);
    if qualifier.is_none() && self.bindings.is_bound(id) {
      return;
    }
    let container = self.container();
    self.out.add_reference(id, line, role, qualifier, container, shape, coroutine);
  }

  /// A member reference qualified by `qualifier`, with the type of its receiver when it has one
  #[expect(clippy::too_many_arguments, reason = "the fields of a reference that a walk decides")]
  pub(crate) fn member_reference(
    &mut self,
    name: &'t str,
    line: u32,
    role: Role,
    qualifier: Vec<u32>,
    shape: Option<u32>,
    coroutine: Option<bool>,
    receiver: Option<TypedReceiver>,
  ) {
    let before = self.out.references.len();
    self.reference(name, line, role, Some(qualifier), shape, coroutine);
    if let Some(receiver) = receiver.filter(|_| self.out.references.len() > before) {
      receiver.give(&mut self.out, before);
    }
  }

  /// The qualifier of a member of the object whose path of names is `path`: its names, or empty for an expression
  /// (`None`) or a path from a local
  pub(crate) fn qualifier_ids(&mut self, path: Option<Vec<&'t str>>) -> Vec<u32> {
    let Some(path) = path else { return Vec::new() };
    let ids = self.out.ids(&path);
    if ids.first().is_some_and(|it| self.bindings.is_bound(*it)) { Vec::new() } else { ids }
  }

  /// What the receiver of a member reference may have a type by: the bound name `bound` with a declared type, or a
  /// property `[m]` or `[this, m]` of the qualifier, not enclosed
  pub(crate) fn typed_receiver(&self, bound: Option<&str>, qualifier: &[u32]) -> Option<TypedReceiver> {
    if let Some(id) = bound.and_then(|name| self.out.name_id(name))
      && self.bindings.is_bound(id)
    {
      return TypedReceiver::bound(self.bindings.binding(id)?);
    }
    TypedReceiver::property(&self.out, qualifier, self.classes.enclosed())
  }

  /// The `Typed` of a value of the type `path`, `constructed` when a constructor call gave it; none for a bound name (a
  /// type parameter)
  pub(crate) fn typed(&mut self, path: &[&'t str], constructed: bool) -> Option<Typed> {
    if path.len() == 1 && self.out.name_id(path[0]).is_some_and(|it| self.bindings.is_bound(it)) {
      return None;
    }
    Some(Typed {
      path: self.out.ids(path),
      constructed,
    })
  }
}

/// Whether the sorted ids `errors` hold one in the subtree from `start` to `end` (excluded)
pub(crate) fn has_error_in(errors: &[u32], start: u32, end: u32) -> bool {
  let index = errors.partition_point(|it| *it < start);
  errors.get(index).is_some_and(|it| *it < end)
}

/// A language's summary walk over a psi-syntax tree, which `run` drives
pub(crate) trait Walker<'t> {
  type Lang: TreeLanguage;
  type Extra: Default;
  type Mark;

  fn core(&mut self) -> &mut WalkCore<'t, Self::Lang, Self::Extra, Self::Mark>;

  /// A composite node whose frame was just pushed, with the mark a parent left on it; whether the walk goes into its
  /// subtree
  fn enter(&mut self, id: NodeId, mark: Option<Self::Mark>) -> bool;

  /// A leaf
  fn token(&mut self, _id: NodeId) {}

  /// The walk left a frame: what the walk kept of its node, to undo
  fn left(&mut self, _extra: Self::Extra) {}
}

/// Walks the tree in preorder, a frame per composite node, its frames left at the end of their subtrees
pub(crate) fn run<'t, W: Walker<'t>>(walk: &mut W) {
  let tree = walk.core().tree;
  let root = tree.root();
  let end = tree.subtree_end(root).0;
  let mut id = root.0;
  while id < end {
    while walk.core().frames.last().is_some_and(|it| it.end <= id) {
      leave(walk);
    }
    let node = NodeId(id);
    if tree.is_leaf(node) {
      walk.token(node);
      id += 1;
      continue;
    }
    let end = tree.subtree_end(node).0;
    walk.core().frames.push(Frame {
      end,
      scopes: 0,
      container: false,
      supertypes: 0,
      extra: W::Extra::default(),
    });
    let mark = walk.core().marks.remove(&id);
    id = if walk.enter(node, mark) { id + 1 } else { end };
  }
  while !walk.core().frames.is_empty() {
    leave(walk);
  }
}

fn leave<'t, W: Walker<'t>>(walk: &mut W) {
  if let Some(extra) = walk.core().pop_frame() {
    walk.left(extra);
  }
}
