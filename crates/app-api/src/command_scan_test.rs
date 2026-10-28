//! The rules of a command's span and operation (docs/design/async-patterns.md#operation-spans), checked on the sources:
//! the bodies of the commands of the registry (`commands.rs` names the body of each entry), which are every command of
//! the app: `src-tauri` has no `#[tauri::command]` beside the wrappers of the entries (its `api.rs`). Every command has
//! an `#[instrument]` span; a command that can fail (`Result<_, ApiError>`) runs its body as an operation
//! (`operation`, `operation_now`, or the one blocking step `run_blocking`); a command of a session has both from the
//! frame of its kind (`registry.rs`), so its body opens neither; the span records no argument that holds a
//! collection, whose Debug text a line of the log would carry whole (`skip_all` and a field or two instead); and an
//! async body opens the object cache, and a body of the registry reads the git directories and the GitHub repository
//! (`RepositoryStateCache`), only in a blocking step, or through the services of its session, whose host reads them
//! off the workers. The key of a session is built in one place, `context.rs`.

use crate::commands::COMMANDS;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// A command of the sources: a handwritten `#[tauri::command]`, or the body of an entry of the registry
struct Command {
  /// `crates/app-api/src/app/sign_in.rs:17`
  place: String,
  name: String,
  /// The text of its `#[instrument(…)]` attribute, None without one
  instrument: Option<String>,
  /// `name: Type` of each argument
  arguments: Vec<(String, String)>,
  fallible: bool,
  /// `pub async fn`: its body runs on a worker of the async runtime
  asynchronous: bool,
  body: String,
  /// The kind of an entry of the registry (`json`, `session`, …); None for a handwritten command
  kind: Option<String>,
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
  for entry in fs::read_dir(directory).unwrap_or_else(|e| panic!("{}: {e}", directory.display())).flatten() {
    let path = entry.path();
    if path.is_dir() {
      rust_files(&path, files);
    } else if path.extension().is_some_and(|extension| extension == "rs") && !path.to_string_lossy().ends_with("_test.rs") {
      files.push(path);
    }
  }
}

fn workspace() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace directory")
}

/// Splits `text` at the commas outside brackets
fn split_top_level(text: &str) -> Vec<String> {
  let mut parts = Vec::new();
  let mut depth = 0i32;
  let mut current = String::new();
  for c in text.chars() {
    match c {
      '<' | '(' | '[' => depth += 1,
      '>' | ')' | ']' => depth -= 1,
      ',' if depth == 0 => {
        parts.push(std::mem::take(&mut current));
        continue;
      }
      _ => {}
    }
    current.push(c);
  }
  if !current.trim().is_empty() {
    parts.push(current);
  }
  parts.into_iter().map(|part| part.trim().to_owned()).filter(|part| !part.is_empty()).collect()
}

fn is_function(line: &str) -> bool {
  let line = line.trim_start();
  line.starts_with("pub fn ") || line.starts_with("pub async fn ")
}

/// The function whose signature starts at `lines[at]`, with `instrument` its attribute; it ends at the first `}` of
/// column 0
fn function_at(place: String, lines: &[&str], mut at: usize, instrument: Option<String>) -> Command {
  let mut signature = String::new();
  while !lines[at].trim_end().ends_with('{') {
    signature.push_str(lines[at].trim());
    signature.push(' ');
    at += 1;
  }
  signature.push_str(lines[at].trim());
  let body_start = at + 1;
  let mut end = body_start;
  while lines[end] != "}" {
    end += 1;
  }
  let name = signature
    .split("fn ")
    .nth(1)
    .and_then(|rest| rest.split(['(', '<']).next())
    .expect("the name of the command")
    .to_owned();
  let open = signature.find('(').expect("the arguments of the command");
  let mut depth = 0;
  let close = signature[open..]
    .char_indices()
    .find(|(_, c)| {
      match c {
        '(' => depth += 1,
        ')' => depth -= 1,
        _ => {}
      }
      depth == 0
    })
    .map(|(offset, _)| open + offset)
    .expect("the end of the arguments");
  let arguments = split_top_level(&signature[open + 1..close])
    .into_iter()
    .map(|argument| {
      let (name, type_name) = argument.split_once(':').expect("name: Type");
      (name.trim().trim_start_matches("mut ").to_owned(), type_name.trim().to_owned())
    })
    .collect();
  let returns = signature[close..].split("->").nth(1).map_or_default(str::trim);
  Command {
    place,
    name,
    instrument,
    arguments,
    fallible: returns.starts_with("Result<") && returns.contains("ApiError>"),
    asynchronous: signature.contains("async fn "),
    body: lines[body_start..end].join("\n"),
    kind: None,
  }
}

