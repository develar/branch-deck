use super::{ConfigError, EnvValue, ReadError, RepoConfig, Step, is_inner_path, parse_branchdeck, read_commit_config};
use pretty_assertions::assert_eq;
use std::cell::RefCell;
use std::convert::Infallible;

/// The example of openspec/specs/cli/host/spec.md, with one more step and one more output, and a table of another area
const EXAMPLE: &str = r#"
[lease]
clean = ["target/debug"]

[host]
outputs = ["tests/results", "playwright-report", "target/nextest/ci/junit.xml"]
heavy = ["cargo nextest run*--workspace*", "pnpm e2e*"]

[host.env]
PATH = { prepend = ["~/.cargo/bin"] }
E2E_PORT = { port = 3100 }
RUST_BACKTRACE = "1"
CI = { unset = true }

[[host.prepare]]
name = "pnpm install"
run = ["pnpm", "install", "--frozen-lockfile"]
inputs = ["pnpm-lock.yaml", "pnpm-workspace.yaml"]
creates = ["node_modules"]

[[host.prepare]]
run = ["./gradlew", "--version"]
"#;

fn example() -> RepoConfig {
  RepoConfig {
    outputs: vec!["tests/results".to_owned(), "playwright-report".to_owned(), "target/nextest/ci/junit.xml".to_owned()],
    heavy: Some(vec!["cargo nextest run*--workspace*".to_owned(), "pnpm e2e*".to_owned()]),
    env: vec![
      ("CI".to_owned(), EnvValue::Unset),
      ("E2E_PORT".to_owned(), EnvValue::Port(3100)),
      (
        "PATH".to_owned(),
        EnvValue::List {
          prepend: vec!["~/.cargo/bin".to_owned()],
          append: vec![],
        },
      ),
      ("RUST_BACKTRACE".to_owned(), EnvValue::Text("1".to_owned())),
    ],
    prepare: vec![
      Step {
        name: "pnpm install".to_owned(),
        run: vec!["pnpm".to_owned(), "install".to_owned(), "--frozen-lockfile".to_owned()],
        inputs: vec!["pnpm-lock.yaml".to_owned(), "pnpm-workspace.yaml".to_owned()],
        creates: vec!["node_modules".to_owned()],
      },
      Step {
        name: "./gradlew --version".to_owned(),
        run: vec!["./gradlew".to_owned(), "--version".to_owned()],
        inputs: vec![],
        creates: vec![],
      },
    ],
  }
}

#[test]
fn the_host_table_of_branchdeck_toml_parses_and_an_unknown_key_refuses() {
  assert_eq!(parse_branchdeck(EXAMPLE).expect("the example"), Some(example()));
  assert_eq!(parse_branchdeck("[lease]\nclean = []\n").expect("no host table"), None);
  assert_eq!(parse_branchdeck("").expect("an empty file"), None);
  assert_eq!(parse_branchdeck("[host]\n").expect("an empty host table"), Some(RepoConfig::default()));
  // the other tables are not the agent's: an unknown key there is no refusal of a run
  assert_eq!(parse_branchdeck("[lease]\nunknown = 1\n[host]\n").expect("another table"), Some(RepoConfig::default()));

  for (text, said) in [
    ("[host]\nprepar = []\n", ".branchdeck.toml [host]: unknown key `prepar`"),
    (
      "[[host.prepare]]\nrun = [\"true\"]\ninput = [\"a\"]\n",
      ".branchdeck.toml [host]: unknown key `prepare.input`",
    ),
    ("host = 1\n", ".branchdeck.toml: `host` is a integer, not a table"),
    ("not toml", ".branchdeck.toml: "),
  ] {
    let message = parse_branchdeck(text).expect_err(text).to_string();
    assert!(message.starts_with(said), "{text}: {message}");
  }
}

