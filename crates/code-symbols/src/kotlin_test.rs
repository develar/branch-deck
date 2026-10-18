use crate::extract::summarize;
use crate::language::Language;
use crate::summary::{Argument, DeclKind, ImportTarget, MemberKind, Reach, Role, Shape, TypeRef};
use crate::test_support::{NONE, bare_refs, constructors_of, declaration, declarations_of, lines_of, modalities, reference_text, refs, signatures, summary_of, supertypes_of};

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
  assert_eq!(refs(&summary, "draw"), ["15 <Canvas>canvas.draw(_,_)", "28 draw(_){}"], "in a lambda");
  assert_eq!(refs(&summary, "DEFAULT_SCALE"), ["11 DEFAULT_SCALE"]);
  assert_eq!(refs(&summary, "threshold"), ["18 threshold"]);
  assert_eq!(refs(&summary, "render"), ["29 render"]);
  assert_eq!(refs(&summary, "length"), ["30 String.length"]);
  assert_eq!(refs(&summary, "Renderer"), ["28 :Renderer"]);
  assert_eq!(refs(&summary, "log"), ["22 log(_,level=_,tag=_)", "31 com.example.util.Logger.log(Text)"]);
  assert_eq!(refs(&summary, "to"), ["32 ?.to()"]);
  assert_eq!(refs(&summary, "size"), ["12 <List>.size", "24 <List>.size"]);
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
  assert_eq!(refs(&summary, "turnTracker"), ["10 <Tracker>.turnTracker"]);
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
    ["42 ~StripeButtonManager.remove(ToolWindowAnchor,Boolean)", "43 <StripeButtonManager>.remove(_,_){}"],
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
fn a_member_knows_whether_it_is_jvm_static() {
  let source = b"class Owner {
  companion object {
    @JvmStatic fun create(): Owner = Owner()
    @kotlin.jvm.JvmStatic
    internal fun make() = 1
    @Suppress(\"x\") @JvmStatic val size = 1
    fun plain() = 2
    @JvmName(\"other\") fun renamed() = 3
    @JvmStatic() fun called() = 4
  }
}
";
  let summary = summarize(Language::Kotlin, source);
  assert_eq!(modalities(&summary, "create"), ["Companion jvm_static"]);
  assert_eq!(modalities(&summary, "make"), ["Companion jvm_static"], "by its full name, on the line above");
  assert_eq!(modalities(&summary, "size"), ["Companion jvm_static"], "a property, after another annotation");
  assert_eq!(modalities(&summary, "plain"), ["Companion"]);
  assert_eq!(modalities(&summary, "renamed"), ["Companion jvm_name"], "another annotation");
  assert_eq!(modalities(&summary, "called"), ["Companion jvm_static"], "with parentheses");
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
    ["136 <ToolWindowManagerImpl>.convert(Number,into=_,Lambda)"],
    "a signed number, a named argument, the trailing lambda last"
  );
  assert_eq!(
    refs(&summary, "mutableListOf"),
    ["136 mutableListOf(Number,_)", "153 mutableListOf()"],
    "a hex number; a real is an expression"
  );
  assert_eq!(
    refs(&summary, "paneButtons"),
    ["137 <ToolWindowManagerImpl>.paneButtons(Text,Boolean,_,_)"],
    "`null` and `1L` are expressions"
  );
  assert_eq!(refs(&summary, "listOf"), ["138 listOf(_,Text)*"], "a spread");
  assert_eq!(refs(&summary, "stripes"), ["139 <ToolWindowManagerImpl>.stripes(ids=_)*"], "a named spread");
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

#[test]
fn a_class_knows_its_constructors() {
  let summary = summary_of("kotlin/constructors.kt");
  // a default value, a vararg as an array, a type parameter of the class as any type
  assert_eq!(
    constructors_of(&summary, "ToolWindowManagerImpl").unwrap(),
    ["new(project: Project, isNewUi: Boolean =, isEdtRequired: Boolean =, vararg ids: String[], value: _)"]
  );
  assert_eq!(
    constructors_of(&summary, "Hidden").unwrap(),
    [
      "Container new(size: Int)",
      "new(name: String, vararg rest: Int[])",
      "Container new()",
      "Module new(first: Long, second: Long =)"
    ],
    "the primary constructor with its reach, then the secondary ones"
  );
  assert_eq!(constructors_of(&summary, "Injected").unwrap(), ["Module new(project: Project)"], "with an annotation");
  assert_eq!(constructors_of(&summary, "Implicit").unwrap(), ["new()"]);
  assert_eq!(constructors_of(&summary, "Private").unwrap(), ["new()"], "public, the class is private");
  assert_eq!(
    constructors_of(&summary, "OnlySecondary").unwrap(),
    ["new(size: Int)", "new(name: String)"],
    "no implicit one"
  );
  for name in ["Listener", "Registry", "Companion", "Level", "Marker", "Managers"] {
    assert_eq!(declaration(&summary, name).constructors, None, "{name}");
  }
  // a local class declares nothing, not even into the class around it
  assert_eq!(constructors_of(&summary, "Outer").unwrap(), ["new()"]);
  assert_eq!(declarations_of(&summary, "InFunction"), NONE);
  // two that the summary can't tell apart are both there, neither replaces the other
  assert_eq!(constructors_of(&summary, "Box").unwrap(), ["new(value: _)", "new(list: _)"]);
  assert_eq!(constructors_of(&summary, "Stamp").unwrap(), ["new(date: Date)", "Container new(date: Date)"]);
  // a class the grammar misread, and one with a parse error in it, which may have swallowed a constructor
  assert_eq!(declaration(&summary_of("kotlin/misread.kt"), "Base").constructors, None);
  let broken = summary_of("kotlin/broken.kt");
  assert_eq!(declaration(&broken, "Broken").constructors, None);
  assert_eq!(constructors_of(&broken, "Valid").unwrap(), ["new()"]);
}

#[test]
fn a_constructor_invocation_of_a_supertype_has_its_argument_shape() {
  let summary = summary_of("kotlin/overrides.kt");
  assert_eq!(
    refs(&summary, "ToolWindowManagerImpl"),
    [
      "13 :ToolWindowManagerImpl",
      "14 ToolWindowManagerImpl(project=_,isNewUi=Boolean,isEdtRequired=Boolean,coroutineScope=_)",
      "48 :ToolWindowManagerImpl",
      "49 ToolWindowManagerImpl(_,isNewUi=Boolean,isEdtRequired=Boolean,coroutineScope=_)",
      "135 :ToolWindowManagerImpl",
    ],
    "an object expression's, of the code around it, beside the types"
  );
  let summary = summary_of("kotlin/constructors.kt");
  assert_eq!(
    refs(&summary, "ToolWindowManagerImpl"),
    [
      "54 :ToolWindowManagerImpl",
      "70 ToolWindowManagerImpl(_,isEdtRequired=Boolean,value=_)",
      "72 com.example.windows.ToolWindowManagerImpl(_,Boolean,value=Text)",
      "75 ToolWindowManagerImpl(_,isNewUi=Boolean,isEdtRequired=Boolean,value=_)",
    ],
    "a class's, a qualified one"
  );
  assert_eq!(refs(&summary, "windows"), ["72 :com.example.windows"], "a qualifier is a type");
  assert_eq!(
    refs(&summary, "Base"),
    ["78 Base(Number){}", "80 Base(Number)"],
    "in a lambda, and a one-line object expression"
  );
  // an annotation is no call; `this(…)` calls the class, `super(…)` of a class without a supertype nothing
  assert_eq!(refs(&summary, "Suppress"), ["79 :Suppress"]);
  assert_eq!(refs(&summary, "Inject"), ["3 ^javax.inject.Inject", "25 :Inject"]);
  let on_line = |line: u32| {
    let mut texts: Vec<String> = summary.references.iter().filter(|it| it.line == line).map(|it| reference_text(&summary, it)).collect();
    texts.sort();
    texts
  };
  assert_eq!(on_line(14), [":Int", ":String", "<String>.length", "Hidden(_)"]);
  assert_eq!(on_line(34), [":String"]);
  let sub = summary.references_to("ToolWindowManagerImpl").find(|it| it.line == 70).unwrap();
  assert_eq!(
    (sub.role, summary.declarations[sub.container.unwrap() as usize].name),
    (Role::Call, summary.name_id("Sub").unwrap())
  );
  let in_function = summary.references_to("ToolWindowManagerImpl").find(|it| it.line == 75).unwrap();
  assert_eq!(
    summary.declarations[in_function.container.unwrap() as usize].name,
    summary.name_id("createManager").unwrap()
  );
}

#[test]
fn a_bound_name_carries_its_declared_type() {
  let source = b"class Use {
  fun <T> f(m: a.Manager<String>?, t: T, callback: () -> Unit, vararg rest: Manager) {
    m!!.one()
    val local = Manager(1)
    local.two()
    val typed: Other = make()
    typed.three()
    val made = make()
    made.four()
    t.five()
    callback.six()
    rest.seven()
    run { x: Manager -> x.eight() }
    run { y -> y.nine() }
    val lazy by lazy { Manager() }
    lazy.ten()
  }
}
";
  let summary = summarize(Language::Kotlin, source);
  assert_eq!(refs(&summary, "one"), ["3 <a.Manager>.one()"], "without its type arguments, through `!!`");
  assert_eq!(refs(&summary, "two"), ["5 <Manager>.two()"], "the type its initializer constructs");
  assert_eq!(refs(&summary, "three"), ["7 <Other>.three()"], "a declared type wins");
  assert_eq!(refs(&summary, "eight"), ["13 <Manager>.eight(){}"], "a typed lambda parameter");
  for name in ["four", "five", "six", "seven", "nine", "ten"] {
    assert!(
      refs(&summary, name)[0].contains("?."),
      "{name}: an inferred type, a type parameter, a function type, a vararg, an untyped lambda parameter, a delegate: {:?}",
      refs(&summary, name)
    );
  }
}