fn relative(path: &Path) -> String {
  path.strip_prefix(workspace()).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// The handwritten `#[tauri::command]`s of one source file
fn tauri_commands_of(path: &Path, text: &str) -> Vec<Command> {
  let lines: Vec<&str> = text.lines().collect();
  let mut commands = Vec::new();
  let mut index = 0;
  while index < lines.len() {
    if lines[index].trim() != "#[tauri::command]" {
      index += 1;
      continue;
    }
    let mut instrument = None;
    let mut at = index + 1;
    while !is_function(lines[at]) {
      let attribute = lines[at].trim();
      if attribute.starts_with("#[instrument") {
        instrument = Some(attribute.to_owned());
      }
      at += 1;
    }
    commands.push(function_at(format!("{}:{}", relative(path), index + 1), &lines, at, instrument));
    index = at + 1;
  }
  commands
}

/// An entry of `for_each_command!` in `commands.rs`: its name, its kind and the path of its body under `$crate::`
fn registry_entries(text: &str) -> Vec<(String, String, String)> {
  let mut entries = Vec::new();
  for line in text.lines().map(str::trim) {
    let mut words = line.splitn(3, ' ');
    let (Some(class), Some(kind), Some(rest)) = (words.next(), words.next(), words.next()) else {
      continue;
    };
    // `app` with its reason: `app(SettingsWriter)`
    let class = class.split_once('(').map_or(class, |(class, _)| class);
    if !matches!(class, "read" | "engine" | "app") {
      continue;
    }
    let name = rest.split('(').next().expect("the name of the entry");
    let body = rest
      .rsplit_once("= $crate::")
      .and_then(|(_, body)| body.strip_suffix(';'))
      .unwrap_or_else(|| panic!("the entry `{name}` names no body as `= $crate::<module>::<function>;` on its line"));
    entries.push((name.to_owned(), kind.to_owned(), body.to_owned()));
  }
  entries
}

/// The body of the entry `name`: the function at `body` (`reads::commits::get_commit_files`) of this crate
fn registry_body(name: &str, body: &str) -> Command {
  let (module, function) = body.rsplit_once("::").expect("a body in a module");
  let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(module.replace("::", "/"));
  let file = [source.with_extension("rs"), source.join("mod.rs")]
    .into_iter()
    .find(|file| file.is_file())
    .unwrap_or_else(|| panic!("{name}: no module file for {module}"));
  let text = fs::read_to_string(&file).expect("readable source");
  let lines: Vec<&str> = text.lines().collect();
  let at = lines
    .iter()
    .position(|line| is_function(line) && line.contains(&format!("fn {function}(")))
    .unwrap_or_else(|| panic!("{name}: no `pub fn {function}(` in {}", relative(&file)));
  let attributes = lines[..at].iter().rev().take_while(|line| line.starts_with("#[") || line.starts_with("///"));
  let instrument = attributes.map(|line| line.trim()).find(|line| line.starts_with("#[instrument")).map(str::to_owned);
  function_at(format!("{}:{}", relative(&file), at + 1), &lines, at, instrument)
}

/// The bodies of the registry, checked against the compiled list: no entry escapes the scan
fn registry_bodies() -> Vec<Command> {
  let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.rs")).expect("commands.rs");
  let entries = registry_entries(&text);
  let names: Vec<&str> = entries.iter().map(|(name, _, _)| name.as_str()).collect();
  let compiled: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
  assert_eq!(names, compiled, "the scan reads the entries of commands.rs one per line: the parse needs an update");
  entries
    .iter()
    .map(|(name, kind, body)| {
      let mut command = registry_body(name, body);
      command.kind = Some(kind.clone());
      assert_eq!(
        command.fallible,
        !matches!(kind.as_str(), "plain" | "now"),
        "{name}: a `{kind}` body answers {}a `Result<_, ApiError>`",
        if command.fallible { "" } else { "no " }
      );
      command
    })
    .collect()
}

/// The `#[tauri::command]`s of the app beside the wrappers of the entries (`api.rs`)
fn tauri_commands() -> Vec<Command> {
  let mut files = Vec::new();
  rust_files(&workspace().join("src-tauri/src"), &mut files);
  let generated = workspace().join("src-tauri/src/api.rs");
  files
    .iter()
    .filter(|path| **path != generated)
    .flat_map(|path| tauri_commands_of(path, &fs::read_to_string(path).expect("readable source")))
    .collect()
}

fn commands() -> Vec<Command> {
  registry_bodies()
}

/// A command is declared once, in the registry: the app wraps the entries (`api.rs`) and has no command of its own
#[test]
fn the_app_has_no_command_beside_the_registry() {
  let handwritten: Vec<String> = tauri_commands().into_iter().map(|command| format!("  {} ({})", command.name, command.place)).collect();
  assert!(
    handwritten.is_empty(),
    "these commands of src-tauri are no entry of app_api::for_each_command!: declare each in crates/app-api/src/commands.rs, its body in a family of app-api\n{}",
    handwritten.join("\n")
  );
  // the scan finds a command where there is one: the wrappers of the entries have the attribute it looks for
  let generated = workspace().join("src-tauri/src/api.rs");
  assert!(
    fs::read_to_string(&generated).expect("api.rs").lines().any(|line| line.trim() == "#[tauri::command]"),
    "the wrappers of api.rs have no #[tauri::command] line: the scan needs an update"
  );
}

/// The fields of every struct the workspace declares, by the struct's name: `(field, type)`
fn struct_fields() -> BTreeMap<String, Vec<(String, String)>> {
  let mut files = Vec::new();
  rust_files(&workspace().join("src-tauri/src"), &mut files);
  for entry in fs::read_dir(workspace().join("crates")).expect("crates").flatten() {
    let source = entry.path().join("src");
    if source.is_dir() {
      rust_files(&source, &mut files);
    }
  }
  let mut structs = BTreeMap::new();
  for path in files {
    let text = fs::read_to_string(&path).expect("readable source");
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
      let Some(rest) = line.trim_start().strip_prefix("pub struct ") else { continue };
      let Some((name, _)) = rest.split_once([' ', '<', '(', ';']) else { continue };
      if !rest.contains('{') {
        continue;
      }
      let fields = lines[index + 1..]
        .iter()
        .take_while(|line| line.trim() != "}")
        .filter_map(|line| {
          let line = line.trim();
          let line = line.strip_suffix(',').unwrap_or(line);
          let line = line.trim_start_matches("pub(crate) ").trim_start_matches("pub ");
          let (field, type_name) = line.split_once(": ")?;
          (!field.starts_with('#') && !field.starts_with("//")).then(|| (field.to_owned(), type_name.to_owned()))
        })
        .collect();
      structs.insert(name.to_owned(), fields);
    }
  }
  structs
}

