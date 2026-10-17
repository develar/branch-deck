use crate::extract::summarize;
use crate::language::Language;
use crate::summary::{Argument, DeclKind, ImportTarget, MemberKind, Reach, Role, Shape, TypeRef};
use crate::test_support::{NONE, bare_refs, declaration, declarations_of, lines_of, modalities, refs, signatures, summary_of, supertypes_of};

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
  assert_eq!(refs(&summary, "Circle"), ["3 ^com.example.shapes.Circle", "8 Circle(_,Text,Number)"]);
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
  assert_eq!(refs(&summary, "draw"), ["15 canvas.draw(_,_)", "28 draw(_){}"], "in a lambda");
  assert_eq!(refs(&summary, "DEFAULT_SCALE"), ["11 DEFAULT_SCALE"]);
  assert_eq!(refs(&summary, "threshold"), ["18 threshold"]);
  assert_eq!(refs(&summary, "render"), ["29 render"]);
  assert_eq!(refs(&summary, "length"), ["30 String.length"]);
  assert_eq!(refs(&summary, "Renderer"), ["28 :Renderer"]);
  assert_eq!(refs(&summary, "log"), ["22 log(_,level=_,tag=_)", "31 com.example.util.Logger.log(Text)"]);
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

#[test]
fn an_override_is_a_reference_per_supertype() {
  let summary = summary_of("kotlin/overrides.kt");
  assert_eq!(supertypes_of(&summary, "ToolWindowManagerImpl"), ["ToolWindowManagerEx", "Disposable"]);
  assert_eq!(supertypes_of(&summary, "Sub"), ["Base"], "without type arguments");
  assert_eq!(
    refs(&summary, "dispose"),
    ["32 ~Disposable.dispose()", "32 ~ToolWindowManagerEx.dispose()"],
    "one per supertype"
  );
  // object expressions in a companion and in a function, with both overloads
  assert_eq!(
    refs(&summary, "getButtonManager"),
    [
      "20 ~ToolWindowManagerImpl.getButtonManager(ToolWindow)",
      "50 ~ToolWindowManagerImpl.getButtonManager(ToolWindow)",
      "52 ~ToolWindowManagerImpl.getButtonManager(String)",
    ]
  );
  let in_test = summary.references_to("getButtonManager").find(|it| it.line == 50).unwrap();
  assert_eq!(summary.declarations[in_test.container.unwrap() as usize].name, summary.name_id("createManager").unwrap());
  assert_eq!(
    refs(&summary, "remove"),
    ["42 ~StripeButtonManager.remove(ToolWindowAnchor,Boolean)", "43 ?.remove(_,_){}"],
    "by delegation"
  );
  assert_eq!(refs(&summary, "label"), ["72 label", "72 ~Base.label"], "an `override val` constructor parameter");
  assert_eq!(refs(&summary, "size"), ["82 ~Base.size", "121 ~Sized.size"], "also one of a local class");
  assert_eq!(refs(&summary, "create"), ["127 ~Creator.create()"], "a companion with a supertype");
  // a function type has no path, but still counts as a supertype
  assert_eq!(supertypes_of(&summary, "Handler"), ["Weighted", ""]);
  assert_eq!(refs(&summary, "invoke"), ["132 ~?.invoke(Event)", "132 ~Weighted.invoke(Event)"]);
  assert_eq!(refs(&summary, "decorate"), ["84 ~Base.decorate[String](Int)"], "an extension with its receiver");
  assert_eq!(refs(&summary, "run"), ["77 ~Runnable.run()"], "a local class");
  assert_eq!(refs(&summary, "weight"), ["93 ~Level.weight()"], "the body of an enum entry overrides its enum");
  // an object declaration, and a class body the grammar reads as the trailing lambda of its delegate
  assert_eq!(refs(&summary, "changed"), ["107 ~Listener.changed(List/String)", "115 ~Listener.changed(List/String)"]);
  assert_eq!(declarations_of(&summary, "toString"), ["Function toString in Nested"]);
  assert_eq!(refs(&summary, "toString"), NONE, "a class without supertypes");
}

#[test]
fn a_signature_has_the_parameter_types() {
  let summary = summary_of("kotlin/overrides.kt");
  assert_eq!(
    signatures(&summary, "getButtonManager"),
    ["ToolWindowManagerImpl (toolWindow: ToolWindow)", "ToolWindowManagerImpl (paneId: String)"]
  );
  assert_eq!(
    signatures(&summary, "describe"),
    ["Base (prefix: String =, suffix: String? =)", "Sub (prefix: String, suffix: String?)"],
    "default values and nullable types"
  );
  // a type parameter of the class or of the function matches any type
  assert_eq!(
    signatures(&summary, "accept"),
    ["Base (value: _, vararg tags: String[])", "Sub (value: String, vararg tags: String[])"],
    "a vararg is an array"
  );
  assert_eq!(signatures(&summary, "convert"), ["ToolWindowManagerImpl (value: _, into: _, onDone: ToolWindow/Unit)"]);
  assert_eq!(signatures(&summary, "size"), ["Base -", "Sub -"], "a property has none");
  // the last name of each type in it, in order
  let summary = summarize(Language::Kotlin, b"fun show(windows: kotlin.collections.List<a.Window>?) {}");
  let parameter = &declaration(&summary, "show").signature.as_ref().unwrap().parameters[0];
  let names = ["List", "Window"].map(|it| summary.name_id(it).unwrap()).to_vec();
  assert_eq!(parameter.type_ref, Some(TypeRef { names, nullable: true, arrays: 0 }));
  assert_eq!(summary.name(parameter.name), "windows");
}

