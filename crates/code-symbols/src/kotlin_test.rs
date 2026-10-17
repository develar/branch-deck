use crate::summary::{DeclKind, ImportTarget, MemberKind, Reach, Role};
use crate::test_support::{NONE, bare_refs, declaration, declarations_of, lines_of, refs, summary_of};

#[test]
fn declarations() {
  let summary = summary_of("kotlin/declarations.kt");
  assert_eq!(summary.package.as_deref(), Some("com.example.shapes"));
  let circle = declaration(&summary, "Circle");
  assert_eq!((circle.kind, circle.reach, circle.member), (DeclKind::Type, Reach::Module, MemberKind::TopLevel));
  assert_eq!((circle.start_line, circle.name_line), (10, 10));
  assert_eq!(declarations_of(&summary, "radius"), ["Value radius in Circle"]);
  assert_eq!(declaration(&summary, "label").reach, Reach::Container);
  assert_eq!(declarations_of(&summary, "scale"), NONE, "a plain constructor parameter is no property");
  assert_eq!(declaration(&summary, "helper").reach, Reach::Container);
  assert_eq!(declaration(&summary, "Inner").member, MemberKind::Instance);
  assert_eq!(declaration(&summary, "Nested").member, MemberKind::Static);
  assert_eq!(declarations_of(&summary, "Companion"), ["Type Companion in Circle"]);
  assert_eq!(declaration(&summary, "UNIT").kind, DeclKind::Constant);
  assert_eq!(declaration(&summary, "unit").member, MemberKind::Static);
  // the accessors on the lines below a member property belong to it
  assert_eq!((declaration(&summary, "counter").start_line, declaration(&summary, "counter").end_line), (25, 29));
  assert_eq!(declaration(&summary, "register").member, MemberKind::Static);
  assert_eq!(declaration(&summary, "RED").kind, DeclKind::EnumEntry);
  assert_eq!(declarations_of(&summary, "toString"), NONE, "an enum entry's body overrides, it declares nothing");
  assert_eq!(declaration(&summary, "hex").member, MemberKind::Instance);
  assert_eq!(declarations_of(&summary, "Factory"), ["Type Factory in Color"]);
  assert_eq!(declaration(&summary, "Shapes").kind, DeclKind::TypeAlias);
  assert_eq!(declaration(&summary, "topSecret").reach, Reach::File);
  assert_eq!(declaration(&summary, "moduleWide").reach, Reach::Module);
  let shout = declaration(&summary, "shout");
  assert_eq!(shout.receiver.map(|it| summary.name(it)), Some("String"));
  assert_eq!(declaration(&summary, "orEmpty").receiver.map(|it| summary.name(it)), Some("Shape"));
  assert_eq!(declaration(&summary, "largest").receiver.map(|it| summary.name(it)), Some("List"));
  // a type parameter is local to its declaration
  assert_eq!(refs(&summary, "T"), NONE);
}

#[test]
fn imports() {
  let summary = summary_of("kotlin/imports.kt");
  let figure = &summary.imports[1];
  assert_eq!(figure.target, ImportTarget::Name(summary.name_id("Shape").unwrap()));
  assert_eq!(figure.alias, summary.name_id("Figure"));
  assert_eq!(summary.imports[2].target, ImportTarget::All);
  assert_eq!(declarations_of(&summary, "Figure"), ["Binding Figure in -"]);
  assert_eq!(declarations_of(&summary, "Circle"), NONE, "an import without an alias binds no new name");
  assert_eq!(refs(&summary, "Circle"), ["3 ^com.example.shapes.Circle", "8 Circle()"]);
}