/// The first field (`params.robots: Vec<PatronusRobot>`, through nested structs) of `type_name` that is a collection
fn collection_in(type_name: &str, structs: &BTreeMap<String, Vec<(String, String)>>, visited: &mut HashSet<String>) -> Option<String> {
  let inner = type_name.trim_start_matches('&').trim_start_matches("Option<").trim_end_matches('>');
  if ["Vec<", "HashMap<", "BTreeMap<", "HashSet<", "BTreeSet<", "VecDeque<"]
    .iter()
    .any(|collection| inner.starts_with(collection))
    || inner.starts_with('[')
  {
    return Some(String::new());
  }
  let name = inner.split("::").last().unwrap_or(inner);
  if !visited.insert(name.to_owned()) {
    return None;
  }
  structs.get(name).and_then(|fields| {
    fields
      .iter()
      .find_map(|(field, field_type)| collection_in(field_type, structs, visited).map(|path| format!(".{field}{path}: {field_type}")))
  })
}

/// Whether `body` is one `run_blocking(…).await`, with only infallible preparation (clones of the state) before it:
/// nothing that can fail or wait runs outside the step, so the step alone is the operation. As rustfmt lays it out, the
/// call ends on its own line or at the `.await` line at the body's indentation after it
fn is_one_blocking_step(body: &str) -> bool {
  let lines: Vec<&str> = body.lines().collect();
  let Some(step) = lines.iter().position(|line| line.contains("run_blocking(")) else {
    return false;
  };
  let outside = |line: &&str| line.contains(".await") || line.contains('?') || line.contains("return");
  let end = if lines[step].contains(".await") {
    step
  } else {
    match lines[step + 1..].iter().position(|line| line.starts_with("  .await")) {
      Some(offset) => step + 1 + offset,
      None => return false,
    }
  };
  let after_await = lines[end].split_once(".await").map_or("", |(_, rest)| rest);
  !lines[..step].iter().any(outside) && lines[end].matches(".await").count() == 1 && !after_await.contains('?') && !lines[end + 1..].iter().any(outside)
}