#[test]
fn a_member_knows_whether_it_overrides_and_can_be_overridden() {
  let summary = summary_of("kotlin/overrides.kt");
  assert_eq!(
    modalities(&summary, "getButtonManager"),
    ["ToolWindowManagerImpl overridable", "ToolWindowManagerImpl overridable"]
  );
  assert_eq!(modalities(&summary, "dispose"), ["ToolWindowManagerImpl overrides overridable"]);
  assert_eq!(modalities(&summary, "describe"), ["Base overridable", "Sub overrides"], "`final override`");
  assert_eq!(modalities(&summary, "label"), ["Base overridable", "Sub overrides overridable"], "constructor properties");
  assert_eq!(modalities(&summary, "accept"), ["Base overridable", "Sub overrides overridable"], "`abstract`");
  for name in ["fixed", "hidden", "log", "reset"] {
    assert_eq!(modalities(&summary, name).len(), 1, "{name}");
    assert!(!modalities(&summary, name)[0].contains("overrid"), "{name}: {:?}", modalities(&summary, name));
  }
  // a member of an interface can be overridden, one of an object can't
  assert_eq!(modalities(&summary, "changed"), ["Listener overridable", "Registry overrides"]);
}

#[test]
fn an_override_in_a_lambda_gets_no_supertype() {
  // the grammar reads a one-line object expression as a call of the literal with a trailing lambda: its override is
  // no member of the class around it
  let source = b"class Holder : Base() {
  val quiet = object : Foo() { override fun one() = 1 }
  val loud = object : Foo() {
    override fun two() = 2
  }
  fun later() = run { listOf(object : Foo() { override fun three() = 3 }) }
}
";
  let summary = summarize(Language::Kotlin, source);
  assert_eq!(refs(&summary, "two"), ["4 ~Foo.two()"]);
  assert_eq!(refs(&summary, "one"), NONE);
  assert_eq!(refs(&summary, "three"), NONE);
  // the same one-line literal in the fixture
  let summary = summary_of("kotlin/overrides.kt");
  assert!(summary.references_to("changed").all(|it| it.line != 112), "{:?}", refs(&summary, "changed"));
  assert!(summary.references_to("changed").any(|it| it.role == Role::Override));
}

#[test]
fn a_call_has_its_argument_shape() {
  let summary = summary_of("kotlin/overrides.kt");
  assert_eq!(
    refs(&summary, "convert"),
    ["136 ?.convert(Number,into=_,Lambda)"],
    "a signed number, a named argument, the trailing lambda last"
  );
  assert_eq!(
    refs(&summary, "mutableListOf"),
    ["136 mutableListOf(Number,_)", "153 mutableListOf()"],
    "a hex number; a real is an expression"
  );
  assert_eq!(refs(&summary, "paneButtons"), ["137 ?.paneButtons(Text,Boolean,_,_)"], "`null` and `1L` are expressions");
  assert_eq!(refs(&summary, "listOf"), ["138 listOf(_,Text)*"], "a spread");
  assert_eq!(refs(&summary, "stripes"), ["139 ?.stripes(ids=_)*"], "a named spread");
  // in an initializer and a getter the receiver is the class; a lambda, `apply`'s included, an object expression, an
  // anonymous function and a local function may have one of their own
  assert_eq!(
    refs(&summary, "add"),
    [
      "144 add(Number,Text)",
      "147 add(Number,Text)",
      "151 add(Number,Text){}",
      "153 this.add(Number,Text){}",
      "157 add(Number,Text){}",
      "161 add(Number,_){}",
      "164 add(Number,Text){}"
    ]
  );
  // the name of a named argument, a call without arguments, and an infix call, which has no shape
  let summary = summarize(Language::Kotlin, b"fun f() = a.b() to c(flag = \"$x\")");
  let shape = |name: &str| summary.references_to(name).next().and_then(|it| it.shape).map(|it| &summary.shapes[it as usize]);
  let flag = summary.name_id("flag");
  assert_eq!(
    shape("c"),
    Some(&Shape::Call {
      arguments: vec![(flag, Argument::Text)],
      spread: false,
      enclosed: false,
    }),
    "a template is text"
  );
  assert_eq!(
    shape("b"),
    Some(&Shape::Call {
      arguments: Vec::new(),
      spread: false,
      enclosed: false,
    })
  );
  assert_eq!(refs(&summary, "to"), ["1 ?.to()"]);
  assert_eq!(shape("to"), None);
}
