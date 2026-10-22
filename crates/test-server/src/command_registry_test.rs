//! The three registries of the Tauri commands, held together: the commands of the app (`collect_commands!` in
//! `src-tauri/src/lib.rs`), the routes of this server (`/invoke/<name>` in `lib.rs`) and the handlers of the page mock
//! (`COMMAND_HANDLERS` in `tests/e2e/setup/tauri-mocks.ts`). The mock proxies every command it does not handle to this
//! server, and a command without a route answers 405, which fails the test ("Unimplemented Tauri command"). So every
//! command either has a route or is named in [`PAGE_MOCKED`] with the reason it never reaches Rust in an E2E test.
//! The sources are read as text, as the git scan tests read theirs (`source_scan_test.rs` of git-executor).

use std::collections::BTreeSet;
use std::path::Path;

/// The number of commands of `collect_commands!`: a new command gets a route (or, with a reason, a [`PAGE_MOCKED`]
/// entry) and raises this number; it also proves the parse found the whole macro
const COMMAND_COUNT: usize = 111;

const SUB_WINDOW: &str = "a sub-window is a page of its own: the page mock records the call and the test opens the page";
const UPDATER: &str = "the test server has no updater: the page mock answers that the app is up to date (`NO_UPDATE`)";
const PATRONUS: &str = "Patronus: a test answers it with `routeCommand` (patronus-helpers.ts); unrouted it fails with 405";
const GITHUB: &str = "GitHub: a test answers it with `routeCommand` (pull-request-helpers.ts); unrouted it fails with 405";
const GITHUB_SIGN_IN: &str = "the GitHub device flow: a test answers it with `routeCommand`; unrouted it fails with 405";
const YOUTRACK: &str = "YouTrack: a test answers it with `routeCommand` (youtrack-helpers.ts); unrouted it fails with 405";
const NOT_TESTED: &str = "no E2E test installs an update or restarts the app: a call fails with 405";

/// The commands that have no route of this server, each with its reason. The number only goes down: a command moves
/// out of here when the server answers it from a fake of its service (`s0b-remote-bridge` of rust-domain-authority.md)
const PAGE_MOCKED: &[(&str, &str)] = &[
  // the handlers of `COMMAND_HANDLERS` in tauri-mocks.ts: windows, the window menu, the updater
  ("open_sub_window", SUB_WINDOW),
  ("sub_window_loaded", SUB_WINDOW),
  ("sub_window_ready", SUB_WINDOW),
  ("show_window_menu", "the window manager's menu: the page mock records it with the window calls"),
  ("check_for_updates", UPDATER),
  ("get_update_status", UPDATER),
  // reached only through `routeCommand` of a test
  ("start_patronus_robot", PATRONUS),
  ("restart_patronus_robot", PATRONUS),
  ("cancel_patronus_robot", PATRONUS),
  ("get_patronus_robot_status", PATRONUS),
  ("get_patronus_target_branch", PATRONUS),
  ("post_patronus_command", PATRONUS),
  ("read_patronus_command_reply", PATRONUS),
  ("create_pull_request", GITHUB),
  ("retarget_pull_request", GITHUB),
  ("close_pull_request", GITHUB),
  ("get_pull_request_head", GITHUB),
  ("get_new_pull_request", GITHUB),
  ("request_pull_request_reviewers", GITHUB),
  ("submit_pull_request_review", GITHUB),
  ("search_github_users", GITHUB),
  ("wait_for_github_branch", GITHUB),
  ("start_github_sign_in", GITHUB_SIGN_IN),
  ("wait_github_sign_in", GITHUB_SIGN_IN),
  ("cancel_github_sign_in", GITHUB_SIGN_IN),
  ("sign_out_of_github", GITHUB_SIGN_IN),
  ("start_youtrack_sign_in", YOUTRACK),
  ("wait_youtrack_sign_in", YOUTRACK),
  ("cancel_youtrack_sign_in", YOUTRACK),
  ("save_youtrack_token", YOUTRACK),
  ("sign_out_of_youtrack", YOUTRACK),
  ("get_youtrack_issue", YOUTRACK),
  ("set_youtrack_issue_state", YOUTRACK),
  ("install_update", NOT_TESTED),
  ("restart_app", NOT_TESTED),
];

/// The routes that name no command of `collect_commands!`, each with its reason
const RAW_ROUTES: &[(&str, &str)] = &[(
  "get_blob_syntax",
  "its answer is raw bytes, which the typed bindings cannot say: the app registers it with `generate_handler!` beside the typed commands",
)];

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
    ["get_file_diffs", "get_file_merges", "set_repository_config", "sync_branches"],
    "the handlers of COMMAND_HANDLERS that wrap a route"
  );
}

#[test]
fn page_mocked_only_goes_down() {
  assert_eq!(
    PAGE_MOCKED.len(),
    35,
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