/// A key that this build does not know refuses with a message that names it, says it can be a key of a later Branch
/// Deck, and names the updates of both ends, so the next key of `[host]` is no dark refusal of an older brd-host
#[test]
fn an_unknown_key_names_itself_the_keys_of_this_build_and_the_update_of_brd_host() {
  let message = parse_branchdeck("[host]\nheavy = [\"x\"]\nlater = true\n").expect_err("a later key").to_string();
  assert_eq!(
    message,
    ".branchdeck.toml [host]: unknown key `later`: a typo, or a key of a later Branch Deck than this brd-host, which knows `outputs`, `heavy`, `env`, `prepare`; `brd install` updates brd-host on this Mac, and `brd-host upgrade <alias>` the agent of a host"
  );
  let step = parse_branchdeck("[[host.prepare]]\nrun = [\"true\"]\nlater = 1\n")
    .expect_err("a later key of a step")
    .to_string();
  assert!(
    step.contains("unknown key `prepare.later`") && step.contains("which knows `name`, `run`, `inputs`, `creates`"),
    "{step}"
  );
  let env = parse_branchdeck("[host.env]\nA = { later = 1 }\n").expect_err("a later key of a variable").to_string();
  assert!(env.contains("unknown key `env.A.later`") && env.contains("`brd-host upgrade <alias>`"), "{env}");
}

/// The keys that the check knows are the fields of the serde types, each in the order of its struct; `heavy`, even
/// empty, needs an agent with the feature `heavy`, and a table without it needs none
#[test]
fn the_known_keys_are_the_fields_of_serde_and_heavy_needs_its_feature() {
  use super::{EnvTable, RawConfig, fields_of};
  assert_eq!(fields_of::<RawConfig>(), ["outputs", "heavy", "env", "prepare"]);
  assert_eq!(fields_of::<EnvTable>(), ["prepend", "append", "port", "unset"]);
  assert_eq!(fields_of::<Step>(), ["name", "run", "inputs", "creates"]);
  let needs = |text: &str| parse_branchdeck(text).expect(text).expect("a host table").needs_features();
  assert_eq!(needs("[host]\nheavy = []\n"), ["heavy"]);
  assert_eq!(needs("[host]\nheavy = [\"pnpm e2e*\"]\n"), ["heavy"]);
  assert_eq!(needs("[host]\noutputs = [\"out\"]\n"), Vec::<&str>::new());
}

/// A pattern of `heavy` matches the words of the command joined by one space, whole, `*` any characters, after a
/// leading `env` and its assignments; the first pattern that matches names the run
#[test]
fn a_heavy_pattern_matches_the_whole_command_line_after_env_and_its_assignments() {
  let config = RepoConfig {
    heavy: Some(
      ["cargo nextest run*--workspace*", "pnpm e2e*", "pnpm test", "*bt.cmd --lane *", "pnpm *"]
        .map(str::to_owned)
        .to_vec(),
    ),
    ..RepoConfig::default()
  };
  let entry = |line: &str| config.heavy_entry(&line.split(' ').map(str::to_owned).collect::<Vec<_>>()).map(str::to_owned);
  for (line, expected) in [
    ("cargo nextest run --workspace", Some("cargo nextest run*--workspace*")),
    ("cargo nextest run --no-fail-fast --workspace -R latest", Some("cargo nextest run*--workspace*")),
    ("cargo nextest run -p host-link", None),
    ("cargo nextest run", None),
    ("pnpm e2e", Some("pnpm e2e*")),
    ("pnpm e2e-up", Some("pnpm e2e*")),
    ("env DEBUG_E2E=1 E2E_PORT=3031 pnpm e2e tests/e2e/x.spec.ts", Some("pnpm e2e*")),
    ("E2E_PORT=3031 pnpm e2e", Some("pnpm e2e*")),
    ("env -i pnpm e2e", None),
    ("pnpm test", Some("pnpm test")),
    ("pnpm test:unit", Some("pnpm *")),
    ("./community/tools/bt.cmd --lane fast", Some("*bt.cmd --lane *")),
    ("./community/tools/bt.cmd AgentTest", None),
    ("npm test", None),
  ] {
    assert_eq!(entry(line), expected.map(str::to_owned), "{line}");
  }
  assert_eq!(RepoConfig::default().heavy_entry(&["pnpm".to_owned(), "e2e".to_owned()]), None, "no pattern, no heavy run");
}