#[test]
fn a_property_knows_its_type() {
  let source = b"class Use(private val manager: Manager, plain: Plain) {
  private val built = Manager(1)
  val typed: Other? = null
  val first = plain.zero()

  fun f() {
    manager.one()
    this.built.two()
    typed.three()
    run { manager.four() }
  }

  inner class Inner {
    fun g() = manager.five()
  }

  class Nested {
    fun h() = manager.six()
  }

  companion object {
    fun i() = this.manager.seven()
  }
}

class Sub : Base() {
  val manager = Manager()

  inner class Deep : Other() {
    fun j() = manager.eight()
  }
}
";
  let summary = summarize(Language::Kotlin, source);
  assert_eq!(modalities(&summary, "manager"), ["Use", "Sub"]);
  assert_eq!(declaration(&summary, "built").value_type.as_ref().map(Vec::len), Some(1));
  assert_eq!(refs(&summary, "zero"), ["4 <Plain>.zero()"], "a constructor parameter");
  assert_eq!(refs(&summary, "one"), ["7 <Manager>manager.one()"]);
  assert_eq!(refs(&summary, "two"), ["8 <Manager>this.built.two()"]);
  assert_eq!(refs(&summary, "three"), ["9 <Other>typed.three()"]);
  assert_eq!(refs(&summary, "five"), ["14 <Manager>manager.five()"], "an inner class sees the class around it");
  for name in ["four", "six", "seven", "eight"] {
    assert!(
      !refs(&summary, name)[0].contains('<'),
      "{name}: enclosed, past a nested class, a companion, a class with supertypes: {:?}",
      refs(&summary, name)
    );
  }
}

