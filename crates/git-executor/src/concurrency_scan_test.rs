//! Nothing across the network or a wait (docs/design/rust-domain-authority.md, «Инварианты», 2; AGENTS.md, "Repository
//! lock"). The guard of an async mutex or read-write lock lives across no other `.await` of its scope: a read that the
//! others wait for is one shared future or a `watch` (single-flight), not a mutex. The guard of the repository lock lives
//! across no call of the network: a git of the remote (`execute_remote_command*`), the fetch and the push of the baseline
//! (`fetch_baseline`, `push_baseline`), the leased push (`push_leased`, `push::send`), HTTP (`reqwest`, the `.send()` of a
//! request, a method of a `GitHubService`, `PatronusService`, `YouTrackService` or `ServiceFactSources` or of what the
//! hosts answer for one), a runtime blocked on a future (`block_on`); nor across a sync or a wait for the index lock of
//! another process (its own check, below).
//!
//! Checked on the production sources like the git scans (`source_scan_test.rs`), on their code without literals and
//! comments, by the `let` bindings: a guard lives from its statement to the end of the block that holds it or to the
//! `drop` of its name in that block (of the last name of a tuple, where `lock_prepared` puts the lock), and a guard that
//! its block hands on as its value (`Some(guard)`) lives on in the binding that takes it. A guard handed to a function,
//! kept in a field or returned is not followed, nor a call of the network inside a function the scope calls: the lock span
//! tests of the cores (`test_utils::lock_span`) name every git that runs under the lock. A file is exempt only with its
//! reason and its exact number of hits.

use crate::source_scan_test::{Exemption, Source, check, code_text, line_at, production_sources};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::OnceLock;

/// The ends of the initializer of a binding that holds the guard of an async mutex or read-write lock (tokio's)
const ASYNC_GUARDS: [&str; 6] = [
  ".lock().await",
  ".read().await",
  ".write().await",
  ".lock_owned().await",
  ".read_owned().await",
  ".write_owned().await",
];

/// The functions of `history_rewrite::repository_lock` that take the repository lock, by the start of their name
/// (`lock_repository_in`, `lock_prepared_move`, `RewriteOptions::lock_prepared`)
const LOCK_FUNCTIONS: [&str; 5] = ["lock_repository", "try_lock_repository", "lock_prepared", "lock_checked_undo", "lock_as_another_process"];

/// The methods of `LockWait` and `RewriteOptions` that take it, called with an argument: the `lock()` of a mutex has none
const LOCK_METHODS: [&str; 3] = ["lock", "lock_with", "lock_in"];

/// The calls of the network by the start of their name
const NETWORK_CALLS: [&str; 5] = ["execute_remote_command", "fetch_baseline", "push_baseline", "push_leased", "block_on"];

/// The services that answer over HTTP
const SERVICES: [&str; 4] = ["GitHubService", "PatronusService", "YouTrackService", "ServiceFactSources"];

/// The methods of the hosts that answer a service (`ServiceHost::github`, `SignInHost::youtrack`): a call of one in the
/// scope is there to call the service
const SERVICE_ACCESSORS: [&str; 3] = ["github", "patronus", "youtrack"];

/// The functions of `bd_core::services` that make a service
const SERVICE_CONSTRUCTORS: [&str; 3] = ["github_service", "patronus_service", "youtrack_service"];

/// No guard of an async mutex lives across another `.await` since s14-2b: a read that the others wait for is a single
/// flight (`sync_utils::single_flight`, the renewal of `youtrack::service`); an exemption here names its file, reason
/// and count as the exemptions of the git scans do
const EXEMPT_GUARDS: [Exemption; 0] = [];

/// No repository lock lives across the network today; an exemption here names its file, reason and count as above
const EXEMPT_LOCKS: [Exemption; 0] = [];

const fn is_word(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'_'
}

/// A `let` binding of the code of a source (`code_text`)
struct Binding<'a> {
  /// The offset of its `let`
  at: usize,
  /// The names it binds (`(prepared, _lock)`), without `_`, `mut`, `ref` and constructors (`Some`)
  names: Vec<&'a str>,
  /// Its type, when it has one (`let lock: Locker`)
  ty: Option<Range<usize>>,
  /// Its initializer, from after its `=` to its `;`
  value: Range<usize>,
  /// Where the bound value lives: from after its `;` to the end of the block that holds it, or to the `drop` of its last
  /// name in that block
  scope: Range<usize>,
}

/// The index of the first byte of `text` at its own bracket depth that `stop` selects, or of the bracket that closes one
/// opened before `text`; None when neither comes
fn first_at_top(text: &str, stop: impl Fn(&str, usize) -> bool) -> Option<usize> {
  let mut depth = 0usize;
  for (index, byte) in text.bytes().enumerate() {
    match byte {
      b'(' | b'[' | b'{' => depth += 1,
      b')' | b']' | b'}' if depth == 0 => return Some(index),
      b')' | b']' | b'}' => depth -= 1,
      _ if depth == 0 && stop(text, index) => return Some(index),
      _ => {}
    }
  }
  None
}

