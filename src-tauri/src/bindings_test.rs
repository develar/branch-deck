use crate::specta_builder;
use std::path::Path;

/// The command that writes `src/lib/bindings.ts`, headless: the app is never started for it
const UPDATE_COMMAND: &str = "UPDATE_BINDINGS=1 PATH=$HOME/.cargo/bin:$PATH cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'";

/// The catch of `typedError`, the runtime tauri-specta writes into the bindings: it rethrows an `Error` and answers any
/// other rejection as the error of the command, so a failure of the IPC (an `Error` of the page: a fetch of the E2E
/// bridge, the Air shim) rejects, and a refusal of Tauri itself ("Command not found", an ACL refusal, invalid arguments:
/// a string) is an error without a code
const TYPED_ERROR_CATCH: &str = "    } catch (e) {
        if (e instanceof Error) throw e;
        return { status: \"error\", error: e as any };
    }";

/// Its replacement: every failure is the `ApiError` of an error result (`commandError` of `src/lib/api-error.ts`), so a
/// command that answers a `Result` never rejects and the page reads one shape of a failure
const TYPED_ERROR_CATCH_NORMALIZED: &str = "    } catch (e) {
        return { status: \"error\", error: commandError(e) as E };
    }";

const HEADER: &str = "// @ts-nocheck\nimport { commandError } from \"@/lib/api-error\";\n";

/// Write the TypeScript bindings of the app's commands to `path`, with the catch of `typedError` normalized: a new
/// runtime of tauri-specta that writes another catch fails the export, so the bindings never rethrow unnoticed
fn export_bindings(path: &Path) {
  specta_builder().export(specta_typescript::Typescript::default().header(HEADER), path).unwrap();
  let exported = std::fs::read_to_string(path).unwrap();
  assert_eq!(
    exported.matches(TYPED_ERROR_CATCH).count(),
    1,
    "the runtime of tauri-specta no longer writes the catch of `typedError` this export normalizes: update `TYPED_ERROR_CATCH`"
  );
  std::fs::write(path, exported.replace(TYPED_ERROR_CATCH, TYPED_ERROR_CATCH_NORMALIZED)).unwrap();
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
