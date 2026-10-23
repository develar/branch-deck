use crate::check_summary::{DanglingReferenceSummary, DeclarationPlace, dangling_reference_message};
use pretty_assertions::assert_eq;

/// The file that uses the declaration
const USE_FILE: &str = "plugins/air/tests/contract/AcpJourneyLifecycleTest.kt";
/// The file of the declaration
const DECLARATION_FILE: &str = "plugins/air/tests/bridge/AcpOwnershipLedger.kt";

/// The refusal of `bd safe-push` describes every dangling reference up to one more than it describes, so it never says
/// "1 more": two of two, else the first and the count of the rest, which `bd check` lists (`list_cut::kept`)
#[test]
fn a_dangling_reference_message_describes_two_whole_and_counts_from_three_on() {
  let reference = |identifier: &str, fixed_by: &str| DanglingReferenceSummary {
    kind: "dangling_reference",
    identifier: identifier.to_string(),
    file: USE_FILE.to_string(),
    line: 8,
    removed_by: "c372cb86".to_string(),
    fixed_by: fixed_by.to_string(),
    mutual: false,
    via: "syntax_tree",
    declared_in: DeclarationPlace {
      file: DECLARATION_FILE.to_string(),
      line: Some(4),
    },
    fix: None,
  };
  let of = |references: &[(&str, &str)]| -> Vec<(String, DanglingReferenceSummary)> {
    references
      .iter()
      .map(|(identifier, fixed_by)| ("suspend".to_string(), reference(identifier, fixed_by)))
      .collect()
  };
  let cleanup = format!(
    "suspend removes cleanup in c372cb86, and {USE_FILE}:8 still uses it: aa0fb473 changes that use in another branch, so the build fails. To fix it, move aa0fb473 into suspend, or stack suspend on its branch"
  );
  let flush = format!(
    "suspend removes flush in c372cb86, and {USE_FILE}:8 still uses it: bb1fc584 changes that use in another branch, so the build fails. To fix it, move bb1fc584 into suspend, or stack suspend on its branch"
  );
  assert_eq!(
    dangling_reference_message("suspend", &of(&[("cleanup", "aa0fb473"), ("flush", "bb1fc584")])),
    format!("{cleanup}. {flush}; --allow-dangling starts anyway")
  );
  assert_eq!(
    dangling_reference_message("suspend", &of(&[("cleanup", "aa0fb473"), ("flush", "bb1fc584"), ("close", "cc2fd695")])),
    format!("{cleanup} (2 more: bd check); --allow-dangling starts anyway")
  );
}

/// The refusal of `bd safe-push` says what the branch does to the declaration: a change of its form, a second
/// declaration
#[test]
fn a_dangling_reference_message_names_the_kind() {
  let reference = |kind: &'static str| DanglingReferenceSummary {
    kind,
    identifier: "cleanup".to_string(),
    file: USE_FILE.to_string(),
    line: 8,
    removed_by: "c372cb86".to_string(),
    fixed_by: "aa0fb473".to_string(),
    mutual: false,
    via: "syntax_tree",
    declared_in: DeclarationPlace {
      file: DECLARATION_FILE.to_string(),
      line: Some(4),
    },
    fix: None,
  };
  let message = |kind| dangling_reference_message("suspend", &[("suspend".to_string(), reference(kind))]);
  assert_eq!(
    message("changed_declaration"),
    format!(
      "suspend changes the form of cleanup in c372cb86, and {USE_FILE}:8 still uses it: aa0fb473 changes that use in another branch, so the build fails. To fix it, move aa0fb473 into suspend, or stack suspend on its branch; --allow-dangling starts anyway"
    )
  );
  assert_eq!(
    message("duplicate_declaration"),
    format!(
      "suspend declares again cleanup in c372cb86, and {USE_FILE}:8 still declares it too: aa0fb473 removes that declaration in another branch, so the build fails. To fix it, move aa0fb473 into suspend, or stack suspend on its branch; --allow-dangling starts anyway"
    )
  );
}
