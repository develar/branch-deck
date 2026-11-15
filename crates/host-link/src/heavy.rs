//! What the command of a remote run runs, for `heavy` of `[host]` ([`crate::config::RepoConfig::heavy_reason`]): the
//! command itself; the commands of the script of a shell that it starts with `-c` (`zsh -c '…; pnpm e2e …'`, `bash -lc`,
//! `env FOO=1 zsh -c`, a shell in that script too); the commands of a script file that a shell runs (`zsh probe/run.sh`)
//! or that the command runs by its path, a file of the working tree whose first line names a shell (`./probe/run.sh`
//! with `#!/bin/zsh`), and of one that a script sources (`. ./lib.sh`, `source lib.sh`), which the client reads from its
//! checkout. A script file of a shell that it cannot read, and a shell nested deeper than `MAX_DEPTH`, are
//! [`Ran::Unread`], which counts as a heavy run: a wrapper is how a heavy run of an agent slipped beside the one that
//! held the turn (2026-11-15). A file run by its path without a shell's first line (a binary of the tree) and a file
//! sourced from outside the tree (`source ~/.cargo/env`) run nothing that counts.
//!
//! The script is split as a shell splits it, as far as a heavy command shows: at `;`, `&`, `&&`, `|`, `||`, a newline,
//! `(`, `)`, a backquote and `$(`, outside quotes, and the substitutions in double quotes (`"$(pnpm e2e)"`) are scripts
//! of their own; each command into its words, the quotes taken out, a backslash escaping one character, a comment
//! dropped; a redirection (`> e2e.log`, `2>&1`) is no word of its command. The keywords, assignments and wrappers
//! before a command (`do`, `then`, `!`, `time`, `exec`, `nohup`, `command`, `nice`, `timeout`, `env` without options)
//! are no word of it either. Text in quotes stays the word of its command: `echo 'pnpm e2e'` runs `echo`. `eval`,
//! `xargs`, `sudo`, `$'…'` and `caffeinate` are not read: a run through them names its weight with `--heavy`.

use std::path::Path;

/// The shells whose `-c` script and script file count
const SHELLS: [&str; 5] = ["sh", "bash", "zsh", "dash", "ksh"];

/// How deep a shell in the script of a shell is read
const MAX_DEPTH: usize = 4;

/// The largest script file that the client reads: a larger one counts as one it cannot read
pub const MAX_SCRIPT_BYTES: u64 = 256 * 1024;

/// The keywords and the wrappers that may stand before the command of a line of a script
const PREFIXES: [&str; 14] = [
  "if", "then", "else", "elif", "while", "until", "do", "!", "{", "time", "exec", "nohup", "command", "builtin",
];

/// What a command runs, as far as `heavy` matches it
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ran {
  /// A simple command, its words without the keywords, wrappers and redirections before or in it; `scripted` when it
  /// comes from a script that was read, not from the command of the run itself
  Command { words: Vec<String>, scripted: bool },
  /// A script file of a shell that the reader could not give (its path as the command names it), or a shell nested
  /// deeper than `MAX_DEPTH` (its words)
  Unread(String),
}

/// The commands that `words` runs: itself, unless it is a shell with `-c` (the commands of its script) or with a script
/// file (the commands of the file that `read` gives for its path without `./` before it, else [`Ran::Unread`])
pub fn commands_of(words: &[String], read: &dyn Fn(&str) -> Option<String>) -> Vec<Ran> {
  let mut ran = Vec::new();
  collect(words.to_vec(), read, 0, &mut ran);
  ran
}

fn collect(words: Vec<String>, read: &dyn Fn(&str) -> Option<String>, depth: usize, ran: &mut Vec<Ran>) {
  let words = without_prefixes(words);
  let script = |script: &str, ran: &mut Vec<Ran>| {
    for command in script_commands(script) {
      collect(command, read, depth + 1, ran);
    }
  };
  match shell_input(&words) {
    // past the depth a shell may run anything: not read, it counts as heavy
    Some(ShellInput::Script(_) | ShellInput::File(_)) if depth >= MAX_DEPTH => ran.push(Ran::Unread(words.join(" "))),
    Some(ShellInput::Script(text)) => script(&text, ran),
    Some(ShellInput::File(path)) => match read(inner_script_path(&path)) {
      Some(text) => script(&text, ran),
      None => ran.push(Ran::Unread(path)),
    },
    Some(ShellInput::Nothing) => {}
    None => {
      // a file of the tree run by its path whose first line names a shell, and one that a script sources, are scripts
      // too; a binary of the tree and a file sourced from elsewhere run nothing that counts
      let program = words.first().map_or("", String::as_str);
      let sourced = matches!(program, "." | "source").then(|| words.get(1)).flatten();
      let file = sourced.or_else(|| words.first().filter(|first| first.contains('/')));
      let text = file
        .filter(|_| depth < MAX_DEPTH)
        .and_then(|path| read(inner_script_path(path)))
        .filter(|text| sourced.is_some() || runs_a_shell(text));
      match text {
        Some(text) => script(&text, ran),
        None if !words.is_empty() => ran.push(Ran::Command { words, scripted: depth > 0 }),
        None => {}
      }
    }
  }
}

