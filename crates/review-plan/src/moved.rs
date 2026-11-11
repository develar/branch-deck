//! Whether a declaration that stands at another place of its file, with the same code, changed nothing but its place
//! (openspec/specs/branch/review-units/spec.md, "Units: A pair is shape, behaviour, moved or whitespace"). Only then
//! is the move one a review folds: a fold is never wrong, and where a declaration stands is behaviour wherever the
//! order of the declarations of a file means something.
//!
//! - **A place without a meaning** (`place_free`): a function, a type or a type alias of Kotlin, Java, C# and Rust,
//!   which name their declarations whatever their order. Of TypeScript and JavaScript a type alias, and a function
//!   that is hoisted: a `function` declaration, a method, a signature; never a function a `const` holds, a class or
//!   an enum, which are there only from their line on. Never a value or a constant (its initializer runs in its
//!   turn), an enum entry (its ordinal), a macro of Rust (in scope only below its definition), a module, a binding,
//!   a target, and nothing of Starlark, whose file runs from its first line to its last.
//! - **A decorator** of TypeScript and JavaScript gives the place of what it stands on a meaning (`ordering_token`):
//!   the decorators of the members of a class are evaluated and applied in the order of the members, each a call
//!   of the program's own code, so a method that moved with its decorator, or with a decorator of a parameter, is
//!   read. A framework that registers what its decorators see keeps their order: the routes of a controller are
//!   matched in the order of its methods, and a route with a parameter moved above one with a fixed path takes its
//!   requests. A method without a decorator may cross decorated ones: their order among themselves stays.
//! - **What it moved across** (`crossable`): a move puts a declaration before or after each declaration between its
//!   two places. That changes nothing when no order between the two has a meaning: across any declaration of Kotlin,
//!   Java and C# but an enum entry; of Rust not across a macro or a module either (a module may bring macros into
//!   scope); of TypeScript and JavaScript not across a declaration of its own name, the overloads of a function
//!   being tried in their order. Code between the two places that is no declaration beside it (a statement, an
//!   initializer block, the end of an `impl` and the start of the next) is crossed by no fold.

use code_symbols::highlight::Grammar;
use code_symbols::summary::DeclKind;
use trial_check::symbol_dependencies::declaration_checks::DeclarationForm;

/// What starts the header of a variable of TypeScript and JavaScript (`Declaration::header`): the keyword of its
/// statement
const VARIABLE_KEYWORDS: [&str; 5] = ["const", "let", "var", "using", "await"];

/// Whether the place of the declaration `form` among the declarations beside it has no meaning in the language of
/// `grammar`
pub(crate) fn place_free(grammar: Grammar, form: &DeclarationForm) -> bool {
  match grammar {
    Grammar::Kotlin | Grammar::KotlinScript | Grammar::Java | Grammar::CSharp | Grammar::Rust => {
      matches!(form.kind, DeclKind::Function | DeclKind::Type | DeclKind::TypeAlias)
    }
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => match form.kind {
      DeclKind::TypeAlias => true,
      // hoisted, unless a variable holds it: the header of a variable starts with the keyword of its statement
      DeclKind::Function => form
        .form
        .header
        .as_deref()
        .and_then(|header| header.split(' ').next())
        .is_some_and(|first| !VARIABLE_KEYWORDS.contains(&first)),
      _ => false,
    },
    Grammar::Starlark | Grammar::StarlarkBuild => false,
  }
}

/// The token that gives the place of the declaration it stands on a meaning, whatever the declaration is, in the
/// language of `grammar`: the `@` of a decorator of TypeScript and JavaScript, which is no other token of their
/// code. None in the other languages: no order of the annotations of two declarations of Kotlin, Java and C# and of
/// the attributes of two items of Rust is defined
pub(crate) const fn ordering_token(grammar: Grammar) -> Option<&'static str> {
  match grammar {
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => Some("@"),
    Grammar::Kotlin | Grammar::KotlinScript | Grammar::Java | Grammar::CSharp | Grammar::Rust | Grammar::Starlark | Grammar::StarlarkBuild => None,
  }
}

/// Whether the declaration `moved`, whose place has no meaning, may stand on the other side of `crossed`, a
/// declaration beside it, with nothing changed
pub(crate) fn crossable(grammar: Grammar, moved: &DeclarationForm, crossed: &DeclarationForm) -> bool {
  match grammar {
    Grammar::Kotlin | Grammar::KotlinScript | Grammar::Java | Grammar::CSharp => crossed.kind != DeclKind::EnumEntry,
    Grammar::Rust => !matches!(crossed.kind, DeclKind::EnumEntry | DeclKind::Macro | DeclKind::Module),
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => crossed.key != moved.key,
    Grammar::Starlark | Grammar::StarlarkBuild => false,
  }
}
