use crate::extract::summarize as summarize_at;
use crate::language::{Language, summary_key};
use crate::summary::{DeclKind, FileSummary, ImportTarget, MemberKind, Reach, Role};
use crate::test_support::{NONE, bare_refs, declaration, declarations_of, line_references, refs, summarize, summary_of};

/// The summary of `source` as the TypeScript, TSX or JavaScript file `path`
fn summary_at(path: &str, source: &str) -> FileSummary {
  summarize_at(Language::TypeScript, path, source.as_bytes())
}

fn header<'s>(summary: &'s FileSummary, name: &str) -> &'s str {
  declaration(summary, name).header.as_deref().unwrap_or_else(|| panic!("{name} has no header"))
}

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

/// A `.ts` file is TypeScript without JSX, where `<T>x` asserts a type; a `.tsx` and a `.js` file read JSX; the cache
/// keeps the three apart
#[test]
fn the_extension_chooses_the_dialect() {
  let assertion = "const size = <Size>value\n";
  let ts = summary_at("a.ts", assertion);
  assert_eq!(
    (ts.error_bytes, refs(&ts, "Size"), refs(&ts, "value")),
    (0, vec!["1 :Size".to_string()], vec!["1 value".to_string()])
  );
  // as TSX, `<Size>` opens an element that never closes
  assert!(summary_at("a.tsx", assertion).error_bytes > 0);
  let element = "export const view = <Panel title={name} />\n";
  for path in ["a.tsx", "a.js", "a.jsx", "a.mjs"] {
    let summary = summary_at(path, element);
    assert_eq!((summary.error_bytes, refs(&summary, "Panel")), (0, vec!["1 :Panel".to_string()]), "{path}");
  }
  // JavaScript has no type annotations
  assert!(summary_at("a.js", "let size: number = 1\n").error_bytes > 0);
  let keys: Vec<u8> = ["a.ts", "a.mts", "a.cts", "a.tsx", "a.js", "a.jsx", "a.mjs", "a.cjs"]
    .iter()
    .map(|path| summary_key(Language::TypeScript, path))
    .collect();
  assert_eq!(keys[..3], [keys[0]; 3]);
  assert_eq!(keys[5..], [keys[4]; 3]);
  assert!(keys[0] != keys[3] && keys[3] != keys[4] && keys[0] != keys[4], "{keys:?}");
  assert_eq!(keys[0], Language::TypeScript.id());
}

/// The header is oxc's tokens: a string or a template literal is one token or a few, a type parameter keeps its default
#[test]
fn a_header_is_the_tokens_of_the_parse() {
  let summary = summarize(
    Language::TypeScript,
    concat!(
      "export function pick<T extends object = {}>(mode: \"a\" | 'b', prefix = `x-${name}`): T { return null! }\n",
      "declare function ambient(): void\n",
      "export declare const LIMIT: number\n",
      "class Box {\n",
      "  @observable private readonly value: string = \"\";\n",
      "  constructor(@inject() protected readonly size: number = 1) {}\n",
      "}\n",
    )
    .as_bytes(),
  );
  assert_eq!(
    header(&summary, "pick"),
    "function pick < T extends object = { } > ( mode : \"a\" | 'b' , prefix = `x-${ name }` ) : T"
  );
  assert_eq!(header(&summary, "ambient"), "function ambient ( ) : void", "no `declare`");
  assert_eq!(header(&summary, "LIMIT"), "const LIMIT : number");
  assert_eq!(header(&summary, "value"), "readonly value : string", "no decorator, accessibility or initializer");
  assert_eq!(header(&summary, "size"), "readonly size : number");
}

/// A call with type arguments (`f<T>()`, `this.m<T>()`) is a call, as one without them
#[test]
fn a_call_with_type_arguments_is_a_call() {
  let summary = summarize(Language::TypeScript, b"const a = read<Config>(path)\nconst b = this.load<Data>()\nconst c = new Box<T>()\n");
  assert_eq!(refs(&summary, "read"), ["1 read()"]);
  assert_eq!(refs(&summary, "load"), ["2 this.load()"]);
  assert_eq!(refs(&summary, "Box"), ["3 Box()"]);
}