/// `first_at_top` of the byte `wanted`, None when a closing bracket comes first
fn top_byte(text: &str, wanted: u8, also: impl Fn(&[u8], usize) -> bool) -> Option<usize> {
  let bytes = text.as_bytes();
  first_at_top(text, |_, index| bytes[index] == wanted && also(bytes, index)).filter(|&index| bytes[index] == wanted)
}

/// Whether `text` ends with the word `word` (`} else if`)
fn ends_with_word(text: &str, word: &str) -> bool {
  text.strip_suffix(word).is_some_and(|before| !before.bytes().next_back().is_some_and(is_word))
}

/// The bindings of `code`; a `let` of an `if let`, a `while let` or a let chain binds for its block only and is none
fn bindings(code: &str) -> Vec<Binding<'_>> {
  let mut found = Vec::new();
  for (at, _) in code.match_indices("let ") {
    let before = code[..at].trim_end();
    if code[..at].bytes().next_back().is_some_and(is_word) || ["if", "while", "&&"].iter().any(|word| ends_with_word(before, word)) {
      continue;
    }
    let start = at + "let ".len();
    let Some(end) = top_byte(&code[start..], b';', |_, _| true).map(|end| start + end) else {
      continue;
    };
    let statement = &code[start..end];
    // the `=` of the binding, not of `==`, `=>`, `<=`, `>=` or `!=`
    let Some(equals) = top_byte(statement, b'=', |bytes, index| {
      !matches!(bytes.get(index + 1), Some(b'=' | b'>')) && !matches!(bytes.get(index.wrapping_sub(1)), Some(b'=' | b'!' | b'<' | b'>'))
    }) else {
      continue;
    };
    let pattern = &statement[..equals];
    // without its type: a `:` of its own, not of a path
    let typed = top_byte(pattern, b':', |bytes, index| {
      bytes.get(index + 1) != Some(&b':') && bytes.get(index.wrapping_sub(1)) != Some(&b':')
    });
    let names = names_of(&pattern[..typed.unwrap_or(pattern.len())]);
    let Some(last) = names.last().copied() else { continue };
    let rest = &code[end + 1..];
    let scope = first_at_top(rest, |text, index| drops(text, index, last)).unwrap_or(rest.len());
    found.push(Binding {
      at,
      names,
      ty: typed.map(|colon| start + colon + 1..start + equals),
      value: start + equals + 1..end,
      scope: end + 1..end + 1 + scope,
    });
  }
  found
}

/// The names a pattern binds
fn names_of(pattern: &str) -> Vec<&str> {
  pattern
    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
    .filter(|word| word.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') && !matches!(*word, "_" | "mut" | "ref"))
    .collect()
}

/// Whether the `drop` of `name` starts at `index` of `text` (`drop(lock)`, `std::mem::drop(lock)`)
fn drops(text: &str, index: usize, name: &str) -> bool {
  let Some(argument) = text[index..].strip_prefix("drop(") else { return false };
  !text[..index].bytes().next_back().is_some_and(|byte| is_word(byte) || byte == b'.')
    && argument.trim_start().strip_prefix(name).is_some_and(|after| after.trim_start().starts_with(')'))
}

/// The code a binding's value lives in: its scope, or, when its block hands it on as its value (a name of it in the tail
/// of the scope, not as the receiver of a method or behind `&` or `*`), the scope of the binding whose initializer holds
/// that block, and so on
fn lifetime<'a>(code: &'a str, all: &[Binding<'_>], binding: &Binding<'_>) -> &'a str {
  let (mut at, mut names, mut scope) = (binding.at, binding.names.clone(), binding.scope.clone());
  while hands_on(&code[scope.clone()], &names) {
    let Some(outer) = all.iter().filter(|outer| outer.value.contains(&at)).min_by_key(|outer| outer.value.len()) else {
      break;
    };
    (at, names, scope) = (outer.at, outer.names.clone(), outer.scope.clone());
  }
  &code[scope]
}

/// Whether the tail of `scope` (after its last statement) is or holds one of `names` as a value
fn hands_on(scope: &str, names: &[&str]) -> bool {
  let mut tail = scope;
  while let Some(end) = top_byte(tail, b';', |_, _| true) {
    tail = &tail[end + 1..];
  }
  names.iter().any(|name| {
    words(tail, name).any(|at| {
      let before = tail[..at].trim_end().bytes().next_back();
      let after = tail[at + name.len()..].trim_start().bytes().next();
      !matches!(before, Some(b'&' | b'*' | b'.')) && after != Some(b'.')
    })
  })
}

