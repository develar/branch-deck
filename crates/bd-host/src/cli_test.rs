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

/// The command line of bt in the spec gives the run options that bt needs: the working tree, niceness 0, so a run of
/// bt does not yield the CPU, and no outputs, so the local copy of an output of `[host]` stays. A person gets niceness
/// 10 and the outputs
#[test]
fn the_command_line_of_bt_gives_niceness_0_and_no_outputs() {
  let bt = "brd-host run <alias> --nice 0 --no-outputs --fetch out/bt-runs --report-fd 3 -- <command…>";
  assert!(SPEC.contains(&format!("`{bt}`")), "the spec names the command line of bt");
  let bt = bt.replace("<alias>", "studio").replace("<command…>", "bt test");
  let argv: Vec<&str> = bt.split(' ').collect();
  assert_eq!(
    options_of(&argv),
    RunOptions {
      working_tree: true,
      nice: Some(0),
      outputs: false,
      apply: false,
      repo_by_name: true,
      heavy: false,
      exclusive: false,
      max: None,
      admit_now: false,
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
      max: None,
      admit_now: false,
    }
  );
  assert!(options_of(&["brd-host", "run", "studio", "--heavy", "--", "make"]).heavy, "--heavy");
  assert!(options_of(&["brd-host", "run", "studio", "--exclusive", "--", "make"]).exclusive, "--exclusive");
}

/// `--measure-as <class>` names the class of the measures of a run, which the client sends with it; it takes a value
#[test]
fn measure_as_names_the_class_of_the_measures() {
  let class_of = |argv: &[&str]| match Cli::try_parse_from(argv).map(|cli| cli.command) {
    Ok(Command::Run(args)) => Ok(args.measure_as),
    Ok(command) => panic!("not a run: {command:?}"),
    Err(error) => Err(error.kind()),
  };
  assert_eq!(
    class_of(&["brd-host", "run", "studio", "--measure-as", "accio-gate", "--", "bash", "-c", "./gate.sh 1a2b"]),
    Ok(Some("accio-gate".to_owned()))
  );
  assert_eq!(class_of(&["brd-host", "run", "studio", "--", "make"]), Ok(None));
  assert!(class_of(&["brd-host", "run", "studio", "--measure-as", "--", "make"]).is_err(), "a class is a value");
}

/// `--group <group>` names the group of a run in the heavy queue, and so does `BRD_HOST_GROUP`, which an orchestrator
/// sets once for all its agents; the flag wins over the variable, and an empty variable names none
#[test]
fn group_names_the_group_of_the_run_by_the_flag_or_the_environment() {
  let group_of = |argv: &[&str]| match Cli::try_parse_from(argv).map(|cli| cli.command) {
    Ok(Command::Run(args)) => args.group().map(str::to_owned),
    Ok(command) => panic!("not a run: {command:?}"),
    Err(error) => panic!("{error}"),
  };
  let flagged = ["brd-host", "run", "studio", "--group", "typing", "--", "make"];
  {
    let _unset = test_utils::env_vars::EnvVars::set_and_remove::<&str>(&[], &["BRD_HOST_GROUP".to_owned()]);
    assert_eq!(group_of(&flagged), Some("typing".to_owned()));
    assert_eq!(group_of(&["brd-host", "run", "studio", "--", "make"]), None, "the affinity is the group");
  }
  let _set = test_utils::env_vars::EnvVars::set(&[("BRD_HOST_GROUP", "session-7")]);
  assert_eq!(group_of(&["brd-host", "run", "studio", "--", "make"]), Some("session-7".to_owned()));
  assert_eq!(group_of(&flagged), Some("typing".to_owned()), "the flag wins");
  let _empty = test_utils::env_vars::EnvVars::set(&[("BRD_HOST_GROUP", "")]);
  assert_eq!(group_of(&["brd-host", "run", "studio", "--", "make"]), None, "an empty variable names no group");
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

/// `--max` takes seconds, or numbers with `h`, `m` and `s` in this order, from 1 s to 24 h, and refuses the rest
#[test]
fn max_takes_seconds_minutes_and_hours_and_refuses_the_rest() {
  for (text, seconds) in [("90", 90), ("90s", 90), ("9m", 540), ("1h", 3600), ("1h30m", 5400), ("2m5s", 125), ("24h", 86_400)] {
    assert_eq!(parse_max(text), Ok(Duration::from_secs(seconds)), "{text}");
  }
  for text in ["", "0", "0s", "25h", "m", "9x", "5s2m", "1m1m", "9m30", "-1m", "1.5h"] {
    assert!(parse_max(text).is_err(), "{text}");
  }
  let Cli {
    command: Command::Wait { alias, run, max },
  } = Cli::try_parse_from(["brd-host", "wait", "studio", "1-2", "--max", "5m"]).expect("a wait")
  else {
    panic!("not a wait");
  };
  assert_eq!((alias.as_str(), run.as_str(), max), ("studio", "1-2", Some(Duration::from_mins(5))));
  Cli::try_parse_from(["brd-host", "wait", "studio", "1-2", "--max", "10x"]).expect_err("no duration");
}

/// `cancel-unfinished` says one line per run, the cancelled ones first, and one line when the checkout has no such run;
/// `--max` takes a duration as `wait --max` does
#[test]
fn cancel_unfinished_says_a_line_per_run_and_takes_a_max() {
  use host_client::unfinished::{CancelledRun, UncancelledRun};
  assert_eq!(unfinished_lines(&Unfinished::default()), ["no run of this checkout waits for its end here"]);
  let answer = Unfinished {
    cancelled: vec![CancelledRun {
      run: "1-1".to_owned(),
      host: "studio".to_owned(),
      answer: "the supervisor of the run 1-1 has the cancel request".to_owned(),
    }],
    not_cancelled: vec![UncancelledRun {
      run: "2-2".to_owned(),
      host: "laptop".to_owned(),
      code: Code::TimedOut,
      message: "laptop did not answer the cancel within 5.0s".to_owned(),
    }],
  };
  assert_eq!(
    unfinished_lines(&answer),
    [
      "cancelled the run 1-1 on studio: the supervisor of the run 1-1 has the cancel request",
      "cannot cancel the run 2-2 on laptop: timed_out: laptop did not answer the cancel within 5.0s",
    ]
  );
  let Cli {
    command: Command::CancelUnfinished { max, json },
  } = Cli::try_parse_from(["brd-host", "cancel-unfinished", "--json", "--max", "5s"]).expect("a command line")
  else {
    panic!("not a cancel-unfinished");
  };
  assert_eq!((max, json), (Some(Duration::from_secs(5)), true));
}
