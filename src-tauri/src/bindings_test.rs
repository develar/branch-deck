use crate::{export_bindings, specta_builder};

/// A debug run of the app writes `src/lib/bindings.ts`; the committed file must be what it writes, and the export must
/// accept every type (a `skip_serializing_if` without a phased export panics at start-up)
#[test]
fn the_bindings_are_what_the_export_writes() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("bindings.ts");
  export_bindings(&specta_builder(), &path).unwrap();
  let committed = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/bindings.ts")).unwrap();
  assert!(
    std::fs::read_to_string(&path).unwrap() == committed,
    "src/lib/bindings.ts is stale: run the app in debug (`pnpm tauri dev`) to regenerate it"
  );
}