/// Whether `body` is one `operation(…).await` or `operation_now(…)` call and nothing else: no statement before it, whose
/// error would leave without an operation id, and none after it. As rustfmt lays it out, the call opens on the first line
/// at the body's indentation, its argument lies deeper, and it closes with `  })` and, for a future, `  .await` (or the
/// whole call on one line)
fn is_one_operation(body: &str) -> bool {
  let lines: Vec<&str> = body.lines().collect();
  let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
    return false;
  };
  let synchronous = first.starts_with("  operation_now(");
  if !synchronous && !first.starts_with("  operation(") {
    return false;
  }
  let closes = if lines.len() == 1 {
    last.ends_with(if synchronous { ")" } else { ").await" })
  } else if synchronous {
    *last == "  })"
  } else {
    *last == "  .await"
  };
  let inside = |line: &&str| line.is_empty() || line.starts_with("   ") || *line == "  })";
  closes && lines.get(1..lines.len() - 1).unwrap_or_default().iter().all(inside)
}

#[test]
fn a_body_is_one_operation_only_without_work_outside_it() {
  assert!(is_one_operation("  operation(async move {\n    let x = read().await?;\n    Ok(x)\n  })\n  .await"));
  assert!(is_one_operation("  operation(async move { read(&git, params).await }).await"));
  assert!(is_one_operation("  operation_now(|| {\n    let x = read()?;\n    Ok(x)\n  })"));
  assert!(is_one_operation("  operation_now(|| Ok(7))"));
  // fallible work before the operation leaves without its id, work after it runs outside the span
  assert!(!is_one_operation(
    "  let space = patronus.space().await?;\n  operation(async move { core(&space).await }).await"
  ));
  assert!(!is_one_operation("  operation(async move {\n    prepare().await\n  })\n  .await?;\n  finish().await"));
  assert!(!is_one_operation("  operation(async move {\n    prepare().await\n  })\n  .await?;\n  Ok(())"));
  // another name that only contains the word
  assert!(!is_one_operation("  undo_operation(async move { core().await }).await"));
  assert!(!is_one_operation("  run_blocking(move || core(&git)).await"));
}

#[test]
fn a_body_is_one_blocking_step_only_without_work_outside_it() {
  assert!(is_one_blocking_step(
    "  let git = (*git_executor).clone();\n  run_blocking(move || core(&git, params)).await"
  ));
  assert!(is_one_blocking_step(
    "  run_blocking(move || {\n    let x = read()?;\n    Ok(x)\n  })\n  .await\n  .map_err(api_error)"
  ));
  // the preparation can fail, the sync waits after the step
  assert!(!is_one_blocking_step(
    "  let (a, b) = run_blocking(move || prepare()).await?;\n  sync(a, b).await.map_err(api_error)"
  ));
  assert!(!is_one_blocking_step("  let space = patronus.space().await?;\n  run_blocking(move || core(&space)).await"));
  assert!(!is_one_blocking_step("  run_blocking(move || {\n    prepare()\n  })\n  .await?;\n  finish().await"));
  assert!(!is_one_blocking_step("  service.restart().await.map_err(ApiError::internal)"));
}

/// Whether `command` is the body of a command of a session: the frame of its kind gives it the span and the operation
fn framed(command: &Command) -> bool {
  command.kind.as_deref() == Some("session")
}

