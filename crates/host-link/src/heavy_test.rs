use crate::config::RepoConfig;
use crate::heavy::{MAX_SCRIPT_BYTES, Ran, commands_of, script_commands, script_in};
use pretty_assertions::assert_eq;

/// The patterns of `heavy` of this repository that the cases name
fn config() -> RepoConfig {
  RepoConfig {
    heavy: Some(
      [
        "cargo nextest run*--workspace*",
        "pnpm e2e*",
        "pnpm lint",
        "pnpm lint:backend*",
        "pnpm exec playwright test*",
      ]
      .map(str::to_owned)
      .to_vec(),
    ),
    ..RepoConfig::default()
  }
}

fn words(line: &[&str]) -> Vec<String> {
  line.iter().map(|word| (*word).to_owned()).collect()
}

/// A checkout whose scripts run the whole E2E suite: `probe/run.sh` and `probe/env.sh` start with the line of a shell,
/// `probe/lib.sh` is sourced and has none; `probe/tool.py` is of another interpreter and `target/debug/brd` a binary
fn checkout(path: &str) -> Option<String> {
  let e2e = "pnpm e2e --reporter=line > e2e.log 2>&1\n";
  match path {
    "probe/run.sh" => Some(format!("#!/bin/zsh\nset -u\n{e2e}echo done\n")),
    "probe/env.sh" => Some(format!("#!/usr/bin/env bash -e\n{e2e}")),
    "probe/lib.sh" => Some(e2e.to_owned()),
    // read as a shell would read them, each would run the suite
    "probe/tool.py" => Some(format!("#!/usr/bin/python3\n{e2e}")),
    "target/debug/brd" => Some(format!("\u{7f}ELF\n{e2e}")),
    _ => None,
  }
}

/// A command of a script that was read
fn scripted(line: &[&str]) -> Ran {
  Ran::Command {
    words: words(line),
    scripted: true,
  }
}

