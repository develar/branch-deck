//! Helpers of the tests: fixtures in `testdata/<language>/`, a text rendering of a summary for the golden files beside
//! them (`<fixture>.summary`), and lookups by name.
//!
//! A golden file is the whole summary of its fixture, one line per import, declaration and line of references, so a
//! rule change shows as a diff. `CODE_SYMBOLS_UPDATE=1 cargo nextest run -p code-symbols` rewrites them; review the diff.

use crate::extract::summarize;
use crate::language::language_of;
use crate::summary::{Argument, Constructor, Declaration, FileSummary, ImportTarget, Reach, Reference, Role, Shape, Signature, TypeRef};
use std::fmt::Write as _;
use std::path::PathBuf;

/// No references or declarations, for `assert_eq!` against a list
pub(crate) const NONE: [String; 0] = [];

pub(crate) fn fixture(name: &str) -> (String, Vec<u8>) {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata").join(name);
  let source = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
  (name.to_string(), source)
}

pub(crate) fn summary_of(name: &str) -> FileSummary {
  let (path, source) = fixture(name);
  let language = language_of(&path).unwrap_or_else(|| panic!("no language for {path}"));
  summarize(language, &source)
}

/// Compares the rendering of a fixture's summary with its golden file
pub(crate) fn check_golden(name: &str) {
  let summary = summary_of(name);
  let actual = render(&summary);
  let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata").join(format!("{name}.summary"));
  if std::env::var_os("CODE_SYMBOLS_UPDATE").is_some() {
    std::fs::write(&golden, &actual).unwrap();
    return;
  }
  let expected = std::fs::read_to_string(&golden).unwrap_or_default();
  pretty_assertions::assert_eq!(expected, actual, "{name}: rerun with CODE_SYMBOLS_UPDATE=1 to rewrite the golden file");
}

/// The names of `ids` joined by dots
fn path_text(summary: &FileSummary, ids: &[u32]) -> String {
  ids.iter().map(|it| summary.name(*it)).collect::<Vec<_>>().join(".")
}

pub(crate) fn render(summary: &FileSummary) -> String {
  let mut out = String::new();
  if let Some(package) = &summary.package {
    let _ = writeln!(out, "package {package}");
  }
  if let Some(jvm_name) = &summary.jvm_name {
    let _ = writeln!(out, "jvm_name {jvm_name}");
  }
  for import in &summary.imports {
    let target = match import.target {
      ImportTarget::Name(id) => summary.name(id).to_string(),
      ImportTarget::All => "*".to_string(),
      ImportTarget::Default => "default".to_string(),
      ImportTarget::Namespace => "namespace".to_string(),
    };
    let _ = write!(out, "import {}{}{target}", path_text(summary, &import.path), if import.path.is_empty() { "" } else { "." });
    if let Some(alias) = import.alias {
      let _ = write!(out, " as {}", summary.name(alias));
    }
    if let Some(specifier) = &import.specifier {
      let _ = write!(out, " from {specifier:?}");
    }
    let _ = writeln!(out, "{} @{}", if import.exported { " exported" } else { "" }, import.line);
  }
  for declaration in &summary.declarations {
    let container = declaration
      .container
      .and_then(|it| summary.declarations.get(it as usize))
      .map_or("-", |it| summary.name(it.name));
    let _ = write!(
      out,
      "{:?} {} in {container} {:?} {:?} {}-{} @{}",
      declaration.kind,
      summary.name(declaration.name),
      declaration.reach,
      declaration.member,
      declaration.start_line,
      declaration.end_line,
      declaration.name_line,
    );
    if let Some(receiver) = declaration.receiver {
      let _ = write!(out, " receiver {}", summary.name(receiver));
    }
    if let Some(signature) = &declaration.signature {
      let _ = write!(out, " {}", signature_text(summary, signature));
    }
    for constructor in declaration.constructors.iter().flatten() {
      let _ = write!(out, " {}", constructor_text(summary, constructor));
    }
    if !declaration.supertypes.is_empty() {
      // `_` for a function type, which has no path
      let supertypes: Vec<String> = declaration
        .supertypes
        .iter()
        .map(|it| if it.is_empty() { "_".to_string() } else { path_text(summary, it) })
        .collect();
      let _ = write!(out, " : {}", supertypes.join(", "));
    }
    if let Some(value_type) = &declaration.value_type {
      let _ = write!(out, " <{}>", path_text(summary, value_type));
    }
    let _ = writeln!(out, "{}{}", modality_text(declaration), if declaration.in_error { " in_error" } else { "" });
  }
  let mut by_line: Vec<&Reference> = summary.references.iter().collect();
  by_line.sort_by_key(|it| it.line);
  let mut line = 0;
  for reference in by_line {
    if reference.line != line {
      if line != 0 {
        out.push('\n');
      }
      line = reference.line;
      let _ = write!(out, "@{line}");
    }
    let _ = write!(out, " {}", reference_text(summary, reference));
  }
  if line != 0 {
    out.push('\n');
  }
  if summary.error_bytes > 0 {
    let _ = writeln!(out, "error_bytes {}", summary.error_bytes);
  }
  out
}

