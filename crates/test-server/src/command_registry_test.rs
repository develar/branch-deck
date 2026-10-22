//! The three registries of the Tauri commands, held together: the commands of the app (`collect_commands!` in
//! `src-tauri/src/lib.rs`), the routes of this server (`/invoke/<name>` in `lib.rs`) and the handlers of the page mock
//! (`COMMAND_HANDLERS` in `tests/e2e/setup/tauri-mocks.ts`). The mock proxies every command it does not handle to this
//! server, and a command without a route answers 405, which fails the test ("Unimplemented Tauri command"). So every
//! command either has a route or is named in [`PAGE_MOCKED`] with the reason it never reaches Rust in an E2E test.
//! A fourth source, the E2E tests (`tests/e2e/**/*.ts`), is held by a ratchet: a test answers a command in the page
//! only where [`ROUTED_IN_PAGE`] or [`PUSHES_IN_PAGE`] pin it, never a write of [`MUTATING`].
//! The sources are read as text, as the git scan tests read theirs (`source_scan_test.rs` of git-executor).

use std::collections::BTreeSet;
use std::path::Path;

/// The number of commands of `collect_commands!`: a new command gets a route (or, with a reason, a [`PAGE_MOCKED`]
/// entry) and raises this number; it also proves the parse found the whole macro
const COMMAND_COUNT: usize = 112;

const SUB_WINDOW: &str = "a sub-window is a page of its own: the page mock records the call and the test opens the page";

/// The commands that have no route of this server, each with its reason. The number only goes down: a command moves
/// out of here when the server answers it from a fake of its service (`s0b-remote-bridge` of rust-domain-authority.md)
const PAGE_MOCKED: &[(&str, &str)] = &[
  // the handlers of `COMMAND_HANDLERS` in tauri-mocks.ts: the windows and the window menu
  ("open_sub_window", SUB_WINDOW),
  ("sub_window_loaded", SUB_WINDOW),
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
  // Patronus
  "start_patronus_robot",
  "restart_patronus_robot",
  "cancel_patronus_robot",
  "post_patronus_command",
  // GitHub
  "create_pull_request",
  "retarget_pull_request",
  "close_pull_request",
  "request_pull_request_reviewers",
  "submit_pull_request_review",
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
  "set_youtrack_issue_state",
  // git: the pushes, the fetches, the sync and the rewrites
  "push_branch",
  "push_branches",
  "fetch_review_head",
  "cancel_review_fetch",
  "prune_review_refs",
  "fetch_baseline",
  "sync_branches",
  "update_project",
  "move_commits_to_branch",
  "rename_branch",
  "restack_branch",
  "park_branches",
  "unpark_branches",
  "undo_regroup",
  "add_issue_reference_to_commits",
  "reword",
  "squash_commits",
  "squash_fixup_commits",
  "restore_branch_head",
  "amend_uncommitted_to_branch",
  "apply_conflict_fix",
  "cancel_conflict_fix",
  "unapply_branch",
  "undo_unapply_branch",
  "reapply_branch",
  "undo_reapply_branch",
  "delete_unapplied_branch",
  "delete_archived_branch",
  "dismiss_landed",
  // the app
  "set_repository_config",
  "set_auto_sync_menu_checked",
  "install_agent_cli",
  "install_update",
  "restart_app",
];

/// The read commands that E2E tests still answer in the page, with the number of answers in `tests/e2e`: a state that
/// neither the fixtures of the fakes nor the templates make yet (a robot started by a `/patronus` comment, a running
/// robot's details with the build progress, a refused landing gate). The numbers only go down: a new answer of a
/// command in the page fails, so it comes from a fixture of the fake; a removed one lowers its number
const ROUTED_IN_PAGE: &[(&str, usize)] = &[
  ("agent_cli_status", 1),
  ("check_landing_gate", 1),
  ("check_target_conflict", 1),
  ("get_branch_review_readiness", 1),
  ("get_patronus_failing_robots", 1),
  ("get_patronus_robot_details", 2),
  ("get_patronus_robots", 4),
  ("get_repository_config", 2),
  ("poll_patronus_robots", 1),
  ("prefetch_branch_review", 1),
];

