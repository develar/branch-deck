//! The forms of the API as the card of a review shows them (openspec/specs/branch/review-plan/spec.md, "An entry shows
//! its forms as code reads"): a line per constructor of a type, else one, each parameter by its type as the source
//! writes it (`ParameterForm::shown_type`), and of a change what changed in each parameter (`mark_changes`).
//!
//! - **As the language writes it**: Kotlin `vararg names: String = …`, Java `String... names`; a parameter whose summary
//!   has no written type (C#) shows `type_text` unless it holds several names (`List/Window`, its structure gone), then
//!   no type.
//! - **Only shown**: what two forms compare is `Form` (`ParameterForm::type_text`, `ConstructorForm::key`); a type
//!   written in other words is no change and gets no mark.
//! - **Marks of a change**: the lines of the two sides pair first where they are the same, then by the most parameters
//!   they keep, the only line of each side (a function, a type of one constructor) with the other whatever it keeps; a
//!   line left without a pair (an overload added or removed) is marked whole. In a pair, a Kotlin parameter pairs by its name (a call names it), a Java one by
//!   its place (a call binds by the order alone, its name is free): the longest run of the same types stays, and the rest
//!   between pair by their order. A parameter of one side only is `Added` or `Removed`, a pair of another type or vararg
//!   `TypeChanged` on both sides.

use code_symbols::language::Language;
use serde::Serialize;
use trial_check::symbol_dependencies::declaration_checks::{ConstructorForm, Form, ParameterForm};

/// A form of an API entry as code reads: a line per constructor of a type, else one
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ShownForm {
  pub lines: Vec<ShownLine>,
}

/// A line of a form: what stands before its parameters, the parameters, what stands after them
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ShownLine {
  /// Up to the first parameter: `internal Event(`, `fun open(`; the whole line of a form without parameters
  pub head: String,
  /// `, ` apart
  pub parameters: Vec<ShownParameter>,
  /// After the last parameter: `)`, `): Int`
  pub tail: String,
  /// Of a change: the line has no pair on the other side, an overload added or removed
  pub mark: Option<FormMark>,
}

/// A parameter of a line: its type as the source writes it and what stands around it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ShownParameter {
  /// Before its type: Kotlin's `vararg names: `; empty in Java
  pub lead: String,
  /// Its type as the source writes it (`List<Path>`, `String...`); empty where none is shown
  pub shown_type: String,
  /// After its type: Java's ` names`, Kotlin's ` = …` of a default
  pub trail: String,
  /// Of a change: what changed in it
  pub mark: Option<FormMark>,
}

/// What a change did to a parameter, or to a whole line of a form
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FormMark {
  /// Only the form after the change has it
  Added,
  /// Only the form before the change has it
  Removed,
  /// Both have it, of another type or no longer a vararg (or one now)
  TypeChanged,
}

impl ShownForm {
  /// The form as text, a line each: what `brd review-plan` prints
  pub fn text(&self) -> String {
    self.lines.iter().map(ShownLine::text).collect::<Vec<_>>().join("\n")
  }
}

impl ShownLine {
  /// A line of `parameters` between `head` and `tail`, no mark
  pub(crate) fn new(head: String, parameters: &[ParameterForm], tail: String, language: Option<Language>) -> Self {
    Self {
      head,
      parameters: parameters.iter().map(|parameter| shown_parameter(parameter, language)).collect(),
      tail,
      mark: None,
    }
  }

  pub fn text(&self) -> String {
    let parameters: Vec<String> = self.parameters.iter().map(ShownParameter::text).collect();
    format!("{}{}{}", self.head, parameters.join(", "), self.tail)
  }
}

impl ShownParameter {
  pub fn text(&self) -> String {
    format!("{}{}{}", self.lead, self.shown_type, self.trail)
  }
}

/// A parameter as the language of its file writes it: Kotlin (and any other) `vararg name: Type = …`, Java `Type name`
/// with its `...`. Without a written type, `type_text` unless it holds several names
pub(crate) fn shown_parameter(parameter: &ParameterForm, language: Option<Language>) -> ShownParameter {
  let shown_type = parameter.shown_type.0.clone().unwrap_or_else(|| {
    if parameter.type_text.contains('/') {
      String::new()
    } else {
      parameter.type_text.clone()
    }
  });
  let name = &parameter.name;
  let (lead, trail) = if language == Some(Language::Java) {
    if shown_type.is_empty() {
      (name.clone(), String::new())
    } else {
      (String::new(), format!(" {name}"))
    }
  } else {
    let vararg = if parameter.vararg { "vararg " } else { "" };
    let colon = if shown_type.is_empty() { "" } else { ": " };
    let default = if parameter.optional { " = …" } else { "" };
    (format!("{vararg}{name}{colon}"), default.to_string())
  };
  ShownParameter {
    lead,
    shown_type,
    trail,
    mark: None,
  }
}

/// The parameter lists of the lines of the form `shown` makes of `form`: of a header the one of its parameters, else one
/// per constructor
fn parameter_lists(form: &Form) -> Vec<&[ParameterForm]> {
  if form.header.is_some() {
    form.parameters.as_deref().into_iter().collect()
  } else {
    form
      .constructors
      .iter()
      .flatten()
      .map(|constructor: &ConstructorForm| constructor.parameters.as_slice())
      .collect()
  }
}

