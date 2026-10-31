use crate::extract::summarize;
use crate::language::Language;
use crate::summary::{DeclKind, ImportTarget, MemberKind, Reach, Role};
use crate::test_support::{NONE, bare_refs, declaration, declarations_of, refs, summary_of};

#[test]
fn a_function_under_a_test_attribute_is_a_test() {
  let source = b"#[test]
fn plain_test() {}

/// The doc
#[tokio::test]
async fn async_test() {}

#[test_log::test]
#[ignore]
fn logged_test() {}

#[rstest]
fn cases() {}

#[cfg(test)]
fn helper() {}

fn production() {}
";
  let summary = summarize(Language::Rust, source);
  let test = |name: &str| declaration(&summary, name).test;
  assert!(test("plain_test"));
  assert!(test("async_test"), "under a doc comment");
  assert!(test("logged_test"), "above another attribute");
  assert!(test("cases"));
  assert!(!test("helper"), "#[cfg(test)] is no test");
  assert!(!test("production"));
}

#[test]
fn a_rust_use_binds_the_name() {
  let summary = summary_of("rust/use_binding.rs");
  let owner = declaration(&summary, "ControlOwner");
  assert_eq!((owner.kind, owner.reach), (DeclKind::Binding, Reach::Module));
  assert!(summary.declares("ControlOwner"));
  let import = summary
    .imports
    .iter()
    .find(|it| it.target == ImportTarget::Name(summary.name_id("ControlOwner").unwrap()))
    .unwrap();
  assert!(import.exported);
  assert_eq!(import.path.iter().map(|it| summary.name(*it)).collect::<Vec<_>>(), ["avl_wire", "program", "lane"]);
  assert_eq!(
    refs(&summary, "ControlOwner"),
    ["3 ^avl_wire.program.lane.ControlOwner", "10 :ControlOwner", "12 ControlOwner"]
  );
  // `pub use` passes names on; a list, an alias and `self` each bind
  assert_eq!(declaration(&summary, "Lane").reach, Reach::Public);
  assert_eq!(declarations_of(&summary, "Id"), ["Binding Id in -"]);
  assert_eq!(declarations_of(&summary, "LaneId"), NONE);
  assert_eq!(declarations_of(&summary, "lane"), ["Binding lane in -"]);
  assert_eq!(declaration(&summary, "HashMap").reach, Reach::File);
  assert_eq!(declarations_of(&summary, "Serialize"), NONE, "`as _` binds nothing");
  assert_eq!(declarations_of(&summary, "heap"), ["Binding heap in -"]);
  assert!(
    summary
      .imports
      .iter()
      .any(|it| it.target == ImportTarget::All && it.path.iter().map(|it| summary.name(*it)).eq(["super"]))
  );
}