/// The pushes that E2E tests answer in the page (`routePush`, `routePushBranches` of push-helpers.ts and two answers in
/// patronus-actions.spec.ts): the templates name the repository itself as `origin` (`.`), so a test of a rejected or
/// moved remote, or of the progress, has no remote to push to. The one exception to [`MUTATING`]; the numbers only go
/// down
const PUSHES_IN_PAGE: &[(&str, usize)] = &[("push_branch", 3), ("push_branches", 1)];

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

/// The top-level keys of `COMMAND_HANDLERS` in the page mock, plugin prefixes (`plugin:store`) included
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
    handlers.iter().any(|key| key == "plugin:store"),
    "the parse of COMMAND_HANDLERS found no `plugin:store`: {handlers:?}"
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
      "get_file_diffs",
      "get_file_merges",
      "save_youtrack_token",
      "set_repository_config",
      "sign_out_of_github",
      "sign_out_of_youtrack",
      "sync_branches",
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
    4,
    "PAGE_MOCKED may only shrink: a new command gets a route of the test server, not a page mock; when a command gets its route, lower this number"
  );
}

#[test]
fn the_page_handler_parser_reads_every_entry_form_it_knows_and_refuses_another() {
  let known = r#"
  const COMMAND_HANDLERS: Record<string, (cmd: string, payload: any) => any> = {
    // a comment between the entries
    "plugin:store": handleStoreCommand,
    "plugin:webview": (cmd: string, payload: any) => {
      return null
    },
    get_zoom: () => 1,
  }
"#;
  assert_eq!(page_handlers_of(known), ["plugin:store", "plugin:webview", "get_zoom"]);

  let shorthand = r#"
  const COMMAND_HANDLERS: Record<string, (cmd: string, payload: any) => any> = {
    "plugin:store": handleStoreCommand,
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

/// The commands that `text` answers in the page, one entry per answer: each `routeCommand(page, "<name>", …)` and each
/// `.route("**/invoke/<name>", …)` whose handler fulfills the request without the server's answer (`route.fetch()`
/// runs the command on the server: the rewrite of its answer is no page answer). A `routeCommand` or a route with a
/// computed name fails (it would hide the command from the ratchet); a computed route is only the helpers' own
/// (`wait-helpers.ts`)
fn page_answers_of(file: &str, text: &str) -> Vec<String> {
  let mut answers = Vec::new();
  for (at, _) in text.match_indices("routeCommand(") {
    if text[..at].ends_with("function ") {
      continue;
    }
    let arguments = &text[at + "routeCommand(".len()..];
    let name = arguments
      .split_once(',')
      .and_then(|(_, rest)| string_literal(rest.trim_start()))
      .unwrap_or_else(|| panic!("{file}:{}: routeCommand names its command by a string literal", line_of(text, at)));
    answers.push(name.to_owned());
  }
  for (at, _) in text.match_indices(".route(") {
    let call = call_text(text, at + ".route".len());
    let target = call[1..].split(',').next().unwrap_or_default().trim();
    let Some(after_invoke) = target.split_once("/invoke/").map(|(_, name)| name) else {
      continue;
    };
    if !call.contains(".fulfill(") || call.contains(".fetch(") {
      // a hold or a record goes on to the server; a rewrite of the server's answer (`rewriteNextSyncEvents`) ran there
      continue;
    }
    let name = after_invoke.strip_suffix('"').filter(|name| is_identifier(name));
    let name = name.unwrap_or_else(|| {
      assert!(
        file.ends_with("tests/e2e/helpers/wait-helpers.ts"),
        "{file}:{}: a route of `/invoke/` names its command by a string literal",
        line_of(text, at)
      );
      ""
    });
    if !name.is_empty() {
      answers.push(name.to_owned());
    }
  }
  answers
}

/// The content of the string literal `"…"` at the start of `text`
fn string_literal(text: &str) -> Option<&str> {
  let quoted = text.strip_prefix('"')?;
  quoted.split_once('"').map(|(content, _)| content)
}

fn line_of(text: &str, at: usize) -> usize {
  text[..at].matches('\n').count() + 1
}

/// The text of the call whose argument list opens at `open` (a `(`), through its closing parenthesis; parentheses in
/// string literals do not count
fn call_text(text: &str, open: usize) -> &str {
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
      '(' => depth += 1,
      ')' => {
        depth -= 1;
        if depth == 0 {
          return &text[open..=open + offset];
        }
      }
      _ => {}
    }
  }
  panic!("a call without its closing parenthesis");
}