/// `export declare` exports: an ambient declaration of a module is public
#[test]
fn an_exported_ambient_declaration_is_public() {
  let summary = summarize(
    Language::TypeScript,
    b"export declare const LIMIT: number\nexport declare class Box {}\nexport declare function make(): Box\ndeclare const hidden: number\n",
  );
  for name in ["LIMIT", "Box", "make"] {
    assert_eq!(declaration(&summary, name).reach, Reach::Public, "{name}");
  }
  assert_eq!(declaration(&summary, "hidden").reach, Reach::File);
}

/// A declaration starts at its doc comment, its decorators and its `export`, whichever is first; a `///` directive is
/// no doc comment, and the bindings of an import start with it
#[test]
fn a_declaration_starts_at_its_doc_comment_and_decorators() {
  let summary = summarize(
    Language::TypeScript,
    concat!(
      "/** Imports. */\n",
      "import { Model } from \"./model\"\n",
      "/** A box. */\n",
      "@sealed\n",
      "export class Box {\n",
      "  /** The size. */\n",
      "  @input()\n",
      "  size = 1\n",
      "}\n",
      "/// <reference types=\"node\" />\n",
      "export declare namespace Space {}\n",
      "enum Kind {\n",
      "  /** Round. */\n",
      "  Round,\n",
      "}\n",
    )
    .as_bytes(),
  );
  let start = |name: &str| (declaration(&summary, name).start_line, declaration(&summary, name).name_line);
  assert_eq!(start("Model"), (2, 2));
  assert_eq!(start("Box"), (3, 5));
  assert_eq!(start("size"), (6, 8));
  assert_eq!(start("Space"), (11, 11));
  assert_eq!(start("Round"), (13, 14));
}

/// The references of a decorator belong to the declaration it decorates
#[test]
fn a_decorator_is_of_the_declaration_it_decorates() {
  let summary = summarize(
    Language::TypeScript,
    b"@Component({ imports: [Child] })\nexport class Parent {\n  @Input() name = \"\"\n  @HostListener(\"click\") onClick() {}\n}\n",
  );
  assert_eq!(line_references(&summary, 1), ["Child#Parent", "Component()#Parent"]);
  assert_eq!(line_references(&summary, 3), ["Input()#name"]);
  assert_eq!(line_references(&summary, 4), ["HostListener()#onClick"]);
}

/// A constructor declares nothing, its overloads neither; an accessor is a value, an overload signature of one too
#[test]
fn a_constructor_signature_declares_nothing_and_an_accessor_signature_is_a_value() {
  let summary = summarize(
    Language::TypeScript,
    b"export declare class Box {\n  constructor(size: number);\n  get size(): number;\n  set size(value: number);\n  grow(by: number): void;\n}\n",
  );
  assert_eq!(declarations_of(&summary, "constructor"), NONE);
  assert_eq!(declarations_of(&summary, "size"), ["Value size in Box", "Value size in Box"]);
  assert_eq!(declarations_of(&summary, "grow"), ["Function grow in Box"]);
}

/// `namespace a.b.c` is one module, named `c`, which holds what the body declares
#[test]
fn a_dotted_namespace_is_one_module_of_its_last_name() {
  let summary = summarize(Language::TypeScript, b"export namespace app.view.panels {\n  export class Panel {}\n}\n");
  assert_eq!(declarations_of(&summary, "panels"), ["Module panels in -"]);
  assert_eq!(declarations_of(&summary, "app"), NONE);
  assert_eq!(declarations_of(&summary, "Panel"), ["Type Panel in panels"]);
  assert_eq!(refs(&summary, "app"), NONE);
}

/// A computed identifier names a member, any other computed key (`[Symbol.iterator]`) none, and its names are references
#[test]
fn only_a_computed_identifier_names_a_member() {
  let summary = summarize(Language::TypeScript, b"class Bag {\n  [key]() {}\n  [Symbol.iterator]() {}\n  \"quoted\" = 1\n  0 = 2\n}\n");
  assert_eq!(declarations_of(&summary, "key"), ["Function key in Bag"]);
  assert_eq!(declarations_of(&summary, "Symbol"), NONE);
  assert_eq!(declarations_of(&summary, "quoted"), ["Value quoted in Bag"]);
  assert_eq!(refs(&summary, "key"), NONE);
  assert_eq!(refs(&summary, "iterator"), ["3 Symbol.iterator"]);
}

