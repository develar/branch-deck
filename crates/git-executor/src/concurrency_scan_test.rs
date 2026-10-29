//! Nothing across the network or a wait (docs/design/rust-domain-authority.md, «Инварианты», 2; AGENTS.md, "Repository
//! lock"). The guard of an async mutex or read-write lock lives across no other `.await` of its scope: a read that the
//! others wait for is one shared future or a `watch` (single-flight), not a mutex. The guard of the repository lock lives
//! across no call of the network: a git of the remote (`execute_remote_command*`), the fetch and the push of the baseline
//! (`fetch_baseline`, `push_baseline`), the leased push (`push_leased`, `push::send`), HTTP (`reqwest`, the `.send()` of a
//! request, a method of a `GitHubService`, `PatronusService`, `YouTrackService` or `ServiceFactSources` or of what the
//! hosts answer for one), a runtime blocked on a future (`block_on`).
//!
//! Checked on the production sources like the git scans (`source_scan_test.rs`), on their code without literals and
//! comments, by the `let` bindings: a guard lives from its statement to the end of the block that holds it or to the
//! `drop` of its name in that block (of the last name of a tuple, where `lock_prepared` puts the lock), and a guard that
//! its block hands on as its value (`Some(guard)`) lives on in the binding that takes it. A guard handed to a function,
//! kept in a field or returned is not followed, nor a call of the network inside a function the scope calls: the lock span
//! tests of the cores (`test_utils::lock_span`) name every git that runs under the lock. A file is exempt only with its
//! reason and its exact number of hits.

use crate::source_scan_test::{Exemption, Source, check, code_text, line_at};
use std::ops::Range;

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

/// The guards across an `.await` today, each until the step of s14-fence that removes it (s14-2b)
const EXEMPT_GUARDS: [Exemption; 6] = [
  Exemption {
    path: "crates/github/src/token.rs",
    hits: 1,
    reason: "s14-2b: the slot of a token source is held across its read (a process, a file): one shared future of the read instead",
  },
  Exemption {
    path: "crates/youtrack/src/service.rs",
    hits: 2,
    reason: "s14-2b: the store lock across the write of the token file (`replace_stored_with`), the renewal lock across the refresh request to Hub (`access`): one shared renewal future instead",
  },
  Exemption {
    path: "crates/remote-facts/src/robot_stats.rs",
    hits: 1,
    reason: "s14-2b: the read of the statistics of a repository and target holds its mutex across the lists of Patronus: one shared read instead",
  },
  Exemption {
    path: "crates/operations/src/registry.rs",
    hits: 1,
    reason: "s14-2b: the turn of a reconcile of a repository across the reads of its registry and of Patronus (`reconcile_after_robots`): one reconcile future per repository that the next list joins",
  },
  Exemption {
    path: "src-tauri/src/services.rs",
    hits: 1,
    reason: "s14-2b: the TeamCity CLI is looked for under its mutex, across the process that finds it: one shared future of the discovery instead",
  },
  Exemption {
    path: "crates/test-server/src/lib.rs",
    hits: 1,
    reason: "s14-2b: the mock server holds the in-flight read lock of a repository across its command, so a deletion waits for the commands: a count of the commands in flight that the deletion waits out instead",
  },
];

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
  let code = code_text(&source.text);
  let all = bindings(&code);
  let services = service_names(&code, &all);
  let takers = lock_takers(&code, &typed_names(&code, "Locker"));
  all
    .iter()
    // a `Locker` holds the closure that takes the lock, not the lock
    .filter(|binding| binding.ty.as_ref().is_none_or(|ty| words(&code[ty.clone()], "Locker").next().is_none()))
    .filter(|binding| takes_repository_lock(&code[binding.value.clone()], &takers) && reaches_network(lifetime(&code, &all, binding), &services))
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
