//! The facts the summary of Rust reads differently on rust-analyzer's tree than it did on tree-sitter's, one test per
//! class of the oracle's moved facts (docs/design/symbol-dependencies.md, decision 26)

use crate::language::Language;
use crate::summary::{DeclKind, MemberKind};
use crate::test_support::{NONE, declaration, declarations_of, line_references, refs, summarize};

fn header(source: &str, name: &str) -> String {
  let summary = summarize(Language::Rust, source.as_bytes());
  declaration(&summary, name).header.clone().unwrap_or_default()
}

/// The header is the tokens rust-analyzer lexes: a lifetime is one, the `;` of an array type and the `=` of an
/// associated type are the header's, the `,` that ends a `where` and the `;` that ends the item are not, an ABI is
/// one string
#[test]
fn a_header_is_the_tokens_of_the_item_as_lexed() {
  assert_eq!(header("fn f<'a>(x: &'a str) -> &'static str {}\n", "f"), "fn f < 'a > ( x : & 'a str ) -> & 'static str");
  assert_eq!(header("const N: [u8; 3] = [1, 2, 3];\n", "N"), "const N : [ u8 ; 3 ]");
  assert_eq!(
    header("fn ids(&self) -> impl Iterator<Item = &str> {}\n", "ids"),
    "fn ids ( & self ) -> impl Iterator < Item = & str >"
  );
  assert_eq!(header("fn g<T>(t: T) where T: Clone, {}\n", "g"), "fn g < T > ( t : T ) where T : Clone");
  assert_eq!(header("unsafe extern \"C\" fn h(x: u8);\n", "h"), "unsafe extern \"C\" fn h ( x : u8 )");
  assert_eq!(header("#[inline]\n/// Doc\npub(crate) fn k(#[cfg(x)] a: u8) {}\n", "k"), "fn k ( a : u8 )");
  assert_eq!(header("struct S { pub(super) a: Vec<u8>, }\n", "a"), "a : Vec < u8 >");
}

/// After a group among the tokens of a macro, the names of a chain are members of an expression (`?.unwrap`), as
/// outside a macro, not of the path before the group
#[test]
fn a_member_after_a_group_in_a_macro_is_of_an_expression() {
  let summary = summarize(Language::Rust, b"fn f() {\n  assert_eq!(std::fs::read(path).unwrap().len, 3);\n}\n");
  assert_eq!(
    line_references(&summary, 2),
    ["?.len#f", "?.unwrap#f", "assert_eq!#f", "path#f", "std#f", "std.fs#f", "std.fs.read#f"]
  );
}

/// Keywords, lifetimes, `_`, literals (`1f16` too) and metavariables among the tokens of a macro are no names; the
/// patterns of `macro_rules!` are skipped, and a macro in a pattern binds its names there
#[test]
fn the_tokens_of_a_macro_that_are_no_names() {
  let source = "macro_rules! m {\n  ($x:expr, $t:ty) => { for i in $x { let _: $t = i; } };\n}\nfn f(v: u8) {\n  check!(1f16.to_bits(), 'a, ref v, _, else);\n  match v {\n    id!(Some(a)) => use_it(a),\n    _ => {}\n  }\n}\n";
  let summary = summarize(Language::Rust, source.as_bytes());
  assert_eq!(line_references(&summary, 2), ["i#m"], "no keywords or metavariables of a body, nothing of its pattern");
  assert_eq!(line_references(&summary, 5), ["?.to_bits#f", "check!#f"]);
  assert_eq!(line_references(&summary, 7), ["Some#f", "id!#f", "use_it()#f"], "`a` is bound by the pattern");
}

/// A capitalized name alone as the pattern of a match arm is a variant or a constant, a reference, as in any pattern
#[test]
fn a_capitalized_pattern_of_an_arm_is_a_reference() {
  let summary = summarize(
    Language::Rust,
    b"fn f(x: Option<u8>) -> u8 {\n  match x {\n    None => 0,\n    MAX => 1,\n    n => n,\n  }\n}\n",
  );
  assert_eq!(refs(&summary, "None"), ["3 None"]);
  assert_eq!(refs(&summary, "MAX"), ["4 MAX"]);
  assert_eq!(refs(&summary, "n"), NONE);
}

/// `self: Box<Self>` and `self: &Arc<Self>` are a receiver: the method is an instance member, and the type a reference
#[test]
fn a_typed_self_makes_an_instance_member() {
  let summary = summarize(Language::Rust, b"impl S {\n  fn start(self: &Arc<Self>) {}\n  fn make() {}\n}\n");
  assert_eq!(declaration(&summary, "start").member, MemberKind::Instance);
  assert_eq!(declaration(&summary, "make").member, MemberKind::Static);
  assert_eq!(refs(&summary, "Arc"), ["2 :Arc"]);
}