/// The names of an index signature, a named tuple member, a type predicate and `as const` are no references
#[test]
fn names_that_name_no_declaration_are_no_references() {
  let summary = summarize(
    Language::TypeScript,
    b"interface Map { [key: string]: number }\ntype Pair = [label: string, count: number]\nconst isText = (x: unknown): x is string => true\nconst modes = [\"a\"] as const\nimport Alias = Outer.Inner\n",
  );
  for name in ["key", "label", "count", "x", "const", "Alias"] {
    assert_eq!(refs(&summary, name), NONE, "{name}");
  }
  assert_eq!(refs(&summary, "Inner"), ["5 Outer.Inner"]);
}

/// `import("m").A.B` names a type of a module the file does not name: no qualifier
#[test]
fn a_type_of_an_import_type_is_a_reference() {
  let summary = summarize(Language::TypeScript, b"let bar: import(\"./title-bar\").TitleBar\nlet kind: typeof import(\"./m\").kinds\n");
  assert_eq!(refs(&summary, "TitleBar"), ["1 :?.TitleBar"]);
  assert_eq!(refs(&summary, "kinds"), ["2 ?.kinds"]);
}

/// What `infer` declares is seen in its whole conditional type; a function type's parameters are its own
#[test]
fn infer_is_seen_in_its_conditional_type() {
  let summary = summarize(
    Language::TypeScript,
    b"type Instance<T> = T extends new (...args: any) => infer R ? R : never\nfunction run(callback: (value: number) => void) { return value }\n",
  );
  assert_eq!(refs(&summary, "R"), NONE);
  assert_eq!(refs(&summary, "args"), NONE);
  assert_eq!(refs(&summary, "value"), ["2 value"], "the parameter of the function type is not the function's");
}

/// A class expression binds its name in its body; an object literal's method binds nothing
#[test]
fn a_class_expression_binds_its_name() {
  let summary = summarize(
    Language::TypeScript,
    b"function f() {\n  const A = class Named { m() { return Named } }\n  const o = { helper() {} }\n  return helper()\n}\n",
  );
  assert_eq!(refs(&summary, "Named"), NONE);
  assert_eq!(refs(&summary, "helper"), ["4 helper()"]);
}

/// The errors are what a recovery blanked and what the parse reports; a declaration with one between its start and
/// its name is `in_error`; a file no recovery saves has every byte in error, and is read
#[test]
fn errors_are_what_the_parse_recovered_from() {
  let clean = summarize(Language::TypeScript, b"export function ok() {}\n");
  assert_eq!((clean.error_bytes, declaration(&clean, "ok").in_error), (0, false));
  let broken = summarize(Language::TypeScript, b"export function ok() {}\nfunction broken(a: ) {}\nclass After {}\n");
  assert!(broken.error_bytes > 0);
  assert!(!declaration(&broken, "ok").in_error);
  assert!(!declaration(&broken, "After").in_error);
  let binary = summarize(Language::TypeScript, b"\x00\x01{{{ )))\n");
  assert!(!binary.truncated);
}

/// A name nested `depth` times in `open` and `close`, a hundred a line, as no minified file has them
pub(crate) fn nested_source(depth: usize, open: &str, close: &str) -> String {
  let repeat = |text: &str| -> String { (0..depth).map(|index| if index % 100 == 99 { format!("{text}\n") } else { text.to_string() }).collect() };
  format!("export const deep =\n{}x{}\n", repeat(open), repeat(close))
}

/// The forms of nesting a file of code writes deepest: arrays, objects, calls, arrows, conditions, members of a chain
pub(crate) const NESTINGS: [(&str, &str); 6] = [("[", "]"), ("{k:", "}"), ("f(", ")"), ("() => (", ")"), ("c ? (", ") : 0"), ("(", ").m()")];

/// The parse and every walk of the tree fit a 2 MiB thread at any nesting the parse reads, as trial-check's workers
/// and tokio's blocking threads have; past what the parse reads the summary is truncated
#[test]
fn deep_nesting_is_read_on_a_small_stack() {
  let thread = std::thread::Builder::new().stack_size(2 << 20).spawn(|| {
    for (open, close) in NESTINGS {
      let deep = summarize(Language::TypeScript, nested_source(20_000, open, close).as_bytes());
      assert!(!deep.truncated, "{open}");
      assert_eq!(deep.error_bytes, 0, "{open}");
      assert_eq!(deep.references_to("x").count(), 1, "{open}");
      assert!(deep.declares("deep"), "{open}");
    }
    // a gibibyte of stack is the most a parse takes
    assert!(summarize(Language::TypeScript, nested_source(300_000, "[", "]").as_bytes()).truncated);
  });
  thread.unwrap().join().unwrap();
}
