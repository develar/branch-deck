//! The rules of a Tauri command's span and operation (docs/design/async-patterns.md#operation-spans), checked on the
//! sources of `src-tauri`: every `#[tauri::command]` has an `#[instrument]` span; a command that can fail
//! (`Result<_, ApiError>`) runs its body as an operation (`operation`, `operation_now`, or the one blocking step
//! `run_blocking`); and the span records no argument that holds a collection, whose Debug text a line of the log would
//! carry whole (`skip_all` and a field or two instead, as `add_issue_reference` does).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// A `#[tauri::command]` of the sources
struct Command {
  /// `src-tauri/src/commands/patronus.rs:122`
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
  Path::new(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize().expect("workspace directory")
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

/// The commands of one source file
fn commands_of(path: &Path, text: &str) -> Vec<Command> {
  let relative = path.strip_prefix(workspace()).unwrap_or(path).to_string_lossy().replace('\\', "/");
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
    while !lines[at].trim_start().starts_with("pub fn ") && !lines[at].trim_start().starts_with("pub async fn ") {
      let attribute = lines[at].trim();
      if attribute.starts_with("#[instrument") {
        instrument = Some(attribute.to_owned());
      }
      at += 1;
    }
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
    commands.push(Command {
      place: format!("{relative}:{}", index + 1),
      name,
      instrument,
      arguments,
      fallible: returns.starts_with("Result<") && returns.contains("ApiError>"),
      asynchronous: signature.contains("async fn "),
      body: lines[body_start..end].join("\n"),
    });
    index = end;
  }
  commands
}

fn commands() -> Vec<Command> {
  let mut files = Vec::new();
  rust_files(&workspace().join("src-tauri/src"), &mut files);
  let mut commands: Vec<Command> = files
    .iter()
    .flat_map(|path| commands_of(path, &fs::read_to_string(path).expect("readable source")))
    .collect();
  commands.sort_by(|a, b| a.name.cmp(&b.name));
  // the commands of the app go down as the actions move onto `execute`: the bound only proves the parse found them
  assert!(commands.len() > 30, "the scan found {} commands, is src-tauri laid out differently?", commands.len());
  commands
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

/// Whether `body` is one `run_blocking(…).await` (or `run_waiting(…).await`, the blocking step of a rewrite that waits
/// for the repository lock, `lock_wait.rs`, or `run_waiting_with_prefix(…).await`, that step with the branch prefix
/// resolved in it, `commands/branch_prefix.rs`), with only infallible preparation (clones of the state) before it: nothing
/// that can fail or wait runs outside the step, so the step alone is the operation. As rustfmt lays it out, the call
/// ends on its own line or at the `.await` line at the body's indentation after it
fn is_one_blocking_step(body: &str) -> bool {
  let lines: Vec<&str> = body.lines().collect();
  let Some(step) = lines
    .iter()
    .position(|line| line.contains("run_blocking(") || line.contains("run_waiting(") || line.contains("run_waiting_with_prefix("))
  else {
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
  assert!(is_one_blocking_step(
    "  let git = (*git_executor).clone();\n  run_waiting(&lock_waits, wait, move |lock_wait| core(&git, lock_wait)).await"
  ));
  assert!(is_one_blocking_step(
    "  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);\n  run_waiting_with_prefix(inputs, &lock_waits, wait, &path, move |git, prefix, lock_wait| {\n    core(git, prefix, lock_wait)\n  })\n  .await"
  ));
}

#[test]
fn every_command_has_a_span_and_a_fallible_one_runs_as_an_operation() {
  let mut report = Vec::new();
  for command in commands() {
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

/// The byte ranges of the arguments of each `run_blocking(…)` call of `body`, and of each `run_waiting(…)` and
/// `run_waiting_with_prefix(…)`, which run their step in one (`lock_wait::run_waiting`)
fn blocking_spans(body: &str) -> Vec<std::ops::Range<usize>> {
  ["run_blocking(", "run_waiting(", "run_waiting_with_prefix("]
    .iter()
    .flat_map(|call| body.match_indices(call))
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

/// The lines of an async `body` that open the object cache on the worker: neither inside a `run_blocking(…)` (or a
/// `run_waiting…(…)` of a rewrite) nor in a `move ||` closure a core runs off the workers
fn object_cache_on_the_worker(body: &str) -> Vec<String> {
  let spans = blocking_spans(body);
  let mut offset = 0;
  let mut lines = Vec::new();
  for line in body.split_inclusive('\n') {
    if let Some(at) = line.find("object_cache(")
      && !line.contains("move ||")
      && !spans.iter().any(|span| span.contains(&(offset + at)))
    {
      lines.push(line.trim().to_owned());
    }
    offset += line.len();
  }
  lines
}

#[test]
fn an_async_command_opens_the_object_cache_off_the_async_workers() {
  let mut report = Vec::new();
  for command in commands().iter().filter(|command| command.asynchronous) {
    for line in object_cache_on_the_worker(&command.body) {
      report.push(format!("  {} ({}): {line}", command.name, command.place));
    }
  }
  assert!(
    report.is_empty(),
    "the first open of the object cache runs `rev-parse` and opens LMDB, which blocks a worker of the async runtime: open it inside `run_blocking(…)` (docs/design/async-patterns.md):\n{}",
    report.join("\n")
  );
  assert_eq!(
    object_cache_on_the_worker("  let cache = repository_state.object_cache(&git, &path);\n  run_blocking(move || {\n    let cache = state.object_cache(&git, &path);\n  })"),
    ["let cache = repository_state.object_cache(&git, &path);"]
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
  let commands = commands_of(Path::new("x.rs"), text);
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