/// The offsets of the word `word` in `text`
fn words<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
  let bytes = text.as_bytes();
  text
    .match_indices(word)
    .map(|(at, _)| at)
    .filter(move |&at| !bytes[..at].last().copied().is_some_and(is_word) && !bytes.get(at + word.len()).copied().is_some_and(is_word))
}

/// The offsets in `text` of the calls of a name that starts with `prefix` (or is `prefix`, when `whole`): the name a word
/// of its own, then `(` or a turbofish; `receiver` selects by the text before the name
fn calls<'a>(text: &'a str, prefix: &'a str, whole: bool, receiver: impl Fn(&str) -> bool + 'a) -> impl Iterator<Item = usize> + 'a {
  let bytes = text.as_bytes();
  text.match_indices(prefix).map(|(at, _)| at).filter(move |&at| {
    let end = at
      + prefix.len()
      + if whole {
        0
      } else {
        bytes[at + prefix.len()..].iter().take_while(|&&byte| is_word(byte)).count()
      };
    let called = text[end..].starts_with('(') || text[end..].starts_with("::<");
    called && !bytes[..at].last().copied().is_some_and(is_word) && receiver(&text[..at])
  })
}

/// Whether the call at `at` of `text` has an argument
fn has_argument(text: &str, at: usize) -> bool {
  let open = at + text[at..].find('(').expect("a call has its parenthesis");
  !text[open + 1..].trim_start().starts_with(')')
}

/// `text` without the statements of its blocks, only their tails: what a block gives as its value
fn block_values(text: &str) -> String {
  let mut kept = text.as_bytes().to_vec();
  let mut opens = Vec::new();
  for (index, byte) in text.bytes().enumerate() {
    match byte {
      b'{' => opens.push(index),
      b'}' => {
        let Some(open) = opens.pop() else { continue };
        let block = &text[open + 1..index];
        let mut statements = 0;
        while let Some(end) = top_byte(&block[statements..], b';', |_, _| true) {
          statements += end + 1;
        }
        kept[open + 1..open + 1 + statements].fill(b' ');
      }
      _ => {}
    }
  }
  String::from_utf8(kept).expect("only whole statements are blanked")
}

/// Whether `value`, the initializer of a binding, takes the repository lock; `bare`: the names whose call by the bare name
/// takes it in this file (`lock_takers`)
fn takes_repository_lock(value: &str, bare: &[&str]) -> bool {
  let value = block_values(value);
  let value = value.as_str();
  LOCK_FUNCTIONS.iter().any(|name| calls(value, name, false, |_| true).next().is_some())
    || LOCK_METHODS
      .iter()
      .any(|name| calls(value, name, true, |before| before.trim_end().ends_with('.')).any(|at| has_argument(value, at)))
    || calls(value, "lock", true, |before| before.ends_with("sync_command::")).next().is_some()
    || bare
      .iter()
      .any(|name| calls(value, name, true, |before| !before.ends_with([':', '.'])).any(|at| has_argument(value, at)))
}

/// The names whose bare call takes the repository lock in the file of `code`: its `lockers`, what it types as a `Locker`
/// (`operations::land_execute`: the closure of a start that takes it), and the lock of `bd`, when the file imports it
/// (`use bd_core::sync_command::{…, lock, …}`)
fn lock_takers<'a>(code: &str, lockers: &[&'a str]) -> Vec<&'a str> {
  let imports_bd_lock = code.match_indices("sync_command::{").any(|(at, head)| {
    let items = &code[at + head.len()..];
    items[..items.find('}').unwrap_or(items.len())].split(',').any(|item| item.trim() == "lock")
  }) || code.contains("sync_command::lock;");
  let mut names = lockers.to_vec();
  if imports_bd_lock {
    names.push("lock");
  }
  names
}

/// The names that the file of `code` declares with a type that names `ty`: a parameter, a field or a binding
/// (`github: &GitHubService`, `patronus: Option<Arc<PatronusService>>`, `let lock: Locker`)
fn typed_names<'a>(code: &'a str, ty: &str) -> Vec<&'a str> {
  let mut names = Vec::new();
  for at in words(code, ty) {
    let segment = &code[code[..at].rfind([',', '(', '{', ';', '\n']).map_or(0, |start| start + 1)..at];
    let Some(colon) = segment.find(':').filter(|&colon| segment.as_bytes().get(colon + 1) != Some(&b':')) else {
      continue;
    };
    // the rest is a type up to `ty` (`std::sync::Arc<`): no `=` or call, no `)` or `->` of a signature
    if !segment[colon + 1..].bytes().all(|byte| is_word(byte) || b" &<':".contains(&byte)) {
      continue;
    }
    let declared = segment[..colon].trim_end();
    let name = &declared[declared.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_').len()..];
    if name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') {
      names.push(name);
    }
  }
  names
}

