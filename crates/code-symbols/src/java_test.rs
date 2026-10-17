use crate::summary::{DeclKind, ImportTarget, MemberKind, Reach};
use crate::test_support::{NONE, bare_refs, declaration, declarations_of, modalities, refs, signatures, summary_of, supertypes_of};

#[test]
fn declarations() {
  let summary = summary_of("java/Declarations.java");
  assert_eq!(summary.package.as_deref(), Some("com.example.shapes"));
  let top = declaration(&summary, "Declarations");
  assert_eq!((top.reach, top.start_line, top.name_line), (Reach::Public, 3, 4));
  let max = declaration(&summary, "MAX");
  assert_eq!((max.kind, max.member, max.reach), (DeclKind::Constant, MemberKind::Static, Reach::Public));
  assert_eq!(
    (declaration(&summary, "counter").kind, declaration(&summary, "counter").reach),
    (DeclKind::Value, Reach::Container)
  );
  assert_eq!(
    declarations_of(&summary, "label"),
    ["Value label in Declarations", "Function label in Kind"],
    "the second declarator of a field"
  );
  assert_eq!(declaration(&summary, "packageMethod").reach, Reach::Module);
  assert_eq!(declarations_of(&summary, "area"), ["Value area in Declarations", "Function area in Declarations"]);
  assert_eq!(declarations_of(&summary, "Declarations").len(), 1, "a constructor is no declaration");
  // interface members
  assert_eq!(
    (declaration(&summary, "DEPTH").kind, declaration(&summary, "DEPTH").reach),
    (DeclKind::Constant, Reach::Public)
  );
  assert_eq!(declaration(&summary, "visit").member, MemberKind::Instance);
  assert_eq!(declaration(&summary, "empty").member, MemberKind::Static);
  // nested types: only a class without `static` is inner
  for nested in ["Visitor", "Kind", "Point", "Marker", "Nested"] {
    assert_eq!(declaration(&summary, nested).member, MemberKind::Static, "{nested}");
  }
  assert_eq!(declaration(&summary, "Inner").member, MemberKind::Instance);
  assert_eq!(declaration(&summary, "SQUARE").kind, DeclKind::EnumEntry);
  assert_eq!(declarations_of(&summary, "toString"), NONE, "an enum constant's body overrides, it declares nothing");
  assert_eq!(declarations_of(&summary, "x"), ["Value x in Point"], "a record component is a field");
  assert_eq!(declaration(&summary, "value").kind, DeclKind::Function);
  assert_eq!(declaration(&summary, "PackagePrivate").reach, Reach::Module);
  assert_eq!(refs(&summary, "T"), NONE, "type parameters are local");
  assert_eq!(refs(&summary, "R"), NONE);
}

#[test]
fn references() {
  let summary = summary_of("java/References.java");
  assert_eq!(summary.imports[1].target, ImportTarget::All);
  assert_eq!(summary.imports[2].target, ImportTarget::Name(summary.name_id("requireNonNull").unwrap()));
  for word in ["Phantom", "Ghost", "unused"] {
    assert_eq!(refs(&summary, word), NONE, "{word}");
  }
  // parameters, locals, loop variables, lambda parameters, resources, catch and pattern variables
  for local in [
    "shapes", "scale", "title", "count", "i", "shape", "s", "area", "writer", "output", "e", "circle", "job", "a",
  ] {
    assert_eq!(bare_refs(&summary, local), NONE, "{local}: {:?}", bare_refs(&summary, local));
  }
  assert_eq!(refs(&summary, "value"), NONE, "an annotation's element name is a label");
  assert_eq!(refs(&summary, "outer"), NONE, "a label");
  assert_eq!(refs(&summary, "var"), NONE);
  assert_eq!(refs(&summary, "draw"), ["24 canvas.draw()"]);
  assert_eq!(refs(&summary, "area"), ["27 Shape.area"]);
  assert_eq!(refs(&summary, "THRESHOLD"), ["27 THRESHOLD"]);
  assert_eq!(refs(&summary, "radius"), ["33 ?.radius()"]);
  assert_eq!(refs(&summary, "NONE"), ["37 Kind.NONE"]);
  assert_eq!(refs(&summary, "requireNonNull"), ["5 ^java.util.Objects.requireNonNull", "39 requireNonNull()"]);
  assert_eq!(refs(&summary, "Runnable"), ["42 Runnable()", "42 :Runnable"]);
  assert_eq!(declarations_of(&summary, "run"), NONE, "an anonymous class declares nothing");
  assert_eq!(refs(&summary, "emptyList"), ["46 java.util.Collections.emptyList()"]);
}