#[test]
fn a_smart_cast_drops_the_type() {
  let source = b"class Use(val manager: Manager) {
  fun f(m: Manager, n: Manager, k: Manager) {
    if (m is Sub) m.one()
    val s = n as Sub
    n.two()
    when (k) {
      is Sub -> k.three()
      else -> {}
    }
    if (manager is Sub) manager.four()
  }

  fun g(m: Manager) = m.five()
}
";
  let summary = summarize(Language::Kotlin, source);
  for name in ["one", "two", "three", "four"] {
    assert!(!refs(&summary, name)[0].contains('<'), "{name}: {:?}", refs(&summary, name));
  }
  assert_eq!(refs(&summary, "five"), ["13 <Manager>.five()"], "a name of another scope keeps its type");
}

#[test]
fn an_extension_receiver_or_a_function_hides_the_type() {
  let source = b"class C(m0: Manager, val p: Manager) : Base() {
  val m: Manager = Manager()
  val early = m0.zero()

  init {
    m0.one()
  }

  fun Other.e1() = this.m.two()

  val Other.e2 get() = m.three()

  fun C.e3() = m.four()

  fun g() = m0.five()

  val h get() = m0.six()

  fun k() {
    val (d) = Manager()
    d.seven()
    if (1 in list) p.eight()
  }
}
";
  let summary = summarize(Language::Kotlin, source);
  assert_eq!(refs(&summary, "zero"), ["3 <Manager>.zero()"], "a constructor parameter in an initializer");
  assert_eq!(refs(&summary, "one"), ["6 <Manager>.one()"], "and in `init`");
  assert_eq!(refs(&summary, "four"), ["13 <Manager>m.four()"], "an extension of the class itself");
  assert_eq!(refs(&summary, "eight"), ["22 <Manager>p.eight()"], "`in` is no type check");
  for name in ["two", "three", "five", "six", "seven"] {
    assert!(!refs(&summary, name)[0].contains('<'), "{name}: {:?}", refs(&summary, name));
  }
}

