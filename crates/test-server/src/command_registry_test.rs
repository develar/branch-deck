//! The three registries of the Tauri commands, held together: the commands of the app (`collect_commands!` in
//! `src-tauri/src/lib.rs`), the routes of this server (`/invoke/<name>` in `lib.rs`) and the handlers of the page mock
//! (`COMMAND_HANDLERS` in `tests/e2e/setup/tauri-mocks.ts`). The mock proxies every command it does not handle to this
//! server, and a command without a route answers 405, which fails the test ("Unimplemented Tauri command"). So every
//! command either has a route or is named in [`PAGE_MOCKED`] with the reason it never reaches Rust in an E2E test.
//! A fourth source, the E2E tests (`tests/e2e/**/*.ts`), is held by a ratchet: a test answers a command in the page
//! only where [`ROUTED_IN_PAGE`] pins it, never a write of [`MUTATING`] (a push included: it goes to the remote of the
//! repository, `remote_git_fixtures`). An answer counts where it is given: a helper that answers
//! (`routeReviewRequirement`, a spec's own function) counts once per call, not once per definition. The sources are read as text, as the git scan tests read theirs
//! (`source_scan_test.rs` of git-executor).

use std::collections::BTreeSet;
use std::path::Path;

/// The number of commands of `collect_commands!`: a new command gets a route (or, with a reason, a [`PAGE_MOCKED`]
/// entry) and raises this number; it also proves the parse found the whole macro
const COMMAND_COUNT: usize = 87;

const SUB_WINDOW: &str = "a sub-window is a page of its own: the page mock records the call and the test opens the page";

/// The commands that have no route of this server, each with its reason. The number only goes down: a command moves
/// out of here when the server answers it from a fake of its service (`s0b-remote-bridge` of rust-domain-authority.md)
const PAGE_MOCKED: &[(&str, &str)] = &[
  // the handlers of `COMMAND_HANDLERS` in tauri-mocks.ts: the windows and the window menu
  ("open_sub_window", SUB_WINDOW),
  ("sub_window_ready", SUB_WINDOW),
  ("show_window_menu", "the window manager's menu: the page mock records it with the window calls"),
];

/// The routes that name no command of `collect_commands!`, each with its reason
const RAW_ROUTES: &[(&str, &str)] = &[(
  "get_blob_syntax",
  "its answer is raw bytes, which the typed bindings cannot say: the app registers it with `generate_handler!` beside the typed commands",
)];

/// The commands that write: to a remote service (Patronus, GitHub, YouTrack), to the repository or to the app. No E2E
/// test answers one in the page (`routeCommand`, or a `page.route` of `/invoke/<name>` that fulfills): the test server
/// runs it on the real core, behind the fakes of the remote services, and the test asserts what the fake got (its
/// log, `getPatronusLog`, `getGitHubLog`, `getYouTrack`). The failure of a write stays a page answer
/// (`routeCommandError`), as does a hold that lets the call go on to the server (`holdCommand`, `recordCommand`)
const MUTATING: &[&str] = &[
  // the operations of the remote and the rewrites (`operations::engine`)
  "execute",
  "cancel_operation",
  // the links of Reviews (`repository-state.json`)
  "link_pull_request",
  "unlink_pull_request",
  // the sign-ins
  "start_github_sign_in",
  "wait_github_sign_in",
  "cancel_github_sign_in",
  "sign_out_of_github",
  "start_youtrack_sign_in",
  "wait_youtrack_sign_in",
  "cancel_youtrack_sign_in",
  "save_youtrack_token",
  "sign_out_of_youtrack",
  // git: the fetches, the sync and the rewrites (a push is an intent, `execute`)
  "fetch_review_head",
  "cancel_review_fetch",
  "cancel_symbol_search",
  "prune_review_refs",
  "sync",
  "set_window_focus",
  "cancel_conflict_fix",
  // the app
  "record_activity",
  "clear_activity",
  "set_repository_config",
  "set_auto_sync_menu_checked",
  "set_setting",
  "install_agent_cli",
  // the CLIs of Settings → Tools, on the fakes of the repository
  "run_cli_tool_action",
  "cancel_cli_tool_action",
  "start_youtrack_cli_sign_in",
  "wait_youtrack_cli_sign_in",
  "install_update",
  "restart_app",
];

/// The read commands that E2E tests still answer in the page, with the number of answers in `tests/e2e` (a call of a
/// helper that answers counts as its answers): a state that neither the fixtures of the fakes nor the templates make
/// yet. The numbers only go down: a new answer of a command in the page fails, so it comes from a fixture of the fake;
/// a removed one lowers its number
const ROUTED_IN_PAGE: &[(&str, usize)] = &[("agent_cli_status", 2), ("get_repository_config", 3), ("prefetch_branch_review", 1)];

fn read_workspace_file(path: &str) -> String {
  let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
  std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("Cannot read {}: {e}", file.display()))
}

/// The names inside `collect_commands![...]`, in their order
fn app_commands() -> Vec<String> {
  let text = read_workspace_file("src-tauri/src/lib.rs");
  let start = text.find("collect_commands![").expect("collect_commands! in src-tauri/src/lib.rs") + "collect_commands![".len();
  let end = start + text[start..].find(']').expect("the end of collect_commands!");
  let names: Vec<String> = text[start..end].split(',').map(str::trim).filter(|name| !name.is_empty()).map(str::to_owned).collect();
  for name in &names {
    assert!(is_identifier(name), "collect_commands! holds `{name}`, which is no command name: the parse needs an update");
  }
  names
}

/// The command names of the `/invoke/<name>` routes of this server (`/invoke/browse_repository/{repo_id}` names
/// `browse_repository`)
fn server_routes() -> Vec<String> {
  let text = read_workspace_file("crates/test-server/src/lib.rs");
  text
    .split("\"/invoke/")
    .skip(1)
    .map(|rest| rest.split(['"', '/']).next().expect("a route name").to_owned())
    .collect()
}

/// The top-level keys of `COMMAND_HANDLERS` in the page mock, plugin prefixes (`plugin:event`) included
fn page_handlers() -> Vec<String> {
  page_handlers_of(&read_workspace_file("tests/e2e/setup/tauri-mocks.ts"))
}