/// A path from the crate root (`::core::mem`) and one up two modules (`super::super`) keep their segments
#[test]
fn a_path_from_the_root_keeps_its_segments() {
  let source = "use ::http::{HeaderMap, Method};\nuse super::super::state::Output;\nfn f() -> usize {\n  ::core::mem::size_of::<::core::ffi::c_uint>()\n}\n";
  let summary = summarize(Language::Rust, source.as_bytes());
  assert_eq!(refs(&summary, "HeaderMap"), ["1 ^http.HeaderMap"]);
  assert_eq!(declarations_of(&summary, "Output"), ["Binding Output in -"]);
  assert_eq!(refs(&summary, "Output"), ["2 ^super.super.state.Output"]);
  assert_eq!(refs(&summary, "size_of"), ["4 core.mem.size_of()"]);
  assert_eq!(refs(&summary, "c_uint"), ["4 :core.ffi.c_uint"]);
}

/// An attribute is in the declaration's node: its references belong to the declaration around it, and those of a
/// `use` are read too
#[test]
fn the_references_of_an_attribute_are_of_the_container_around_its_item() {
  let source = "#[derive(Debug)]\nstruct S {\n  #[serde(rename = \"b\")]\n  a: u8,\n}\n#[diagnostic::on_unknown(note = \"x\")]\nuse std::x;\n";
  let summary = summarize(Language::Rust, source.as_bytes());
  assert_eq!(line_references(&summary, 1), [":Debug#-"]);
  assert_eq!(line_references(&summary, 3), ["serde!#S"]);
  assert_eq!(line_references(&summary, 6), ["diagnostic.on_unknown!#-"]);
}

/// `_` is no type, and a name with type arguments before `::` is a type (`Vec::<u8>::new()`)
#[test]
fn the_types_of_paths() {
  let summary = summarize(Language::Rust, b"fn f() {\n  let v: Vec<_> = Vec::<u8>::new();\n}\n");
  assert_eq!(line_references(&summary, 2), [":Vec#f", "Vec.new()#f"]);
  assert_eq!(refs(&summary, "_"), NONE);
}

/// rust-analyzer recovers inside an item: a declaration whose name comes before the error is read whole and not in
/// error, one whose node has an error before its name is; the bytes of an `ERROR` node and one per missing token are
/// the error bytes
#[test]
fn an_error_marks_the_declarations_it_lies_before_the_name_of() {
  let summary = summarize(Language::Rust, b"struct S {\n  a: r#\"raw\"#,\n}\nfn f() { let x = 1 }\npub(in) fn g() {}\n");
  let a = declaration(&summary, "a");
  assert_eq!((a.kind, a.in_error, a.header.as_deref()), (DeclKind::Value, false, Some("a : r#\"raw\"#")));
  assert!(!declaration(&summary, "f").in_error, "the missing `;` is after the name");
  assert!(declaration(&summary, "g").in_error, "the broken visibility is before the name");
  // `r#"raw"#` (8 bytes) in an ERROR node, and three missing: the type before it, the `;` after `1`, the path of `pub(in)`
  assert_eq!(summary.error_bytes, 8 + 3);
  assert!(!summary.truncated);
}

/// A file nested past what any code nests is not parsed (`rust_syntax::nesting::MAX_LEVELS`): a truncated summary,
/// never a stack overflow; one nested 20 000 deep is summarized on a thread of 2 MiB, its parse, walk and drop on a
/// stack `rust_syntax::guarded` sizes for it. Both within the size limits: five levels a line
#[test]
fn deep_nesting_is_summarized_on_a_small_stack() {
  let thread = std::thread::Builder::new().stack_size(2 << 20).spawn(|| {
    let deep = format!("fn f() {}x{}\nfn g() {{}}\n", "{ { { { {\n".repeat(4_000), "} } } } }\n".repeat(4_000));
    let summary = summarize(Language::Rust, deep.as_bytes());
    assert!(!summary.truncated);
    assert_eq!(declarations_of(&summary, "g"), ["Function g in -"]);
    assert_eq!(refs(&summary, "x"), ["4001 x"]);
    let too_deep = "((((((((\n".repeat(rust_syntax::nesting::MAX_LEVELS / 8 + 1);
    let cut = summarize(Language::Rust, too_deep.as_bytes());
    assert!(cut.truncated && cut.lines < crate::extract::MAX_LINES, "past the nesting, within the size limits");
  });
  thread.unwrap().join().unwrap();
}
