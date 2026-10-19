//! The scopes a walk is in, for what IntelliJ resolves and a syntax tree only shows by its scopes: an unqualified name
//! that is a member of an enclosing class and no local or parameter in between is a field (`count++`), a name that an
//! enclosing declaration declares as a type parameter is one.

/// A scope: a class with its members, or a function or block with its locals
#[derive(Debug)]
struct Frame {
  node: usize,
  class: bool,
  /// The members of a class with their styles, or the locals of a function or block
  names: Vec<(String, u32)>,
  type_parameters: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Scopes {
  frames: Vec<Frame>,
}

impl Scopes {
  /// A class (or a file): `members` are its fields and properties with their styles, visible in its whole body
  pub(crate) fn push_class(&mut self, node: usize, members: Vec<(String, u32)>) {
    self.frames.push(Frame {
      node,
      class: true,
      names: members,
      type_parameters: Vec::new(),
    });
  }

  /// A function, a lambda or a block: its locals are bound as the walk meets them
  pub(crate) fn push_local(&mut self, node: usize) {
    self.frames.push(Frame {
      node,
      class: false,
      names: Vec::new(),
      type_parameters: Vec::new(),
    });
  }

  /// The walk leaves `node`: its scope ends, if it opened one
  pub(crate) fn pop(&mut self, node: usize) {
    if self.frames.last().is_some_and(|frame| frame.node == node) {
      self.frames.pop();
    }
  }

  /// A parameter or a local of the innermost function or block
  pub(crate) fn bind(&mut self, name: &str) {
    if let Some(frame) = self.frames.iter_mut().rev().find(|frame| !frame.class) {
      frame.names.push((name.to_string(), 0));
    }
  }

  /// A local whose uses have a style: a `mut` binding of Rust
  pub(crate) fn bind_styled(&mut self, name: &str, style: u32) {
    if let Some(frame) = self.frames.iter_mut().rev().find(|frame| !frame.class) {
      frame.names.push((name.to_string(), style));
    }
  }

  /// The style of `name` as a local or a parameter, if it is bound as one
  pub(crate) fn local(&self, name: &str) -> Option<u32> {
    for frame in self.frames.iter().rev() {
      if let Some((_, style)) = frame.names.iter().find(|it| it.0 == name) {
        return (!frame.class).then_some(*style);
      }
    }
    None
  }

  /// A type parameter of the innermost scope
  pub(crate) fn bind_type_parameter(&mut self, name: &str) {
    if let Some(frame) = self.frames.last_mut() {
      frame.type_parameters.push(name.to_string());
    }
  }

  /// The style of `name` as a member of an enclosing class with no local or parameter of that name in between
  pub(crate) fn member(&self, name: &str) -> Option<u32> {
    for frame in self.frames.iter().rev() {
      if let Some((_, style)) = frame.names.iter().find(|it| it.0 == name) {
        return frame.class.then_some(*style);
      }
    }
    None
  }

  /// Whether `name` is bound as a local or a parameter
  pub(crate) fn is_local(&self, name: &str) -> bool {
    for frame in self.frames.iter().rev() {
      if frame.names.iter().any(|it| it.0 == name) {
        return !frame.class;
      }
    }
    false
  }

  pub(crate) fn is_type_parameter(&self, name: &str) -> bool {
    self.frames.iter().any(|frame| frame.type_parameters.iter().any(|it| it == name))
  }

  /// Whether the walk is in a class body, not in a function of it
  pub(crate) fn in_class_body(&self) -> bool {
    self.frames.last().is_some_and(|frame| frame.class)
  }
}