/// The keys of `COMMAND_HANDLERS` in `text`: a `"quoted": …` or `identifier: …` entry at the indentation of the entries;
/// another form there (a method shorthand `name(cmd, payload) {`, a computed key) fails, because it would hide a handler
fn page_handlers_of(text: &str) -> Vec<String> {
  let mut lines = text.lines().skip_while(|line| !line.contains("const COMMAND_HANDLERS"));
  let declaration = lines.next().expect("COMMAND_HANDLERS in tauri-mocks.ts");
  let indent = indentation(declaration);
  let mut keys = Vec::new();
  for line in lines {
    if indentation(line) == indent && line.trim_start().starts_with('}') {
      return keys;
    }
    if indentation(line) != indent + 2 {
      continue;
    }
    let entry = line.trim_start();
    if entry.starts_with("//") || entry.starts_with('}') || entry.starts_with(')') {
      continue;
    }
    let key = match entry.strip_prefix('"') {
      Some(quoted) => quoted.split_once('"').map(|(key, _)| key),
      None => entry.split_once(':').map(|(key, _)| key).filter(|key| is_identifier(key)),
    };
    let key = key.unwrap_or_else(|| panic!("unparsed entry of COMMAND_HANDLERS in tauri-mocks.ts: `{entry}`; write it as `name: handler` or `\"name\": handler`"));
    keys.push(key.to_owned());
  }
  panic!("COMMAND_HANDLERS in tauri-mocks.ts has no closing brace at its own indentation");
}

fn indentation(line: &str) -> usize {
  line.len() - line.trim_start().len()
}

