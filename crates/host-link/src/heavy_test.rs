use crate::config::RepoConfig;
use crate::heavy::{MAX_READS, MAX_SCRIPT_BYTES, Ran, commands_of, run_key, run_words, script_commands, script_in};
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
    // sourced, and a text without a first line `#!` that a shell runs itself
    "probe/lib.sh" | "probe/plain" => Some(e2e.to_owned()),
    // read as a shell would read them, each would run the suite
    "probe/tool.py" => Some(format!("#!/usr/bin/python3\n{e2e}")),
    // a binary holds a NUL, which no text a shell runs does
    "target/debug/brd" => Some(format!("\u{7f}ELF\u{2}\u{1}\u{0}\n{e2e}")),
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
    (&["./probe/plain"], Some("`pnpm e2e*` of [host] in its script")),
    // a file run by its path as no script, then by a shell, which runs it as one: read once, expanded by the shell
    (&["bash", "-c", "./probe/tool.py; bash ./probe/tool.py"], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", "./probe/run.sh"], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", ". ./probe/lib.sh"], Some("`pnpm e2e*` of [host] in its script")),
    (&["zsh", "-c", "source probe/lib.sh && echo sourced"], Some("`pnpm e2e*` of [host] in its script")),
    // a substitution in double quotes
    (&["bash", "-c", "out=\"$(pnpm e2e 2>&1)\"; echo \"$out\""], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", "out=\"`pnpm e2e`\""], Some("`pnpm e2e*` of [host] in its script")),
    // a `#` inside a word is no comment
    (&["bash", "-c", "echo a#b; pnpm e2e"], Some("`pnpm e2e*` of [host] in its script")),
    // a substitution in quotes ends at its own parenthesis: one in a subshell, one in quotes inside it, are not its end
    (&["bash", "-c", "x=\"$( (cd probe; ls); pnpm e2e)\""], Some("`pnpm e2e*` of [host] in its script")),
    (&["bash", "-c", "x=\"$(echo ')'; pnpm e2e)\""], Some("`pnpm e2e*` of [host] in its script")),
    // an `env` that keeps the environment, its `-u` too
    (&["env", "-u", "CI", "zsh", "-c", "pnpm e2e"], Some("`pnpm e2e*` of [host] in its script")),
    (&["env", "--unset=CI", "pnpm", "e2e"], Some("`pnpm e2e*` of [host]")),
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

