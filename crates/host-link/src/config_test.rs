use super::{EnvValue, RepoConfig, Step, is_inner_path, parse_branchdeck, parse_yonder, resolve};
use pretty_assertions::assert_eq;

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

#[test]
fn a_commit_without_the_host_table_gets_its_yonder_toml() {
  let yonder = "outputs = [\"out\"]\n";
  let from_yonder = RepoConfig {
    outputs: vec!["out".to_owned()],
    ..RepoConfig::default()
  };
  assert_eq!(resolve(Some(EXAMPLE), Some(yonder)).expect("the host table wins"), example());
  assert_eq!(resolve(Some("[lease]\n"), Some(yonder)).expect("no host table"), from_yonder);
  assert_eq!(resolve(None, Some(yonder)).expect("no .branchdeck.toml"), from_yonder);
  assert_eq!(resolve(None, None).expect("no file"), RepoConfig::default());
  assert_eq!(resolve(Some(EXAMPLE), Some("not toml")).expect("an unread .yonder.toml"), example());
  let refused = resolve(Some("[lease]\n"), Some("[[prepar]]\n")).expect_err("a typo in .yonder.toml");
  assert!(refused.to_string().starts_with(".yonder.toml: "), "{refused}");
}

#[test]
fn a_typo_or_a_bad_value_is_an_error_that_names_the_file() {
  for (text, said) in [
    ("[[prepar]]\nrun = [\"true\"]\n", "unknown field `prepar`"),
    ("[[prepare]]\nrun = [\"true\"]\ninput = [\"a\"]\n", "unknown field `input`"),
    ("[[prepare]]\nrun = []\n", "no command"),
    ("outputs = [\"../up\"]\n", "not a path inside the slot"),
    ("outputs = [\"/abs\"]\n", "not a path inside the slot"),
    ("[[prepare]]\nrun = [\"true\"]\ncreates = [\"a/../../b\"]\n", "not a path inside the slot"),
    ("[env]\nA = 1\n", "env.A is a integer"),
    ("[env]\nA = { port = 1, unset = true }\n", "env.A takes one of"),
    ("[env]\nA = { unset = false }\n", "env.A takes one of"),
    ("[env]\nA = { prepand = [\"x\"] }\n", "unknown field `prepand`"),
    ("[env]\nA = { port = 70000 }\n", "env.A"),
    ("outputs = \"out\"\n", "invalid type"),
    ("not toml", "key"),
  ] {
    let message = parse_yonder(text).expect_err(text).to_string();
    assert!(message.contains(said), "{text}: {message}");
    assert!(message.starts_with(".yonder.toml: "), "{message}");
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