/// Whether a script file starts with the line of a shell of `SHELLS`: `#!/bin/zsh`, `#!/usr/bin/env bash -e`
fn runs_a_shell(text: &str) -> bool {
  let Some(line) = text.lines().next().and_then(|line| line.strip_prefix("#!")) else {
    return false;
  };
  let mut words = line.split_whitespace();
  let program = words.next().unwrap_or_default();
  let program = if program.rsplit('/').next() == Some("env") {
    words.next().unwrap_or_default()
  } else {
    program
  };
  SHELLS.contains(&program.rsplit('/').next().unwrap_or(program))
}

/// What a shell runs
enum ShellInput {
  /// The script of `-c`
  Script(String),
  /// A script file, the first word after its options
  File(String),
  /// Nothing that a run gives it: an interactive shell, `-s` reading its input
  Nothing,
}

/// What `words` runs when it is a shell (`zsh`, `/bin/bash`), `None` for any other command
fn shell_input(words: &[String]) -> Option<ShellInput> {
  let program = words.first()?;
  let name = program.rsplit('/').next().unwrap_or(program);
  if !SHELLS.contains(&name) {
    return None;
  }
  let mut script = false;
  let mut stdin = false;
  let mut rest = words[1..].iter();
  while let Some(word) = rest.next() {
    if word == "--" {
      break;
    }
    if word == "--rcfile" || word == "--init-file" {
      rest.next();
    } else if word.starts_with("--") {
      // a long option without an argument: `--login`, `--norc`
    } else if (word.starts_with('-') || word.starts_with('+')) && word.len() > 1 {
      script |= word.contains('c');
      stdin |= word.contains('s');
      if word.contains('o') || word.contains('O') {
        // `-o pipefail`: the option names the next word
        rest.next();
      }
    } else {
      return Some(if script { ShellInput::Script(word.clone()) } else { ShellInput::File(word.clone()) });
    }
  }
  // the words after `--`
  match rest.next() {
    Some(word) if script => Some(ShellInput::Script(word.clone())),
    Some(word) if !stdin => Some(ShellInput::File(word.clone())),
    _ => Some(ShellInput::Nothing),
  }
}

/// `words` without the keywords, assignments and wrappers before its command, the options and the argument of `nice`
/// and `timeout`, and an `env` (`/usr/bin/env`) without options; `env -i` stays, as a command of its own
fn without_prefixes(words: Vec<String>) -> Vec<String> {
  let mut at = 0;
  while let Some(word) = words.get(at) {
    let bare_env = word.rsplit('/').next() == Some("env") && !words.get(at + 1).is_some_and(|next| next.starts_with('-'));
    if PREFIXES.contains(&word.as_str()) || crate::config::is_assignment(word) || bare_env {
      at += 1;
    } else if word == "nice" {
      at += 1;
      // `nice -n 10`, `nice -10`
      if words.get(at).is_some_and(|word| word == "-n") {
        at += 2;
      } else if words.get(at).is_some_and(|word| word.starts_with('-')) {
        at += 1;
      }
    } else if word == "timeout" {
      at += 1;
      while words.get(at).is_some_and(|word| word.starts_with('-')) {
        at += 1;
      }
      // the duration
      at += 1;
    } else {
      break;
    }
  }
  words.into_iter().skip(at).collect()
}

