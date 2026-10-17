use crate::summary::{DeclKind, ImportTarget, MemberKind, Reach, Role};
use crate::test_support::{NONE, bare_refs, declaration, declarations_of, refs, summary_of};

#[test]
fn declarations() {
  let summary = summary_of("typescript/declarations.ts");
  let shape = declaration(&summary, "Shape");
  assert_eq!((shape.reach, shape.start_line, shape.name_line), (Reach::Public, 1, 2));
  assert_eq!(declarations_of(&summary, "onClick"), ["Value onClick in Props"], "a member of an object type alias");
  assert_eq!(declaration(&summary, "Internal").reach, Reach::File);
  assert_eq!(
    declarations_of(&summary, "area"),
    ["Function area in Shape", "Function area in Base", "Function area in Circle"]
  );
  assert_eq!(declaration(&summary, "Circle").reach, Reach::Public, "export default");
  assert_eq!(declaration(&summary, "unit").member, MemberKind::Static);
  assert_eq!(declaration(&summary, "secret").reach, Reach::Container);
  assert_eq!(declaration(&summary, "#hidden").reach, Reach::Container);
  assert_eq!(declaration(&summary, "guarded").reach, Reach::Public);
  assert_eq!(declarations_of(&summary, "radius"), ["Value radius in Circle"], "a parameter property");
  assert_eq!(declaration(&summary, "radius").reach, Reach::Container);
  assert_eq!(declaration(&summary, "label").reach, Reach::Public);
  assert_eq!(declarations_of(&summary, "constructor"), NONE);
  assert_eq!(declaration(&summary, "diameter").kind, DeclKind::Value, "an accessor");
  assert_eq!(declarations_of(&summary, "Square"), ["EnumEntry Square in Kind"]);
  assert_eq!(declarations_of(&summary, "ORIGIN"), ["Constant ORIGIN in Geometry"]);
  assert_eq!(declaration(&summary, "hidden").reach, Reach::Container);
  assert_eq!(declarations_of(&summary, "extra"), ["Function extra in shapes-extra"]);
  assert_eq!(declaration(&summary, "MAX_SIZE").kind, DeclKind::Constant);
  assert_eq!(declaration(&summary, "createShape").kind, DeclKind::Function);
  assert_eq!(
    (declaration(&summary, "counter").kind, declaration(&summary, "counter").reach),
    (DeclKind::Value, Reach::File)
  );
  assert_eq!(declarations_of(&summary, "renamed"), ["Value renamed in -"], "top-level destructuring declares each name");
  assert_eq!(declarations_of(&summary, "second"), NONE, "a destructuring key is no name");
  assert_eq!(declaration(&summary, "helper").reach, Reach::Public, "`export {{ helper }}` below it");
  assert_eq!(
    (declaration(&summary, "count").kind, declaration(&summary, "count").reach),
    (DeclKind::Binding, Reach::Public)
  );
  assert_eq!(refs(&summary, "radius"), ["26 this.radius", "30 this.radius"]);
}

#[test]
fn imports() {
  let summary = summary_of("typescript/imports.ts");
  let react = &summary.imports[0];
  assert_eq!(
    (react.target, react.alias, react.specifier.as_deref()),
    (ImportTarget::Default, summary.name_id("React"), Some("react"))
  );
  assert_eq!(summary.imports[1].target, ImportTarget::Name(summary.name_id("useState").unwrap()));
  assert_eq!(summary.imports[1].alias, summary.name_id("useLocalState"));
  assert_eq!(summary.imports[3].target, ImportTarget::Namespace);
  assert!(
    summary.imports.iter().all(|it| it.specifier.as_deref() != Some("./side-effect")),
    "a side-effect import binds nothing"
  );
  let reexport = summary
    .imports
    .iter()
    .find(|it| it.target == ImportTarget::Name(summary.name_id("Circle").unwrap()))
    .unwrap();
  assert!(reexport.exported);
  assert_eq!(declaration(&summary, "Circle").reach, Reach::Public);
  assert!(summary.imports.iter().any(|it| it.target == ImportTarget::All && it.exported));
  assert_eq!(declarations_of(&summary, "geometry"), ["Binding geometry in -"]);
  assert_eq!(declaration(&summary, "useLocalState").reach, Reach::File);
  assert_eq!(refs(&summary, "useLocalState"), ["11 useLocalState()"]);
  assert_eq!(refs(&summary, "join"), ["11 path.join()"]);
}

#[test]
fn references() {
  let summary = summary_of("typescript/references.tsx");
  for word in ["Phantom", "Ghost", "row", "className", "div", "mode"] {
    assert_eq!(bare_refs(&summary, word), NONE, "{word}: {:?}", bare_refs(&summary, word));
  }
  // destructured parameters, array destructuring, loop and catch variables, hoisted `var` and functions
  for local in ["title", "items", "open", "setOpen", "label", "handler", "item", "error", "options", "hoisted", "later"] {
    assert_eq!(bare_refs(&summary, local), NONE, "{local}: {:?}", bare_refs(&summary, local));
  }
  assert_eq!(refs(&summary, "DEFAULT_ITEMS"), ["3 DEFAULT_ITEMS"], "a default value is code");
  assert_eq!(refs(&summary, "index"), ["10 index", "11 index"], "a loop without `const` assigns");
  assert_eq!(refs(&summary, "key"), ["18 key"], "a computed key is code");
  assert_eq!(refs(&summary, "Dark"), ["18 Mode.Dark"]);
  assert_eq!(refs(&summary, "Card"), ["21 :Card", "23 Card", "24 :Card"]);
  assert_eq!(refs(&summary, "Footer"), ["23 :Card.Footer"]);
  assert!(summary.references_to("Card").any(|it| it.role == Role::Type));
}

#[test]
fn javascript() {
  let summary = summary_of("typescript/script.js");
  assert_eq!(declarations_of(&summary, "join"), ["Value join in -"]);
  assert_eq!(refs(&summary, "join"), ["4 join()"]);
  assert_eq!(refs(&summary, "args"), NONE);
  assert_eq!(refs(&summary, "out"), NONE);
  assert_eq!(refs(&summary, "log"), ["5 console.log()"]);
}
