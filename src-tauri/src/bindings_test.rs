use crate::specta_builder;
use std::path::Path;

/// The command that writes `src/lib/bindings.ts`, headless: the app is never started for it
const UPDATE_COMMAND: &str = "UPDATE_BINDINGS=1 PATH=$HOME/.cargo/bin:$PATH cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'";

/// `typedError`, the runtime tauri-specta writes into the bindings: its catch rethrows an `Error` and answers any other
/// rejection as the error of the command, so a failure of the IPC (an `Error` of the page: a fetch of the E2E bridge,
/// the Air shim) rejects, and a refusal of Tauri itself ("Command not found", an ACL refusal, invalid arguments: a
/// string) is an error without a code
const TYPED_ERROR: &str = "async function typedError<T, E>(result: Promise<T>): Promise<{ status: \"ok\"; data: T } | { status: \"error\"; error: E }> {
    try {
        return { status: \"ok\", data: await result };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { status: \"error\", error: e as any };
    }
}";

/// Its replacement: it takes the name of the command, and every failure is the `ApiError` of an error result
/// (`commandError` of `src/lib/api-error.ts`, which warns of a failure that is no answer of the command by that name), so
/// no command rejects and the page reads one shape of a failure
const TYPED_ERROR_NORMALIZED: &str =
  "async function typedError<T, E>(command: string, result: Promise<T>): Promise<{ status: \"ok\"; data: T } | { status: \"error\"; error: E }> {
    try {
        return { status: \"ok\", data: await result };
    } catch (e) {
        return { status: \"error\", error: commandError(e, command) as E };
    }
}";

/// A call of `typedError` in an entry of the commands, up to the name of the command it invokes
const TYPED_ERROR_CALL: &str = ">(__TAURI_INVOKE(\"";

const HEADER: &str = "// @ts-nocheck\nimport { commandError } from \"@/lib/api-error\";\n";

/// Write the TypeScript bindings of the app's commands to `path`, with `typedError` normalized: a new runtime of
/// tauri-specta that writes another one fails the export, so the bindings never rethrow unnoticed
fn export_bindings(path: &Path) {
  specta_builder().export(specta_typescript::Typescript::default().header(HEADER), path).unwrap();
  let exported = std::fs::read_to_string(path).unwrap();
  assert_eq!(
    exported.matches(TYPED_ERROR).count(),
    1,
    "the runtime of tauri-specta no longer writes the `typedError` this export normalizes: update `TYPED_ERROR`"
  );
  std::fs::write(path, with_command_names(&exported.replace(TYPED_ERROR, TYPED_ERROR_NORMALIZED))).unwrap();
}

/// `bindings` with the name of its command as the first argument of each call of `typedError`:
/// `typedError<T, E>(__TAURI_INVOKE("name", …))` → `typedError<T, E>("name", __TAURI_INVOKE("name", …))`
fn with_command_names(bindings: &str) -> String {
  let mut named = String::with_capacity(bindings.len() + bindings.len() / 16);
  for line in bindings.split_inclusive('\n') {
    let call = line.find("=> typedError<").and_then(|start| line[start..].find(TYPED_ERROR_CALL).map(|at| start + at));
    match call {
      Some(at) => {
        let name_start = at + TYPED_ERROR_CALL.len();
        let name = &line[name_start..name_start + line[name_start..].find('"').expect("the name of the command ends")];
        // after `>(`, before `__TAURI_INVOKE`
        named.push_str(&line[..at + 2]);
        named.push('"');
        named.push_str(name);
        named.push_str("\", ");
        named.push_str(&line[at + 2..]);
      }
      None => named.push_str(line),
    }
  }
  named
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

/// Every command of the bindings answers a `Result` (`typedError`): its failed call is its error answer, so no call of
/// the page rejects and none needs a catch; a command that answered its value as it is rejected as its call failed (the
/// IPC of the E2E bridge or the Air shim, a refusal of Tauri), an unhandled rejection wherever the page voided it
#[test]
fn every_command_of_the_bindings_answers_a_result() {
  let dir = tempfile::tempdir().unwrap();
  let exported_path = dir.path().join("bindings.ts");
  export_bindings(&exported_path);
  let exported = std::fs::read_to_string(&exported_path).unwrap();
  let start = exported.find("export const commands = {").expect("the bindings export their commands");
  let end = start + exported[start..].find("\n};").expect("the commands end");
  // an entry is one line, `\t<name>: (<arguments>) => <call>`, after its doc comment
  let entries: Vec<&str> = exported[start..end].lines().filter(|line| line.starts_with('\t') && line.contains(") => ")).collect();
  assert!(entries.len() > 50, "the commands of the bindings: {}", entries.len());
  let without_result: Vec<&str> = entries
    .iter()
    .filter(|line| !line.contains("=> typedError<") || !line.contains(">(\""))
    .map(|line| line.trim_start().split(':').next().unwrap())
    .collect();
  assert!(
    without_result.is_empty(),
    "these commands answer no Result, so their failed call rejects: give them a kind that answers one ({without_result:?})"
  );
}
