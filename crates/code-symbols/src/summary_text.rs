//! A summary as text, one line per import, declaration and line of references: the golden files of the tests
//! (`render`, `testdata/<language>/<fixture>.summary`) and the oracle of `examples/facts.rs` (`render_full`, with the
//! facts the golden files leave out), so a rule change, or a change of parser, shows as a diff of lines.

use crate::language::{Family, Language};
use crate::summary::{Argument, Constructor, Declaration, FileSummary, ImportTarget, Reach, Reference, Role, Shape, Signature, TypeRef};
use std::fmt::Write as _;

/// The names of `ids` joined by dots
pub(crate) fn path_text(summary: &FileSummary, ids: &[u32]) -> String {
  ids.iter().map(|it| summary.name(*it)).collect::<Vec<_>>().join(".")
}

/// The text of the golden files
pub fn render(summary: &FileSummary) -> String {
  render_with(summary, false)
}

/// The text of `render` with every other fact of the summary: its size (`lines`, `total_bytes`, `truncated`), the
/// `import_lines`, a declaration's `value_constructed` (` constructed`), and of each reference its container
/// (`#Name`), its `receiver_constructed` (`+new`) and, in Java too, an unknown coroutine context (`@co?`)
pub fn render_full(summary: &FileSummary) -> String {
  render_with(summary, true)
}

fn render_with(summary: &FileSummary, full: bool) -> String {
  let mut out = String::new();
  if full {
    let _ = writeln!(
      out,
      "file {:?} lines {} bytes {}{}",
      summary.language,
      summary.lines,
      summary.total_bytes,
      if summary.truncated { " truncated" } else { "" }
    );
  }
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
  if full && !summary.import_lines.is_empty() {
    let spans: Vec<String> = summary.import_lines.iter().map(|it| format!("{}-{}", it.start, it.end)).collect();
    let _ = writeln!(out, "import_lines {}", spans.join(" "));
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
    if let Some(header) = &declaration.header {
      let _ = write!(out, " `{header}`");
    }
    let constructed = if full && declaration.value_constructed { " constructed" } else { "" };
    let _ = writeln!(out, "{}{constructed}{}", modality_text(declaration), if declaration.in_error { " in_error" } else { "" });
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
    // a suspend call is allowed there (`@co`), or may be (`@co?`, Kotlin's lambda of an unknown function)
    let coroutine = match reference.coroutine {
      Some(true) => "@co",
      None if summary.language == Language::Kotlin || (full && summary.language.family() == Family::Jvm) => "@co?",
      _ => "",
    };
    let _ = write!(out, " {}{coroutine}", reference_text(summary, reference));
    if full {
      if let Some(container) = reference.container.and_then(|it| summary.declarations.get(it as usize)) {
        let _ = write!(out, "#{}", summary.name(container.name));
      }
      if reference.receiver_constructed {
        out.push_str("+new");
      }
    }
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
pub(crate) fn signature_text(summary: &FileSummary, signature: &Signature) -> String {
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
pub(crate) fn constructor_text(summary: &FileSummary, constructor: &Constructor) -> String {
  let reach = if constructor.reach == Reach::Public {
    String::new()
  } else {
    format!("{:?} ", constructor.reach)
  };
  format!("{reach}new{}", signature_text(summary, &constructor.signature))
}

/// ` overrides overridable jvm_static test entry_point unstable`, without the words that don't hold
pub(crate) fn modality_text(declaration: &Declaration) -> String {
  let overrides = if declaration.overrides { " overrides" } else { "" };
  let overridable = if declaration.overridable { " overridable" } else { "" };
  let jvm_static = if declaration.jvm_static { " jvm_static" } else { "" };
  let jvm_field = if declaration.jvm_field { " jvm_field" } else { "" };
  let jvm_name = if declaration.jvm_name { " jvm_name" } else { "" };
  let lateinit = if declaration.lateinit { " lateinit" } else { "" };
  let test = if declaration.test { " test" } else { "" };
  let entry_point = if declaration.entry_point { " entry_point" } else { "" };
  let unstable = if declaration.unstable { " unstable" } else { "" };
  let setter = match declaration.setter {
    None => String::new(),
    Some(reach) if reach == declaration.reach => " var".to_string(),
    Some(reach) => format!(" var({reach:?} set)"),
  };
  format!("{overrides}{overridable}{jvm_static}{jvm_field}{jvm_name}{lateinit}{test}{entry_point}{unstable}{setter}")
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