/// A pattern matches its literal start and end, and the two never share a character of the text
#[test]
fn a_pattern_matches_its_literal_ends_and_never_lets_them_overlap() {
  use super::glob_matches;
  assert!(glob_matches("*--workspace", "cargo nextest run --workspace"));
  assert!(!glob_matches("*--workspace", "cargo nextest run --workspace -R latest"), "the end is literal");
  assert!(glob_matches("ab*ba", "abba"));
  assert!(!glob_matches("ab*ba", "aba"), "the start and the end overlap");
  assert!(glob_matches("a*b*c", "a-b-c") && !glob_matches("a*b*c", "a-c-b"));
  assert!(glob_matches("*", "") && glob_matches("pnpm e2e", "pnpm e2e") && !glob_matches("pnpm e2e", "pnpm e2e x"));
}

/// The configuration of a commit whose `.branchdeck.toml` is `branchdeck`, and the names of the files that the reader
/// got
fn read_files(branchdeck: Option<&str>) -> (Result<RepoConfig, ConfigError>, Vec<String>) {
  let asked = RefCell::new(Vec::new());
  let config = read_commit_config(|file| {
    asked.borrow_mut().push(file.to_owned());
    Ok::<_, Infallible>(branchdeck.map(str::to_owned))
  })
  .map_err(|error| match error {
    ReadError::Config(error) => error,
    ReadError::Read(never) => match never {},
  });
  (config, asked.into_inner())
}

#[test]
fn a_commit_gets_the_host_table_of_its_branchdeck_toml_else_the_default() {
  assert_eq!(read_files(Some(EXAMPLE)), (Ok(example()), vec![".branchdeck.toml".to_owned()]));
  assert_eq!(read_files(Some("[lease]\n")).0, Ok(RepoConfig::default()), "no host table");
  assert_eq!(read_files(None).0, Ok(RepoConfig::default()), "no .branchdeck.toml");
  let refused = read_files(Some("[[host.prepar]]\n")).0.expect_err("a typo in the host table");
  assert!(refused.to_string().starts_with(".branchdeck.toml [host]: "), "{refused}");
}

#[test]
fn a_typo_or_a_bad_value_is_an_error_that_names_the_file() {
  for (text, said) in [
    ("[[host.prepar]]\nrun = [\"true\"]\n", "unknown key `prepar`"),
    ("[[host.prepare]]\nrun = [\"true\"]\ninput = [\"a\"]\n", "unknown key `prepare.input`"),
    ("[[host.prepare]]\nrun = []\n", "no command"),
    ("[host]\noutputs = [\"../up\"]\n", "not a path inside the slot"),
    ("[host]\noutputs = [\"/abs\"]\n", "not a path inside the slot"),
    ("[[host.prepare]]\nrun = [\"true\"]\ncreates = [\"a/../../b\"]\n", "not a path inside the slot"),
    ("[host.env]\nA = 1\n", "env.A is a integer"),
    ("[host.env]\nA = { port = 1, unset = true }\n", "env.A takes one of"),
    ("[host.env]\nA = { unset = false }\n", "env.A takes one of"),
    ("[host.env]\nA = { prepand = [\"x\"] }\n", "unknown key `env.A.prepand`"),
    ("[host]\nheavy = [\"pnpm e2e*\", \" \"]\n", "`heavy` has an empty pattern"),
    ("[host]\nheavy = \"pnpm e2e*\"\n", "invalid type"),
    ("[host.env]\nA = { port = 70000 }\n", "env.A"),
    ("[host]\noutputs = \"out\"\n", "invalid type"),
  ] {
    let message = parse_branchdeck(text).expect_err(text).to_string();
    assert!(message.contains(said), "{text}: {message}");
    assert!(message.starts_with(".branchdeck.toml [host]: "), "{message}");
  }
}