fn is_identifier(name: &str) -> bool {
  !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn names_of(entries: &[(&str, &str)]) -> BTreeSet<String> {
  entries.iter().map(|(name, _)| (*name).to_owned()).collect()
}

#[test]
fn the_app_registers_the_pinned_number_of_commands() {
  let commands = app_commands();
  let unique: BTreeSet<&String> = commands.iter().collect();
  assert_eq!(unique.len(), commands.len(), "collect_commands! names a command twice");
  assert_eq!(
    commands.len(),
    COMMAND_COUNT,
    "collect_commands! has {} commands: route the new one in crates/test-server/src/lib.rs (or name it in PAGE_MOCKED with its reason), then update COMMAND_COUNT",
    commands.len()
  );
}

#[test]
fn every_command_is_routed_or_page_mocked() {
  let routes: BTreeSet<String> = server_routes().into_iter().collect();
  let page_mocked = names_of(PAGE_MOCKED);
  let unreached: Vec<String> = app_commands().into_iter().filter(|name| !routes.contains(name) && !page_mocked.contains(name)).collect();
  assert!(
    unreached.is_empty(),
    "these commands have no route of the test server, so an E2E test that calls them fails with 405: {unreached:?}; route them in crates/test-server/src/lib.rs"
  );
}

#[test]
fn every_page_mocked_command_is_a_command_without_a_route() {
  let commands: BTreeSet<String> = app_commands().into_iter().collect();
  let routes: BTreeSet<String> = server_routes().into_iter().collect();
  assert_eq!(names_of(PAGE_MOCKED).len(), PAGE_MOCKED.len(), "PAGE_MOCKED names a command twice");
  for (name, _) in PAGE_MOCKED {
    assert!(
      commands.contains(*name),
      "PAGE_MOCKED names `{name}`, which collect_commands! does not have: remove the entry"
    );
    assert!(
      !routes.contains(*name),
      "`{name}` has a route of the test server now: remove it from PAGE_MOCKED and lower its pinned number"
    );
  }
}

#[test]
fn every_route_names_a_command() {
  let commands: BTreeSet<String> = app_commands().into_iter().collect();
  let routes = server_routes();
  let unique: BTreeSet<&String> = routes.iter().collect();
  assert_eq!(unique.len(), routes.len(), "the test server routes a command twice");
  let raw = names_of(RAW_ROUTES);
  for name in &routes {
    assert!(
      commands.contains(name) || raw.contains(name),
      "the route /invoke/{name} names no command of collect_commands!: remove it"
    );
  }
  for (name, _) in RAW_ROUTES {
    assert!(!commands.contains(*name), "`{name}` is a typed command now: remove it from RAW_ROUTES");
    assert!(unique.contains(&(*name).to_owned()), "RAW_ROUTES names `{name}`, which has no route: remove the entry");
  }
}

#[test]
fn every_page_handler_of_a_command_is_page_mocked_or_wraps_its_route() {
  let commands: BTreeSet<String> = app_commands().into_iter().collect();
  let routes: BTreeSet<String> = server_routes().into_iter().collect();
  let page_mocked = names_of(PAGE_MOCKED);
  let handlers = page_handlers();
  assert!(
    handlers.iter().any(|key| key == "plugin:event"),
    "the parse of COMMAND_HANDLERS found no `plugin:event`: {handlers:?}"
  );
  let mut wrappers = Vec::new();
  for key in handlers.iter().filter(|key| !key.starts_with("plugin:")) {
    assert!(
      commands.contains(key),
      "COMMAND_HANDLERS of tauri-mocks.ts handles `{key}`, which collect_commands! does not have: remove the handler"
    );
    if routes.contains(key) {
      // a handler over the route: it adapts a channel or emits an event, and proxies the command to this server
      wrappers.push(key.as_str());
    } else {
      assert!(
        page_mocked.contains(key),
        "COMMAND_HANDLERS answers `{key}` in the page: name it in PAGE_MOCKED with its reason"
      );
    }
  }
  wrappers.sort_unstable();
  assert_eq!(
    wrappers,
    [
      "clear_activity",
      "execute",
      "get_file_diffs",
      "get_file_merges",
      "get_settings_changes",
      "record_activity",
      "save_youtrack_token",
      "set_repository_config",
      "set_setting",
      "sign_out_of_github",
      "sign_out_of_youtrack",
      "sub_window_drawn",
      "sub_window_loaded",
      "wait_github_sign_in",
      "wait_youtrack_sign_in"
    ],
    "the handlers of COMMAND_HANDLERS that wrap a route"
  );
}

#[test]
fn page_mocked_only_goes_down() {
  assert_eq!(
    PAGE_MOCKED.len(),
    3,
    "PAGE_MOCKED may only shrink: a new command gets a route of the test server, not a page mock; when a command gets its route, lower this number"
  );
}

#[test]
fn the_page_handler_parser_reads_every_entry_form_it_knows_and_refuses_another() {
  let known = r#"
  const COMMAND_HANDLERS: Record<string, (cmd: string, payload: any) => any> = {
    // a comment between the entries
    "plugin:event": handleEventCommand,
    "plugin:webview": (cmd: string, payload: any) => {
      return null
    },
    get_zoom: () => 1,
  }
"#;
  assert_eq!(page_handlers_of(known), ["plugin:event", "plugin:webview", "get_zoom"]);

  let shorthand = r#"
  const COMMAND_HANDLERS: Record<string, (cmd: string, payload: any) => any> = {
    "plugin:event": handleEventCommand,
    start_patronus_robot(_cmd, payload) {
      return robot
    },
  }
"#;
  let failure = std::panic::catch_unwind(|| page_handlers_of(shorthand)).unwrap_err();
  let message = failure.downcast_ref::<String>().cloned().unwrap_or_default();
  assert!(message.contains("unparsed entry") && message.contains("start_patronus_robot(_cmd, payload) {"), "{message}");
}

/// The TypeScript files under `tests/e2e`, by their path relative to the workspace
fn e2e_sources() -> Vec<(String, String)> {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let mut sources = Vec::new();
  let mut directories = vec![root.join("tests/e2e")];
  while let Some(directory) = directories.pop() {
    let entries = std::fs::read_dir(&directory).unwrap_or_else(|e| panic!("Cannot read {}: {e}", directory.display()));
    for entry in entries {
      let path = entry.expect("a directory entry").path();
      if path.is_dir() {
        directories.push(path);
      } else if path.extension().is_some_and(|extension| extension == "ts") {
        let relative = path.strip_prefix(&root).expect("under the workspace").to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
        sources.push((relative, text));
      }
    }
  }
  sources.sort();
  sources
}

/// A source of `tests/e2e`: its text with the comments and the regex literals blanked (so a `routeCommand(` or an
/// apostrophe in a comment or a regex is no call and no string) and its declared functions
struct Source {
  file: String,
  text: String,
  declarations: Vec<Declaration>,
}

/// A declared function, `[export] [async] function <name>(…) {…}`: a helper whose body answers a command answers it
/// once per call
struct Declaration {
  name: String,
  /// From `function` through the closing brace of the body
  range: std::ops::Range<usize>,
  body: std::ops::Range<usize>,
  /// Declared inside another declaration: no helper (its calls are not counted), so it answers nothing
  nested: bool,
}

/// An answer in the page: where it is given and the command it answers
struct Answer {
  at: usize,
  command: String,
}

impl Source {
  fn new(file: &str, text: &str) -> Self {
    let text = without_comments(text);
    let declarations = declarations(&text);
    Self {
      file: file.to_owned(),
      text,
      declarations,
    }
  }

  /// Outside every declared function: the tests themselves (`test("…", async ({ page }) => {…})`)
  fn is_top_level(&self, at: usize) -> bool {
    !self.declarations.iter().any(|declaration| declaration.range.contains(&at))
  }

  /// The helpers of this source named `name`: the outermost declarations
  fn helpers_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Declaration> + 'a {
    self.declarations.iter().filter(move |declaration| declaration.name == name && !declaration.nested)
  }

  /// The commands the region `within` answers itself, one entry per answer: each `routeCommand(page, "<name>", …)` and
  /// each `.route("**/invoke/<name>", …)` whose handler fulfills the request with an answer of its own. A handler that
  /// binds `const response = await route.fetch()` ran the command on the server, and a `fulfill` that passes that
  /// binding as its `response` (`{ response, body }`, `{ response: response }`) rewrites the server's answer, which
  /// is no page answer, nor is a hold that falls back to the server; any other `fulfill`, also one of a value that
  /// happens to be named `response`, answers. A `routeCommand` or a route with a computed name fails (it would hide
  /// the command from the ratchet); a computed route is only the primitives' own (`wait-helpers.ts`)
  fn direct_answers(&self, within: impl Fn(usize) -> bool) -> Vec<Answer> {
    let (file, text) = (&self.file, &self.text);
    let mut answers = Vec::new();
    for (at, _) in text
      .match_indices("routeCommand(")
      .filter(|(at, _)| within(*at) && !is_identifier_char(text[..*at].chars().next_back()))
    {
      let arguments = &text[at + "routeCommand(".len()..];
      let name = arguments
        .split_once(',')
        .and_then(|(_, rest)| string_literal(rest.trim_start()))
        .unwrap_or_else(|| panic!("{file}:{}: routeCommand names its command by a string literal", line_of(text, at)));
      answers.push(Answer { at, command: name.to_owned() });
    }
    for (at, _) in text.match_indices(".route(").filter(|(at, _)| within(*at)) {
      let call = &text[at + ".route".len()..balanced_end(text, at + ".route".len())];
      let target = call[1..].split(',').next().unwrap_or_default().trim();
      let Some(after_invoke) = target.split_once("/invoke/").map(|(_, name)| name) else {
        continue;
      };
      let fetched = fetched_bindings(file, text, at, call);
      let fulfills: Vec<&str> = call
        .match_indices(".fulfill(")
        .map(|(fulfill, _)| &call[fulfill..balanced_end(call, fulfill + ".fulfill".len())])
        .collect();
      if fulfills.is_empty() || fulfills.iter().all(|fulfill| passes_fetched_response(fulfill, &fetched)) {
        continue;
      }
      let name = after_invoke.strip_suffix('"').filter(|name| is_identifier(name));
      let Some(name) = name else {
        assert!(
          file.ends_with("tests/e2e/helpers/wait-helpers.ts"),
          "{file}:{}: a route of `/invoke/` names its command by a string literal",
          line_of(text, at)
        );
        continue;
      };
      answers.push(Answer { at, command: name.to_owned() });
    }
    answers
  }

  /// The commands of `direct_answers`
  fn direct_commands(&self, within: impl Fn(usize) -> bool) -> Vec<String> {
    self.direct_answers(within).into_iter().map(|answer| answer.command).collect()
  }

  /// How often `range` of the text calls the function `name` (`name(`, not a method `.name(`), the calls at the top
  /// level only with `top_level`
  fn calls_of(&self, name: &str, range: std::ops::Range<usize>, top_level: bool) -> usize {
    self.calls_at(name, range, top_level).len()
  }

  /// Where `range` of the text calls the function `name` (`calls_of`)
  fn calls_at(&self, name: &str, range: std::ops::Range<usize>, top_level: bool) -> Vec<usize> {
    let pattern = format!("{name}(");
    let start = range.start;
    self.text[range]
      .match_indices(&pattern)
      .map(|(at, _)| start + at)
      .filter(|at| !top_level || self.is_top_level(*at))
      .filter(|at| {
        let before = &self.text[..*at];
        let last = before.chars().next_back();
        // not a method (`.name(`), not the name of a declaration or an overload signature (`function name(`)
        !is_identifier_char(last) && last != Some('.') && !before.trim_end().ends_with("function")
      })
      .collect()
  }

  fn whole(&self) -> std::ops::Range<usize> {
    0..self.text.len()
  }
}

