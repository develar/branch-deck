//! Go to Declaration and Find Usages in Starlark, beside `navigate_test.rs`: the two scopes of the language and the
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