#[test]
fn an_override_is_a_reference_per_supertype() {
  let summary = summary_of("java/Overrides.java");
  assert_eq!(supertypes_of(&summary, "Overrides"), ["ToolWindowManagerEx", "Disposable", "java.io.Serializable"]);
  assert_eq!(supertypes_of(&summary, "Listener"), ["EventListener"], "an interface's `extends`");
  assert_eq!(supertypes_of(&summary, "Point"), ["Comparable"], "a record's `implements`, without type arguments");
  assert_eq!(
    refs(&summary, "dispose"),
    ["8 ~Disposable.dispose()", "8 ~ToolWindowManagerEx.dispose()", "8 ~java.io.Serializable.dispose()"]
  );
  assert_eq!(refs(&summary, "toString").len(), 3, "`@java.lang.Override`");
  assert_eq!(refs(&summary, "run"), ["32 ~Runnable.run()"], "an anonymous class overrides the type it constructs");
  assert_eq!(refs(&summary, "accept"), ["36 ~Base.accept(String)"], "a local class");
  assert_eq!(refs(&summary, "weight"), ["55 ~Overrides.Level.weight()"], "an enum constant's body overrides its enum");
  assert_eq!(refs(&summary, "compareTo"), ["68 ~Comparable.compareTo(Point)"]);
  // a method without `@Override` may override too, but says nothing
  assert_eq!(modalities(&summary, "open"), ["Overrides overridable"]);
  assert_eq!(refs(&summary, "open"), NONE);
}

#[test]
fn a_signature_has_the_parameter_types() {
  let summary = summary_of("java/Overrides.java");
  // a vararg is an array, as on the JVM
  assert_eq!(signatures(&summary, "open"), ["Overrides (paneId: String, vararg sizes: int[])"]);
  // a type parameter of the method matches any type; an array keeps its dimensions
  assert_eq!(signatures(&summary, "convert"), ["Overrides (value: _, names: String[], counts: Map/String/Integer)"]);
  assert_eq!(signatures(&summary, "fill"), ["Overrides (grid: int[][], rows: String[])"], "also after the name");
  assert_eq!(signatures(&summary, "create"), ["Overrides (ids: List/String)"]);
  assert_eq!(signatures(&summary, "x"), ["Point -"], "a record component is a field");
}

#[test]
fn a_member_knows_whether_it_overrides_and_can_be_overridden() {
  let summary = summary_of("java/Overrides.java");
  assert_eq!(modalities(&summary, "dispose"), ["Overrides overrides overridable"]);
  // `final`, `private` and `static` methods, a method of a `final` class
  for name in ["closed", "hidden", "create", "empty", "log", "member"] {
    assert_eq!(modalities(&summary, name).len(), 1, "{name}");
    assert!(!modalities(&summary, name)[0].contains("overrid"), "{name}: {:?}", modalities(&summary, name));
  }
  assert_eq!(modalities(&summary, "changed"), ["Listener overridable"], "an interface method");
  assert_eq!(modalities(&summary, "reset"), ["Listener overridable"], "a default method");
  assert_eq!(modalities(&summary, "weight"), ["Level overridable"], "an enum's method, which its constants may override");
  assert_eq!(modalities(&summary, "compareTo"), ["Point overrides"], "a record is final");
  assert_eq!(modalities(&summary, "x"), ["Point"], "a field overrides nothing");
}