/// A shell that a run starts with a script (`-c`, `-lc`, after `-o pipefail`) runs the commands of the script, split as
/// the shell splits it: a heavy command anywhere in it makes the run heavy, said as a pattern of the script
#[test]
fn a_heavy_command_in_the_script_of_a_shell_makes_the_run_heavy() {
  let config = config();
  let reason = |line: &[&str]| config.heavy_reason(&words(line), &checkout);
  for (line, expected) in [
    // the wrapper of 2026-11-15 that ran beside the heavy turn
    (
      &[
        "zsh",
        "-c",
        "pnpm install --frozen-lockfile >/dev/null 2>&1; pnpm e2e > e2e-c.log 2>&1; c=$?; pnpm e2e:webkit > e2e-w.log 2>&1",
      ][..],
      Some("`pnpm e2e*` of [host] in its script"),
    ),
    (
      &[
        "bash",
        "-lc",
        "set -u\necho \"== full chromium e2e\"\npnpm e2e --reporter=line 2>&1 | grep -v x | tail -n 400",
      ],
      Some("`pnpm e2e*` of [host] in its script"),
    ),
    (
      &["sh", "-c", "pnpm install && cargo nextest run --workspace --no-fail-fast"],
      Some("`cargo nextest run*--workspace*` of [host] in its script"),
    ),
    (
      &["/bin/zsh", "-o", "pipefail", "-c", "cd sub || exit 1; pnpm lint:backend"],
      Some("`pnpm lint:backend*` of [host] in its script"),
    ),
    (&["zsh", "-c", "for spec in a b; do pnpm e2e \"$spec\"; done"], Some("`pnpm e2e*` of [host] in its script")),
    (
      &["zsh", "-c", "if true; then nohup pnpm exec playwright test x.spec.ts; fi"],
      Some("`pnpm exec playwright test*` of [host] in its script"),
    ),
    (&["zsh", "-c", "log=$(pnpm e2e)"], Some("`pnpm e2e*` of [host] in its script")),
    (&["zsh", "-c", "(cd x && E2E_PORT=3031 pnpm e2e)"], Some("`pnpm e2e*` of [host] in its script")),
    // a redirection is no word of its command: `pnpm lint` matches whole
    (&["zsh", "-c", "pnpm lint > lint.log 2>&1"], Some("`pnpm lint` of [host] in its script")),
    (&["zsh", "-c", "time nice -n 10 pnpm lint"], Some("`pnpm lint` of [host] in its script")),
    // a shell in the script of a shell
    (&["bash", "-c", "zsh -c 'pnpm e2e tests/e2e/x.spec.ts'"], Some("`pnpm e2e*` of [host] in its script")),
    // a script file of the checkout
    (&["zsh", "probe/run.sh"], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "./probe/run.sh", "arg"], Some("`pnpm e2e*` of [host] in its script")),
    // an `env` before the shell, also by its path, and in a script
    (&["env", "FOO=1", "zsh", "-c", "pnpm e2e"], Some("`pnpm e2e*` of [host] in its script")),
    (&["/usr/bin/env", "bash", "-c", "pnpm e2e"], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", "env CI=1 bash -c \"pnpm e2e\""], Some("`pnpm e2e*` of [host] in its script")),
    // a script of the tree run by its path, whose first line names a shell, and a file it sources
    (&["./probe/run.sh"], Some("`pnpm e2e*` of [host] in its script")),
    (&["probe/env.sh", "x"], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", "./probe/run.sh"], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", ". ./probe/lib.sh"], Some("`pnpm e2e*` of [host] in its script")),
    (&["zsh", "-c", "source probe/lib.sh && echo sourced"], Some("`pnpm e2e*` of [host] in its script")),
    // a substitution in double quotes
    (&["bash", "-c", "out=\"$(pnpm e2e 2>&1)\"; echo \"$out\""], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", "out=\"`pnpm e2e`\""], Some("`pnpm e2e*` of [host] in its script")),
    // a `#` inside a word is no comment
    (&["bash", "-c", "echo a#b; pnpm e2e"], Some("`pnpm e2e*` of [host] in its script")),
    // a shell nested past the depth that is read may run anything
    (
      &["sh", "-c", "sh -c 'sh -c \"sh -c \\\"sh -c echo\\\"\"'"],
      Some("its script `sh -c echo`, which brd-host cannot read for `heavy` of [host]"),
    ),
    // the command itself, with its wrappers, read from no script
    (&["pnpm", "e2e", "tests/e2e/x.spec.ts"], Some("`pnpm e2e*` of [host]")),
    (&["nohup", "pnpm", "e2e"], Some("`pnpm e2e*` of [host]")),
    (&["nice", "-n", "5", "pnpm", "e2e"], Some("`pnpm e2e*` of [host]")),
    (&["timeout", "7200", "pnpm", "e2e"], Some("`pnpm e2e*` of [host]")),
  ] {
    assert_eq!(reason(line), expected.map(str::to_owned), "{line:?}");
  }
}

/// A script that runs no heavy command is no heavy run: text in quotes is the word of its command, a light Rust run, an
/// interactive shell and a shell that reads its input run nothing heavy; a table without `heavy` makes nothing heavy
#[test]
fn a_script_without_a_heavy_command_and_a_table_without_heavy_make_no_heavy_run() {
  let config = config();
  let reason = |line: &[&str]| config.heavy_reason(&words(line), &checkout);
  for line in [
    &["zsh", "-c", "echo 'pnpm e2e'; printf '%s' \"cargo nextest run --workspace\""][..],
    &["zsh", "-c", "cargo nextest run -p host-link -E 'test(/heavy/)'"],
    &["bash", "-c", "echo hi # pnpm e2e"],
    &["zsh", "-l"],
    &["bash", "-s"],
    &["bash", "-s", "--", "x"],
    &["./community/tools/bt.cmd", "FooTest"],
    // a binary of the tree, a script of another interpreter, a file sourced from elsewhere, `env -i` as its own command
    &["./target/debug/brd", "sync"],
    &["./probe/tool.py"],
    &["zsh", "-c", "source ~/.cargo/env; cargo build"],
    &["env", "-i", "zsh", "-c", "pnpm e2e"],
  ] {
    assert_eq!(reason(line), None, "{line:?}");
  }
  let without = RepoConfig::default();
  assert_eq!(without.heavy_reason(&words(&["zsh", "-c", "pnpm e2e"]), &checkout), None);
  assert_eq!(without.heavy_reason(&words(&["zsh", "elsewhere.sh"]), &|_: &str| None), None);
}