/// A polyglot wrapper that runs itself again under bash (`exec /bin/bash "$0" "$@"`) runs no other script: `$0`,
/// `"$0"`, `${0}` in a script file is that file, expanded already, and in a sourced file the script that sources it.
/// The wrapper stays a run of what it names and of what it runs. `$0` outside the place of the script is a word. In the
/// script of `-c` it is the word after that script (the `$0` around it for `"$0"`), and without one the name of the
/// shell, which no more names a file than `$0` of the command of the run: a script that cannot be read
#[test]
fn dollar_zero_in_a_script_file_is_the_file_itself() {
  let mut config = config();
  config.heavy.get_or_insert_default().push("./tools/wrapper.cmd*--lane*".to_owned());
  // the sh half of a file that cmd.exe runs too, without a first line `#!`
  let polyglot = |again: &str, rest: &str| format!(":<<\"::CMDLITERAL\"\n@ECHO OFF\nGOTO :CMDSCRIPT\n::CMDLITERAL\n[ -z \"$BASH_VERSION\" ] && {again}\n{rest}exit\n:CMDSCRIPT\n");
  let again = "exec /bin/bash \"$0\" \"$@\"";
  let tree = |path: &str| -> Option<String> {
    match path {
      "tools/wrapper.cmd" => Some(polyglot(again, "exec \"$resolved\" \"$@\"\n")),
      "tools/braces.cmd" => Some(polyglot("exec /bin/bash \"${0}\" \"$@\"", "")),
      "tools/bare.sh" => Some("#!/bin/sh\nexec bash $0 \"$@\"\n".to_owned()),
      "tools/e2e.cmd" => Some(polyglot(again, "pnpm e2e\n")),
      "tools/runner.cmd" => Some(polyglot(again, "exec /bin/bash \"$root/bazel.cmd\" run //build:tests\n")),
      "tools/argument.cmd" => Some("bash -c 'echo started' \"$0\"\necho \"$0\"\nbash -x \"$0\"\n".to_owned()),
      "tools/outer.sh" => Some("#!/bin/zsh\n. tools/inner.sh\n".to_owned()),
      "tools/inner.sh" => Some(format!("{again}\n")),
      // the `$0` of `-c` is the word after its script, not the file around it
      "tools/argzero.sh" => Some("#!/bin/sh\nbash -c 'bash \"$0\"' tools/e2e.cmd\n".to_owned()),
      "tools/argzero-exec.sh" => Some("#!/bin/sh\nexec bash -c 'exec bash \"$0\" \"$@\"' tools/e2e.cmd x\n".to_owned()),
      "tools/no-argzero.sh" => Some("#!/bin/sh\nbash -c 'bash \"$0\"'\n".to_owned()),
      "tools/after-c.sh" => Some(format!("#!/bin/sh\nbash -c 'true' tools/e2e.cmd\n{again}\n")),
      "tools/own-argzero.sh" => Some("#!/bin/sh\nexec bash -c 'set -e; exec bash \"$0\" \"$@\"' \"$0\" \"$@\"\n".to_owned()),
      _ => None,
    }
  };
  let reason = |line: &[&str]| config.heavy_reason(&words(line), &tree);
  let unread = |path: &str| Some(format!("its script `{path}`, which brd-host cannot read for `heavy` of [host]"));
  for (line, expected) in [
    (&["./tools/wrapper.cmd", "--module", "m"][..], None),
    (&["zsh", "-c", "./tools/wrapper.cmd --module m; echo $?"][..], None),
    (&["./tools/braces.cmd"][..], None),
    (&["./tools/bare.sh"][..], None),
    // the wrapper that a pattern names, and what the wrapper runs
    (&["./tools/wrapper.cmd", "--lane", "fast"][..], Some("`./tools/wrapper.cmd*--lane*` of [host]".to_owned())),
    (
      &["zsh", "-c", "./tools/wrapper.cmd --lane fast"][..],
      Some("`./tools/wrapper.cmd*--lane*` of [host] in its script".to_owned()),
    ),
    (&["./tools/e2e.cmd"][..], Some("`pnpm e2e*` of [host] in its script".to_owned())),
    // another script of a shell that the client cannot read
    (&["./tools/runner.cmd"][..], unread("$root/bazel.cmd")),
    // an argument of `-c`, a word of `echo`, and the script after an option of the shell
    (&["./tools/argument.cmd"][..], None),
    // a sourced file has the `$0` of the script that sources it
    (&["zsh", "tools/outer.sh"][..], None),
    (&["zsh", "-c", ". tools/inner.sh"][..], unread("$0")),
    // the script of `-c` names its `$0` by the word after it, the `$0` around it by `"$0"`, and none without a word
    (&["./tools/argzero.sh"][..], Some("`pnpm e2e*` of [host] in its script".to_owned())),
    (&["./tools/argzero-exec.sh"][..], Some("`pnpm e2e*` of [host] in its script".to_owned())),
    (&["./tools/own-argzero.sh"][..], None),
    (&["./tools/no-argzero.sh"][..], unread("$0")),
    // the `$0` of `-c` ends with its script: the file around it names itself again
    (&["./tools/after-c.sh"][..], None),
    (&["zsh", "-c", again][..], unread("$0")),
    (&["bash", "$0"][..], unread("$0")),
    // the `$0` of a file ends with the file: the next command of the script around it is the shell's own
    (&["bash", "-c", "./tools/wrapper.cmd; bash \"$0\""][..], unread("$0")),
  ] {
    assert_eq!(reason(line), expected, "{line:?}");
  }
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

/// A command of the run itself
fn command(line: &[&str]) -> Ran {
  Ran::Command {
    words: words(line),
    scripted: false,
  }
}

/// The commands a run runs, as the classification reads them: the keywords and wrappers before a command are no word of
/// it, and a shell comes first, then what it runs
#[test]
fn a_shell_comes_before_the_commands_it_runs() {
  let script = "do exec pnpm e2e; timeout --signal=KILL 60 cargo nextest run";
  assert_eq!(
    commands_of(&words(&["zsh", "-euc", script]), &|_: &str| None),
    vec![command(&["zsh", "-euc", script]), scripted(&["pnpm", "e2e"]), scripted(&["cargo", "nextest", "run"])]
  );
  assert_eq!(
    commands_of(&words(&["sh", "-c", "a", "zero", "one"]), &|_: &str| None),
    vec![command(&["sh", "-c", "a", "zero", "one"]), scripted(&["a"])]
  );
  assert_eq!(
    commands_of(&words(&["bash", "--norc", "--", "s.sh"]), &|_: &str| None),
    vec![command(&["bash", "--norc", "--", "s.sh"]), Ran::Unread("s.sh".to_owned())]
  );
}

/// A pattern that names the wrapper itself catches it: the shell, the script run by its path, the binary that a shim of
/// the shell starts (`./node_modules/.bin/playwright`, `#!/bin/sh`), in the run and in a script, though what the script
/// runs is light
#[test]
fn a_pattern_that_names_the_wrapper_catches_it() {
  let config = RepoConfig {
    heavy: Some(
      ["./probe/quiet.sh*", "zsh -c *", "bash probe/quiet.sh*", "./node_modules/.bin/playwright test*"]
        .map(str::to_owned)
        .to_vec(),
    ),
    ..RepoConfig::default()
  };
  let read = |path: &str| match path {
    "probe/quiet.sh" => Some("#!/bin/sh\necho hi\n".to_owned()),
    "node_modules/.bin/playwright" => Some("#!/bin/sh\nexec node playwright.js \"$@\"\n".to_owned()),
    _ => None,
  };
  for (line, expected) in [
    (&["./probe/quiet.sh", "a"][..], "`./probe/quiet.sh*` of [host]"),
    (&["zsh", "-c", "echo hi"], "`zsh -c *` of [host]"),
    (&["bash", "probe/quiet.sh"], "`bash probe/quiet.sh*` of [host]"),
    (&["./node_modules/.bin/playwright", "test"], "`./node_modules/.bin/playwright test*` of [host]"),
    (&["bash", "-c", "./probe/quiet.sh"], "`./probe/quiet.sh*` of [host] in its script"),
  ] {
    assert_eq!(config.heavy_reason(&words(line), &read), Some(expected.to_owned()), "{line:?}");
  }
  // the wrapper by what it runs: the shim of the binary is `playwright test` for `playwright test*`, in the run and in
  // a script, though the shim runs only `node`
  let by_what = RepoConfig {
    heavy: Some(vec!["playwright test*".to_owned()]),
    ..RepoConfig::default()
  };
  for line in [
    &["./node_modules/.bin/playwright", "test", "x.spec.ts"][..],
    &["bash", "-c", "cd . && node_modules/.bin/playwright test"],
  ] {
    assert!(by_what.heavy_reason(&words(line), &read).is_some(), "{line:?}");
  }
}

/// A script file is read once a classification and `MAX_READS` paths at most: a script that sources itself, or calls
/// itself again and again, reads one file; a chain longer than the reads counts as heavy, as a script not read
#[test]
fn a_script_is_read_once_and_the_reads_are_bounded() {
  use std::cell::Cell;
  let reads = Cell::new(0);
  let counting = |text: String| {
    reads.set(reads.get() + 1);
    Some(text)
  };
  // sources itself 40 times; and calls itself 20 times, as a dispatcher of its own tasks
  let itself = |path: &str| match path {
    "probe/self.sh" => counting(format!("#!/bin/sh\n{}", ". ./probe/self.sh\n".repeat(40))),
    "run.sh" => counting(format!("#!/bin/sh\n{}", "./run.sh task\n".repeat(20))),
    _ => None,
  };
  assert_eq!(config().heavy_reason(&words(&["./probe/self.sh"]), &itself), None);
  assert_eq!(reads.replace(0), 1, "the script that sources itself");
  assert_eq!(config().heavy_reason(&words(&["bash", "-c", "./run.sh unit; ./run.sh e2e"]), &itself), None);
  assert_eq!(reads.replace(0), 1, "the script that calls itself");
  // a tree of scripts, each sourcing eight more (585 files to the depth that is read): past the reads, the next one
  // may run anything
  let chain = |path: &str| {
    let at: usize = path.strip_prefix("chain/")?.strip_suffix(".sh")?.parse().ok()?;
    counting((1..=8).map(|child| format!(". chain/{}.sh", at * 8 + child)).collect::<Vec<_>>().join("\n"))
  };
  assert_eq!(
    config()
      .heavy_reason(&words(&["bash", "-c", ". chain/0.sh"]), &chain)
      .map(|reason| reason.starts_with("its script `chain/")),
    Some(true)
  );
  assert!(reads.get() <= MAX_READS, "{} reads", reads.get());
}

/// A script that the depth would leave unread counts as heavy, run by its path or sourced, as a shell does: the fifth of
/// a chain of scripts, each run by its path from the one before
#[test]
fn a_script_past_the_depth_counts_as_heavy() {
  let chain = |path: &str| {
    let names = ["a", "b", "c", "d", "e"];
    let at = names.iter().position(|name| path == format!("probe/{name}.sh"))?;
    Some(
      names
        .get(at + 1)
        .map_or_else(|| "#!/bin/sh\npnpm e2e\n".to_owned(), |next| format!("#!/bin/sh\n./probe/{next}.sh\n")),
    )
  };
  assert_eq!(
    config().heavy_reason(&words(&["./probe/a.sh"]), &chain),
    Some("its script `./probe/e.sh`, which brd-host cannot read for `heavy` of [host]".to_owned())
  );
  let sourced = |path: &str| (path == "lib/deep.sh").then(|| "pnpm e2e\n".to_owned());
  assert_eq!(
    config().heavy_reason(&words(&["sh", "-c", "sh -c 'sh -c \"sh -c \\\". lib/deep.sh\\\"\"'"]), &sourced),
    Some("its script `lib/deep.sh`, which brd-host cannot read for `heavy` of [host]".to_owned())
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

/// Two runs are the same run by what they run: the wrappers before the command that change nothing it runs go, as the
/// classification drops them, and the environment stays, as it can change what the command checks; any other word
/// makes another run
#[test]
fn the_key_of_a_run_drops_the_wrappers_and_keeps_the_environment() {
  assert_eq!(run_words(&words(&["nice", "-n", "5", "time", "pnpm", "e2e"])), words(&["pnpm", "e2e"]));
  assert_eq!(run_words(&words(&["timeout", "--foreground", "1h", "nohup", "pnpm", "e2e"])), words(&["pnpm", "e2e"]));
  assert_eq!(
    run_words(&words(&["env", "DEBUG_E2E=1", "nice", "pnpm", "e2e"])),
    words(&["env", "DEBUG_E2E=1", "pnpm", "e2e"])
  );
  assert_eq!(run_words(&words(&["E2E_PORT=3031", "pnpm", "e2e"])), words(&["E2E_PORT=3031", "pnpm", "e2e"]));
  assert_eq!(run_words(&words(&["timeout"])), Vec::<String>::new(), "a wrapper without its command");
  let script = |text: &str| words(&["zsh", "-c", text]);
  assert_eq!(run_key(&words(&["nice", "pnpm", "e2e"])), run_key(&words(&["pnpm", "e2e"])));
  assert_eq!(run_key(&script("pnpm e2e; pnpm e2e")), run_key(&script("pnpm e2e; pnpm e2e")));
  assert_ne!(run_key(&script("pnpm e2e; pnpm e2e")), run_key(&script("pnpm e2e")));
  assert_ne!(run_key(&words(&["E2E_PORT=3031", "pnpm", "e2e"])), run_key(&words(&["pnpm", "e2e"])));
  assert_ne!(run_key(&words(&["pnpm", "e2e", "x"])), run_key(&words(&["pnpm", "e2e x"])), "the words, not their line");
  assert_eq!(run_key(&words(&["pnpm"])).len(), 64);
}
