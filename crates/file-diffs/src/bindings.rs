//! The bindings of the names of a text that the renames of a diff ask (`diff_marks::bindings::Bindings`), answered by
//! the navigation of `code-symbols` (`code_symbols::navigate::names_at_within`, openspec/specs/commit/name-bindings/spec.md):
//! its answers as the marks read them.

use code_symbols::navigate::{self, LocalBinding, LocalKind};
use code_tokens::Grammar;
use diff_marks::bindings::{Access, Bindings, Local, NameAt, NameQuestion, NameUse, NamesAt, Position, Site};

/// The bindings of `code-symbols`
#[derive(Debug, Clone, Copy, Default)]
pub struct CodeSymbolBindings;

impl Bindings for CodeSymbolBindings {
  fn names_at_within(&self, grammar: Grammar, text: &str, questions: &[NameQuestion<'_>], resolutions: usize) -> NamesAt {
    let questions: Vec<navigate::NameQuestion<'_>> = questions
      .iter()
      .map(|question| navigate::NameQuestion {
        position: navigate::Position {
          line: question.position.line,
          column: question.position.column,
        },
        renamed_to: question.renamed_to,
      })
      .collect();
    let answer = navigate::names_at_within(grammar, text, &questions, resolutions);
    NamesAt {
      asked: answer.asked,
      names: answer.names.into_iter().map(name_at).collect(),
      resolutions: answer.resolutions,
    }
  }

  fn resolution_work(&self, grammar: Grammar, tokens: usize) -> u64 {
    navigate::resolution_work(grammar, tokens)
  }
}

fn name_at(name: navigate::NameAt) -> NameAt {
  NameAt {
    name: name.name,
    local: name.local.map(local),
    uses: name.uses.iter().map(name_use).collect(),
    clashes: name.clashes.as_ref().map(Vec::len),
    statements: name.statements,
  }
}

const fn local(binding: LocalBinding) -> Local {
  Local {
    kind: match binding.kind {
      LocalKind::Variable => diff_marks::bindings::LocalKind::Variable,
      LocalKind::Parameter => diff_marks::bindings::LocalKind::Parameter,
      LocalKind::LambdaParameter => diff_marks::bindings::LocalKind::LambdaParameter,
      LocalKind::LoopVariable => diff_marks::bindings::LocalKind::LoopVariable,
      LocalKind::CatchParameter => diff_marks::bindings::LocalKind::CatchParameter,
      LocalKind::Other => diff_marks::bindings::LocalKind::Other,
    },
    of_body_alone: binding.is_of_body_alone(),
  }
}

const fn name_use(used: &navigate::NameUse) -> NameUse {
  NameUse {
    start: Position {
      line: used.start.line,
      column: used.start.column,
    },
    end_column: used.end_column,
    access: match used.access {
      navigate::Access::Declaration => Access::Declaration,
      navigate::Access::Import => Access::Import,
      navigate::Access::Call => Access::Call,
      navigate::Access::Write => Access::Write,
      navigate::Access::Read => Access::Read,
    },
    site: match used.site {
      navigate::Site::Plain => Site::Plain,
      navigate::Site::Shorthand => Site::Shorthand,
      navigate::Site::NamedArgument => Site::NamedArgument,
      navigate::Site::Label => Site::Label,
      navigate::Site::Namesake => Site::Namesake,
      navigate::Site::Spelled => Site::Spelled,
      navigate::Site::Macro => Site::Macro,
      navigate::Site::Destructured => Site::Destructured,
    },
  }
}