#[test]
fn every_command_has_a_span_and_a_fallible_one_runs_as_an_operation() {
  let mut report = Vec::new();
  for command in commands().iter().filter(|command| !framed(command)) {
    if command.instrument.is_none() {
      report.push(format!("  {} ({}): no #[instrument] span", command.name, command.place));
    }
    if command.fallible && !is_one_operation(&command.body) && !is_one_blocking_step(&command.body) {
      report.push(format!(
        "  {} ({}): can fail, but its body is no operation: the whole body is `operation(async move {{ … }}).await` (a synchronous command: `operation_now(|| {{ … }})`), nothing before or after it, unless it is one `run_blocking(…).await` step",
        command.name, command.place
      ));
    }
  }
  assert!(report.is_empty(), "docs/design/async-patterns.md#operation-spans:\n{}", report.join("\n"));
}

/// The frame of a session command (`registry.rs`) opens the span of the command and its operation: a body of its own
/// would open a second span of the same name, or a second operation whose id its error would leave with
#[test]
fn the_body_of_a_session_command_opens_no_span_and_no_operation_of_its_own() {
  let mut report = Vec::new();
  let bodies = registry_bodies();
  for command in bodies.iter().filter(|command| framed(command)) {
    if command.instrument.is_some() {
      report.push(format!("  {} ({}): an #[instrument] span beside the one of its frame", command.name, command.place));
    }
    if command.body.contains("operation(") || command.body.contains("operation_now(") {
      report.push(format!("  {} ({}): an operation inside the one of its frame", command.name, command.place));
    }
  }
  assert!(bodies.iter().any(framed), "the scan found no command of a session");
  assert!(report.is_empty(), "docs/design/async-patterns.md#operation-spans:\n{}", report.join("\n"));
}

/// The key of a session is built in one place (`AppContext::session_key`): the frame of a session command resolves it
/// from the path of its `params`, and no body builds one of a path it read again
#[test]
fn the_key_of_a_session_is_built_only_by_the_context() {
  let mut files = Vec::new();
  rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
  let mut builders = Vec::new();
  for path in &files {
    let text = fs::read_to_string(path).expect("readable source");
    for (index, line) in text.lines().enumerate() {
      if line.contains(concat!("SessionKey", " {")) {
        builders.push(format!("{}:{}", relative(path), index + 1));
      }
    }
  }
  assert_eq!(builders.len(), 1, "a `SessionKey` is built outside `AppContext::session_key`: {builders:?}");
  assert!(builders[0].starts_with("crates/app-api/src/context.rs:"), "{builders:?}");
}

#[test]
fn the_span_of_a_command_records_no_argument_that_holds_a_collection() {
  let structs = struct_fields();
  let mut report = Vec::new();
  for command in commands() {
    let Some(instrument) = &command.instrument else { continue };
    if instrument.contains("skip_all") {
      continue;
    }
    let skipped: Vec<String> = instrument
      .split_once("skip(")
      .map_or_default(|(_, rest)| split_top_level(&rest[..rest.find(')').expect("the end of skip(…)")]));
    for (argument, type_name) in &command.arguments {
      if skipped.contains(argument) {
        continue;
      }
      if let Some(path) = collection_in(type_name, &structs, &mut HashSet::new()) {
        report.push(format!(
          "  {} ({}): the span records `{argument}: {type_name}`, and `{argument}{path}` is a collection",
          command.name, command.place
        ));
      }
    }
  }
  assert!(
    report.is_empty(),
    "a line of the log carries the Debug text of every recorded argument, whole:\n{}\nInstead: `#[instrument(skip_all, fields(repository_path = %params.repository_path, robots = params.robots.len()))]`, before `#[specta::specta]`",
    report.join("\n")
  );
}

/// The byte ranges of the arguments of each `run_blocking(…)` call of `body`
fn blocking_spans(body: &str) -> Vec<std::ops::Range<usize>> {
  body
    .match_indices("run_blocking(")
    .filter_map(|(start, call)| {
      let open = start + call.len() - 1;
      let mut depth = 0;
      body[open..].char_indices().find_map(|(offset, c)| {
        match c {
          '(' => depth += 1,
          ')' => depth -= 1,
          _ => {}
        }
        (depth == 0).then_some(open..open + offset)
      })
    })
    .collect()
}