/// The header of a function keeps what a use depends on (`suspend`, the receiver, the return type) and leaves out the
/// body, the parameters the signature holds, annotations and the modifiers the reach or the overriding say
#[test]
fn a_header_is_the_form_before_the_body() {
  let header = |source: &str| {
    let summary = summarize(Language::Kotlin, source.as_bytes());
    declaration(&summary, "cleanup").header.clone()
  };
  let plain = header("class L {\n  fun cleanup() {\n    a()\n  }\n}\n");
  assert_eq!(plain.as_deref(), Some("fun cleanup"));
  assert_eq!(header("class L {\n  fun cleanup() {\n    b()\n  }\n}\n"), plain, "a change of the body");
  assert_eq!(header("class L {\n  @Synchronized\n  public open fun cleanup(force: Boolean = false) = b()\n}\n"), plain);
  assert_eq!(header("class L {\n  suspend fun cleanup() {}\n}\n").as_deref(), Some("suspend fun cleanup"));
  assert_eq!(header("fun <T> String.cleanup(): T? = null\n").as_deref(), Some("fun < T > String . cleanup : T ?"));
  assert_eq!(header("class L {\n  val cleanup: Int = 1\n}\n").as_deref(), Some("val cleanup : Int"));
  assert_eq!(header("class L {\n  val cleanup: Int by lazy { 1 }\n}\n").as_deref(), Some("val cleanup : Int"));
  assert_eq!(header("class cleanup\n"), None, "a type has none");
}

/// A reference knows whether a suspend call is allowed where it is: in a `suspend` function and the lambda of a
/// coroutine builder, not in another function, an initializer or the lambda of `sequence`; unknown in the lambda of
/// another function
#[test]
fn a_reference_knows_its_coroutine_context() {
  let source = "class L {
  val ready = check()
  fun plain() { check() }
  suspend fun suspending() { check() }
  fun blocking() = runBlocking { check() }
  suspend fun nested() { fun local() { check() } }
  fun scoped(scope: CoroutineScope) { scope.launch(Dispatchers.IO) { check() } }
  fun lazy() = sequence { check() }
  fun other() = assertThrows(E::class.java) { check() }
}
";
  let summary = summarize(Language::Kotlin, source.as_bytes());
  let contexts: Vec<(u32, Option<bool>)> = summary.references_to("check").map(|it| (it.line, it.coroutine)).collect();
  assert_eq!(
    contexts,
    [
      (2, Some(false)),
      (3, Some(false)),
      (4, Some(true)),
      (5, Some(true)),
      (6, Some(false)),
      (7, Some(true)),
      (8, Some(false)),
      (9, None)
    ]
  );
  let java = summarize(Language::Java, b"class L { void plain() { check(); } }\n");
  assert_eq!(java.references_to("check").map(|it| it.coroutine).collect::<Vec<_>>(), [Some(false)]);
}
