use pretty_assertions::assert_eq;

use super::*;

const SPEC: &str = include_str!("../../../openspec/specs/cli/host/spec.md");

/// The options of a run of the command line `argv`
fn options_of(argv: &[&str]) -> RunOptions {
  let Cli { command: Command::Run(args) } = Cli::try_parse_from(argv).expect("a command line") else {
    panic!("not a run: {argv:?}");
  };
  run_options(&args)
}

/// The command line of bt in the spec gives the run options that bt needs: niceness 0, so a run of bt does
/// not yield the CPU, and no outputs, so the local copy of an output of `[host]` stays. A person gets niceness 10 and
/// the outputs
#[test]
fn the_command_line_of_bt_gives_niceness_0_and_no_outputs() {
  let bt = "brd-host run <alias> --head --nice 0 --no-outputs --fetch out/bt-runs --report-fd 3 -- <command…>";
  assert!(SPEC.contains(&format!("`{bt}`")), "the spec names the command line of bt");
  let bt = bt.replace("<alias>", "studio").replace("<command…>", "bt test");
  let argv: Vec<&str> = bt.split(' ').collect();
  assert_eq!(
    options_of(&argv),
    RunOptions {
      working_tree: false,
      nice: Some(0),
      outputs: false,
      apply: false,
      repo_by_name: true,
      heavy: false,
      exclusive: false,
    }
  );
  assert_eq!(
    options_of(&["brd-host", "run", "studio", "--", "make"]),
    RunOptions {
      working_tree: true,
      nice: Some(10),
      outputs: true,
      apply: false,
      repo_by_name: true,
      heavy: false,
      exclusive: false,
    }
  );
  assert!(options_of(&["brd-host", "run", "studio", "--heavy", "--", "make"]).heavy, "--heavy");
  assert!(options_of(&["brd-host", "run", "studio", "--exclusive", "--", "make"]).exclusive, "--exclusive");
}

/// The setup of the command line `argv`, or the error of clap
fn setup_of(argv: &[&str]) -> Result<SetupArgs, clap::Error> {
  Cli::try_parse_from(argv).map(|cli| match cli.command {
    Command::Setup(args) => args,
    command => panic!("not a setup: {command:?}"),
  })
}

/// `setup --local` takes an alias, `local` by default, and the options of `setup <alias>`; `--agent` goes alone
#[test]
fn setup_takes_an_alias_or_local_or_agent() {
  let local = setup_of(&["brd-host", "setup", "--local"]).expect("setup --local");
  assert_eq!((local.local, local.alias.as_deref(), local.port), (true, None, DEFAULT_PORT));
  let named = setup_of(&["brd-host", "setup", "--local", "laptop", "--repo", "~/projects/branch-deck", "--port", "7743"]).expect("an alias");
  assert_eq!((named.alias.as_deref(), named.repo.as_str(), named.port), (Some("laptop"), "~/projects/branch-deck", 7743));
  let ssh = setup_of(&["brd-host", "setup", "studio"]).expect("setup <alias>");
  assert_eq!((ssh.local, ssh.alias.as_deref()), (false, Some("studio")));
  let client = "ab".repeat(32);
  let agent = setup_of(&["brd-host", "setup", "--agent", "--client", &client, "--name", "x"]).expect("setup --agent");
  assert!(agent.agent && !agent.local);
  for argv in [
    &["brd-host", "setup"][..],
    &["brd-host", "setup", "--local", "--agent", "--client", &client, "--name", "x"],
    &["brd-host", "setup", "studio", "--agent", "--client", &client, "--name", "x"],
    &["brd-host", "setup", "--local", "--client", &client],
  ] {
    assert!(setup_of(argv).is_err(), "{argv:?}");
  }
}
