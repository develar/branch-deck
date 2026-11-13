//! C# through psi-syntax's `csharp-syntax`: a port of Roslyn's own lexer and parser (`Microsoft.CodeAnalysis.CSharp`
//! 5.9.0) whose tree is Roslyn's syntax tree (`ClassDeclaration`, `MethodDeclaration`, `InvocationExpression`,
//! `IdentifierName`, …) on psi-builder's preorder tree: a token's trivia are leaves beside it, a missing token an empty
//! leaf, a directive or a documentation comment a subtree of structured trivia. What a Roslyn node answers of its slots
//! (`ClassDeclarationSyntax.Identifier`, `UsingDirectiveSyntax.Alias`, …) and whether a name stands where C# reads a
//! type (`is_in_type_slot`) come from their ports in `csharp_syntax::syntax_getters`, the tree reads every consumer
//! shares (trivia, a declaration's documentation comment, the identifier and the text of a name) from
//! `csharp_syntax::read`, never from a walk of this module. `parse` is the one call into the parser;
//! `summary` makes the `FileSummary`, `highlight` the `LineRuns`, `navigate` the answers of Go to Declaration and Find
//! Usages, each by the parser-neutral builders, as Java's (`java`).
//!
//! - **The text** is the file's bytes as they are: a UTF-8 byte order mark is the first token's leading whitespace,
//!   every line terminator is Roslyn's. The tree's offsets index its own text (`Tree::source`), which is the file's but
//!   where Roslyn drops a token (one case of `ParseConversionOperatorDeclaration`, in no corpus file); a walk slices the
//!   tree's text, never the file's.
//! - **The file kind**: a `.cs` file at `LanguageVersion.Latest` with its documentation comments parsed
//!   (`FileKind::default()`, `DocumentationMode.Parse`), so a `///` comment's XML is a tree the highlighting paints.
//! - **One preprocessor configuration**: no symbol defined, as `CSharpSyntaxTree.ParseText` without
//!   `WithPreprocessorSymbols` reads a file. A branch of `#if` it does not take is disabled text, a comment to every
//!   consumer: its declarations and references are not the file's (accio reads a second configuration; a review of a
//!   branch reads the code the build without symbols compiles).
//! - **Bounded by the parser**: any bytes parse, losslessly and without a panic, on a 1 MB stack; past Roslyn's nesting
//!   (a depth counter of 1000 levels) or the parser's work and arena budgets the whole file is one skipped token, as
//!   Roslyn answers an exhausted stack. The threads that parse have 2 MiB or more (`kotlin`, "Bounded by the parser").
//! - **The size limits** stay the callers': `extract::{MAX_BYTES, MAX_LINES, MAX_AVERAGE_LINE}` before a summary,
//!   `highlight::MAX_BYTES` before the highlighting.

pub(crate) mod highlight;
pub(crate) mod navigate;
pub(crate) mod summary;

#[cfg(test)]
mod parse_test;

use csharp_syntax::NodeKind;

/// A type declaration (an extension block too): its members and type parameters are seen in all of it
pub(crate) const fn is_type_declaration(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::ClassDeclaration
      | NodeKind::StructDeclaration
      | NodeKind::InterfaceDeclaration
      | NodeKind::RecordDeclaration
      | NodeKind::RecordStructDeclaration
      | NodeKind::EnumDeclaration
      | NodeKind::UnionDeclaration
      | NodeKind::ExtensionBlockDeclaration
  )
}

/// A node of code that runs, whose parameters and declarations are locals: a member with a body, a delegate's
/// parameters, an accessor, a local function, a lambda, an anonymous method, a top-level statement
pub(crate) const fn is_code(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::MethodDeclaration
      | NodeKind::ConstructorDeclaration
      | NodeKind::DestructorDeclaration
      | NodeKind::OperatorDeclaration
      | NodeKind::ConversionOperatorDeclaration
      | NodeKind::IndexerDeclaration
      | NodeKind::DelegateDeclaration
      | NodeKind::GetAccessorDeclaration
      | NodeKind::SetAccessorDeclaration
      | NodeKind::InitAccessorDeclaration
      | NodeKind::AddAccessorDeclaration
      | NodeKind::RemoveAccessorDeclaration
      | NodeKind::UnknownAccessorDeclaration
      | NodeKind::LocalFunctionStatement
      | NodeKind::SimpleLambdaExpression
      | NodeKind::ParenthesizedLambdaExpression
      | NodeKind::AnonymousMethodExpression
      | NodeKind::GlobalStatement
  )
}

/// A statement or a clause whose locals no name outside it sees: a block, a loop, a `using`, a `fixed`, a `catch`, a
/// section or an arm of a `switch`, a query. A variable an expression declares (`out var x`, `o is string s`) belongs to
/// the statement list around it, as C# scopes it, so no other statement opens a scope
pub(crate) const fn is_block_scope(kind: NodeKind) -> bool {
  matches!(
    kind,
    NodeKind::Block
      | NodeKind::ForStatement
      | NodeKind::ForEachStatement
      | NodeKind::ForEachVariableStatement
      | NodeKind::UsingStatement
      | NodeKind::FixedStatement
      | NodeKind::CatchClause
      | NodeKind::SwitchSection
      | NodeKind::SwitchExpressionArm
      | NodeKind::QueryExpression
  )
}
