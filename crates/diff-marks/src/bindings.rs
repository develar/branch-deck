//! What the renames of a diff ask of the bindings of a text (`marks_renames`; openspec/specs/commit/name-bindings/spec.md):
//! the name at each of many places of one parse, what a body declares it as, every name bound as it is, and what
//! another name for it would clash with. The bindings are the reader's (`Bindings`): Branch Deck answers them by the
//! navigation of `code-symbols` (`file_diffs::bindings`), another reader by its own scopes. These types are what the
//! marks read of an answer, nothing more.

use code_tokens::Grammar;
use std::ops::Range;

/// The bindings of the names of a text, by its grammar
pub trait Bindings {
  /// The names at `questions` of `text`, read by `grammar`, from one parse, within `resolutions` resolutions of a
  /// name (one per spelling asked and one more per binding asked with another name): a place past them is not
  /// answered. Nothing is answered for no question or no resolution, nor for a text the reader has no tree of
  fn names_at_within(&self, grammar: Grammar, text: &str, questions: &[NameQuestion<'_>], resolutions: usize) -> NamesAt;

  /// The work of one resolution of a name in a text of `grammar` with `tokens` tokens: the tokens of the text where
  /// the reader walks the tree, a share of them where it reads the names of one spelling
  fn resolution_work(&self, grammar: Grammar, tokens: usize) -> u64;
}

/// A place in a text: a line from 0 and a UTF-16 column, as the view counts them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
  pub line: u32,
  pub column: u32,
}

/// A place asked of `Bindings::names_at_within`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameQuestion<'a> {
  pub position: Position,
  /// Another name for the binding of the place: the answer says whether it would clash (`NameAt::clashes`)
  pub renamed_to: Option<&'a str>,
}

/// How a use uses its name
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
  Declaration,
  Import,
  Call,
  Write,
  Read,
}

/// What the site of a name makes of it beside the binding the scopes give it: another name there changes more than
/// the binding, or the binding is not what the site names. One per name, the first of this order that holds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
  /// The name of the binding and nothing else
  Plain,
  /// The name is also the property the site reads or sets (`{ total }`)
  Shorthand,
  /// The name of a named argument: the parameter of what is called
  NamedArgument,
  /// A label, or the name of a jump to it
  Label,
  /// Only the types, or only what runs, tell whether the name is the binding the scopes give it
  Namesake,
  /// The spelling of the name is a value there (`nameof(total)`)
  Spelled,
  /// Among the tokens of a macro, which may read the name as it likes
  Macro,
  /// A pattern that takes a value apart declares or assigns the name
  Destructured,
}

/// What a body declares a local as
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalKind {
  Variable,
  Parameter,
  LambdaParameter,
  LoopVariable,
  CatchParameter,
  /// Any other declaration inside code: a local function, a local type, a type parameter, a label
  Other,
}

/// A declaration inside code, which no other file sees
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Local {
  pub kind: LocalKind,
  /// It is a value of its body alone, every name of which the scopes bind: no code outside its body may name it and
  /// the scopes read its text whole. Only of such a declaration do the scopes of the two sides of a change prove that
  /// another name for it changes nothing
  pub of_body_alone: bool,
}

/// A name bound to the binding asked, or of the text asked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameUse {
  pub start: Position,
  pub end_column: u32,
  pub access: Access,
  pub site: Site,
}

/// The name of a place: its binding, and every name of the text bound as it is
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameAt {
  pub name: String,
  /// The declaration inside code the name is bound to; `None` for any other name
  pub local: Option<Local>,
  /// In the order of the text: for a local the names its scopes bind to it, its declaration among them; else every
  /// name of that text that no local binds
  pub uses: Vec<NameUse>,
  /// The places the name asked as `NameQuestion::renamed_to` would change the meaning of; `None` where none was
  /// asked or the name is no local, 0 where the rename changes no binding the scopes show
  pub clashes: Option<usize>,
  /// For a place asked with another name: the lines (from 0, the end excluded, in order) of the code that says what
  /// the name is and gives it its value: the member around the uses of a local with the heads of the types around
  /// it, the statements of the declarations of any other name. `None` where no other name was asked
  pub statements: Option<Vec<Range<u32>>>,
}

/// The answers of `Bindings::names_at_within`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamesAt {
  /// For each place asked, in the order asked, its entry of `names`; `None` where no name is. The places of one
  /// binding asked with the same new name share an entry, as the places of one text that no local binds
  pub asked: Vec<Option<usize>>,
  pub names: Vec<NameAt>,
  /// The resolutions the answers took
  pub resolutions: usize,
}