/// How often the E2E tests answer each command in the page
fn page_answers() -> std::collections::BTreeMap<String, usize> {
  let mut counts = std::collections::BTreeMap::new();
  for (file, text) in e2e_sources() {
    for name in page_answers_of(&file, &text) {
      *counts.entry(name).or_insert(0) += 1;
    }
  }
  counts
}

#[test]
fn no_e2e_test_answers_a_write_in_the_page() {
  let answers = page_answers();
  let pushes: BTreeSet<&str> = PUSHES_IN_PAGE.iter().map(|(name, _)| *name).collect();
  let written: Vec<(&String, &usize)> = answers
    .iter()
    .filter(|(name, _)| MUTATING.contains(&name.as_str()) && !pushes.contains(name.as_str()))
    .collect();
  assert!(
    written.is_empty(),
    "E2E tests answer these writes in the page: {written:?}; let the test server run them behind the fake of their service and assert the fake's log (a failure is `routeCommandError`)"
  );
}

#[test]
fn the_page_answers_of_the_e2e_tests_are_pinned() {
  let pinned: std::collections::BTreeMap<String, usize> = ROUTED_IN_PAGE.iter().chain(PUSHES_IN_PAGE).map(|(name, count)| ((*name).to_owned(), *count)).collect();
  let answers: std::collections::BTreeMap<String, usize> = page_answers()
    .into_iter()
    .filter(|(name, _)| !MUTATING.contains(&name.as_str()) || pinned.contains_key(name))
    .collect();
  assert_eq!(
    answers, pinned,
    "the commands that E2E tests answer in the page (routeCommand, a fulfilling page.route of /invoke/): a new answer comes from a fixture of the fake instead; for a removed one, lower its number in ROUTED_IN_PAGE or PUSHES_IN_PAGE"
  );
}

#[test]
fn the_ratchet_names_commands_of_the_app() {
  let commands: BTreeSet<String> = app_commands().into_iter().collect();
  let mutating: BTreeSet<&str> = MUTATING.iter().copied().collect();
  assert_eq!(mutating.len(), MUTATING.len(), "MUTATING names a command twice");
  for name in MUTATING.iter().chain(ROUTED_IN_PAGE.iter().chain(PUSHES_IN_PAGE).map(|(name, _)| name)) {
    assert!(commands.contains(*name), "the ratchet names `{name}`, which collect_commands! does not have");
  }
  for (name, _) in ROUTED_IN_PAGE {
    assert!(!mutating.contains(name), "`{name}` writes: it cannot be pinned in ROUTED_IN_PAGE");
  }
  for (name, _) in PUSHES_IN_PAGE {
    assert!(mutating.contains(name), "PUSHES_IN_PAGE names `{name}`, which MUTATING does not: pin it in ROUTED_IN_PAGE");
  }
}

#[test]
fn the_page_answer_scan_reads_routes_and_refuses_a_computed_name() {
  let text = r#"
    await routeCommand(page, "get_patronus_robots", () => [])
    const calls = await routeCommand(
      page,
      "get_patronus_robot_details",
      () => ({ status: "ok (done)" }),
    )
    await routeCommandError(page, "push_branch", "rejected")
    await page.route("**/invoke/push_branches", async route => {
      await route.fulfill({ json: { type: "pushed" } })
    })
    await page.route("**/invoke/get_branch_review_readiness", async route => {
      await held
      await route.fallback()
    })
    await page.route("**/invoke/sync_branches", async route => {
      const response = await route.fetch()
      await route.fulfill({ response, body: rewrite(await response.text()) })
    })
    export async function routeCommand(page: Page, command: string) {}
"#;
  assert_eq!(
    page_answers_of("tests/e2e/a.spec.ts", text),
    ["get_patronus_robots", "get_patronus_robot_details", "push_branches"]
  );

  let computed = "await routeCommand(page, command, () => null)";
  let failure = std::panic::catch_unwind(|| page_answers_of("tests/e2e/b.spec.ts", computed)).unwrap_err();
  let message = failure.downcast_ref::<String>().cloned().unwrap_or_default();
  assert!(message.contains("tests/e2e/b.spec.ts:1") && message.contains("string literal"), "{message}");
}