/// The simple commands of a shell script, each as its words (see the header of the module)
pub fn script_commands(script: &str) -> Vec<Vec<String>> {
  let mut commands = Vec::new();
  let mut words: Vec<String> = Vec::new();
  let mut word = String::new();
  // whether the word holds something, an empty pair of quotes too
  let mut in_word = false;
  // the next word is the target of a redirection, which no command runs
  let mut target = false;
  let mut chars = script.chars().peekable();
  // the scripts of the substitutions in double quotes, split after the script
  let mut nested: Vec<String> = Vec::new();
  while let Some(char) = chars.next() {
    match char {
      '\\' => match chars.next() {
        // a line that goes on
        Some('\n') | None => {}
        Some(escaped) => {
          word.push(escaped);
          in_word = true;
        }
      },
      '\'' => {
        in_word = true;
        for quoted in chars.by_ref() {
          if quoted == '\'' {
            break;
          }
          word.push(quoted);
        }
      }
      '"' => {
        in_word = true;
        while let Some(quoted) = chars.next() {
          match quoted {
            '"' => break,
            '\\' if chars.peek().is_some_and(|next| matches!(next, '"' | '\\' | '$' | '`' | '\n')) => {
              let escaped = chars.next().unwrap_or_default();
              if escaped != '\n' {
                word.push(escaped);
              }
            }
            // a substitution in the quotes runs a script of its own: `out="$(pnpm e2e 2>&1)"`
            '$' if chars.peek() == Some(&'(') => {
              chars.next();
              nested.push(substitution(&mut chars));
            }
            '`' => {
              let mut inner = String::new();
              for char in chars.by_ref() {
                if char == '`' {
                  break;
                }
                inner.push(char);
              }
              nested.push(inner);
            }
            _ => word.push(quoted),
          }
        }
      }
      '#' if !in_word => {
        for skipped in chars.by_ref() {
          if skipped == '\n' {
            break;
          }
        }
        end_word(&mut word, &mut in_word, &mut words, &mut target);
        end_command(&mut words, &mut commands);
      }
      // a redirection of an output (`>&2`, `&>`, `2>&1`) is a word, not a command that goes on in the background
      '&' if word.ends_with('>') || word.ends_with('<') || chars.peek() == Some(&'>') => {
        word.push(char);
        in_word = true;
      }
      '$' if chars.peek() == Some(&'(') => {
        chars.next();
        end_word(&mut word, &mut in_word, &mut words, &mut target);
        end_command(&mut words, &mut commands);
      }
      ';' | '&' | '|' | '\n' | '(' | ')' | '`' => {
        end_word(&mut word, &mut in_word, &mut words, &mut target);
        end_command(&mut words, &mut commands);
      }
      ' ' | '\t' | '\r' => end_word(&mut word, &mut in_word, &mut words, &mut target),
      _ => {
        word.push(char);
        in_word = true;
      }
    }
  }
  end_word(&mut word, &mut in_word, &mut words, &mut target);
  end_command(&mut words, &mut commands);
  for inner in nested {
    commands.extend(script_commands(&inner));
  }
  commands
}

/// The script of a substitution `$(…)` after its `$(`, up to its `)`: the parentheses in it counted, those in quotes not
fn substitution(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
  let mut inner = String::new();
  let mut depth = 1;
  let mut quote: Option<char> = None;
  while let Some(char) = chars.next() {
    match (quote, char) {
      (Some(open), _) if char == open => quote = None,
      (None, '\'' | '"') => quote = Some(char),
      (None, '\\') => {
        inner.push(char);
        if let Some(escaped) = chars.next() {
          inner.push(escaped);
        }
        continue;
      }
      (None, '(') => depth += 1,
      (None, ')') => {
        depth -= 1;
        if depth == 0 {
          break;
        }
      }
      _ => {}
    }
    inner.push(char);
  }
  inner
}

/// Ends the word that `word` holds: a word of the command, or a redirection and its target, which are none
fn end_word(word: &mut String, in_word: &mut bool, words: &mut Vec<String>, target: &mut bool) {
  if *in_word {
    if *target {
      *target = false;
    } else if is_redirection(word) {
      // `>file` and `2>&1` hold their target, `>` and `2>` name the next word
      *target = !redirection_has_target(word);
    } else {
      words.push(std::mem::take(word));
    }
    word.clear();
    *in_word = false;
  }
}

fn end_command(words: &mut Vec<String>, commands: &mut Vec<Vec<String>>) {
  if !words.is_empty() {
    commands.push(std::mem::take(words));
  }
}

/// A word that redirects: digits, then `>`, `>>`, `<`, `<<`, `&>`, `>&` or `<&`, maybe with its target after it
fn is_redirection(word: &str) -> bool {
  let rest = word.trim_start_matches(|char: char| char.is_ascii_digit());
  rest.starts_with('>') || rest.starts_with('<') || rest.starts_with("&>")
}

/// Whether a redirection holds its target (`>e2e.log`, `2>&1`), so no next word is its target
fn redirection_has_target(word: &str) -> bool {
  let rest = word.trim_start_matches(|char: char| char.is_ascii_digit());
  let operator = rest.trim_start_matches(['>', '<', '&']);
  !operator.is_empty()
}

/// A path of a script file as a command names it, as the reader takes it: without `./` before it
fn inner_script_path(path: &str) -> &str {
  let mut path = path;
  while let Some(rest) = path.strip_prefix("./") {
    path = rest;
  }
  path
}

/// The script file at `path` inside the working tree `root` that a run ships, as the shell of its command reads it
/// in the slot: `None` for a path outside of it (absolute, with `..`), a file it does not hold, or one larger than
/// `MAX_SCRIPT_BYTES`
pub fn script_in(root: &Path, path: &str) -> Option<String> {
  if !crate::config::is_inner_path(path) {
    return None;
  }
  let file = root.join(path);
  let size = std::fs::metadata(&file).ok()?.len();
  if size > MAX_SCRIPT_BYTES {
    return None;
  }
  std::fs::read_to_string(file).ok()
}
