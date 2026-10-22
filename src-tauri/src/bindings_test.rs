use crate::specta_builder;
use std::path::Path;

/// The command that writes `src/lib/bindings.ts`, headless: the app is never started for it
const UPDATE_COMMAND: &str = "UPDATE_BINDINGS=1 PATH=$HOME/.cargo/bin:$PATH cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'";

/// Write the TypeScript bindings of the app's commands to `path`
fn export_bindings(path: &Path) {
  specta_builder().export(specta_typescript::Typescript::default().header("// @ts-nocheck\n"), path).unwrap();
}

/// The committed `src/lib/bindings.ts` must be what the export writes, and the export must accept every type (a
/// `skip_serializing_if` without a phased export panics). With `UPDATE_BINDINGS` set, the test writes the file instead:
/// the export goes to a temporary file beside it, then a rename, so a failed export leaves the committed file whole
#[test]
fn the_bindings_are_what_the_export_writes() {
  let committed_path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/bindings.ts"));

  if std::env::var_os("UPDATE_BINDINGS").is_some() {
    let exported = tempfile::Builder::new()
      .prefix(".bindings-")
      .suffix(".ts")
      .tempfile_in(committed_path.parent().unwrap())
      .unwrap();
    export_bindings(exported.path());
    // a temporary file is private (0600); the bindings keep the mode they have
    if let Ok(metadata) = std::fs::metadata(committed_path) {
      std::fs::set_permissions(exported.path(), metadata.permissions()).unwrap();
    }
    exported.persist(committed_path).unwrap();
    return;
  }

  // outside the source tree, where a dev server or a linter could see a half-written file
  let dir = tempfile::tempdir().unwrap();
  let exported_path = dir.path().join("bindings.ts");
  export_bindings(&exported_path);
  let committed = std::fs::read_to_string(committed_path).unwrap();
  assert!(
    std::fs::read_to_string(&exported_path).unwrap() == committed,
    "src/lib/bindings.ts is stale: run `{UPDATE_COMMAND}` to regenerate it"
  );
}