/// `name`, `a.b.name`, `?.name` (a member of an expression), `<Manager>.name` (of a receiver of that type, `this.` or a
/// property before it as in its qualifier), with `()` for a call (its arguments when it has their
/// shape: `paneOf(Text,flag=Boolean,_,Lambda)*{}`, `_` for an expression, `*` after them for a spread, `{}` for an
/// enclosed call), `:` for a type, `!` for a macro, `^` for an import and `~` for an override
/// (`~Base.name(Int,List/Window?)`, the receiver of an extension in `[]`)
pub(crate) fn reference_text(summary: &FileSummary, reference: &Reference) -> String {
  let path = |segments: &[u32]| segments.iter().map(|it| summary.name(*it)).collect::<Vec<_>>().join(".");
  let receiver = reference.receiver_type.as_deref().map_or_else(String::new, |it| format!("<{}>", path(it)));
  let qualifier = match &reference.qualifier {
    None => String::new(),
    Some(segments) if segments.is_empty() && !receiver.is_empty() => format!("{receiver}."),
    Some(segments) if segments.is_empty() => "?.".to_string(),
    Some(segments) => format!("{receiver}{}.", path(segments)),
  };
  let name = summary.name(reference.name);
  match reference.role {
    Role::Value => format!("{qualifier}{name}"),
    Role::Call if reference.shape.is_some() => format!("{qualifier}{name}{}", shape_text(summary, reference.shape)),
    Role::Call => format!("{qualifier}{name}()"),
    Role::Type => format!(":{qualifier}{name}"),
    Role::Import => format!("^{qualifier}{name}"),
    Role::Macro => format!("{qualifier}{name}!"),
    Role::Override => format!("~{qualifier}{name}{}", shape_text(summary, reference.shape)),
  }
}

fn shape_text(summary: &FileSummary, shape: Option<u32>) -> String {
  match shape.and_then(|it| summary.shapes.get(it as usize)) {
    None => String::new(),
    Some(Shape::Override { function, receiver, parameters }) => {
      let receiver = receiver.map_or_else(String::new, |it| format!("[{}]", summary.name(it)));
      let parameters = parameters.iter().map(|it| type_text(summary, it.as_ref())).collect::<Vec<_>>().join(",");
      if *function { format!("{receiver}({parameters})") } else { receiver }
    }
    Some(Shape::Call { arguments, spread, enclosed }) => {
      let arguments = arguments
        .iter()
        .map(|(name, argument)| {
          let name = name.map_or_else(String::new, |it| format!("{}=", summary.name(it)));
          let argument = if *argument == Argument::Expression { "_".to_string() } else { format!("{argument:?}") };
          format!("{name}{argument}")
        })
        .collect::<Vec<_>>()
        .join(",");
      format!("({arguments}){}{}", if *spread { "*" } else { "" }, if *enclosed { "{}" } else { "" })
    }
  }
}

/// `(prefix: String =, vararg tags: String)`: `=` for a default value
fn signature_text(summary: &FileSummary, signature: &Signature) -> String {
  let parameters: Vec<String> = signature
    .parameters
    .iter()
    .map(|it| {
      let vararg = if it.vararg { "vararg " } else { "" };
      let default = if it.optional { " =" } else { "" };
      format!("{vararg}{}: {}{default}", summary.name(it.name), type_text(summary, it.type_ref.as_ref()))
    })
    .collect();
  format!("({})", parameters.join(", "))
}

/// `new(x: Int)`, with the reach before it when it is not public: `Container new()`
fn constructor_text(summary: &FileSummary, constructor: &Constructor) -> String {
  let reach = if constructor.reach == Reach::Public {
    String::new()
  } else {
    format!("{:?} ", constructor.reach)
  };
  format!("{reach}new{}", signature_text(summary, &constructor.signature))
}