/// The lines of an async `body` that call one of `calls` on the worker: neither inside a `run_blocking(…)` nor in a
/// `move ||` closure a core runs off the workers. A read of the services of a session (`ctx.services.object_cache(…)`,
/// `ServiceHost`) is a future of its host, which reads off the workers itself
fn blocking_on_the_worker(body: &str, calls: &[&str]) -> Vec<String> {
  let spans = blocking_spans(body);
  let mut offset = 0;
  let mut lines = Vec::new();
  for line in body.split_inclusive('\n') {
    let at = calls
      .iter()
      .flat_map(|call| line.match_indices(call).map(|(at, _)| at))
      .filter(|at| !line[..*at].ends_with("services."))
      .min();
    if let Some(at) = at
      && !line.contains("move ||")
      && !spans.iter().any(|span| span.contains(&(offset + at)))
    {
      lines.push(line.trim().to_owned());
    }
    offset += line.len();
  }
  lines
}

/// The lines of the async `commands` that call one of `calls` on the worker, one report line each
fn blocking_report(commands: &[Command], calls: &[&str]) -> Vec<String> {
  let mut report = Vec::new();
  for command in commands.iter().filter(|command| command.asynchronous) {
    for line in blocking_on_the_worker(&command.body, calls) {
      report.push(format!("  {} ({}): {line}", command.name, command.place));
    }
  }
  report
}

#[test]
fn an_async_command_opens_the_object_cache_off_the_async_workers() {
  let report = blocking_report(&commands(), &["object_cache("]);
  assert!(
    report.is_empty(),
    "the first open of the object cache runs `rev-parse` and opens LMDB, which blocks a worker of the async runtime: open it inside `run_blocking(…)` (docs/design/async-patterns.md):\n{}",
    report.join("\n")
  );
  assert_eq!(
    blocking_on_the_worker(
      "  let cache = repository_state.object_cache(&git, &path);\n  run_blocking(move || {\n    let cache = state.object_cache(&git, &path);\n  })\n  let kept = ctx.services.object_cache(&key).await;",
      &["object_cache("]
    ),
    ["let cache = repository_state.object_cache(&git, &path);"]
  );
}

/// The reads of `RepositoryStateCache` besides the object cache: a `rev-parse` on the first call for a repository, the
/// GitHub repository up to 3 git processes
#[test]
fn a_body_of_the_registry_reads_the_git_directories_and_the_github_repository_off_the_async_workers() {
  let report = blocking_report(&registry_bodies(), &["git_common_dir(", "github_repository("]);
  assert!(
    report.is_empty(),
    "the git directories and the GitHub repository of a repository are read by git on their first call, which blocks a worker of the async runtime: read them inside `run_blocking(…)` (docs/design/async-patterns.md):\n{}",
    report.join("\n")
  );
  assert_eq!(
    blocking_on_the_worker(
      "  let dir = state.git_common_dir(&git, &path)?;\n  run_blocking(move || state.github_repository(&git, &path)).await",
      &["git_common_dir(", "github_repository("]
    ),
    ["let dir = state.git_common_dir(&git, &path)?;"]
  );
}

#[test]
fn the_scan_reads_a_command_with_a_multi_line_signature() {
  let text = "
#[tauri::command]
#[instrument(skip(git_executor), fields(x = 1))]
#[specta::specta]
pub async fn get_thing(
  git_executor: State<'_, GitCommandExecutor>,
  params: GetThingParams,
) -> Result<Vec<Thing>, ApiError> {
  run_blocking(move || Ok(vec![])).await
}

#[tauri::command]
#[specta::specta]
pub fn note_thing(window: Window, count: u32) {
  let _ = (window, count);
}
";
  let commands = tauri_commands_of(Path::new("x.rs"), text);
  assert_eq!(commands.len(), 2);
  assert_eq!(commands[0].name, "get_thing");
  assert_eq!(
    commands[0].arguments,
    vec![
      ("git_executor".to_owned(), "State<'_, GitCommandExecutor>".to_owned()),
      ("params".to_owned(), "GetThingParams".to_owned())
    ]
  );
  assert!(commands[0].fallible);
  assert_eq!(commands[0].instrument.as_deref(), Some("#[instrument(skip(git_executor), fields(x = 1))]"));
  assert_eq!(commands[1].name, "note_thing");
  assert!(!commands[1].fallible && commands[1].instrument.is_none());
}

/// Each entry names its body, a function of its own name, and the scan finds it
#[test]
fn the_scan_reads_the_body_of_every_entry_of_the_registry() {
  let names: Vec<String> = registry_bodies().into_iter().map(|body| body.name).collect();
  let compiled: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
  assert_eq!(names, compiled);
}