#[test]
fn references() {
  let summary = summary_of("kotlin/references.kt");
  // locals: a destructuring, loop variables, lambda parameters and `it`, a catch parameter, a `when` subject
  for local in ["title", "first", "second", "index", "shape", "s", "area", "it", "e", "count", "scale", "shapes"] {
    assert_eq!(bare_refs(&summary, local), NONE, "{local}: {:?}", bare_refs(&summary, local));
  }
  // named arguments are labels, their values are references
  assert_eq!(refs(&summary, "level"), NONE);
  assert_eq!(refs(&summary, "tag"), NONE);
  assert_eq!(refs(&summary, "Level"), ["22 Level"]);
  assert_eq!(refs(&summary, "draw"), ["15 canvas.draw()", "28 draw()"]);
  assert_eq!(refs(&summary, "DEFAULT_SCALE"), ["11 DEFAULT_SCALE"]);
  assert_eq!(refs(&summary, "threshold"), ["18 threshold"]);
  assert_eq!(refs(&summary, "render"), ["29 render"]);
  assert_eq!(refs(&summary, "length"), ["30 String.length"]);
  assert_eq!(refs(&summary, "Renderer"), ["28 :Renderer"]);
  assert_eq!(refs(&summary, "log"), ["22 log()", "31 com.example.util.Logger.log()"]);
  assert_eq!(refs(&summary, "to"), ["32 ?.to()"]);
  assert_eq!(refs(&summary, "size"), ["12 ?.size", "24 ?.size"]);
  let draw = summary.references_to("draw").next().unwrap();
  assert_eq!(draw.role, Role::Call);
  assert_eq!(summary.declarations[draw.container.unwrap() as usize].name, summary.name_id("render").unwrap());
}

#[test]
fn a_word_in_kdoc_is_no_reference() {
  let summary = summary_of("kotlin/kdoc.kt");
  // KDoc, a line comment, a block comment and a string
  assert_eq!(refs(&summary, "TurnTracker"), NONE);
  assert_eq!(refs(&summary, "Session"), NONE);
  assert_eq!(refs(&summary, "legacy"), NONE);
  // `$name` and `${expression}` are code
  assert_eq!(refs(&summary, "turnTracker"), ["10 ?.turnTracker"]);
  assert_eq!(refs(&summary, "VERSION"), ["11 VERSION"]);
  assert_eq!(refs(&summary, "number"), ["11 Build.number()"]);
  assert_eq!(refs(&summary, "tracker"), NONE, "a parameter is local");
  let start = declaration(&summary, "start");
  assert_eq!((start.start_line, start.name_line, start.end_line), (3, 7, 12), "a declaration starts at its KDoc");
}

#[test]
fn a_local_val_shadows_and_is_no_reference() {
  let summary = summary_of("kotlin/local_val.kt");
  // `turnId` of `step` and of its lambda are locals; the property of `Recorder` is a declaration and its use a reference
  assert_eq!(declarations_of(&summary, "turnId"), ["Value turnId in Recorder"]);
  assert_eq!(refs(&summary, "turnId"), ["18 turnId"]);
  assert_eq!(refs(&summary, "turn"), NONE);
  assert_eq!(lines_of(&summary, "record"), [9]);
  assert_eq!(declarations_of(&summary, "record"), ["Function record in Recorder"]);
}

#[test]
fn a_misread_declaration_is_recovered() {
  let summary = summary_of("kotlin/misread.kt");
  assert_eq!(declarations_of(&summary, "Registrar"), ["Type Registrar in -"]);
  assert_eq!(declaration(&summary, "Registrar").reach, Reach::Module);
  assert_eq!(declarations_of(&summary, "register"), ["Function register in Registrar"]);
  assert_eq!(declarations_of(&summary, "name"), ["Function name in Base"]);
  assert_eq!(declarations_of(&summary, "size"), ["Value size in Tiny"]);
  assert_eq!(declarations_of(&summary, "LIMIT"), ["Constant LIMIT in Companion"]);
  assert_eq!(declarations_of(&summary, "limit"), ["Function limit in Hidden"]);
  let hidden = declaration(&summary, "Hidden");
  assert_eq!((hidden.reach, hidden.start_line, hidden.name_line), (Reach::File, 15, 17), "with its annotations");
  for word in ["internal", "sealed", "interface", "abstract", "class", "private", "companion", "object"] {
    assert_eq!(refs(&summary, word), NONE, "{word}: {:?}", refs(&summary, word));
  }
  assert_eq!(refs(&summary, "LIMIT"), ["22 LIMIT"]);
}