/// ` overrides overridable jvm_static`, without the words that don't hold
fn modality_text(declaration: &Declaration) -> String {
  let overrides = if declaration.overrides { " overrides" } else { "" };
  let overridable = if declaration.overridable { " overridable" } else { "" };
  let jvm_static = if declaration.jvm_static { " jvm_static" } else { "" };
  let jvm_field = if declaration.jvm_field { " jvm_field" } else { "" };
  let jvm_name = if declaration.jvm_name { " jvm_name" } else { "" };
  format!("{overrides}{overridable}{jvm_static}{jvm_field}{jvm_name}")
}

/// `List/Window?` of `[List, Window]` nullable, `int[]` of an array, `_` for any type
fn type_text(summary: &FileSummary, type_ref: Option<&TypeRef>) -> String {
  type_ref.map_or_else(
    || "_".to_string(),
    |it| {
      let names = it.names.iter().map(|it| summary.name(*it)).collect::<Vec<_>>().join("/");
      format!("{names}{}{}", "[]".repeat(usize::from(it.arrays)), if it.nullable { "?" } else { "" })
    },
  )
}

/// The references to a name as `line text`
pub(crate) fn refs(summary: &FileSummary, name: &str) -> Vec<String> {
  summary.references_to(name).map(|it| format!("{} {}", it.line, reference_text(summary, it))).collect()
}

/// The bare references to a name: locals must have none (a member of the same name is qualified)
pub(crate) fn bare_refs(summary: &FileSummary, name: &str) -> Vec<String> {
  summary
    .references_to(name)
    .filter(|it| it.qualifier.is_none())
    .map(|it| format!("{} {}", it.line, reference_text(summary, it)))
    .collect()
}

pub(crate) fn lines_of(summary: &FileSummary, name: &str) -> Vec<u32> {
  summary.references_to(name).map(|it| it.line).collect()
}

pub(crate) fn declaration<'s>(summary: &'s FileSummary, name: &str) -> &'s Declaration {
  let id = summary.name_id(name).unwrap_or_else(|| panic!("no name {name}"));
  summary.declarations.iter().find(|it| it.name == id).unwrap_or_else(|| panic!("no declaration of {name}"))
}

/// The declarations of a name as `Kind name in container`
pub(crate) fn declarations_of(summary: &FileSummary, name: &str) -> Vec<String> {
  of_name(summary, name, |declaration, container| format!("{:?} {name} in {container}", declaration.kind))
}

/// The declarations of a name as `container (parameters)`, `container -` without a signature
pub(crate) fn signatures(summary: &FileSummary, name: &str) -> Vec<String> {
  of_name(summary, name, |declaration, container| {
    let signature = declaration.signature.as_ref().map_or_else(|| "-".to_string(), |it| signature_text(summary, it));
    format!("{container} {signature}")
  })
}

/// The declarations of a name as `container overrides overridable jvm_static`, without the words that don't hold
pub(crate) fn modalities(summary: &FileSummary, name: &str) -> Vec<String> {
  of_name(summary, name, |declaration, container| format!("{container}{}", modality_text(declaration)))
}

/// The constructors of the type `name` as `new(parameters)` (`constructor_text`); `None` when it has no constructors
pub(crate) fn constructors_of(summary: &FileSummary, name: &str) -> Option<Vec<String>> {
  let constructors = declaration(summary, name).constructors.as_ref()?;
  Some(constructors.iter().map(|it| constructor_text(summary, it)).collect())
}

/// The supertypes of the type `name` as dotted paths
pub(crate) fn supertypes_of(summary: &FileSummary, name: &str) -> Vec<String> {
  declaration(summary, name).supertypes.iter().map(|it| path_text(summary, it)).collect()
}

/// The text of each declaration of a name, given the name of its container (`-` at the top level)
fn of_name(summary: &FileSummary, name: &str, text: impl Fn(&Declaration, &str) -> String) -> Vec<String> {
  let Some(id) = summary.name_id(name) else { return Vec::new() };
  summary
    .declarations
    .iter()
    .filter(|it| it.name == id)
    .map(|it| {
      let container = it.container.and_then(|it| summary.declarations.get(it as usize)).map_or("-", |it| summary.name(it.name));
      text(it, container)
    })
    .collect()
}