#[test]
fn declarations() {
  let summary = summary_of("rust/declarations.rs");
  let circle = declaration(&summary, "Circle");
  assert_eq!(
    (circle.reach, circle.start_line, circle.name_line),
    (Reach::Public, 5, 7),
    "with its doc comment and attributes"
  );
  assert_eq!(declaration(&summary, "radius").reach, Reach::Public);
  assert_eq!(declaration(&summary, "label").reach, Reach::File);
  assert_eq!(declaration(&summary, "scale").reach, Reach::Module);
  assert_eq!(declaration(&summary, "Pair").reach, Reach::Module);
  assert_eq!(declarations_of(&summary, "sides"), ["Value sides in Custom"]);
  assert_eq!(declaration(&summary, "sides").reach, Reach::Public, "a variant's field is as public as the enum");
  assert_eq!(declaration(&summary, "Polygon").kind, DeclKind::EnumEntry);
  assert_eq!(declaration(&summary, "Bits").kind, DeclKind::Type);
  assert_eq!(declarations_of(&summary, "Unit"), ["TypeAlias Unit in Shape", "TypeAlias Unit in Circle"]);
  assert_eq!(declaration(&summary, "MAX").kind, DeclKind::Constant);
  assert_eq!(declaration(&summary, "COUNTER").kind, DeclKind::Value);
  assert_eq!(declaration(&summary, "parent_only").reach, Reach::Parent);
  assert_eq!(declaration(&summary, "private_helper").reach, Reach::File);
  assert_eq!(declaration(&summary, "square").kind, DeclKind::Macro);
  assert_eq!(declaration(&summary, "geometry").kind, DeclKind::Module);
  assert_eq!(declarations_of(&summary, "hidden"), ["Function hidden in geometry"]);
  assert_eq!(declaration(&summary, "hidden").reach, Reach::Container);
  // impl members belong to the type and name it as their receiver
  let new = declaration(&summary, "new");
  assert_eq!(declarations_of(&summary, "new"), ["Function new in Circle"]);
  assert_eq!((new.member, new.receiver.map(|it| summary.name(it))), (MemberKind::Static, Some("Circle")));
  assert_eq!(declaration(&summary, "grow").member, MemberKind::Instance);
  assert_eq!(declaration(&summary, "grow").reach, Reach::File);
  assert_eq!(declarations_of(&summary, "area"), ["Function area in Shape", "Function area in Circle"]);
  assert_eq!(declarations_of(&summary, "fmt"), ["Binding fmt in -", "Function fmt in Kind"]);
  assert_eq!(declaration(&summary, "name").member, MemberKind::Static);
  // macro bodies and token trees
  assert_eq!(refs(&summary, "square"), ["72 square!"]);
  assert_eq!(refs(&summary, "radius"), ["57 Self.radius", "61 self.radius", "72 self.radius"]);
  assert_eq!(refs(&summary, "write"), ["78 write!"]);
  assert_eq!(refs(&summary, "label"), ["57 Self.label"]);
  assert_eq!(refs(&summary, "Debug"), ["6 :Debug", "26 :fmt.Debug"]);
  // the `pub` items at the top of the body of a macro in item position
  let encoding = declaration(&summary, "StillEncoding");
  assert_eq!(declarations_of(&summary, "StillEncoding"), ["Type StillEncoding in -"]);
  assert_eq!(
    (encoding.reach, encoding.start_line, encoding.end_line, encoding.name_line),
    (Reach::Public, 83, 89, 84),
    "from its doc comment to its body"
  );
  assert_eq!(declarations_of(&summary, "Lossless"), ["EnumEntry Lossless in StillEncoding"]);
  assert_eq!(declaration(&summary, "Lossy").start_line, 86, "with its doc comment and attribute");
  assert_eq!(declaration(&summary, "Stills").reach, Reach::Module);
  assert_eq!(declarations_of(&summary, "encoding"), NONE, "the fields of a struct in a macro are tokens");
  assert_eq!(refs(&summary, "StillEncoding"), ["92 StillEncoding"]);
  assert_eq!(declaration(&summary, "helper").header.as_deref(), Some("const fn helper ( x : u8 ) -> u8"));
  let cache = declaration(&summary, "CACHE");
  assert_eq!(
    (cache.kind, cache.reach, cache.member),
    (DeclKind::Value, Reach::Public, MemberKind::TopLevel),
    "`lazy_static!`"
  );
  assert_eq!(declarations_of(&summary, "PRIVATE"), NONE, "only `pub` items");
  assert_eq!(declarations_of(&summary, "LocalOnly"), NONE, "a macro in a function body declares nothing");
  assert_eq!(refs(&summary, "LocalOnly"), ["107 LocalOnly"]);
}

#[test]
fn references() {
  let summary = summary_of("rust/references.rs");
  for word in ["Phantom", "Ghost", "comment", "block"] {
    assert_eq!(refs(&summary, word), NONE, "{word}");
  }
  // parameters, `let`, loop and closure patterns, `if let`, `while let`, `let else`, match arms
  for local in [
    "shapes",
    "scale",
    "title",
    "count",
    "index",
    "shape",
    "area",
    "areas",
    "first",
    "n",
    "radius",
    "circle_scale",
    "last",
    "next",
  ] {
    assert_eq!(bare_refs(&summary, local), NONE, "{local}: {:?}", bare_refs(&summary, local));
  }
  assert_eq!(refs(&summary, "outer"), NONE, "a label");
  // a capitalized name in a pattern is a variant, not a binding
  assert_eq!(refs(&summary, "Some"), ["11 Some", "20 Some", "21 Some"]);
  assert_eq!(refs(&summary, "MAX"), ["16 MAX"]);
  // a field of a struct pattern is a member of the struct even where a local has its name
  assert_eq!(refs(&summary, "scale"), ["19 Circle.scale"]);
  // a shorthand names the field and the local
  assert_eq!(refs(&summary, "radius"), ["19 Circle.radius"]);
  assert_eq!(refs(&summary, "encoding"), ["34 Stills.encoding"]);
  assert_eq!(refs(&summary, "draw"), ["8 canvas.draw()"]);
  assert_eq!(refs(&summary, "THRESHOLD"), ["10 THRESHOLD"]);
  assert_eq!(refs(&summary, "info"), ["12 log.info!"]);
  assert_eq!(refs(&summary, "name"), ["12 ?.name"]);
  assert_eq!(refs(&summary, "NAME"), ["27 crate.NAME"]);
  assert_eq!(refs(&summary, "TooMany"), ["16 Error.TooMany()"]);
  assert!(summary.references.iter().filter(|it| it.role == Role::Macro).count() >= 4);
}