/// A script file that the checkout cannot give (absent, outside of it, too large) may run anything: the run is heavy, and
/// says which script it could not read
#[test]
fn a_script_file_the_checkout_cannot_give_makes_the_run_heavy() {
  let config = config();
  assert_eq!(
    config.heavy_reason(&words(&["zsh", "probe/other.sh"]), &checkout),
    Some("its script `probe/other.sh`, which brd-host cannot read for `heavy` of [host]".to_owned())
  );
  // a heavy command found first names the run
  assert_eq!(
    config.heavy_reason(&words(&["bash", "-c", "zsh gone.sh; pnpm e2e"]), &checkout),
    Some("`pnpm e2e*` of [host] in its script".to_owned())
  );
}

/// The script splits at the separators of the shell outside quotes, into words without their quotes; a redirection and
/// its target are no words, a comment ends its line, a backslash escapes one character and joins a line to the next
#[test]
fn a_script_splits_into_its_commands_and_their_words_as_a_shell_splits_it() {
  let commands = |script: &str| script_commands(script);
  assert_eq!(
    commands("a 'b c' \"d \\\"e\\\"\" f\\ g; h && i || j | k & l\nm"),
    vec![
      words(&["a", "b c", "d \"e\"", "f g"]),
      words(&["h"]),
      words(&["i"]),
      words(&["j"]),
      words(&["k"]),
      words(&["l"]),
      words(&["m"])
    ]
  );
  assert_eq!(
    commands("x > out.log 2>&1; y 2> err; z >&2 <in; w &> both"),
    vec![words(&["x"]), words(&["y"]), words(&["z"]), words(&["w"])]
  );
  assert_eq!(commands("a # b; c\nd \\\n e"), vec![words(&["a"]), words(&["d", "e"])]);
  assert_eq!(commands("v=$(w x) `y` (z)"), vec![words(&["v="]), words(&["w", "x"]), words(&["y"]), words(&["z"])]);
  assert_eq!(commands("say ''"), vec![words(&["say", ""])]);
}

/// The commands a run runs, as the classification reads them: the keywords and wrappers before a command are no word of
/// it, and a shell stands for what it runs
#[test]
fn a_shell_stands_for_the_commands_it_runs() {
  assert_eq!(
    commands_of(&words(&["zsh", "-euc", "do exec pnpm e2e; timeout --signal=KILL 60 cargo nextest run"]), &|_: &str| None),
    vec![scripted(&["pnpm", "e2e"]), scripted(&["cargo", "nextest", "run"])]
  );
  assert_eq!(commands_of(&words(&["sh", "-c", "a", "zero", "one"]), &|_: &str| None), vec![scripted(&["a"])]);
  assert_eq!(
    commands_of(&words(&["bash", "--norc", "--", "s.sh"]), &|_: &str| None),
    vec![Ran::Unread("s.sh".to_owned())]
  );
}

/// The client reads a script file from the working tree it ships, as the slot holds it: inside of it only, and no
/// larger than `MAX_SCRIPT_BYTES`
#[test]
fn a_script_file_is_read_from_the_working_tree_inside_it_and_within_its_size() {
  let root = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(root.path().join("probe")).unwrap();
  std::fs::write(root.path().join("probe/run.sh"), "pnpm e2e\n").unwrap();
  std::fs::write(root.path().join("large.sh"), "#".repeat(usize::try_from(MAX_SCRIPT_BYTES).unwrap() + 1)).unwrap();
  assert_eq!(script_in(root.path(), "probe/run.sh").as_deref(), Some("pnpm e2e\n"));
  for path in ["large.sh", "absent.sh", "../probe/run.sh", "/etc/hosts", ""] {
    assert_eq!(script_in(root.path(), path), None, "{path}");
  }
  let read = |path: &str| script_in(root.path(), path);
  assert_eq!(
    config().heavy_reason(&words(&["zsh", "./probe/run.sh"]), &read),
    Some("`pnpm e2e*` of [host] in its script".to_owned())
  );
}
