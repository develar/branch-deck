//! Rules of how the production code runs git (AGENTS.md, "Git Operations"), checked on its sources like the worktree
//! safety scan (`source_scan_test.rs`). A failure names the place and what to do instead. Each rule allows a file only
//! with its reason and exact number of hits, so a new hit and a stale exemption both fail.

use crate::source_scan_test::{Arg, ArgList, Exemption, Source, argument_lists, check, line_at, production_sources, skip_trivia};
use std::path::Path;

/// The lines of the argument lists of `source` that `test` selects
fn list_lines(source: &Source, test: impl Fn(&ArgList<'_>) -> bool) -> Vec<usize> {
  argument_lists(&source.text).iter().filter(|list| test(list)).map(|list| list.line).collect()
}

/// The code lines of `source` (not comments) that contain `needle`
fn code_lines_with(source: &Source, needle: &str) -> Vec<usize> {
  source.code_lines().filter(|(_, line)| line.contains(needle)).map(|(number, _)| number).collect()
}

/// The lines of `source` that start a process whose program may be git: a `Command::new` (std or tokio) whose argument
/// names git (`"git"`, `git_path`, `&git_info.path`, but not `github`), or any `Command::new` of a file that reads the
/// discovered git (`GitInfo`, `.get_info()`)
fn git_spawn_lines(source: &Source) -> Vec<usize> {
  const SPAWN: &str = "Command::new(";
  let reads_git_info = source.code_lines().any(|(_, line)| line.contains("GitInfo") || line.contains(".get_info()"));
  let mut lines = Vec::new();
  for (index, _) in source.text.match_indices(SPAWN) {
    let line_start = source.text[..index].rfind('\n').map_or(0, |end| end + 1);
    if source.text[line_start..index].trim_start().starts_with("//") {
      continue;
    }
    let argument = &source.text[index + SPAWN.len()..];
    let argument = &argument[..argument.find(')').unwrap_or(argument.len())];
    if reads_git_info || argument.split(|c: char| !c.is_ascii_alphanumeric()).any(|word| word.eq_ignore_ascii_case("git")) {
      lines.push(line_at(&source.text, index));
    }
  }
  lines
}

/// Whether the code that ends with `before` passes what follows straight to an executor call (`git.cmd(&`,
/// `git.execute_command(&`), not to a helper
fn is_executor_argument(before: &str) -> bool {
  let Some(call) = before.trim_end().strip_suffix('&').unwrap_or(before).trim_end().strip_suffix('(') else {
    return false;
  };
  let name = &call[call.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_').len()..];
  name == "cmd" || name.starts_with("execute_command")
}

/// Whether `list` puts paths after `--`, where git reads every argument as a pathspec: an element after its literal
/// `--`, or a `push` or `extend` of the list it is bound to (`let mut args = vec![…]`) right after a `--` that ends it
/// or that a later statement pushes (`args.push("--"); args.extend(paths)`, also inside a block). Paths added another
/// way (a loop, a helper) are not seen
fn puts_paths(list: &ArgList<'_>) -> bool {
  let dashes = list.args.iter().position(|arg| *arg == Arg::Literal("--"));
  if dashes.is_some_and(|index| index + 1 < list.args.len()) {
    return true;
  }
  let Some(name) = bound_name(list.before) else { return false };
  // up to the next list bound to the name
  let rest = [format!("let mut {name} "), format!("let {name} ")]
    .iter()
    .filter_map(|binding| list.after.find(binding.as_str()))
    .min()
    .map_or(list.after, |end| &list.after[..end]);
  let adds = |text: &str| {
    let next = skip_trivia(text);
    next.starts_with(&format!("{name}.push(")) || next.starts_with(&format!("{name}.extend"))
  };
  if dashes.is_some() {
    let rest = skip_trivia(rest);
    return adds(rest.strip_prefix(';').unwrap_or(rest));
  }
  let push = format!("{name}.push(\"--\");");
  rest.match_indices(&push).any(|(index, _)| adds(&rest[index + push.len()..]))
}

/// The name `before` binds the array literal after it to: `args` of `let mut args = vec!`
fn bound_name(before: &str) -> Option<&str> {
  let statement = &before[before.rfind("let ")? + "let ".len()..];
  if statement.contains(';') || !statement.trim_end().ends_with("vec!") {
    return None;
  }
  let statement = statement.strip_prefix("mut ").unwrap_or(statement).trim_start();
  let end = statement.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(statement.len());
  (end > 0).then(|| &statement[..end])
}

/// The crates of `bd`: the CLI and the crates it is cut into
const BD_CRATES: &str = "crates/bd-";

#[test]
fn bd_and_the_app_run_no_git_of_their_own() {
  // a bd crate the scan doesn't read would escape the rule
  let sources = production_sources();
  let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
  for entry in std::fs::read_dir(crates).expect("crates directory").flatten() {
    let prefix = format!("crates/{}/", entry.file_name().to_string_lossy());
    if prefix.starts_with(BD_CRATES) {
      assert!(sources.iter().any(|source| source.path.starts_with(&prefix)), "the scan reads no source of {prefix}");
    }
  }
  check(
    "the bd crates (crates/bd-*) and src-tauri start git themselves",
    "they are adapters: put the git call into a function of crates/git-ops (the git) or a core of crates/history-rewrite or \
     crates/sync-core (the lock, the guards, the orchestration), and call that",
    &[],
    |source| {
      if !(source.path.starts_with(BD_CRATES) || source.path.starts_with("src-tauri/src/")) {
        return Vec::new();
      }
      let mut lines = code_lines_with(source, ".execute_command");
      lines.extend(code_lines_with(source, ".cmd("));
      lines
    },
  );
}

#[test]
fn git_starts_only_in_the_executor() {
  check(
    "git is started outside GitCommandExecutor",
    "run git with git.cmd(&args, repo) or a shorthand of GitCommandExecutor, in a git-ops function: only its spawn core \
     gives git the base config and environment, a null stdin, the timeouts and the process stats",
    &[
      Exemption {
        path: "crates/git-executor/src/git_command_executor.rs",
        hits: 1,
        reason: "the spawn core of every git call (`GitCommandExecutor::command`)",
      },
      Exemption {
        path: "crates/git-executor/src/git_info.rs",
        hits: 2,
        reason: "the discovery: `git version` of a candidate, and the login shell's `which git`",
      },
    ],
    git_spawn_lines,
  );
}

#[test]
fn no_caller_passes_no_pager() {
  check(
    "git is given --no-pager",
    "drop it: the executor reads git's stdout through a pipe, so git never starts a pager",
    &[],
    |source| code_lines_with(source, "\"--no-pager\""),
  );
}

#[test]
fn no_caller_overrides_the_credential_helpers() {
  check(
    "git is given a credential.helper",
    "keep the user's credential helpers: run push, fetch and ls-remote with sync_core::remote_command::execute_remote_command*, \
     which keeps them and turns their prompts off",
    &[],
    |source| code_lines_with(source, "credential.helper="),
  );
}

#[test]
fn refs_move_only_through_ref_transaction() {
  check(
    "refs are moved by an update-ref of its own",
    "move refs with git_ops::ref_transaction::RefTransaction: one update-ref --stdin for all of them, each with its old \
     value (compare and swap) and a branch-deck reflog message, and commit_while when the files follow the move",
    &[Exemption {
      path: "crates/git-ops/src/ref_transaction.rs",
      hits: 1,
      reason: "RefTransaction itself",
    }],
    |source| list_lines(source, |list| list.subcommand() == Some("update-ref")),
  );
}

#[test]
fn objects_are_read_through_cat_file_batch() {
  check(
    "objects are read by a cat-file --batch of its own",
    "read objects with git_ops::cat_file_batch: for_each_object or read_objects for the content (streamed, with a size \
     limit), object_types for the types; one cat-file -Z per call, the names NUL-terminated",
    &[
      Exemption {
        path: "crates/git-ops/src/cat_file_batch.rs",
        hits: 2,
        reason: "the shared reader itself",
      },
      Exemption {
        path: "crates/git-ops/src/update_branch.rs",
        hits: 2,
        reason: "the ids of index entries, named by path as bytes (`:<path>`, not always UTF-8), which the str names of cat_file_batch cannot carry",
      },
      Exemption {
        path: "crates/sync-core/src/cache_memo.rs",
        hits: 1,
        reason: "one interactive --batch-check that answers the memo's tree checks one at a time, as they come",
      },
    ],
    |source| list_lines(source, |list| list.subcommand() == Some("cat-file") && list.has(|arg| arg.starts_with("--batch"))),
  );
}

#[test]
fn listed_paths_are_nul_terminated() {
  check(
    "git lists paths (--name-only, --name-status, --raw) without -z",
    "add -z and split the output at NUL: without it git quotes a path with a quote, a backslash or a control character, \
     and a newline in a path breaks the lines",
    &[
      Exemption {
        path: "crates/sync-core/src/branch_name_suggestion.rs",
        hits: 1,
        reason: "`show --name-status` is text for the AI prompt, never parsed into paths",
      },
      Exemption {
        path: "crates/patronus/src/teamcity_cli.rs",
        hits: 1,
        reason: "the teamcity CLI's --raw, not git",
      },
    ],
    |source| {
      list_lines(source, |list| {
        list.has(|arg| matches!(arg, "--name-only" | "--name-status" | "--raw")) && !list.has(|arg| arg == "-z")
      })
    },
  );
}

#[test]
fn network_commands_run_through_remote_command() {
  check(
    "a network command runs on a plain executor call",
    "run push, fetch and ls-remote with sync_core::remote_command::execute_remote_command* (AGENTS.md, Network \
     commands): the credential helpers stay, prompts are off, stalled HTTP transfers abort and the locale is C",
    &[],
    |source| {
      let calls_remote_command = source.code_lines().any(|(_, line)| {
        line
          .split("execute_remote_command")
          .skip(1)
          .any(|rest| rest.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_').starts_with('('))
      });
      list_lines(source, |list| {
        matches!(list.subcommand(), Some("push" | "fetch" | "pull" | "ls-remote" | "clone")) && (!calls_remote_command || is_executor_argument(list.before))
      })
    },
  );
}

#[test]
fn pathspecs_are_literal() {
  check(
    "git is given paths after -- that it reads as patterns",
    "put --literal-pathspecs (a global option) first in the argument list: git reads every argument after -- as a \
     pathspec, and without it `a*.txt` matches `ab.txt` and `:name` is pathspec magic",
    &[],
    |source| list_lines(source, |list| puts_paths(list) && !list.has(|arg| arg == "--literal-pathspecs")),
  );
}

#[test]
fn a_git_spawn_is_told_from_another_process() {
  let lines = |text: &str| {
    git_spawn_lines(&Source {
      path: String::new(),
      text: text.to_string(),
    })
  };
  assert_eq!(lines("let a = Command::new(\"git\");\nlet b = std::process::Command::new(\n  &git_info.path,\n);"), [1, 2]);
  assert_eq!(
    lines("let gh = Command::new(github_cli); let sh = Command::new(\"sh\");\n/// Command::new(\"git\")"),
    Vec::<usize>::new()
  );
  assert_eq!(lines("let info = executor.get_info()?;\nlet c = tokio::process::Command::new(&info.path);"), [2]);
}

#[test]
fn paths_after_the_double_dash_are_told_from_its_end_of_the_revisions() {
  let puts = |text: &str| argument_lists(text).iter().map(puts_paths).collect::<Vec<_>>();
  // in the list, after the `--` that ends it, after a pushed `--`, inside a block
  assert_eq!(puts(r#"git.cmd(&["diff", "HEAD", "--", path], repo)"#), [true]);
  assert_eq!(puts("let mut args = vec![\"log\", \"--\"];\nargs.extend(paths);"), [true]);
  assert_eq!(
    puts("let mut args = vec![\"log\"];\nargs.extend(revs);\nargs.push(\"--\");\n// the files\nargs.extend(files);"),
    [true]
  );
  assert_eq!(
    puts("let mut args = vec![\"archive\", commit];\nif let Some(paths) = paths {\n  args.push(\"--\");\n  args.extend(paths);\n}"),
    [true]
  );
  // a `--` that only ends the revisions, and a later list of the same name
  assert_eq!(puts(r#"git.cmd(&["log", &range, "--"], repo)"#), [false]);
  assert_eq!(
    puts("let mut args = vec![\"log\"];\nargs.push(\"--\");\nlet out = run(&args);\nlet mut args = vec![\"diff\", \"--\"];\nrun(&args);\nargs.push(path);"),
    [false, false]
  );
}

#[test]
fn an_executor_argument_is_told_from_a_helper_argument() {
  assert!(is_executor_argument("let output = git.cmd(&"));
  assert!(is_executor_argument("git_executor.execute_command_with_status(\n    &"));
  assert!(!is_executor_argument("execute_remote_command(git_executor, &"));
  assert!(!is_executor_argument("let args = vec!"));
}