/// The names the file of `code` gives a service: a parameter, a field or a binding typed with one (`github: &GitHubService`,
/// `patronus: Option<Arc<PatronusService>>`), and a binding whose initializer builds one (`GitHubService::new(…)`) or
/// ends with the call of an accessor or a constructor of one (`services.github(&key)?`, `github_service(context, environment)`)
fn service_names<'a>(code: &'a str, all: &[Binding<'a>]) -> Vec<&'a str> {
  let mut names: Vec<&str> = SERVICES.iter().flat_map(|service| typed_names(code, service)).collect();
  for binding in all {
    let value = &code[binding.value.clone()];
    let builds = SERVICES
      .iter()
      .any(|service| words(value, service).any(|at| value[at + service.len()..].trim_start().starts_with(['{', ':'])));
    let made = SERVICE_ACCESSORS.iter().chain(&SERVICE_CONSTRUCTORS).any(|name| {
      calls(value, name, true, |_| true).any(|at| {
        let arguments = &value[at + value[at..].find('(').expect("a call has its parenthesis") + 1..];
        first_at_top(arguments, |_, _| false).is_some_and(|close| arguments[close + 1..].bytes().all(|byte| b"?) \n".contains(&byte)))
      })
    });
    if builds || made {
      names.extend(&binding.names);
    }
  }
  names
}

/// Whether `scope` calls the network; `services`: the names of the file's services
fn reaches_network(scope: &str, services: &[&str]) -> bool {
  NETWORK_CALLS.iter().any(|name| calls(scope, name, false, |_| true).next().is_some())
    || scope.contains("push::send(")
    || scope.contains(".send()")
    || words(scope, "reqwest").next().is_some()
    || SERVICE_ACCESSORS
      .iter()
      .any(|name| calls(scope, name, true, |before| before.trim_end().ends_with('.')).next().is_some())
    || services.iter().any(|service| {
      words(scope, service).any(|at| {
        let after = scope[at + service.len()..].trim_start();
        after.strip_prefix('.').is_some_and(|method| {
          let method = method.trim_start();
          let name = method.bytes().take_while(|&byte| is_word(byte)).count();
          name > 0 && (method[name..].starts_with('(') || method[name..].starts_with("::<"))
        })
      })
    })
}

/// The lines of `source` that bind the guard of an async mutex or read-write lock that lives across another `.await`
fn guards_across_await(source: &Source) -> Vec<usize> {
  let code = code_text(&source.text);
  let all = bindings(&code);
  all
    .iter()
    .filter(|binding| {
      let value: String = code[binding.value.clone()].split_whitespace().collect();
      ASYNC_GUARDS.iter().any(|guard| value.ends_with(guard)) && words(lifetime(&code, &all, binding), "await").next().is_some()
    })
    .map(|binding| line_at(&code, binding.at))
    .collect()
}

/// The lines of `source` that bind the repository lock where it lives across a call of the network
fn locks_across_network(source: &Source) -> Vec<usize> {
  locks_across(source, reaches_network)
}

/// The lines of `source` that bind the repository lock where `reaches` finds a call in its life; `reaches` gets that
/// life and the names of the file's services
fn locks_across(source: &Source, reaches: impl Fn(&str, &[&str]) -> bool) -> Vec<usize> {
  let code = code_text(&source.text);
  let all = bindings(&code);
  let services = service_names(&code, &all);
  let takers = lock_takers(&code, &typed_names(&code, "Locker"));
  all
    .iter()
    // a `Locker` holds the closure that takes the lock, not the lock
    .filter(|binding| binding.ty.as_ref().is_none_or(|ty| words(&code[ty.clone()], "Locker").next().is_none()))
    .filter(|binding| takes_repository_lock(&code[binding.value.clone()], &takers) && reaches(lifetime(&code, &all, binding), &services))
    .map(|binding| line_at(&code, binding.at))
    .collect()
}

#[test]
fn no_guard_of_an_async_mutex_lives_across_another_await() {
  check(
    "the guard of an async mutex or read-write lock lives across another `.await` of its scope, so whoever waits for it waits for that",
    "release it before the `.await` (`drop(guard)`, or a block of its own); a read that the others wait for is one shared future or a `watch`, not a mutex (rust-domain-authority.md, «Инварианты», 2)",
    &EXEMPT_GUARDS,
    guards_across_await,
  );
}

#[test]
fn no_repository_lock_lives_across_the_network() {
  check(
    "the repository lock lives across a call of the network, so every rewrite of every process waits for the remote",
    "read the remote before the lock and send after it: under it only the witness, the freeze of the exact oids and leases and the compare-and-swap write (rust-domain-authority.md, «Инварианты», 1 and 2)",
    &EXEMPT_LOCKS,
    locks_across_network,
  );
}

/// The lines of `cases` that end with the comment `// hit`, and the source of them
fn marked(cases: &[&str]) -> (Source, Vec<usize>) {
  let text = cases.join("\n");
  let hits = text.lines().enumerate().filter(|(_, line)| line.ends_with("// hit")).map(|(index, _)| index + 1).collect();
  (Source { path: "x.rs".to_owned(), text }, hits)
}

#[test]
fn a_guard_lives_to_the_end_of_its_block_or_its_drop_there_and_on_in_the_binding_its_block_hands_it_to() {
  let (source, hits) = marked(&[
    "async fn across_a_read(&self) {",
    "  let mut state = self.state.lock().await; // hit",
    "  *state = read().await;",
    "}",
    "async fn dropped_before_the_await(&self) {",
    "  let state = self.state.lock().await;",
    "  let value = state.clone();",
    "  drop(state);",
    "  send(value).await;",
    "}",
    "async fn its_block_ends_first(&self) {",
    "  {",
    "    let _turn = self.turn.write().await;",
    "    count += 1;",
    "  }",
    "  next().await;",
    "  let value = self.state.lock().await.clone();",
    "  let _ = self.state.lock().await;",
    "  // let x = self.state.lock().await; x.await",
    "  let text = \"let y = z.lock().await; w.await\";",
    "  later().await;",
    "}",
    "async fn dropped_in_one_branch(&self) {",
    "  let renewal = self.renewal.lock().await; // hit",
    "  if again { drop(renewal); return; }",
    "  refresh().await;",
    "}",
    "async fn handed_on(&self, reads: Option<&Mutex<()>>) {",
    "  let _reading = match reads {",
    "    Some(reads) => {",
    "      let reading = reads // hit",
    "        .lock()",
    "        .await;",
    "      Some(reading)",
    "    }",
    "    None => None,",
    "  };",
    "  list().await;",
    "}",
    "async fn moved_into_a_task(&self) {",
    "  let command = in_flight.read_owned().await; // hit",
    "  tokio::spawn(async move { let _command = command; next.run(request).await });",
    "}",
  ]);
  assert_eq!(guards_across_await(&source), hits);
}

#[test]
fn a_repository_lock_lives_across_no_call_of_the_network() {
  let (source, hits) = marked(&[
    "fn merge_under_the_lock(options: &RewriteOptions, git: &GitCommandExecutor) -> Result<()> {",
    "  let (prepared, _lock) = options.lock_prepared(git, path, prepare()?, prepare)?;",
    "  git.execute_command(&[\"merge-tree\", \"--write-tree\", &base])?;",
    "  write(prepared)",
    "}",
    "fn fetch_under_the_lock(git: &GitCommandExecutor, options: &UpdateOptions) -> Result<()> {",
    "  let lock = options.lock_wait.lock_with(git, path, &|| {})?; // hit",
    "  fetch_baseline(git, path, &remote)?;",
    "  drop(lock);",
    "  Ok(())",
    "}",
    "fn push_after_the_lock(git: &GitCommandExecutor) -> Result<()> {",
    "  let frozen = {",
    "    let _lock = lock_repository(git, path, timeout)?;",
    "    freeze(git)?",
    "  };",
    "  let lock = lock_repository_in(dir, timeout, &|| {})?;",
    "  let tips = read_refs(git)?;",
    "  drop(lock);",
    "  execute_remote_command(git, &[\"push\", &frozen])?;",
    "  Ok(())",
    "}",
    "fn a_service_under_the_lock(git: &GitCommandExecutor, github: &std::sync::Arc<GitHubService>) -> Result<()> {",
    "  let lock = try_lock_repository(git, path)?; // hit",
    "  github",
    "    .pull_requests(&repository)?;",
    "  Ok(())",
    "}",
    "fn blocked_on_a_future(wait: LockWait<'_>, git: &GitCommandExecutor) -> Result<()> {",
    "  let (undo, _lock) = lock_checked_undo(wait, git, path, check)?; // hit",
    "  runtime.block_on(request(undo));",
    "  Ok(())",
    "}",
    "fn the_lock_of_bd_and_an_accessor(context: &RepoContext, services: &dyn ServiceHost) -> Result<()> {",
    "  let _lock = lock(context)?; // hit",
    "  services.patronus(&key)?;",
    "  Ok(())",
    "}",
    "fn a_mutex_and_a_lock_wait(&self) -> Result<()> {",
    "  let state = self.state.lock();",
    "  client.get(url).send()?;",
    "  let held = match self.wait.lock(git, path) { Ok(held) => held, Err(error) => return Err(error) }; // hit",
    "  client.get(url).send()?;",
    "  Ok(())",
    "}",
    "fn a_locker(dir: PathBuf) -> Result<()> {",
    "  let lock: Locker = Arc::new(move |on_wait, canceled| lock_repository_while_held(&dir, timeout, on_wait, canceled));",
    "  block_on(start());",
    "  let held = lock(&on_wait, &canceled)?; // hit",
    "  reqwest::get(url)?;",
    "  Ok(())",
    "}",
    "use bd_core::sync_command::{SyncOutput, lock, sync};",
  ]);
  assert_eq!(locks_across_network(&source), hits);
}

// A std mutex across git: a check of its own, apart from the async guards and the repository lock above. The guard of
// a `std` mutex is held only to look up and to keep, never across a git process: whoever takes the mutex meanwhile (a
// view of the session and the checks of a push share the memo of its content walks) would wait for that git. A guard
// is a `let` binding whose initializer ends with `lock()` without an argument or with the call of a function of its
// crate that answers a `MutexGuard` (`lock_state`), as it is or unwrapped (`unwrap()`, `expect(…)`,
// `unwrap_or_else(…)`, `?`), or one typed `MutexGuard`; a guard lives as the guards above do. Git is a name
// the file gives a `GitCommandExecutor` (a parameter, a field, a binding typed with it or made by it or by `git()`),
// used in the life of the guard; a guard handed to a call (`&mut state.lock().unwrap()`) is one when its statement uses
// such a name too. As above, a call of git inside a function the scope calls is not followed.

/// The std mutexes whose turn is the git itself: an exemption names its file, reason and count as above
const EXEMPT_STD_GUARDS: [Exemption; 4] = [
  Exemption {
    path: "crates/branch-integration/src/archive.rs",
    hits: 2,
    reason: "the archive renames and deletes of this process take turns over the git that names and writes the refs of one namespace: the turn is what the mutex is for",
  },
  Exemption {
    path: "crates/sync-core/src/cache_memo.rs",
    hits: 1,
    reason: "the mutex owns the one `cat-file --batch-check` process of the memo: its guard is the turn on that pipe",
  },
  Exemption {
    path: "crates/sync-core/src/repository_config.rs",
    hits: 1,
    reason: "one write of a config file at a time in this process: its guard is the turn of the write",
  },
  Exemption {
    path: "crates/bd-core/src/context.rs",
    hits: 3,
    reason: "one read of the config, the remotes and HEAD for the whole command of `bd`: a caller that comes meanwhile waits for that read instead of running the same git (a single flight of blocking code)",
  },
];

/// `value` without the unwrapping of its end: `?`, `unwrap()`, `expect(…)`, `unwrap_or_else(…)`
fn unwrapped(value: &str) -> &str {
  let mut value = value.trim_end();
  loop {
    let before = value;
    value = value.trim_end_matches('?').trim_end();
    if let Some(stripped) = value.strip_suffix(".unwrap()") {
      value = stripped;
    } else if value.ends_with(')') {
      for call in [".expect(", ".unwrap_or_else("] {
        if let Some(at) = value.rfind(call)
          && first_at_top(&value[at + call.len()..], |_, _| false).is_some_and(|close| at + call.len() + close + 1 == value.len())
        {
          value = &value[..at];
          break;
        }
      }
    }
    if value == before {
      return value;
    }
  }
}

/// The functions of `code` that answer the guard of a std mutex (`-> MutexGuard<…>`), by name
fn guard_functions_of(code: &str) -> Vec<String> {
  words(code, "fn")
    .filter_map(|at| {
      let after = code[at + 2..].trim_start();
      let name = &after[..after.bytes().take_while(|&byte| is_word(byte)).count()];
      let signature = &after[..after.find(['{', ';']).unwrap_or(after.len())];
      let answer = signature.rsplit_once("->")?.1;
      (!name.is_empty() && words(answer, "MutexGuard").next().is_some()).then(|| name.to_owned())
    })
    .collect()
}

/// The crate of the source at `path` (`crates/repository-model/src/facts.rs`: `crates/repository-model`)
fn crate_of(path: &str) -> &str {
  path.find("/src/").map_or(path, |end| &path[..end])
}

/// The functions that answer the guard of a std mutex, by crate (`guard_functions_of`)
fn guard_functions() -> &'static HashMap<String, Vec<String>> {
  static BY_CRATE: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
  BY_CRATE.get_or_init(|| {
    let mut by_crate: HashMap<String, Vec<String>> = HashMap::new();
    for source in production_sources() {
      by_crate
        .entry(crate_of(&source.path).to_owned())
        .or_default()
        .extend(guard_functions_of(&code_text(&source.text)));
    }
    by_crate
  })
}