/// Marks what a change did on its forms `before` (of `old`) and `after` (of `new`), whose lines are those `shown` makes
/// of them (`parameter_lists`); nothing where a form holds no parameters
pub(crate) fn mark_changes(before: &mut ShownForm, old: &Form, after: &mut ShownForm, new: &Form, language: Option<Language>) {
  let (old_lists, new_lists) = (parameter_lists(old), parameter_lists(new));
  if old_lists.is_empty() || new_lists.is_empty() || old_lists.len() != before.lines.len() || new_lists.len() != after.lines.len() {
    return;
  }
  let java = language == Some(Language::Java);
  let mut old_left: Vec<usize> = (0..old_lists.len()).collect();
  let mut new_left: Vec<usize> = (0..new_lists.len()).collect();
  let mut pairs: Vec<(usize, usize)> = Vec::new();
  // the same lines first
  old_left.retain(|&i| {
    let same = new_left
      .iter()
      .position(|&j| old_lists[i] == new_lists[j] && before.lines[i].head == after.lines[j].head && before.lines[i].tail == after.lines[j].tail);
    same.map(|found| pairs.push((i, new_left.remove(found)))).is_none()
  });
  // then by the most parameters kept; the only line of each side pairs whatever it keeps
  let only = old_lists.len() == 1 && new_lists.len() == 1;
  loop {
    let best = old_left
      .iter()
      .enumerate()
      .flat_map(|(a, &i)| new_left.iter().enumerate().map(move |(b, &j)| (a, b, i, j)))
      .map(|(a, b, i, j)| (kept(&parameter_marks(old_lists[i], new_lists[j], java)), a, b))
      .max_by_key(|(kept, a, b)| (*kept, std::cmp::Reverse((*a, *b))));
    match best {
      Some((kept, a, b)) if kept > 0 || only => pairs.push((old_left.remove(a), new_left.remove(b))),
      _ => break,
    }
  }
  for i in old_left {
    before.lines[i].mark = Some(FormMark::Removed);
  }
  for j in new_left {
    after.lines[j].mark = Some(FormMark::Added);
  }
  for (i, j) in pairs {
    let (old_marks, new_marks) = parameter_marks(old_lists[i], new_lists[j], java);
    for (parameter, mark) in before.lines[i].parameters.iter_mut().zip(old_marks) {
      parameter.mark = mark;
    }
    for (parameter, mark) in after.lines[j].parameters.iter_mut().zip(new_marks) {
      parameter.mark = mark;
    }
  }
}

/// The marks of the parameters of `old` and of `new`
type Marks = (Vec<Option<FormMark>>, Vec<Option<FormMark>>);

/// How many parameters of `old` stay unmarked
fn kept((old, _): &Marks) -> usize {
  old.iter().filter(|mark| mark.is_none()).count()
}

/// Two parameters bind a call alike: the same type, both a vararg or neither
fn same_type(a: &ParameterForm, b: &ParameterForm) -> bool {
  a.type_text == b.type_text && a.vararg == b.vararg
}

/// What changed in each parameter of `old` and of `new`: in Java by their places (`java`), else by their names
fn parameter_marks(old: &[ParameterForm], new: &[ParameterForm], java: bool) -> Marks {
  if !java {
    let of = |side: &[ParameterForm], other: &[ParameterForm], missing: FormMark| -> Vec<Option<FormMark>> {
      side
        .iter()
        .map(|parameter| match other.iter().find(|it| it.name == parameter.name) {
          None => Some(missing),
          Some(paired) => (!same_type(parameter, paired)).then_some(FormMark::TypeChanged),
        })
        .collect()
    };
    return (of(old, new, FormMark::Removed), of(new, old, FormMark::Added));
  }
  // the longest run of the same types stays; the parameters between pair by their order
  let (n, m) = (old.len(), new.len());
  let mut longest = vec![vec![0_usize; m + 1]; n + 1];
  for i in (0..n).rev() {
    for j in (0..m).rev() {
      longest[i][j] = if same_type(&old[i], &new[j]) {
        longest[i + 1][j + 1] + 1
      } else {
        longest[i + 1][j].max(longest[i][j + 1])
      };
    }
  }
  let (mut old_marks, mut new_marks) = (vec![None; n], vec![None; m]);
  let (mut i, mut j) = (0, 0);
  let (mut gap_i, mut gap_j) = (0, 0);
  while i < n && j < m {
    if same_type(&old[i], &new[j]) && longest[i][j] == longest[i + 1][j + 1] + 1 {
      close_gap(&mut old_marks[gap_i..i], &mut new_marks[gap_j..j]);
      i += 1;
      j += 1;
      (gap_i, gap_j) = (i, j);
    } else if longest[i + 1][j] >= longest[i][j + 1] {
      i += 1;
    } else {
      j += 1;
    }
  }
  close_gap(&mut old_marks[gap_i..], &mut new_marks[gap_j..]);
  (old_marks, new_marks)
}

/// Marks the parameters between two that stay, of `old` and of `new`: those of the same place a change of type, the
/// rest of the longer side removed or added
fn close_gap(old: &mut [Option<FormMark>], new: &mut [Option<FormMark>]) {
  let paired = old.len().min(new.len());
  for (place, mark) in old.iter_mut().enumerate() {
    *mark = Some(if place < paired { FormMark::TypeChanged } else { FormMark::Removed });
  }
  for (place, mark) in new.iter_mut().enumerate() {
    *mark = Some(if place < paired { FormMark::TypeChanged } else { FormMark::Added });
  }
}
