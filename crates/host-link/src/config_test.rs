use super::{ConfigError, EnvValue, ReadError, RepoConfig, Step, is_inner_path, parse_branchdeck, read_commit_config};
use pretty_assertions::assert_eq;
use std::cell::RefCell;
use std::convert::Infallible;

/// The example of spec/cli/host.spec.md, with one more step and one more output, and a table of another area
const EXAMPLE: &str = r#"
[lease]
clean = ["target/debug"]

[host]
outputs = ["tests/results", "playwright-report", "target/nextest/ci/junit.xml"]

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
    ("[host]\nprepar = []\n", ".branchdeck.toml [host]: unknown field `prepar`"),
    ("[[host.prepare]]\nrun = [\"true\"]\ninput = [\"a\"]\n", ".branchdeck.toml [host]: unknown field `input`"),
    ("host = 1\n", ".branchdeck.toml: `host` is a integer, not a table"),
    ("not toml", ".branchdeck.toml: "),
  ] {
    let message = parse_branchdeck(text).expect_err(text).to_string();
    assert!(message.starts_with(said), "{text}: {message}");
  }
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
    ("[[host.prepar]]\nrun = [\"true\"]\n", "unknown field `prepar`"),
    ("[[host.prepare]]\nrun = [\"true\"]\ninput = [\"a\"]\n", "unknown field `input`"),
    ("[[host.prepare]]\nrun = []\n", "no command"),
    ("[host]\noutputs = [\"../up\"]\n", "not a path inside the slot"),
    ("[host]\noutputs = [\"/abs\"]\n", "not a path inside the slot"),
    ("[[host.prepare]]\nrun = [\"true\"]\ncreates = [\"a/../../b\"]\n", "not a path inside the slot"),
    ("[host.env]\nA = 1\n", "env.A is a integer"),
    ("[host.env]\nA = { port = 1, unset = true }\n", "env.A takes one of"),
    ("[host.env]\nA = { unset = false }\n", "env.A takes one of"),
    ("[host.env]\nA = { prepand = [\"x\"] }\n", "unknown field `prepand`"),
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

/// The `[host]` of this repository parses, and it names the outputs and the prepare steps of its runs
#[test]
fn the_host_table_of_this_repository_parses() {
  let host = parse_branchdeck(include_str!("../../../.branchdeck.toml"))
    .expect("the .branchdeck.toml of the repository")
    .expect("a [host] table");
  assert!(!host.outputs.is_empty() && !host.prepare.is_empty(), "{host:?}");
}
