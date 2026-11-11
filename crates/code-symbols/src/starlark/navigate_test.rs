//! Go to Declaration and Find Usages in Starlark, beside `navigate_test.rs`: the scopes of the language and the
//! forms a name is read in.

use crate::highlight::Grammar;
use crate::navigate::{Access, ReferenceKind, usages_of};
use crate::test_support::{at, kind_and_local, usages};
use pretty_assertions::assert_eq;

const BUILD: &str = "load(\"//defs:jvm.bzl\", \"jps_test\", kt = \"kotlin_library\")

VISIBILITY = [\"//visibility:public\"]

def _tagged(name, extra = []):
    tags = extra + [name]
    return tags

jps_test(
    name = \"monolith\",
    tags = _tagged(\"monolith\"),
    visibility = VISIBILITY,
)

kt(
    name = \"runtime\",
    deps = [\":monolith\"],
)

native.cc_library(name = \"bridge\")
";

#[test]
fn a_call_of_a_loaded_macro_goes_to_the_line_that_loads_it() {
  // the `load` binds the name, so the declaration of the call is the loaded name of line 0
  assert_eq!(kind_and_local(Grammar::StarlarkBuild, BUILD, "jps_test(\n", 0), (ReferenceKind::Call, Some((0, 24))));
  // an alias is bound by its own name, not by the name it loads
  assert_eq!(kind_and_local(Grammar::StarlarkBuild, BUILD, "kt(\n", 0), (ReferenceKind::Call, Some((0, 35))));
}

#[test]
fn a_call_of_a_def_of_the_file_goes_to_the_def() {
  assert_eq!(
    kind_and_local(Grammar::StarlarkBuild, BUILD, "_tagged(\"monolith\")", 0),
    (ReferenceKind::Call, Some((4, 4)))
  );
}

#[test]
fn a_read_of_an_assigned_name_goes_to_its_assignment() {
  assert_eq!(kind_and_local(Grammar::StarlarkBuild, BUILD, "VISIBILITY,", 0), (ReferenceKind::Value, Some((2, 0))));
}

#[test]
fn a_qualified_call_names_its_receiver_and_has_no_declaration_of_its_own() {
  let reference = at(Grammar::StarlarkBuild, BUILD, "cc_library", 0);
  assert_eq!(
    (reference.kind, reference.qualifier.as_deref(), reference.local),
    (ReferenceKind::MemberCall, Some("native"), None)
  );
}

#[test]
fn the_usages_of_a_parameter_stay_inside_its_def() {
  let (local, found) = usages(Grammar::StarlarkBuild, BUILD, "name, extra", 0);
  // the parameter `name` of `_tagged`, used in its body; the `name = …` arguments of the calls are not its usages
  assert_eq!((local, found), (true, vec![(4, Access::Declaration), (5, Access::Read)]));
  // a name the body assigns is a local of that body too; every assignment of Starlark declares its name
  let (local, found) = usages(Grammar::StarlarkBuild, BUILD, "tags = extra", 0);
  assert_eq!((local, found), (true, vec![(5, Access::Declaration), (6, Access::Read)]));
}

#[test]
fn the_usages_of_a_loaded_name_are_its_import_and_its_calls() {
  let (local, found) = usages(Grammar::StarlarkBuild, BUILD, "jps_test(\n", 0);
  assert_eq!((local, found), (false, vec![(0, Access::Import), (8, Access::Call)]));
}

#[test]
fn a_bzl_file_reads_the_same_names_as_a_build_file() {
  let text = "def helper(x):\n    return x\n\nVALUE = helper(1)\n";
  assert_eq!(kind_and_local(Grammar::Starlark, text, "helper(1)", 0), (ReferenceKind::Call, Some((0, 4))));
  let found: Vec<(u32, Access)> = usages_of(Grammar::Starlark, text, "helper").iter().map(|it| (it.start.line, it.access)).collect();
  assert_eq!(found, [(0, Access::Declaration), (3, Access::Call)]);
}

#[test]
fn a_label_in_a_string_is_no_name() {
  // `":monolith"` names a target, but the summary and the navigation read names of the tree alone
  let found = usages_of(Grammar::StarlarkBuild, BUILD, "monolith");
  assert_eq!(found, []);
}

#[test]
fn a_comprehension_is_a_scope_and_a_default_is_read_where_its_function_is_defined() {
  let text = "names = [\"a\"]\nname = \"x\"\n\ndef macro(name, deps = name, key = lambda name = name: name):\n    labels = [name for name in name.split() if name]\n    return [dep for dep in deps for name in dep] + [name]\n\ntops = [name for name in names]\n";
  let local = |needle: &str, occurrence: usize| kind_and_local(Grammar::Starlark, text, needle, occurrence).1;
  // the defaults of the `def` and of the `lambda` read the file's `name`, the body of the `lambda` its parameter
  assert_eq!(
    (local("name, key", 0), local("name: name", 0), local("name):", 0)),
    (Some((1, 0)), Some((1, 0)), Some((3, 42)))
  );
  // a comprehension sees its variable in its element and its condition; its first collection is read outside it
  assert_eq!(
    (local("name for name in name.split", 0), local("name.split", 0), local("name]\n    return", 0)),
    (Some((4, 23)), Some((3, 10)), Some((4, 23)))
  );
  // a later collection sees the variables before it, and past the comprehension the name is the parameter again
  assert_eq!((local("dep] +", 0), local("name]\n\n", 0)), (Some((5, 20)), Some((3, 10))));
  // the variable of a comprehension is a local, of the file's top level too: the file's `name` is none of its usages
  assert_eq!(
    usages(Grammar::Starlark, text, "name for name in names", 0),
    (true, vec![(7, Access::Read), (7, Access::Declaration)])
  );
  let of_file: Vec<(u32, Access)> = usages_of(Grammar::Starlark, text, "name").iter().map(|it| (it.start.line, it.access)).collect();
  assert_eq!(of_file, [(1, Access::Declaration), (3, Access::Read), (3, Access::Read)]);
}

/// A name a `def` binds is one variable in all of it: the first place that binds it declares it (the parameter, else
/// the first assignment), a later one writes it, and a name before its assignment, a closure's too, is the variable's
#[test]
fn a_name_a_def_binds_is_one_variable_in_all_of_it() {
  let text = "total = \"global\"\n\ndef outer(deps, flag):\n    def inner():\n        return total\n    total = 1\n    if flag:\n        total = 2\n    deps = deps + [total]\n    for deps in deps:\n        pass\n    return inner() + total\n";
  let found = |needle: &str, occurrence: usize| kind_and_local(Grammar::Starlark, text, needle, occurrence);
  // the closure reads the variable its `def` assigns after it, not the name of the file
  assert_eq!(found("total\n    total", 0), (ReferenceKind::Value, Some((5, 4))));
  // the first assignment declares, the one of the branch is a use of the same variable
  assert_eq!(
    (found("total = 1", 0), found("total = 2", 0)),
    ((ReferenceKind::Declaration, None), (ReferenceKind::Value, Some((5, 4))))
  );
  // an assignment and a `for` of the name of a parameter write the parameter, and read it on their right
  assert_eq!(
    (found("deps = deps", 0), found("deps + [", 0)),
    ((ReferenceKind::Value, Some((2, 10))), (ReferenceKind::Value, Some((2, 10))))
  );
  assert_eq!(
    (found("deps in deps", 0), found("deps:", 0)),
    ((ReferenceKind::Value, Some((2, 10))), (ReferenceKind::Value, Some((2, 10))))
  );
  assert_eq!(
    usages(Grammar::Starlark, text, "total = 1", 0),
    (
      true,
      vec![(4, Access::Read), (5, Access::Declaration), (7, Access::Write), (8, Access::Read), (11, Access::Read)]
    )
  );
  assert_eq!(
    usages(Grammar::Starlark, text, "deps,", 0),
    (
      true,
      vec![(2, Access::Declaration), (8, Access::Write), (8, Access::Read), (9, Access::Write), (9, Access::Read)]
    )
  );
  // the name of the file is none of them
  let of_file: Vec<(u32, Access)> = usages_of(Grammar::Starlark, text, "total").iter().map(|it| (it.start.line, it.access)).collect();
  assert_eq!(of_file, [(0, Access::Declaration)]);
}

/// The name of a named argument is the parameter of what is called: no name of the scopes of the file
#[test]
fn the_name_of_a_named_argument_is_bound_to_no_local() {
  let text = "def f(name):\n    return rule(name = name)\n";
  let argument = at(Grammar::Starlark, text, "name = ", 0);
  assert_eq!((argument.kind, argument.local), (ReferenceKind::Member, None));
  assert_eq!(kind_and_local(Grammar::Starlark, text, "name)", 1), (ReferenceKind::Value, Some((0, 6))));
  assert_eq!(usages(Grammar::Starlark, text, "name)", 0), (true, vec![(0, Access::Declaration), (1, Access::Read)]));
}

/// A name goes to what the language binds it to: the variable of a comprehension in all of it but its first
/// collection, the parameter of a `lambda` in its body, a default and a first collection outside their scope. The
/// readings of `testdata/navigate/scopes.bzl` are checked by running it (as Python 3, whose scopes these are)
#[test]
fn a_name_is_bound_where_the_language_binds_it() {
  let (wrong, reads) = crate::test_support::readings_unlike_the_compiler(Grammar::Starlark, "scopes.bzl", (2, 0));
  assert!(wrong.is_empty(), "{} of {reads} reads go wrong:\n{}", wrong.len(), wrong.join("\n"));
  assert_eq!(reads, 41);
}