/// The answers of each helper computed so far (`helper_answers`)
type HelperAnswers = std::collections::BTreeMap<String, Vec<String>>;

fn is_identifier_char(c: Option<char>) -> bool {
  c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// The content of the string literal `"…"` at the start of `text`
fn string_literal(text: &str) -> Option<&str> {
  let quoted = text.strip_prefix('"')?;
  quoted.split_once('"').map(|(content, _)| content)
}

fn line_of(text: &str, at: usize) -> usize {
  text[..at].matches('\n').count() + 1
}

/// The names a route handler `call` binds to the server's answer, `const <name> = await route.fetch()`; any other
/// `.fetch(` of a handler fails, because the scan could not tell its fulfills apart
fn fetched_bindings(file: &str, text: &str, route_at: usize, call: &str) -> Vec<String> {
  call
    .match_indices(".fetch(")
    .map(|(at, _)| {
      let before = &call[..at];
      let binding = before.rfind("const ").map(|start| &before[start + "const ".len()..]).and_then(|rest| {
        let name_end = rest.find(|c: char| !is_identifier_char(Some(c)))?;
        (&rest[name_end..] == " = await route").then(|| rest[..name_end].to_owned())
      });
      binding.unwrap_or_else(|| {
        panic!(
          "{file}:{}: a route handler binds the server's answer as `const <name> = await route.fetch()`",
          line_of(text, route_at)
        )
      })
    })
    .collect()
}

/// Whether the `fulfill(…)` text `fulfill` passes one of the `fetched` bindings as its `response` property: a property
/// key `response` after `{` or `,`, shorthand (`{ response, …}`) or with the binding as its value (`response: answer`)
fn passes_fetched_response(fulfill: &str, fetched: &[String]) -> bool {
  fulfill.match_indices("response").any(|(at, _)| {
    let key_before = fulfill[..at].trim_end().chars().next_back();
    if !matches!(key_before, Some('{' | ',')) || is_identifier_char(fulfill[..at].chars().next_back()) {
      return false;
    }
    let after = fulfill[at + "response".len()..].trim_start();
    match after.chars().next() {
      Some(',' | '}') => fetched.iter().any(|binding| binding == "response"),
      Some(':') => {
        let value = after[1..].trim_start();
        let value_end = value.find(|c: char| !is_identifier_char(Some(c))).unwrap_or(value.len());
        let value_ends_there = matches!(value[value_end..].trim_start().chars().next(), Some(',' | '}'));
        value_ends_there && fetched.iter().any(|binding| binding == &value[..value_end])
      }
      _ => false,
    }
  })
}

/// `text` with every `//` and `/* */` comment and every regex literal (`/…/` where `regex_follows`: after an
/// operator, an arrow, an opening bracket, a separator or `return`) blanked, each of its characters a space, the
/// newlines kept: a quote in a regex would unbalance the brackets
fn without_comments(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  let mut quote: Option<char> = None;
  while let Some(c) = rest.chars().next() {
    let (take, blank) = match quote {
      Some(open) => {
        let taken = if c == '\\' {
          c.len_utf8() + rest[c.len_utf8()..].chars().next().map_or(0, char::len_utf8)
        } else {
          c.len_utf8()
        };
        if c == open || (open != '`' && c == '\n') {
          quote = None;
        }
        (taken, false)
      }
      None if rest.starts_with("//") => (rest.find('\n').unwrap_or(rest.len()), true),
      None if rest.starts_with("/*") => (rest.find("*/").map_or(rest.len(), |end| end + 2), true),
      None if c == '/' && regex_follows(&out) => match regex_end(rest) {
        Some(end) => (end, true),
        None => (c.len_utf8(), false),
      },
      None => {
        if matches!(c, '"' | '\'' | '`') {
          quote = Some(c);
        }
        (c.len_utf8(), false)
      }
    };
    let (taken, after) = rest.split_at(take);
    if blank {
      out.extend(taken.chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
    } else {
      out.push_str(taken);
    }
    rest = after;
  }
  out
}

/// Whether a `/` after the text `before` opens a regex literal rather than a division: it follows an operator (an
/// arrow `=>` too), an opening bracket, a separator or a keyword that takes a value (`return`, `await`, …)
fn regex_follows(before: &str) -> bool {
  const KEYWORDS: [&str; 7] = ["return", "await", "yield", "throw", "typeof", "case", "in"];
  let before = before.trim_end();
  before.is_empty()
    || before.ends_with(['(', ',', '=', ':', '[', '!', '&', '|', '?', '{', '}', ';', '+', '-', '*', '%'])
    || before.ends_with("=>")
    || KEYWORDS
      .iter()
      .any(|keyword| before.strip_suffix(keyword).is_some_and(|rest| !is_identifier_char(rest.chars().next_back())))
}

/// The end (exclusive) of the regex literal at the start of `text`, `None` when no unescaped `/` closes it on the line
fn regex_end(text: &str) -> Option<usize> {
  let mut class = false;
  let mut escaped = false;
  for (offset, c) in text.char_indices().skip(1) {
    match c {
      '\n' => return None,
      _ if escaped => escaped = false,
      '\\' => escaped = true,
      '[' => class = true,
      ']' => class = false,
      '/' if !class => return Some(offset + 1),
      _ => {}
    }
  }
  None
}

/// The declared functions of `text` (comments blanked), the nested ones marked; an overload signature (`…): void;`,
/// or `…): void` alone on its line as the formatter writes it, the next line a declaration) without a body is none,
/// and a signature whose body the scan cannot find fails
fn declarations(text: &str) -> Vec<Declaration> {
  let mut found: Vec<Declaration> = Vec::new();
  for (at, _) in text.match_indices("function ") {
    if is_identifier_char(text[..at].chars().next_back()) {
      continue;
    }
    let after_keyword = at + "function ".len();
    let name_end = after_keyword + text[after_keyword..].find(|c: char| !is_identifier_char(Some(c))).unwrap_or(0);
    let name = &text[after_keyword..name_end];
    if name.is_empty() {
      // a function expression
      continue;
    }
    let mut cursor = name_end;
    if text[cursor..].starts_with('<') {
      cursor = balanced_end(text, cursor);
    }
    assert!(text[cursor..].starts_with('('), "{}: the parameters of function {name}", line_of(text, at));
    cursor = balanced_end(text, cursor);
    let open = match body_open(text, cursor) {
      Body::Opens(open) => open,
      Body::Overload => continue,
      Body::Unread => panic!("{}: the body of function {name} is not where the scan reads it", line_of(text, at)),
    };
    let end = balanced_end(text, open);
    let nested = found.iter().any(|declaration| declaration.range.contains(&at));
    found.push(Declaration {
      name: name.to_owned(),
      range: at..end,
      body: open..end,
      nested,
    });
  }
  found
}

/// What follows the signature of a declared function
enum Body {
  /// The body opens at this brace
  Opens(usize),
  /// An overload signature, ended by `;` or by the end of its line when the next line declares (`export`,
  /// `function`, `async`, `declare`): the formatter (`semi: false`) writes no `;`
  Overload,
  /// A signature the scan cannot read
  Unread,
}

/// Where the body opens after the parameters that end at `cursor`. A brace of a type literal in the return type
/// (`: { a: string }`, `Promise<{ a: string }>`, `| { b: number }`) follows `:`, `<`, `|`, `&`, `,`, `=` or `(` and
/// is skipped whole; the body's brace follows the `)` of the parameters, a `>`, a `]`, a `}` of a type literal or a
/// type name
fn body_open(text: &str, cursor: usize) -> Body {
  const DECLARES: [&str; 4] = ["export ", "function ", "async ", "declare "];
  let mut previous = ')';
  let mut at = cursor;
  while let Some(c) = text[at..].chars().next() {
    match c {
      '{' if matches!(previous, ':' | '<' | '|' | '&' | ',' | '=' | '(') => {
        at = balanced_end(text, at);
        previous = '}';
        continue;
      }
      '{' => return Body::Opens(at),
      ';' => return Body::Overload,
      '\n' if DECLARES.iter().any(|word| text[at..].trim_start().starts_with(word)) => return Body::Overload,
      '}' => return Body::Unread,
      c if c.is_whitespace() => {
        at += c.len_utf8();
        continue;
      }
      _ => {}
    }
    previous = c;
    at += c.len_utf8();
  }
  Body::Unread
}

/// The end (exclusive) of the parenthesised, braced or angled text that opens at `open`; brackets in string and
/// template literals do not count
fn balanced_end(text: &str, open: usize) -> usize {
  let opener = text[open..].chars().next().expect("an opening bracket");
  let closer = match opener {
    '(' => ')',
    '{' => '}',
    '<' => '>',
    other => panic!("`{other}` opens no bracket"),
  };
  let mut depth = 0usize;
  let mut quote: Option<char> = None;
  let mut escaped = false;
  for (offset, c) in text[open..].char_indices() {
    if let Some(open_quote) = quote {
      if escaped {
        escaped = false;
      } else if c == '\\' {
        escaped = true;
      } else if c == open_quote {
        quote = None;
      }
      continue;
    }
    match c {
      '"' | '\'' | '`' => quote = Some(c),
      // the `>` of an arrow in a type argument (`<{ f: () => void }>`) closes no angle bracket
      '>' if closer == '>' && text[..open + offset].ends_with('=') => {}
      c if c == opener => depth += 1,
      c if c == closer => {
        depth -= 1;
        if depth == 0 {
          return open + offset + c.len_utf8();
        }
      }
      _ => {}
    }
  }
  panic!("{}: `{opener}` without its closing bracket", line_of(text, open));
}

/// The commands every call of the helper `name` answers, with the ones of the helpers it calls, and an empty list for
/// a function that answers nothing or for a name that no source declares; `visiting` refuses a cycle
fn helper_answers(sources: &[Source], name: &str, visiting: &mut Vec<String>, memo: &mut HelperAnswers) -> Vec<String> {
  if name == "routeCommand" {
    // the primitive: its calls name their command themselves
    return Vec::new();
  }
  if let Some(known) = memo.get(name) {
    return known.clone();
  }
  let defined: Vec<(&Source, &Declaration)> = sources
    .iter()
    .flat_map(|source| source.helpers_named(name).map(move |declaration| (source, declaration)))
    .collect();
  if defined.is_empty() {
    return Vec::new();
  }
  assert!(!visiting.iter().any(|it| it == name), "the helper {name} calls itself through {visiting:?}");
  visiting.push(name.to_owned());
  let answers: Vec<(String, Vec<String>)> = defined
    .iter()
    .map(|(source, declaration)| {
      let place = format!("{}:{}", source.file, line_of(&source.text, declaration.range.start));
      (place, declaration_answers(sources, source, declaration, visiting, memo))
    })
    .collect();
  visiting.pop();
  if answers.len() > 1 && answers.iter().any(|(_, answers)| !answers.is_empty()) {
    let places: Vec<&str> = answers.iter().map(|(place, _)| place.as_str()).collect();
    panic!("the helper {name} is declared in {places:?}: a helper that answers a command has one declaration, so that its calls are counted");
  }
  let answers = answers.into_iter().next().map_or_default(|(_, answers)| answers);
  memo.insert(name.to_owned(), answers.clone());
  answers
}

/// The commands a call of `declaration` answers: its own, and those of each helper it calls, per call
fn declaration_answers(sources: &[Source], source: &Source, declaration: &Declaration, visiting: &mut Vec<String>, memo: &mut HelperAnswers) -> Vec<String> {
  let body = declaration.body.clone();
  let mut answers = source.direct_commands(|at| body.contains(&at));
  let callees: BTreeSet<&str> = helper_names(sources);
  for callee in callees.into_iter().filter(|callee| *callee != declaration.name) {
    let calls = source.calls_of(callee, body.clone(), false);
    if calls > 0 {
      let of_callee = helper_answers(sources, callee, visiting, memo);
      answers.extend(of_callee.iter().cloned().cycle().take(of_callee.len() * calls));
    }
  }
  answers
}

/// The names of the helpers of all sources: the outermost declared functions
fn helper_names(sources: &[Source]) -> BTreeSet<&str> {
  sources
    .iter()
    .flat_map(|source| source.declarations.iter().filter(|it| !it.nested).map(|it| it.name.as_str()))
    .collect()
}

/// How often the E2E tests answer each command in the page: the answers of the tests themselves, and those of every
/// helper per call. A helper is a declared function at the outermost level; an answer anywhere else at the top level,
/// an arrow function or a function expression kept in a `const` in any form, a fixture definition (`test.extend`),
/// or a function declared inside a helper, is refused, because its calls would not be counted. Inside a test the
/// count is of the places that answer: a `beforeEach`, a loop or a local function runs an answer more often
fn page_answers_of(sources: &[Source]) -> std::collections::BTreeMap<String, usize> {
  let mut counts = std::collections::BTreeMap::new();
  let mut count = |names: Vec<String>, times: usize| {
    for name in names {
      *counts.entry(name).or_insert(0) += times;
    }
  };
  let mut memo = HelperAnswers::new();
  let answering: Vec<(&str, Vec<String>)> = helper_names(sources)
    .into_iter()
    .map(|name| (name, helper_answers(sources, name, &mut Vec::new(), &mut memo)))
    .filter(|(_, answers)| !answers.is_empty())
    .collect();
  for source in sources {
    refuse_nested_answering_functions(source, &answering);
    refuse_answers_outside_tests(source, &answering);
    count(source.direct_commands(|at| source.is_top_level(at)), 1);
    for (name, answers) in &answering {
      count(answers.clone(), source.calls_of(name, source.whole(), true));
    }
  }
  counts
}

/// Whether the body `body` of `source` answers a command: itself, or through a call of an answering helper
fn answers_in(source: &Source, body: std::ops::Range<usize>, answering: &[(&str, Vec<String>)]) -> bool {
  !source.direct_answers(|at| body.contains(&at)).is_empty() || answering.iter().any(|(helper, _)| source.calls_of(helper, body.clone(), false) > 0)
}

/// A function declared inside a helper whose body answers a command fails: its calls are not counted
fn refuse_nested_answering_functions(source: &Source, answering: &[(&str, Vec<String>)]) {
  for declaration in source.declarations.iter().filter(|it| it.nested) {
    assert!(
      !answers_in(source, declaration.body.clone(), answering),
      "{}:{}: function {}, declared inside another function, answers a command: declare it at the top level, so that the ratchet counts its calls",
      source.file,
      line_of(&source.text, declaration.range.start),
      declaration.name
    );
  }
}

/// An answer at the top level of `source` (a direct one, or a call of an answering helper) lies inside a test
/// (`test("…", …)`, `test.describe(…)`, `test.beforeEach(…)`, a fixture's `testWith…(…)`): anything else fails,
/// because its calls would not be counted: an arrow function or a function expression kept in a `const` in any of
/// its forms, a method, and the definition of a fixture (`test.extend({…})`, `test.use({…})`, typed or not), which
/// answers once per test that uses it
fn refuse_answers_outside_tests(source: &Source, answering: &[(&str, Vec<String>)]) {
  let text = &source.text;
  let calls = test_calls(text);
  let answers = source.direct_answers(|at| source.is_top_level(at)).into_iter().map(|answer| answer.at);
  let helper_calls = answering.iter().flat_map(|(helper, _)| source.calls_at(helper, source.whole(), true));
  for at in answers.chain(helper_calls) {
    if calls.iter().any(|call| !call.fixture && call.range.contains(&at)) {
      continue;
    }
    let holder = text[..at]
      .rmatch_indices("const ")
      .map(|(start, _)| &text[start + "const ".len()..])
      .map(|rest| &rest[..rest.find(|c: char| !is_identifier_char(Some(c))).unwrap_or(0)])
      .find(|name| !name.is_empty())
      .unwrap_or("<name>");
    let line = line_of(text, at);
    assert!(
      !calls.iter().any(|call| call.fixture && call.range.contains(&at)),
      "{}:{line}: an answer inside the fixture `{holder}` (`test.extend`), which answers once per test that uses it: give it in each test or in a `beforeEach` of their describe, where the ratchet pins it",
      source.file
    );
    panic!(
      "{}:{line}: an answer outside a test and a function declaration (`const {holder} = …`): declare it as `function {holder}(…)`, so that the ratchet counts its calls",
      source.file
    );
  }
}

/// A call of the test object: a test or a fixture definition (`test_calls`)
struct TestCall {
  /// `test.extend(…)` or `test.use(…)`: a fixture, whose answers run once per test
  fixture: bool,
  /// From the name through the closing parenthesis
  range: std::ops::Range<usize>,
}

/// The calls of the test object in `text`: an identifier that starts with `test` or ends with `Test` (`test`, a
/// fixture's `testWithConflicts`, `baseTest`) through a member chain of the test API (`describe`, `beforeEach`, `step`,
/// `only`, `serial`, …) to the opening parenthesis, nested or not; `extend` and `use` make a fixture (their type
/// argument skipped); any other member (`info`, `setTimeout`, `describe.configure`) is no call of a test
fn test_calls(text: &str) -> Vec<TestCall> {
  const MEMBERS: [&str; 13] = [
    "describe",
    "serial",
    "parallel",
    "only",
    "skip",
    "fixme",
    "fail",
    "slow",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "step",
  ];
  ["test", "Test"]
    .into_iter()
    .flat_map(|word| text.match_indices(word))
    .filter_map(|(at, _)| {
      let start = text[..at].rfind(|c: char| !is_identifier_char(Some(c))).map_or(0, |before| before + 1);
      let name_end = at + text[at..].find(|c: char| !is_identifier_char(Some(c)))?;
      let name = &text[start..name_end];
      if !(name.starts_with("test") || name.ends_with("Test")) || text[..start].ends_with('.') {
        return None;
      }
      let mut cursor = name_end;
      let mut fixture = false;
      loop {
        match text[cursor..].chars().next()? {
          '.' => {
            let member_end = cursor + 1 + text[cursor + 1..].find(|c: char| !is_identifier_char(Some(c)))?;
            let member = &text[cursor + 1..member_end];
            match member {
              "extend" | "use" => fixture = true,
              member if MEMBERS.contains(&member) => {}
              _ => return None,
            }
            cursor = member_end;
          }
          '<' if fixture => cursor = balanced_end(text, cursor),
          '(' => {
            return Some(TestCall {
              fixture,
              range: start..balanced_end(text, cursor),
            });
          }
          _ => return None,
        }
      }
    })
    .collect()
}

/// How often the E2E tests answer each command in the page
fn page_answers() -> std::collections::BTreeMap<String, usize> {
  let sources: Vec<Source> = e2e_sources().iter().map(|(file, text)| Source::new(file, text)).collect();
  page_answers_of(&sources)
}
#[test]
fn no_e2e_test_answers_a_write_in_the_page() {
  let answers = page_answers();
  let written: Vec<(&String, &usize)> = answers.iter().filter(|(name, _)| MUTATING.contains(&name.as_str())).collect();
  assert!(
    written.is_empty(),
    "E2E tests answer these writes in the page: {written:?}; let the test server run them behind the fake of their service and assert the fake's log (a failure is `routeCommandError`)"
  );
}

#[test]
fn the_page_answers_of_the_e2e_tests_are_pinned() {
  let pinned: std::collections::BTreeMap<String, usize> = ROUTED_IN_PAGE.iter().map(|(name, count)| ((*name).to_owned(), *count)).collect();
  let answers: std::collections::BTreeMap<String, usize> = page_answers().into_iter().filter(|(name, _)| !MUTATING.contains(&name.as_str())).collect();
  assert_eq!(
    answers, pinned,
    "the commands that E2E tests answer in the page (routeCommand, a fulfilling page.route of /invoke/): a new answer comes from a fixture of the fake instead; for a removed one, lower its number in ROUTED_IN_PAGE"
  );
}

#[test]
fn the_ratchet_names_commands_of_the_app() {
  let commands: BTreeSet<String> = app_commands().into_iter().collect();
  let mutating: BTreeSet<&str> = MUTATING.iter().copied().collect();
  assert_eq!(mutating.len(), MUTATING.len(), "MUTATING names a command twice");
  for name in MUTATING.iter().chain(ROUTED_IN_PAGE.iter().map(|(name, _)| name)) {
    assert!(commands.contains(*name), "the ratchet names `{name}`, which collect_commands! does not have");
  }
  for (name, _) in ROUTED_IN_PAGE {
    assert!(!mutating.contains(name), "`{name}` writes: it cannot be pinned in ROUTED_IN_PAGE");
  }
}

fn answers_of(sources: &[(&str, &str)]) -> Vec<(String, usize)> {
  let sources: Vec<Source> = sources.iter().map(|(file, text)| Source::new(file, text)).collect();
  page_answers_of(&sources).into_iter().collect()
}

fn panic_message(failure: &(dyn std::any::Any + Send)) -> String {
  failure.downcast_ref::<String>().cloned().unwrap_or_default()
}

#[test]
fn the_page_answer_scan_reads_routes_and_refuses_a_computed_name() {
  let text = r#"
    // a comment: routeCommand(page, command) and the robot's answer
    test("a", async ({ page }) => {
    await routeCommand(page, "get_patronus_robots", () => [])
    const calls = await routeCommand(
      page,
      "get_patronus_robot_details",
      () => ({ status: "ok (done)" }),
    )
    await routeCommandError(page, "push_branch", { code: "internal", message: "rejected" })
    await page.route("**/invoke/push_branches", async route => {
      await route.fulfill({ json: { type: "pushed" } })
    })
    await page.route("**/invoke/get_branch_review_readiness", async route => {
      await held
      await route.fallback()
    })
    await page.route("**/invoke/sync", async route => {
      const response = await route.fetch()
      await route.fulfill({ response, body: rewrite(await response.text()) })
    })
    await page.route("**/invoke/get_file_diffs", async route => {
      const answer = await route.fetch()
      await route.fulfill({ response: answer, body: rewrite(await answer.text()) })
    })
    await page.route("**/invoke/start_patronus_robot", async route => {
      if (refuse) return route.fulfill({ json: robot })
      const response = await route.fetch()
      await route.fulfill({ response })
    })
    // a value named `response` is an answer of the page, not the server's
    const response = { robotId: "x" }
    await page.route("**/invoke/restart_patronus_robot", async route => {
      await route.fulfill({ json: response })
    })
    await page.route("**/invoke/cancel_patronus_robot", async route => {
      await route.fulfill({ json: responseFor(route.request()) })
    })
    })
"#;
  let expected = [
    ("cancel_patronus_robot", 1),
    ("get_patronus_robot_details", 1),
    ("get_patronus_robots", 1),
    ("push_branches", 1),
    ("restart_patronus_robot", 1),
    ("start_patronus_robot", 1),
  ];
  assert_eq!(answers_of(&[("tests/e2e/a.spec.ts", text)]), expected.map(|(name, count)| (name.to_owned(), count)));

  let computed = "test(\"b\", async ({ page }) => { await routeCommand(page, command, () => null) })";
  let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/b.spec.ts", computed)])).unwrap_err());
  assert!(message.contains("tests/e2e/b.spec.ts:1") && message.contains("string literal"), "{message}");

  // a fixture's test under another name, a regex with a quote in it (after `(` and after an arrow) and an overload
  // signature as the formatter writes it (no `;`) are no answers outside a test
  let fixture = r#"
    export function overloaded(page: Page): void
    export function overloaded(page: Page) {
      return routeCommand(page, "get_commit_files", () => [])
    }
    baseTest.describe("a", () => {
      baseTest("b", async ({ page }) => {
        await expect(page.getByText(/doesn't have (them)/)).toBeVisible()
        const n = names.filter(x => /doesn't (them)/.test(x))
        await routeCommand(page, "get_patronus_robots", () => [])
        overloaded(page)
      })
    })
"#;
  let expected = [("get_commit_files", 1), ("get_patronus_robots", 1)];
  assert_eq!(answers_of(&[("tests/e2e/d.spec.ts", fixture)]), expected.map(|(name, count)| (name.to_owned(), count)));

  // a fetch bound any other way: the scan cannot tell a rewrite from an answer
  let unbound = r#"
    test("c", async ({ page }) => {
      await page.route("**/invoke/sync", async route => {
        const [response] = [await route.fetch()]
        await route.fulfill({ response })
      })
    })
"#;
  let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/c.spec.ts", unbound)])).unwrap_err());
  assert!(message.contains("tests/e2e/c.spec.ts:3") && message.contains("await route.fetch()"), "{message}");
}

#[test]
fn the_page_answer_scan_counts_a_helper_once_per_call_through_the_helpers_it_calls() {
  let helpers = r#"
    export function routeRobots(page: Page): Promise<Record<string, unknown>[]> {
      return routeCommand(page, "get_patronus_robots", () => [])
    }
    /** `Promise<{…}>`: a brace of the return type opens no body */
    export async function routePushes(page: Page, results: PushResults = {}): Promise<{ pushes: PushRequest[]; stackPushes: PushRequest[][] }> {
      await page.route("**/invoke/push_branch", async route => {
        await route.fulfill({ json: pushOutcome("a") })
      })
      return { pushes: [], stackPushes: [] }
    }
    export async function routePush(page: Page): Promise<PushRequest[]> {
      return (await routePushes(page)).pushes
    }
    export async function holdCommand(page: Page, command: string): Promise<void> {
      await page.route(`**/invoke/${command}`, async route => {
        await route.fallback()
      })
    }
    /** a bare type literal as the return type opens no body either */
    export async function routePushBranches(page: Page): { pushes: PushRequest[] } {
      await page.route("**/invoke/push_branches", async route => {
        await route.fulfill({ json: {} })
      })
      return { pushes: [] }
    }
    /** overload signatures: one ended by `;`, one by its line end as the formatter leaves it */
    export function overloaded(page: Page): void;
    export function overloaded(page: Page, more: string): void
    export function overloaded(page: Page, more?: string) {
      return routeCommand(page, "get_commit_files", () => [])
    }
"#;
  let spec = r#"
    async function routeRobotsTwice(page: Page) {
      await routeRobots(page)
      await routeRobots(page)
    }
    async function setup(page: Page) {
      await holdCommand(page, "sync")
    }
    test.describe("robots", () => {
      test.beforeEach(async ({ page }) => {
        await routePushBranches(page)
      })
      test("a", async ({ page }) => {
        await setup(page)
        await routeRobots(page)
        await routeRobotsTwice(page)
        await routePush(page)
        const { pushes } = await routePushes(page)
        await other.routePush(page)
        await routePushBranches(page)
        overloaded(page)
      })
    })
"#;
  let sources = [("tests/e2e/helpers/wait-helpers.ts", helpers), ("tests/e2e/a.spec.ts", spec)];
  let expected = [("get_commit_files", 1), ("get_patronus_robots", 3), ("push_branch", 2), ("push_branches", 2)];
  assert_eq!(answers_of(&sources), expected.map(|(name, count)| (name.to_owned(), count)));

  // a signature the scan cannot read is no silent "no helper"
  let unread = "function routeRobots(page: Page) }";
  let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/b.spec.ts", unread)])).unwrap_err());
  assert!(message.contains("1: the body of function routeRobots"), "{message}");
}

#[test]
fn the_page_answer_scan_refuses_an_answering_arrow_constant_and_a_helper_declared_twice() {
  // every form of a function kept in a `const`: the ratchet would count its answers once, not once per call
  let constants = [
    "const routeRobots = async (page: Page) => {\n  await routeCommand(page, \"get_patronus_robots\", () => [])\n}",
    "const routeRobots = async page => {\n  await routeCommand(page, \"get_patronus_robots\", () => [])\n}",
    "const routeRobots = (page: Page) =>\n  routeCommand(page, \"get_patronus_robots\", () => [])",
    "const routeRobots = async (page: Page): Promise<void> => {\n  await routeCommand(page, \"get_patronus_robots\", () => [])\n}",
    "export const routeRobots: (page: Page) => Promise<unknown> = async (page) => {\n  await routeCommand(page, \"get_patronus_robots\", () => [])\n}",
    "const routeRobots = async function (page: Page) {\n  await routeCommand(page, \"get_patronus_robots\", () => [])\n}",
    "const routeRobots = {\n  async arrange(page: Page) {\n    await routeCommand(page, \"get_patronus_robots\", () => [])\n  },\n}",
    // a call of an answering helper, not an answer of its own
    "function routeRobotsOnce(page: Page) {\n  return routeCommand(page, \"get_patronus_robots\", () => [])\n}\nconst routeRobots = (page: Page) => routeRobotsOnce(page)",
  ];
  for constant in constants {
    let source = format!("{constant}\ntest(\"a\", async ({{ page }}) => {{\n  await routeRobots(page)\n  await routeRobots(page)\n}})");
    let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/a.spec.ts", &source)])).unwrap_err());
    assert!(
      message.contains("tests/e2e/a.spec.ts:") && message.contains("function routeRobots("),
      "{constant}\n{message}"
    );
  }

  // a fixture answers once per test that uses it: refused in both spellings, also when it calls an answering helper
  let fixtures = [
    "export const testWithRobots = test.extend({\n  page: async ({ page }, use) => {\n    await routeCommand(page, \"get_patronus_robots\", () => [])\n    await use(page)\n  },\n})",
    "export const testWithRobots = test.extend<{ robots: () => void }>({\n  robots: async ({ page }, use) => {\n    await routeCommand(page, \"get_patronus_robots\", () => [])\n    await use(() => {})\n  },\n})",
    "function routeRobots(page: Page) {\n  return routeCommand(page, \"get_patronus_robots\", () => [])\n}\nexport const testWithRobots = test.extend({\n  page: async ({ page }, use) => {\n    await routeRobots(page)\n    await use(page)\n  },\n})",
  ];
  for fixture in fixtures {
    let source = format!("{fixture}\ntestWithRobots(\"a\", async ({{ page }}) => {{\n  await page.reload()\n}})");
    let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/a.spec.ts", &source)])).unwrap_err());
    assert!(
      message.contains("tests/e2e/a.spec.ts:") && message.contains("inside the fixture `testWithRobots`"),
      "{fixture}\n{message}"
    );
  }

  // a function declared inside a helper: its calls are not counted either
  let nested = r#"
    async function setup(page: Page) {
      async function robots() {
        await routeCommand(page, "get_patronus_robots", () => [])
      }
      await robots()
      await robots()
    }
    test("a", async ({ page }) => {
      await setup(page)
    })
"#;
  let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/a.spec.ts", nested)])).unwrap_err());
  assert!(
    message.contains("tests/e2e/a.spec.ts:3") && message.contains("function robots, declared inside"),
    "{message}"
  );

  let one = r#"
    async function setup(page: Page) {
      await routeCommand(page, "get_patronus_robots", () => [])
    }
"#;
  let other = "async function setup(page: Page) {}";
  let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/a.spec.ts", one), ("tests/e2e/b.spec.ts", other)])).unwrap_err());
  assert!(message.contains("the helper setup is declared in") && message.contains("b.spec.ts:1"), "{message}");
  // the same in one file: the second declaration is read too
  let twice = format!("{other}\n{one}\ntest(\"a\", async ({{ page }}) => {{\n  await setup(page)\n}})");
  let message = panic_message(&*std::panic::catch_unwind(|| answers_of(&[("tests/e2e/a.spec.ts", &twice)])).unwrap_err());
  assert!(
    message.contains("the helper setup is declared in") && message.contains("a.spec.ts:1") && message.contains("a.spec.ts:3"),
    "{message}"
  );
  // two declarations that answer nothing are fine
  assert_eq!(answers_of(&[("tests/e2e/a.spec.ts", other), ("tests/e2e/b.spec.ts", other)]), []);
}
