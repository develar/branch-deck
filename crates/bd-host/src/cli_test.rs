use pretty_assertions::assert_eq;

use super::*;

const SPEC: &str = include_str!("../../../spec/cli/host.spec.md");

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
  let bt = "bd-host run <alias> --head --nice 0 --no-outputs --fetch out/bt-runs --report-fd 3 -- <command…>";
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
    }
  );
  assert_eq!(
    options_of(&["bd-host", "run", "studio", "--", "make"]),
    RunOptions {
      working_tree: true,
      nice: Some(10),
      outputs: true,
      apply: false,
      repo_by_name: true,
    }
  );
}