/// The name of the call `value` ends with (`state.lock()`: `lock`, `lock_state(&self.state)`: `lock_state`)
fn last_call(value: &str) -> Option<&str> {
  let inner = value.strip_suffix(')')?;
  let mut depth = 0usize;
  let open = inner.bytes().enumerate().rev().find_map(|(index, byte)| match byte {
    b')' | b']' => {
      depth += 1;
      None
    }
    b'(' | b'[' if depth > 0 => {
      depth -= 1;
      None
    }
    b'(' => Some(index),
    _ => None,
  })?;
  let before = &inner[..open];
  let name = &before[before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_').len()..];
  (!name.is_empty()).then_some(name)
}

/// Whether `value`, the initializer of a binding, holds the guard of a std mutex; `guards`: the functions of its crate
/// that answer one; a closure that takes one holds none
fn holds_std_guard(value: &str, guards: &[String]) -> bool {
  let value: String = value.split_whitespace().collect();
  if value.starts_with('|') || value.starts_with("move|") {
    return false;
  }
  let value = unwrapped(&value);
  value.ends_with(".lock()") || last_call(value).is_some_and(|name| name != "lock" && guards.iter().any(|guard| guard == name))
}

/// The names the file of `code` gives a `GitCommandExecutor`: typed with it, or bound to one it makes or takes
fn executor_names<'a>(code: &'a str, all: &[Binding<'a>]) -> Vec<&'a str> {
  let mut names = typed_names(code, "GitCommandExecutor");
  for binding in all {
    let value: String = code[binding.value.clone()].split_whitespace().collect();
    if value.contains("GitCommandExecutor::") || unwrapped(&value).ends_with(".git()") || value.contains("git_of(") {
      names.extend(&binding.names);
    }
  }
  names
}

/// Whether `scope` uses one of the executors `executors`
fn uses_executor(scope: &str, executors: &[&str]) -> bool {
  executors.iter().any(|name| words(scope, name).next().is_some())
}

/// The lines of `source` that hold the guard of a std mutex across git: a binding that lives across a use of an executor,
/// or a guard handed to a call in a statement that uses one
fn std_guards_across_git(source: &Source) -> Vec<usize> {
  let code = code_text(&source.text);
  let all = bindings(&code);
  let executors = executor_names(&code, &all);
  if executors.is_empty() {
    return Vec::new();
  }
  let mut guards = guard_functions().get(crate_of(&source.path)).cloned().unwrap_or_default();
  // the sources of a test of this scan are of no crate
  guards.extend(guard_functions_of(&code));
  let mut lines: Vec<usize> = all
    .iter()
    .filter(|binding| holds_std_guard(&code[binding.value.clone()], &guards) || binding.ty.as_ref().is_some_and(|ty| words(&code[ty.clone()], "MutexGuard").next().is_some()))
    .filter(|binding| uses_executor(lifetime(&code, &all, binding), &executors))
    .map(|binding| line_at(&code, binding.at))
    .collect();
  for at in code.match_indices("&mut ").map(|(at, _)| at) {
    let argument = &code[at + "&mut ".len()..];
    let end = first_at_top(argument, |text, index| text.as_bytes()[index] == b',').unwrap_or(argument.len());
    if !holds_std_guard(&argument[..end], &guards) {
      continue;
    }
    let start = code[..at].rfind([';', '{', '}']).map_or(0, |start| start + 1);
    let statement_end = code[at..].find(';').map_or(code.len(), |end| at + end);
    if uses_executor(&code[start..statement_end], &executors) {
      lines.push(line_at(&code, at));
    }
  }
  lines.sort_unstable();
  lines.dedup();
  lines
}

#[test]
fn no_guard_of_a_std_mutex_lives_across_git() {
  check(
    "the guard of a std mutex lives across a git process, so whoever takes the mutex meanwhile waits for that git",
    "take the mutex to look up what is kept and again to keep the answer, and run git between without it (`repository_model::patronus_view::ContentMemo`)",
    &EXEMPT_STD_GUARDS,
    std_guards_across_git,
  );
}

#[test]
fn a_std_guard_lives_across_no_git_and_is_handed_to_no_call_that_runs_it() {
  let (source, hits) = marked(&[
    "fn across_git(memo: &Mutex<Memo>, git: &GitCommandExecutor) {",
    "  let kept = memo.lock().unwrap_or_else(PoisonError::into_inner); // hit",
    "  walk(git, &kept);",
    "}",
    "fn looked_up_then_free(&self, memo: &Mutex<Memo>) {",
    "  let known = {",
    "    let kept = self.memo.lock().unwrap();",
    "    kept.get(key).cloned()",
    "  };",
    "  let answer = walk(&self.git, known);",
    "  let mut kept = self.memo.lock().expect(\"the memo\");",
    "  kept.insert(answer);",
    "}",
    "fn dropped_first(state: &Mutex<State>, services: &Services) {",
    "  let git = services.git();",
    "  let state = state.lock()?;",
    "  let value = state.clone();",
    "  drop(state);",
    "  git.execute_command(&[\"status\"]);",
    "  let held: MutexGuard<'_, State> = lock_state(); // hit",
    "  git.execute_command(&[\"status\"]);",
    "}",
    "fn a_closure_that_locks(&self) {",
    "  let memo = || self.memo.lock().unwrap();",
    "  walk(&self.git_executor, memo().get(key));",
    "}",
    "fn lock_memo(memo: &Mutex<Memo>) -> MutexGuard<'_, Memo> {",
    "  memo.lock().unwrap_or_else(PoisonError::into_inner)",
    "}",
    "fn derived(git: &GitCommandExecutor, memo: &Mutex<Memo>) {",
    "  derive_patronus(git, &of, &mut lock_memo(memo)); // hit",
    "}",
    "fn handed(git_executor: &GitCommandExecutor, memo: &Mutex<Memo>) {",
    "  derive(git_executor, &of, &mut memo.lock().unwrap()); // hit",
    "  derive_cached(&of, &mut memo.lock().unwrap());",
    "  let lock = options.lock_wait.lock(git_executor, path)?;",
    "  let value = memo.lock().unwrap().clone();",
    "  git_executor.execute_command(&[\"status\"]);",
    "}",
  ]);
  assert_eq!(std_guards_across_git(&source), hits);
}

// The repository lock across a sync or a wait for the index: a check of its own, beside the one of the network above.
// A sync (`bd_core::sync_command::sync`, `sync_core::sync::sync_branches`) merges and copies every branch, and its refs
// move by compare-and-swap, so it takes no lock (`bd push` and `bd safe-push` held it for theirs until 2026-10-31); a
// wait for the `index.lock` of another process (an IDE's `git status`, up to 30 s) comes before the lock
// (`update_branch::wait_for_index`, `PickPlan::wait_for_index_locks`, `IndexLock::wait_while_locked`), and under it
// a move only looks (`index_held`). The guard lives as above; a call inside a function the scope calls is not followed
// (the fallback wait of a move under the lock its caller holds, `bd update --continue`).

/// The calls, by their whole name, that the repository lock never lives across
const LONG_CALLS: [&str; 5] = ["sync", "sync_branches", "wait_while_locked", "wait_for_index", "wait_for_index_locks"];

/// No repository lock lives across a sync or a wait for the index today; an exemption names its file, reason and count
const EXEMPT_LONG_LOCKS: [Exemption; 0] = [];

/// Whether `scope` calls a sync or waits for the index lock
fn reaches_a_long_call(scope: &str, _services: &[&str]) -> bool {
  LONG_CALLS.iter().any(|name| calls(scope, name, true, |_| true).next().is_some())
}

/// The lines of `source` that bind the repository lock where it lives across a sync or a wait for the index lock
fn locks_across_a_long_call(source: &Source) -> Vec<usize> {
  locks_across(source, reaches_a_long_call)
}

#[test]
fn no_repository_lock_lives_across_a_sync_or_a_wait_for_the_index() {
  check(
    "the repository lock lives across a sync or a wait for the index lock of another process, so every writer of every process waits for its merges or for git's lock",
    "sync before or after the lock (its refs move by compare-and-swap) and wait for the index before it (`wait_for_index`, `lock_for_files`); under it only the witness and the write (git-operations.md, \"Lock spans\", \"Index lock\")",
    &EXEMPT_LONG_LOCKS,
    locks_across_a_long_call,
  );
}

#[test]
fn a_repository_lock_lives_across_no_sync_and_no_wait_for_the_index() {
  let (source, hits) = marked(&[
    "fn the_sync_under_the_lock(context: &RepoContext) -> Result<SyncOutput, CliError> {",
    "  let _lock = lock(context)?; // hit",
    "  sync(context)",
    "}",
    "fn the_sync_after_the_lock(context: &RepoContext) -> Result<SyncOutput, CliError> {",
    "  let synced = sync(context)?;",
    "  let lock = lock(context)?;",
    "  witness(&synced)?;",
    "  drop(lock);",
    "  Ok(synced)",
    "}",
    "fn the_wait_under_the_lock(options: &RewriteOptions, git: &GitCommandExecutor, plan: &PickPlan) -> Result<()> {",
    "  let lock = options.lock(git, target)?; // hit",
    "  plan.wait_for_index_locks(git, target)?;",
    "  Ok(())",
    "}",
    "fn the_wait_before_the_lock(wait: LockWait<'_>, git: &GitCommandExecutor) -> Result<()> {",
    "  wait_for_index(git, path)?;",
    "  let lock = wait.lock_with(git, path, &|| {})?;",
    "  if index_held(git, path) { drop(lock); }",
    "  Ok(())",
    "}",
    "fn a_sync_of_the_core_under_its_lock(git: &GitCommandExecutor) -> Result<()> {",
    "  let held = try_lock_repository(git, path)?; // hit",
    "  runtime_sync_branches(git);",
    "  sync_branches(git, path, prefix, reporter, options);",
    "  Ok(())",
    "}",
    "use bd_core::sync_command::{SyncOutput, lock, sync};",
  ]);
  assert_eq!(locks_across_a_long_call(&source), hits);
}