#[test]
fn an_inner_path_stays_inside_its_base() {
  assert!(is_inner_path("out/bt-runs"));
  assert!(is_inner_path("out"));
  assert!(!is_inner_path(""));
  assert!(!is_inner_path("/out"));
  assert!(!is_inner_path("out/../.."));
  assert!(!is_inner_path("./out"));
}

/// The `[host]` of this repository parses, and it names the outputs and the prepare steps of its runs, and its heavy
/// runs: the whole Rust suite however it is written (a filter or a rerun without `-p` builds all of it), clippy of the
/// workspace, every E2E and Playwright run, the measurements and the release build, and each script of package.json
/// that runs one of them, also as `pnpm run`. Every script is named heavy or light here, so a new one is classified
/// before it runs on a host
#[test]
fn the_host_table_of_this_repository_parses() {
  let host = parse_branchdeck(include_str!("../../../.branchdeck.toml"))
    .expect("the .branchdeck.toml of the repository")
    .expect("a [host] table");
  assert!(!host.outputs.is_empty() && !host.prepare.is_empty(), "{host:?}");
  let heavy = |line: &str| host.heavy_entry(&line.split(' ').map(str::to_owned).collect::<Vec<_>>()).is_some();
  for line in [
    "cargo nextest run",
    "cargo nextest run --no-fail-fast",
    "cargo nextest run --workspace",
    "cargo nextest run --no-fail-fast --workspace -R latest",
    "cargo nextest run -E test(/heavy/)",
    "cargo nextest run --no-fail-fast -E test(/heavy/)",
    "cargo nextest run -R latest",
    "cargo clippy --workspace --all-targets --features branch-deck/auto-update -- -D warnings",
    "pnpm exec playwright test tests/e2e/branch-row.spec.ts",
    "npx playwright test tests/e2e/x.spec.ts",
    "pnpm playwright test -c playwright.webkit.config.ts",
    "pnpm -C . e2e tests/e2e/x.spec.ts",
    "pnpm --dir . e2e",
    "env DEBUG_E2E=1 pnpm e2e tests/e2e/x.spec.ts",
    "pnpm tauri build --bundles app",
    "pnpm run tauri build --bundles app",
  ] {
    assert!(heavy(line), "{line}");
  }
  for line in [
    "cargo nextest run -p host-link",
    "cargo nextest run --no-fail-fast -p host-link",
    "cargo nextest run -p host-link -E test(/heavy/)",
    "cargo clippy -p host-link",
    "pnpm tauri dev",
    "npx playwright install webkit",
    "pnpm -C . test:unit",
  ] {
    assert!(!heavy(line), "{line}");
  }
  let package: serde_json::Value = serde_json::from_str(include_str!("../../../package.json")).expect("package.json");
  let heavy_scripts = [
    "test:backend",
    "e2e",
    "e2e-up",
    "e2e:webkit",
    "test",
    "lint:backend",
    "fix:backend",
    "lint",
    "perf:bd-rebuild",
    "perf:monorepo",
    "perf:test-processes",
    "perf:diff",
    "perf:eager-js",
    "perf:table",
    "screenshots:readme",
    "install-app",
  ];
  let light_scripts = [
    "dev",
    "build",
    "build:air",
    "test:unit",
    "test:perf",
    "setup:e2e",
    "tauri",
    "test-server:dev",
    "lint:js",
    "lint:typecheck",
    "lint:frontend",
    "lint:cpd",
    "lint:specs",
    "lint:arch",
    "lint:e2e-pins",
    "update-deps",
    "update-gh-deps",
  ];
  let scripts = package["scripts"].as_object().expect("the scripts of package.json");
  for script in scripts.keys() {
    let expected = match (heavy_scripts.contains(&script.as_str()), light_scripts.contains(&script.as_str())) {
      (true, false) => true,
      (false, true) => false,
      _ => panic!("name the script {script} of package.json heavy or light here, and add its heavy patterns to .branchdeck.toml"),
    };
    for line in [format!("pnpm {script}"), format!("pnpm run {script}")] {
      assert_eq!(heavy(&line), expected, "{line}");
    }
  }
}
