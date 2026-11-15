//! Nothing across the network or a wait (docs/design/rust-domain-authority.md, «Инварианты», 2; AGENTS.md, "Repository
//! lock"). The guard of an async mutex or read-write lock lives across no other `.await` of its scope: a read that the
//! others wait for is one shared future or a `watch` (single-flight), not a mutex. The guard of the repository lock, and
//! of every other file lock (`lock_functions`, below), lives across no call of the network: a git of the remote
//! (`execute_remote_command*`), the fetch and the push of the baseline (`fetch_baseline`, `push_baseline`), the leased push (`push_leased`, `push::send`), HTTP (`reqwest`, the `.send()` of a
//! request, a method of a `GitHubService`, `PatronusService`, `YouTrackService` or `ServiceFactSources` or of what the
//! hosts answer for one), a runtime blocked on a future (`block_on`); nor across a sync or a wait for the index lock of
//! another process (its own check, below).
//!
//! Checked on the production sources like the git scans (`source_scan_test.rs`), on their code without literals and
//! comments, by the `let` bindings: a guard lives from its statement to the end of the block that holds it or to the
//! `drop` of its name in that block (of the last name of a tuple, where `lock_prepared` puts the lock), and a guard that
//! its block hands on as its value (`Some(guard)`) lives on in the binding that takes it. A lock also lives in the block
//! of an `if let`, a `while let` or a `match` on it (the value it matches lives to its end: `if let
//! PressureTurn::Taken(_pressure) = pressure_turn(…)`), in the body of a function that takes it as a parameter, by value
//! or by reference (`lock: &RepositoryLock`, `held: &Option<RepositoryLock>`: the caller holds it while the body runs),
//! from an assignment of it, or of a name a holder bound, to a variable bound before (`clones_lock = Some(lock)`) to the
//! end of that variable's block or its release there (`drop(name)`, `drop(name.take())`, `name.take()`, `name = None`),
//! and from an assignment to a field (`holder.held = Some(lock_file(…)?)`, through a `&mut` or `self`) to the end of the
//! block. A guard handed to a function that keeps it in a field or returns it is not followed, nor an `impl Trait`
//! answer (`-> impl Drop`), nor a lock erased into a `Box<dyn Any>`, nor a call of the network inside a function the
//! scope calls: the lock span tests of the cores (`test_utils::lock_span`) name every git that runs under the lock. A
//! file is exempt only with its reason and its exact number of hits.
//!
//! A lock is the call of a function whose answer holds a `RepositoryLock` (`lock_functions`: every one of the scanned
//! sources, whatever its name), of what the file types as a `Locker` (the closure of a start that takes it), or of a
//! closure that takes one, with or without an argument: a parameter whose closure type answers a lock (`take: impl
//! FnOnce() -> Result<RepositoryLock>`), a binding whose closure body calls a lock function (`let take = || lock_file(…)`).
//! A type holds one by a field, a variant's field or an alias (`lock_types`: `CloneTurn::Locked { lock }`,
//! `PressureTurn`, `OwnFetch`, `type Held = RepositoryLock`, to the fixed point), in an `Option`, a `Result`, a `Vec`, a
//! `Box` or a tuple, also behind a reference (`RewriteOptions::held_lock`: the lock of a caller, held while the borrow
//! lives); not as the answer of a closure type (`Locker`), not in another container (`Arc`, a map: out of scope). So the repository lock (`lock_repository*`, `LockWait::lock*`,
//! `RewriteOptions::lock_prepared`, the `lock` of `bd`) and every file lock of `repository_lock::lock_file` are one
//! class with every wrapper of one: the locks of the files of a lease and of the companions (`lock_lease`,
//! `lock_landed`, `lock_state`, `lock_companions`, under which the reads and writes of git config are by design), of the
//! journal and of the operations registry, and `StateFile::lock_at` of the pool, each held across the read and the write
//! of a few files and never across the network or a sync.

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

/// The one lock held across the network by design; an exemption here names its file, reason and count as above
const EXEMPT_LOCKS: [Exemption; 1] = [Exemption {
  path: "crates/sync-core/src/baseline.rs",
  hits: 1,
  reason: "`fetch_baseline` holds its own turn at the fetch lock of the tracking ref (`FetchTurn::Own`, `OwnFetch`) across \
           that fetch: a second fetch of the ref, of any process, waits for it and takes its result, one transfer for both \
           (`fetch_lock`); it is no repository lock, and nothing but fetches waits for it",
}];

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
    let Some(equals) = binding_equals(statement) else { continue };
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

/// The `=` of the binding that `text` starts after its `let`, not of `==`, `=>`, `<=`, `>=` or `!=`
fn binding_equals(text: &str) -> Option<usize> {
  top_byte(text, b'=', |bytes, index| {
    !matches!(bytes.get(index + 1), Some(b'=' | b'>')) && !matches!(bytes.get(index.wrapping_sub(1)), Some(b'=' | b'!' | b'<' | b'>'))
  })
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

/// Whether `value`, the initializer of a binding, calls one of `lockers` by its bare name with an argument: what the file
/// types as a `Locker` (`operations::land_execute`: the closure of a start that takes the lock)
fn calls_locker(value: &str, lockers: &[&str]) -> bool {
  let value = block_values(value);
  let value = value.as_str();
  lockers
    .iter()
    .any(|name| calls(value, name, true, |before| !before.ends_with([':', '.'])).any(|at| has_argument(value, at)))
}

/// Whether `value` calls one of `takers`, closures that take a lock, by its bare name, with or without an argument
fn calls_taker(value: &str, takers: &[&str]) -> bool {
  let value = block_values(value);
  let value = value.as_str();
  takers.iter().any(|name| calls(value, name, true, |before| !before.ends_with([':', '.'])).next().is_some())
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

/// The lines of `source` that take or keep the repository lock where `reaches` finds a call in its life; `reaches` gets
/// that life and the names of the file's services. A holder is a `let` whose initializer gives a lock, an `if let`, a
/// `while let` or a `match` on one (its block: the value it matches lives to its end), and an assignment of a lock, or
/// of a name a holder bound, to a variable bound before (`clones_lock = Some(lock)`), to the end of that variable's
/// block or its release there (`drop(name)`, `drop(name.take())`, `name.take()`, `name = None`)
fn locks_across(source: &Source, reaches: impl Fn(&str, &[&str]) -> bool) -> Vec<usize> {
  let code = code_text(&source.text);
  let all = bindings(&code);
  let services = service_names(&code, &all);
  let lockers = typed_names(&code, "Locker");
  // the lock types and functions of every scanned source, with those of this one, which a case of a test declares
  let types = lock_types_with(&code);
  let own = functions_answering(&code, &types);
  let functions: Vec<&LockFunction> = lock_functions().iter().chain(&own).collect();
  let parameters = lock_parameters(&code, &types);
  // the closures that take a lock, called with or without an argument: a parameter that answers one, a binding whose
  // body takes one (`let take = || lock_file(…)`)
  let mut takers: Vec<&str> = parameters.iter().filter(|parameter| parameter.takes).map(|parameter| parameter.name).collect();
  for binding in &all {
    if closure_body(&code[binding.value.clone()]).is_some_and(|body| calls_locker(body, &lockers) || calls_lock_function(body, &functions)) {
      takers.extend(&binding.names);
    }
  }
  let gives_lock = |value: &str| calls_locker(value, &lockers) || calls_taker(value, &takers) || calls_lock_function(value, &functions);
  let mut lines = Vec::new();
  let mut held: Vec<(&str, Range<usize>)> = Vec::new();
  for binding in &all {
    // a `Locker` or a closure holds what takes the lock, not the lock
    let value = &code[binding.value.clone()];
    if binding.ty.as_ref().is_some_and(|ty| words(&code[ty.clone()], "Locker").next().is_some()) || closure_body(value).is_some() || !gives_lock(value) {
      continue;
    }
    held.extend(binding.names.iter().map(|name| (*name, binding.scope.clone())));
    if reaches(lifetime(&code, &all, binding), &services) {
      lines.push(line_at(&code, binding.at));
    }
  }
  for block in conditional_bindings(&code) {
    if !gives_lock(&code[block.value.clone()]) {
      continue;
    }
    held.extend(block.names.iter().map(|name| (*name, block.scope.clone())));
    if reaches(&code[block.scope.clone()], &services) {
      lines.push(line_at(&code, block.at));
    }
  }
  for parameter in parameters.iter().filter(|parameter| !parameter.takes) {
    held.push((parameter.name, parameter.body.clone()));
    if reaches(&code[parameter.life.clone()], &services) {
      lines.push(line_at(&code, parameter.at));
    }
  }
  for assignment in assignments(&code, &all).into_iter().chain(field_assignments(&code)) {
    let value = &code[assignment.value.clone()];
    let names_a_lock = held.iter().any(|(name, scope)| {
      scope.contains(&assignment.value.start) && (value.trim() == *name || value.trim().strip_prefix("Some(").and_then(|rest| rest.strip_suffix(')')).map(str::trim) == Some(name))
    });
    if (names_a_lock || gives_lock(value)) && reaches(&code[assignment.life.clone()], &services) {
      lines.push(line_at(&code, assignment.at));
    }
  }
  lines.sort_unstable();
  lines.dedup();
  lines
}

/// The binding of an `if let`, a `while let` or a `match`: what it matches (`value`), the names its pattern or its arms
/// bind, and its block, where the value it matches lives
struct BlockBinding<'a> {
  at: usize,
  names: Vec<&'a str>,
  value: Range<usize>,
  scope: Range<usize>,
}

/// The `if let`, `while let` and `match` of `code`, each with its block (a let chain: from its first `let`, to the `&&`)
fn conditional_bindings(code: &str) -> Vec<BlockBinding<'_>> {
  let bytes = code.as_bytes();
  let mut found = Vec::new();
  for keyword in ["let", "match"] {
    for at in words(code, keyword) {
      let start = at + keyword.len();
      let (names, value_start) = if keyword == "let" {
        let before = code[..at].trim_end();
        if !["if", "while", "&&"].iter().any(|word| ends_with_word(before, word)) {
          continue;
        }
        let Some(equals) = binding_equals(&code[start..]) else { continue };
        (names_of(&code[start..start + equals]), start + equals + 1)
      } else {
        (Vec::new(), start)
      };
      // the block: the first `{` outside the parentheses and brackets of the value
      let mut depth = 0usize;
      let Some(open) = bytes[value_start..].iter().enumerate().find_map(|(index, &byte)| {
        match byte {
          b'(' | b'[' => depth += 1,
          b')' | b']' => depth = depth.saturating_sub(1),
          b'{' if depth == 0 => return Some(value_start + index),
          _ => {}
        }
        None
      }) else {
        continue;
      };
      let value = &code[value_start..open];
      let value_end = value_start + value.find("&&").unwrap_or(value.len());
      let Some(close) = first_at_top(&code[open + 1..], |_, _| false) else { continue };
      let names = if keyword == "match" {
        arm_patterns(&code[open + 1..open + 1 + close]).into_iter().flat_map(names_of).collect()
      } else {
        names
      };
      found.push(BlockBinding {
        at,
        names,
        value: value_start..value_end,
        scope: open + 1..open + 1 + close,
      });
    }
  }
  found.sort_by_key(|block| block.at);
  found
}

/// A parameter that a function takes a lock by (`lock: RepositoryLock`, `lock: Option<RepositoryLock>`): the lock lives
/// in its body to its end or the `drop` of the name
struct LockParameter<'a> {
  at: usize,
  name: &'a str,
  /// A closure that answers a lock (`take: impl FnOnce() -> Result<RepositoryLock>`): its calls take one, as a `Locker`
  takes: bool,
  body: Range<usize>,
  life: Range<usize>,
}

/// The parameters of the functions of `code` whose type holds one of `types` (`holds`)
fn lock_parameters<'a>(code: &'a str, types: &[String]) -> Vec<LockParameter<'a>> {
  let mut found = Vec::new();
  for at in words(code, "fn") {
    let after_fn = at + 2 + (code[at + 2..].len() - code[at + 2..].trim_start().len());
    let name_len = code[after_fn..].bytes().take_while(|&byte| is_word(byte)).count();
    let Some(generics) = generics_len(&code[after_fn + name_len..]) else { continue };
    let open = after_fn + name_len + generics;
    if !code[open..].starts_with('(') {
      continue;
    }
    let Some(close) = first_at_top(&code[open + 1..], |_, _| false).map(|close| open + 1 + close) else {
      continue;
    };
    // the body: the first `{` after the answer and a `where`, outside brackets; none for a declaration (`;`)
    let mut depth = 0usize;
    let Some(body_open) = code[close + 1..]
      .bytes()
      .enumerate()
      .find_map(|(index, byte)| {
        match byte {
          b'(' | b'[' | b'<' => depth += 1,
          b')' | b']' => depth = depth.saturating_sub(1),
          b'>' if code.as_bytes()[close + index] != b'-' => depth = depth.saturating_sub(1),
          b'{' if depth == 0 => return Some(Some(close + 1 + index)),
          b';' if depth == 0 => return Some(None),
          _ => {}
        }
        None
      })
      .flatten()
    else {
      continue;
    };
    let Some(body_close) = first_at_top(&code[body_open + 1..], |_, _| false).map(|end| body_open + 1 + end) else {
      continue;
    };
    let mut start = open + 1;
    while start < close {
      let end = top_byte(&code[start..close], b',', |_, _| true).map_or(close, |comma| start + comma);
      let parameter = &code[start..end];
      if let Some(colon) = top_byte(parameter, b':', |bytes, index| {
        bytes.get(index + 1) != Some(&b':') && bytes.get(index.wrapping_sub(1)) != Some(&b':')
      }) {
        let ty = &parameter[colon + 1..];
        let names = names_of(&parameter[..colon]);
        let takes = closure_answer(ty).is_some_and(|answer| types.iter().any(|lock| holds(answer, lock)));
        if let [name] = names.as_slice()
          && (takes || types.iter().any(|lock| holds(ty, lock)))
        {
          let body = &code[body_open + 1..body_close];
          let life = first_at_top(body, |text, index| drops(text, index, name)).unwrap_or(body.len());
          found.push(LockParameter {
            at: start + parameter.find(name).unwrap_or(0),
            name,
            takes,
            body: body_open + 1..body_close,
            life: body_open + 1..body_open + 1 + life,
          });
        }
      }
      start = end + 1;
    }
  }
  found
}

/// The patterns of the arms of the block of a `match`, without their guards (`Turn::Locked { lock, .. } if ready`)
fn arm_patterns(block: &str) -> Vec<&str> {
  let mut patterns = Vec::new();
  let mut rest = block;
  loop {
    rest = rest.trim_start();
    let bytes = rest.as_bytes();
    let Some(arrow) = first_at_top(rest, |_, index| bytes[index] == b'=' && bytes.get(index + 1) == Some(&b'>')).filter(|&arrow| bytes[arrow] == b'=') else {
      break;
    };
    let pattern = &rest[..arrow];
    patterns.push(words(pattern, "if").next().map_or(pattern, |guard| &pattern[..guard]));
    let body = rest[arrow + 2..].trim_start();
    let offset = rest.len() - body.len();
    let end = if let Some(block) = body.strip_prefix('{') {
      first_at_top(block, |_, _| false).map_or(body.len(), |close| close + 2)
    } else {
      top_byte(body, b',', |_, _| true).unwrap_or(body.len())
    };
    rest = rest[offset + end..].trim_start();
    rest = rest.strip_prefix(',').unwrap_or(rest);
  }
  patterns
}

/// The answer of a closure type or a function type (`impl FnOnce() -> Result<RepositoryLock>`, `&dyn Fn(&T) -> X`,
/// `fn() -> X`): the type after its `->` outside the parentheses of its parameters; `None` for another type
fn closure_answer(ty: &str) -> Option<&str> {
  let fn_at = ["FnOnce(", "FnMut(", "Fn(", "fn("]
    .iter()
    .filter_map(|name| ty.find(name).map(|at| at + name.len() - 1))
    .min()?;
  let close = fn_at + 1 + first_at_top(&ty[fn_at + 1..], |_, _| false)?;
  ty[close + 1..].trim_start().strip_prefix("->")
}

/// Whether the initializer `value` of a binding is a closure (`|| …`, `move |x| …`), and its body
fn closure_body(value: &str) -> Option<&str> {
  let value = value.trim_start();
  let value = value.strip_prefix("move").filter(|rest| rest.starts_with([' ', '|'])).map_or(value, str::trim_start);
  let rest = value.strip_prefix('|')?;
  let close = rest.find('|')?;
  Some(&rest[close + 1..])
}

/// The assignments to a field of `code` (`holder.held = Some(lock_file(…)?)`, through a `&mut` or `self`): a statement
/// whose left side is a path of names with a `.`; the field keeps what it is given, as this code sees it, to the end of
/// the block of the statement
fn field_assignments(code: &str) -> Vec<Assignment> {
  let bytes = code.as_bytes();
  let mut found = Vec::new();
  for (equals, _) in code.match_indices('=') {
    if matches!(bytes.get(equals + 1), Some(b'=' | b'>'))
      || matches!(
        bytes.get(equals.wrapping_sub(1)),
        Some(b'=' | b'!' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/' | b'|' | b'&' | b'^' | b'%')
      )
    {
      continue;
    }
    let start = code[..equals].rfind([';', '{', '}']).map_or(0, |at| at + 1);
    let target = code[start..equals].trim();
    let path = target.split('.').collect::<Vec<_>>();
    if path.len() < 2
      || !path
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(is_word) && !part.starts_with(|c: char| c.is_ascii_digit()))
    {
      continue;
    }
    let Some(end) = top_byte(&code[equals + 1..], b';', |_, _| true).map(|end| equals + 1 + end) else {
      continue;
    };
    let block_end = first_at_top(&code[end + 1..], |_, _| false).map_or(code.len(), |close| end + 1 + close);
    found.push(Assignment {
      at: start + (code[start..equals].len() - code[start..equals].trim_start().len()),
      value: equals + 1..end,
      life: end + 1..block_end,
    });
  }
  found
}

/// An assignment to a variable a `let` bound before (`name = value;`): its value, and where the value it gives lives:
/// to the end of the block of that `let`, or to the first release of the name there
struct Assignment {
  at: usize,
  value: Range<usize>,
  life: Range<usize>,
}

/// The assignments of `code` to the names of its bindings `all`, each in the scope of the binding of its name
fn assignments(code: &str, all: &[Binding<'_>]) -> Vec<Assignment> {
  let mut found = Vec::new();
  for binding in all {
    for name in &binding.names {
      let scope = &code[binding.scope.clone()];
      for at in words(scope, name) {
        // a statement of its own: `name = …;`, not `==`, `=>`, a field (`.name`), a path or a pattern
        let before = scope[..at].trim_end().bytes().next_back();
        if !matches!(before, None | Some(b';' | b'{' | b'}')) {
          continue;
        }
        let after = &scope[at + name.len()..];
        let Some(rest) = after.trim_start().strip_prefix('=') else { continue };
        if rest.starts_with(['=', '>']) {
          continue;
        }
        let value_start = binding.scope.start + at + name.len() + (after.len() - rest.len());
        let Some(end) = top_byte(&code[value_start..], b';', |_, _| true) else { continue };
        let life_start = value_start + end + 1;
        let life = &code[life_start..binding.scope.end];
        let release = [format!("drop({name})"), format!("drop({name}.take())"), format!("{name}.take()"), format!("{name} = None")]
          .iter()
          .filter_map(|release| life.find(release.as_str()))
          .min()
          .unwrap_or(life.len());
        found.push(Assignment {
          at: binding.scope.start + at,
          value: value_start..value_start + end,
          life: life_start..life_start + release,
        });
      }
    }
  }
  found
}

/// A function that answers a type, as `functions_answering` finds it: a `RepositoryLock` (`lock_functions_of`), a
/// `MutexGuard` (`guard_functions_of`)
#[derive(Debug, Clone, PartialEq, Eq)]
struct LockFunction {
  name: String,
  /// A method (its first parameter is `self`): called as `.name(…)`; a function is called as `name(…)`, bare or by its
  /// path, never after a `.`
  method: bool,
  /// It takes an argument beside `self`: a call without one is of another function of the name (the `lock()` of a
  /// mutex beside `LockWait::lock`)
  arguments: bool,
}

/// The functions that `code` declares with an answer that holds a `RepositoryLock` (`-> Result<RepositoryLock>`,
/// `-> std::io::Result<Vec<RepositoryLock>>`, `-> Result<(Prepared, RepositoryLock)>`, `-> Result<CloneTurn>`): one of
/// the lock types of every scanned source (`lock_types`), not behind a reference
fn lock_functions_of(code: &str) -> Vec<LockFunction> {
  functions_answering(code, lock_types())
}

/// The functions that `code` declares with an answer that holds one of the types `types` (not behind a reference): their
/// parameters to their closing parenthesis, past their generics, their answer from its `->` to the body, a `;` or a
/// `where`
fn functions_answering<T: AsRef<str>>(code: &str, types: &[T]) -> Vec<LockFunction> {
  words(code, "fn")
    .filter_map(|at| {
      let after = code[at + 2..].trim_start();
      let name = &after[..after.bytes().take_while(|&byte| is_word(byte)).count()];
      if name.is_empty() {
        return None;
      }
      let generics = generics_len(&after[name.len()..])?;
      let open = name.len() + generics;
      if !after[open..].starts_with('(') {
        return None;
      }
      let parameters = &after[open + 1..];
      let close = first_at_top(parameters, |_, _| false)?;
      let (parameters, rest) = (&parameters[..close], parameters[close + 1..].trim_start());
      let answer = rest.strip_prefix("->")?;
      let end = answer_end(answer);
      types.iter().any(|ty| holds(&answer[..end], ty.as_ref())).then_some(())?;
      let first = top_byte(parameters, b',', |_, _| true).map_or(parameters, |comma| &parameters[..comma]);
      let method = words(first, "self").next().is_some();
      let beside = if method {
        top_byte(parameters, b',', |_, _| true).map_or("", |comma| &parameters[comma + 1..])
      } else {
        parameters
      };
      Some(LockFunction {
        name: name.to_owned(),
        method,
        arguments: !beside.trim().is_empty(),
      })
    })
    .collect()
}

/// The length of the generics that start `text` (`<T, F: Fn(&T) -> bool>`), with the blanks before them; 0 without
/// them, None when they do not close
fn generics_len(text: &str) -> Option<usize> {
  let start = text.len() - text.trim_start().len();
  if !text[start..].starts_with('<') {
    return Some(start);
  }
  let bytes = text.as_bytes();
  let mut depth = 0usize;
  for (index, &byte) in bytes.iter().enumerate().skip(start) {
    match byte {
      b'<' => depth += 1,
      // the `>` of a `->` closes nothing
      b'>' if bytes[index - 1] != b'-' => {
        depth -= 1;
        if depth == 0 {
          return Some(index + 1 + text[index + 1..].len() - text[index + 1..].trim_start().len());
        }
      }
      b'{' | b';' => return None,
      _ => {}
    }
  }
  None
}

/// The end of the answer of a signature that starts `text`: its body (`{`), a `;` or a `where`, outside the parentheses
/// and the brackets of a tuple or an array in it
fn answer_end(text: &str) -> usize {
  let bytes = text.as_bytes();
  let mut depth = 0usize;
  for (index, &byte) in bytes.iter().enumerate() {
    match byte {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth = depth.saturating_sub(1),
      b'{' | b';' if depth == 0 => return index,
      b'w'
        if depth == 0
          && text[index..].starts_with("where")
          && !bytes[..index].last().copied().is_some_and(is_word)
          && !bytes.get(index + "where".len()).copied().is_some_and(is_word) =>
      {
        return index;
      }
      _ => {}
    }
  }
  text.len()
}

/// Whether `text` (an answer, the fields of a type) holds `ty`: as a field, a variant's field, the type of an alias or an
/// answer, in `Option`, `Result`, `Vec`, `Box` and tuples, not behind a reference (`&'a RepositoryLock` borrows a lock
/// another holds), not the answer of a closure or a function type (`Arc<dyn Fn() -> Result<RepositoryLock>>`), not in
/// another container (`DashMap<String, T>`, `Arc<T>`: out of scope)
fn holds(text: &str, ty: &str) -> bool {
  words(text, ty).any(|at| held_at(text, at))
}

/// `holds` of the occurrence of a type at `at` of `text`
fn held_at(text: &str, at: usize) -> bool {
  let bytes = text.as_bytes();
  let mut index = at;
  loop {
    // the bracket that encloses `index`, outside the `>` of a `->`
    let mut depth = 0usize;
    let open = (0..index).rev().find(|&i| match bytes[i] {
      b')' | b']' | b'}' => {
        depth += 1;
        false
      }
      b'>' if i == 0 || bytes[i - 1] != b'-' => {
        depth += 1;
        false
      }
      b'(' | b'[' | b'{' | b'<' if depth == 0 => true,
      b'(' | b'[' | b'{' | b'<' => {
        depth -= 1;
        false
      }
      _ => false,
    });
    // an answer of a function type at its level: `Fn(…) -> Result<…>`
    let level = &text[open.map_or(0, |open| open + 1)..index];
    if answers_at_top(level) {
      return false;
    }
    let Some(open) = open else { return true };
    let name = text[..open].trim_end();
    let name = &name[name.trim_end_matches(is_word_char).len()..];
    match bytes[open] {
      b'{' => return true,
      b'(' if matches!(name, "Fn" | "FnMut" | "FnOnce" | "fn") => return false,
      b'(' => index = open,
      b'<' if matches!(name, "Option" | "Result" | "Vec" | "Box") => index = open - name.len(),
      _ => return false,
    }
  }
}

/// Whether `level`, the text of one bracket level before an item, has a `->` at its top after its last comma there
fn answers_at_top(level: &str) -> bool {
  let bytes = level.as_bytes();
  let mut depth = 0usize;
  let mut answers = false;
  for (index, &byte) in bytes.iter().enumerate() {
    match byte {
      b'(' | b'[' | b'{' | b'<' => depth += 1,
      b'>' if index > 0 && bytes[index - 1] == b'-' => answers |= depth == 0,
      b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
      b',' if depth == 0 => answers = false,
      _ => {}
    }
  }
  answers
}

fn is_word_char(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

/// A type that `code` declares, with what it holds: the fields of a struct, the variants of an enum, the type an alias
/// names
struct TypeDeclaration<'a> {
  name: &'a str,
  body: &'a str,
}

/// The structs, enums and aliases of `code`
fn type_declarations(code: &str) -> Vec<TypeDeclaration<'_>> {
  let mut found = Vec::new();
  for keyword in ["struct", "enum", "type"] {
    for at in words(code, keyword) {
      let after = code[at + keyword.len()..].trim_start();
      let name = &after[..after.bytes().take_while(|&byte| is_word(byte)).count()];
      if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
        continue;
      }
      let offset = code.len() - after.len() + name.len();
      let Some(generics) = generics_len(&code[offset..]) else { continue };
      let rest = &code[offset + generics..];
      let body = if keyword == "type" {
        let Some(equals) = rest.strip_prefix('=') else { continue };
        top_byte(equals, b';', |_, _| true).map(|end| &equals[..end])
      } else {
        // past a `where` clause to the fields; a unit struct holds nothing
        let open = rest.find(['{', '(', ';']);
        match open.map(|open| (open, rest.as_bytes()[open])) {
          Some((open, b'{' | b'(')) => first_at_top(&rest[open + 1..], |_, _| false).map(|close| &rest[open + 1..open + 1 + close]),
          _ => None,
        }
      };
      if let Some(body) = body {
        found.push(TypeDeclaration { name, body });
      }
    }
  }
  found
}

/// `RepositoryLock` and every type of `declarations` that holds one of `known` or of each other, to the fixed point:
/// a struct or an enum by a field, an alias by its type, through `Option`, `Result`, `Vec` and tuples
fn lock_types_of(known: Vec<String>, declarations: &[TypeDeclaration<'_>]) -> Vec<String> {
  let mut types = known;
  loop {
    let more: Vec<String> = declarations
      .iter()
      .filter(|declaration| !types.iter().any(|ty| ty == declaration.name) && types.iter().any(|ty| holds(declaration.body, ty)))
      .map(|declaration| declaration.name.to_owned())
      .collect();
    if more.is_empty() {
      types.sort_unstable();
      types.dedup();
      return types;
    }
    types.extend(more);
  }
}

/// The lock types of every scanned source (`lock_types_of`)
fn lock_types() -> &'static [String] {
  static ALL: OnceLock<Vec<String>> = OnceLock::new();
  ALL.get_or_init(|| {
    let codes: Vec<String> = production_sources().iter().map(|source| code_text(&source.text)).collect();
    let declarations: Vec<TypeDeclaration<'_>> = codes.iter().flat_map(|code| type_declarations(code)).collect();
    lock_types_of(vec!["RepositoryLock".to_owned()], &declarations)
  })
}

/// The lock types of every scanned source and of `code`, which a case of a test declares
fn lock_types_with(code: &str) -> Vec<String> {
  lock_types_of(lock_types().to_vec(), &type_declarations(code))
}

/// The lock functions of every scanned source (`lock_functions_of`), each name and kind once
fn lock_functions() -> &'static [LockFunction] {
  static ALL: OnceLock<Vec<LockFunction>> = OnceLock::new();
  ALL.get_or_init(|| {
    let mut all: Vec<LockFunction> = production_sources().iter().flat_map(|source| lock_functions_of(&code_text(&source.text))).collect();
    all.sort_by(|a, b| (&a.name, a.method).cmp(&(&b.name, b.method)));
    all.dedup_by(|a, b| {
      a.name == b.name && a.method == b.method && {
        b.arguments |= a.arguments;
        true
      }
    });
    all
  })
}

/// Whether `value`, the initializer of a binding, calls one of `functions` as what it gives (`block_values`)
fn calls_lock_function(value: &str, functions: &[&LockFunction]) -> bool {
  let value = block_values(value);
  let value = value.as_str();
  functions.iter().any(|function| {
    let mut found = calls(value, &function.name, true, |before| before.trim_end().ends_with('.') == function.method);
    found.any(|at| !function.arguments || has_argument(value, at))
  })
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

/// A lock function is one whose answer names `RepositoryLock`, whatever its name and wrapping, a method by its `self`,
/// with the arguments it takes beside it; a `RepositoryLock` in a parameter, a closure or a body is none
#[test]
fn a_lock_function_answers_a_repository_lock() {
  let code = code_text(
    &[
      "pub fn lock_file(path: PathBuf, timeout: Duration, on_wait: &dyn Fn()) -> Result<RepositoryLock> {",
      "  inner(path)",
      "}",
      "fn lock_companions(git: &GitCommandExecutor, paths: &[Option<&str>]) -> std::io::Result<Vec<RepositoryLock>> { todo!() }",
      "impl StateFile {",
      "  pub(crate) fn lock_at(&self, name: &str, timeout: Duration, waiting: &str) -> Result<RepositoryLock> { todo!() }",
      "  fn lock_prepared<T, F: Fn(&T) -> bool>(&self, git: &G, prepared: T, check: F) -> Result<(T, RepositoryLock)>",
      "  where",
      "    T: Clone,",
      "  { todo!() }",
      "  fn held(&self) -> Option<RepositoryLock>;",
      "}",
      "fn run_locked(lock: impl FnOnce() -> Result<RepositoryLock>) -> Result<()> { Ok(()) }",
      "fn reads(path: &Path) -> Result<String> { let _lock: RepositoryLock = lock_file(path)?; read(path) }",
      "fn answers_another(path: &Path) -> Result<String> where F: Fn() -> RepositoryLock { todo!() }",
    ]
    .join("\n"),
  );
  let found: Vec<(String, bool, bool)> = lock_functions_of(&code)
    .into_iter()
    .map(|function| (function.name, function.method, function.arguments))
    .collect();
  let expected = [
    ("lock_file", false, true),
    ("lock_companions", false, true),
    ("lock_at", true, true),
    ("lock_prepared", true, true),
    ("held", true, false),
  ];
  assert_eq!(found, expected.map(|(name, method, arguments)| (name.to_owned(), method, arguments)));
  let names: Vec<&str> = lock_functions().iter().map(|function| function.name.as_str()).collect();
  for name in [
    "lock_repository",
    "lock_file",
    "lock_at",
    "lock_lease",
    "lock_companions",
    "lock_prepared",
    "lock_with",
    "lock_clones",
    "pressure_turn",
    "take_fetch_turn",
  ] {
    assert!(names.contains(&name), "{name} is a lock function of the scanned sources: {names:?}");
  }
}

/// A type holds a lock by a field, a variant's field or an alias, in an `Option`, a `Result`, a `Vec`, a `Box` or a
/// tuple, also behind a reference (`held_lock: Option<&'a RepositoryLock>`: the lock lives while the borrow does), and so
/// does a type that holds one of those; not the answer of a closure type, not in another container
#[test]
fn a_lock_type_holds_a_repository_lock_by_a_field_a_variant_or_an_alias() {
  let code = code_text(
    &[
      "pub enum CloneTurn { SlotFreed { waited: Option<Duration> }, Locked { lock: RepositoryLock, waited: Option<Duration> } }",
      "enum PressureTurn { Taken(Option<RepositoryLock>), Waited }",
      "struct OwnFetch { lock: RepositoryLock, outcomes: PathBuf }",
      "pub enum FetchTurn { Joined { previous: Option<String> }, Own(OwnFetch) }",
      "type ReviewHeld = RepositoryLock;",
      "struct Locks(Vec<Box<RepositoryLock>>);",
      "struct Pair<T> where T: Clone { inner: Result<(T, ReviewHeld), String> }",
      "pub struct RewriteOptions<'a> { pub expect_head: Option<String>, pub held_lock: Option<&'a RepositoryLock> }",
      "struct Shared { lock: &'static mut RepositoryLock }",
      "type Locker = Arc<dyn Fn(&dyn Fn(Option<&LockHolder>), &dyn Fn() -> bool) -> Result<RepositoryLock, ApiError> + Send + Sync>;",
      "struct Repositories { by_id: DashMap<String, OwnFetch>, shared: Arc<RepositoryLock> }",
      "struct Unit;",
    ]
    .join("\n"),
  );
  let types = lock_types_of(vec!["RepositoryLock".to_owned()], &type_declarations(&code));
  assert_eq!(
    types,
    [
      "CloneTurn",
      "FetchTurn",
      "Locks",
      "OwnFetch",
      "Pair",
      "PressureTurn",
      "RepositoryLock",
      "ReviewHeld",
      "RewriteOptions",
      "Shared"
    ]
  );
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
    "fn the_file_locks_of_a_lease_and_of_the_companions(paths: &LeasePaths, git: &GitCommandExecutor) -> Result<()> {",
    "  let _lock = lock_lease(paths, &|| {})?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  let _landed = lock_landed(paths, timeout)?; // hit",
    "  fetch_baseline(git, path, &remote)?;",
    "  let _state = lock_state(&paths)?; // hit",
    "  push_leased(git, path)?;",
    "  let _locks = lock_companions(git, &[Some(path), companion], &|| {})?; // hit",
    "  execute_remote_command(git, &[\"ls-remote\"], path)?;",
    "}",
    "fn git_config_under_the_companion_locks(git: &GitCommandExecutor) -> Result<()> {",
    "  let _locks = lock_companions(git, &[Some(path), companion], &|| {})?;",
    "  run_config_write(git, path, \"--local\", key, None)",
    "}",
    "fn a_name_that_only_starts_as_one(git: &GitCommandExecutor) -> Result<()> {",
    "  let states = lock_states(&paths);",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn lock_lease_of(paths: &LeasePaths) -> std::io::Result<RepositoryLock> {",
    "  lock_lease(paths, &|| {})",
    "}",
    "fn a_wrapper_whatever_its_name(paths: &LeasePaths, git: &GitCommandExecutor) -> Result<()> {",
    "  let _lock = lock_lease_of(paths)?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn the_file_lock_itself(app_data: &Path, git: &GitCommandExecutor) -> Result<()> {",
    "  let _lock = lock_file(app_data.join(PAGE_LOCK), timeout, &|| {}).map_err(|error| failure(&error))?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_lock_method_of_the_pool(&self, git: &GitCommandExecutor) -> Result<()> {",
    "  let _lock = self.state.lock_at(WORKTREES_LOCK_FILE, timeout, \"waiting\")?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  let file = self.lock_file(path);",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  let at = lock_at(WORKTREES_LOCK_FILE);",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "use bd_core::sync_command::{SyncOutput, lock, sync};",
  ]);
  assert_eq!(locks_across_network(&source), hits);
}

/// A lock that a type holds is a lock: a variant matched by `match` and kept in a variable bound before, a variant matched
/// by `if let`, an alias, a struct with a lock field, a parameter that takes one, also by reference, and a struct that
/// borrows one; it lives to the release of the variable it was kept in. A struct that holds a closure that takes one
/// holds none (`Locker`), and an `impl Drop` answer is out of scope
#[test]
fn a_lock_that_a_type_holds_lives_across_no_call_of_the_network() {
  let (source, hits) = marked(&[
    "enum Turn { Free, Locked { lock: RepositoryLock, waited: Option<Duration> } }",
    "fn take_turn(pool: &Pool) -> Result<Turn> { todo!() }",
    "fn kept_in_a_variable_bound_before(pool: &Pool, git: &GitCommandExecutor) -> Result<()> {",
    "  let mut held: Option<RepositoryLock> = None;",
    "  loop {",
    "    match take_turn(&pool)? {",
    "      Turn::Free => continue,",
    "      Turn::Locked { lock, waited } => {",
    "        held = Some(lock); // hit",
    "      }",
    "    }",
    "    execute_remote_command(git, &[\"fetch\"], path)?;",
    "    drop(held.take());",
    "    execute_remote_command(git, &[\"fetch\"], path)?;",
    "  }",
    "}",
    "fn released_by_none(pool: &Pool, git: &GitCommandExecutor) -> Result<()> {",
    "  let mut held = None;",
    "  if let Turn::Locked { lock, .. } = take_turn(&pool)? {",
    "    held = Some(lock);",
    "  }",
    "  held = None;",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn the_network_in_the_match(pool: &Pool, git: &GitCommandExecutor) -> Result<()> {",
    "  match take_turn(&pool)? { // hit",
    "    Turn::Locked { .. } => execute_remote_command(git, &[\"fetch\"], path)?,",
    "    Turn::Free => {}",
    "  }",
    "  Ok(())",
    "}",
    "enum PressureTurn { Taken(Option<RepositoryLock>), Waited }",
    "fn pressure_turn(git: &GitCommandExecutor) -> PressureTurn { todo!() }",
    "fn a_turn_matched_by_if_let(git: &GitCommandExecutor) -> Result<()> {",
    "  if let PressureTurn::Taken(_pressure) = pressure_turn(git) { // hit",
    "    execute_remote_command(git, &[\"fetch\"], path)?;",
    "  }",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  let PressureTurn::Taken(_pressure) = pressure_turn(git) else { return Ok(()) }; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "type ReviewHeld = RepositoryLock;",
    "fn hold_review(path: &Path) -> Result<ReviewHeld> { todo!() }",
    "fn an_alias(git: &GitCommandExecutor) -> Result<()> {",
    "  let _held = hold_review(path)?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "struct ReviewGuard { _lock: RepositoryLock, path: PathBuf }",
    "fn guard_review(path: &Path) -> Result<ReviewGuard> { todo!() }",
    "fn a_struct_that_holds_one(git: &GitCommandExecutor) -> Result<()> {",
    "  let _guard = guard_review(path)?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn under_a_lock_it_takes(git: &GitCommandExecutor, lock: RepositoryLock) -> Result<()> { // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_lock_it_drops_first(git: &GitCommandExecutor, lock: Option<RepositoryLock>) -> Result<()> {",
    "  drop(lock);",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_borrowed_lock(git: &GitCommandExecutor, held_lock: Option<&RepositoryLock>) -> Result<()> { // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "struct Options<'a> { held_lock: Option<&'a RepositoryLock>, waiter: Arc<dyn Fn() -> Result<RepositoryLock>> }",
    "fn options() -> Options<'static> { todo!() }",
    "fn options_that_borrow_and_take_a_lock(git: &GitCommandExecutor) -> Result<()> {",
    "  let options = options(); // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn hold_opaque(path: &Path) -> impl Drop { todo!() }",
    "fn an_impl_drop_out_of_scope(git: &GitCommandExecutor) -> Result<()> {",
    "  let _held = hold_opaque(path);",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn hold_erased(path: &Path) -> Result<Box<dyn std::any::Any>> { todo!() }",
    "fn an_erased_lock_out_of_scope(git: &GitCommandExecutor) -> Result<()> {",
    "  let _held = hold_erased(path)?;",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
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
const EXEMPT_STD_GUARDS: [Exemption; 3] = [
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
  functions_answering(code, &["MutexGuard"]).into_iter().map(|function| function.name).collect()
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
// move by compare-and-swap, so it takes no lock (`brd push` and `brd safe-push` held it for theirs until 2026-10-31); a
// wait for the `index.lock` of another process (an IDE's `git status`, up to 30 s) comes before the lock
// (`update_branch::wait_for_index`, `PickPlan::wait_for_index_locks`, `IndexLock::wait_while_locked`), and under it
// a move only looks (`index_held`). The guard lives as above; a call inside a function the scope calls is not followed
// (the fallback wait of a move under the lock its caller holds, `brd update --continue`).

/// The calls, by their whole name, that the repository lock never lives across
const LONG_CALLS: [&str; 5] = ["sync", "sync_branches", "wait_while_locked", "wait_for_index", "wait_for_index_locks"];

/// The one wait for the index that a lock may seem to live across; an exemption names its file, reason and count
const EXEMPT_LONG_LOCKS: [Exemption; 1] = [Exemption {
  path: "crates/update-core/src/pick.rs",
  hits: 1,
  reason: "`land` takes `&RewriteOptions`, which borrows the lock of a caller that holds it (`held_lock`): with one it \
           breaks out of its checks without the lock at once and checks under that lock, and only without one it waits for \
           the index locks of other processes (`wait_for_index_locks`), before it takes its own",
}];

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
    "fn the_wait_under_the_lock(options: &RewriteOptions, git: &GitCommandExecutor, plan: &PickPlan) -> Result<()> { // hit",
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

/// A lock is held through a borrow and a field too, and taken through a closure: a parameter of `&Option<RepositoryLock>`
/// (its `if let Some(_lock) = held.as_ref()` as well), a field assigned the lock through a `&mut` to the end of the
/// block, a closure parameter that answers a lock and a closure binding that takes one, called with no argument. A name
/// bound again (`let lock = 5`) keeps the lock it shadows to the end of the block
#[test]
fn a_lock_held_through_a_borrow_or_a_field_or_taken_through_a_closure_lives_across_no_call_of_the_network() {
  let (source, hits) = marked(&[
    "struct Holder { held: Option<RepositoryLock> }",
    "fn a_field(holder: &mut Holder, git: &GitCommandExecutor, path: &Path) -> Result<()> { // hit",
    "  holder.held = Some(lock_file(path.to_path_buf(), timeout, &|| {})?); // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_field_of_self(&mut self, git: &GitCommandExecutor) -> Result<()> {",
    "  {",
    "    self.turn = lock_file(self.path.clone(), timeout, &|| {})?; // hit",
    "    execute_remote_command(git, &[\"fetch\"], path)?;",
    "  }",
    "  Ok(())",
    "}",
    "fn a_borrowed_option(git: &GitCommandExecutor, held: &Option<RepositoryLock>) -> Result<()> { // hit",
    "  if let Some(_lock) = held.as_ref() {",
    "    execute_remote_command(git, &[\"fetch\"], path)?;",
    "  }",
    "  Ok(())",
    "}",
    "fn a_closure_parameter(git: &GitCommandExecutor, take: impl FnOnce() -> Result<RepositoryLock>) -> Result<()> {",
    "  let _lock = take()?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_closure_binding(git: &GitCommandExecutor, path: &Path) -> Result<()> {",
    "  let take = move || lock_file(path.to_path_buf(), timeout, &|| {});",
    "  let _lock = take()?; // hit",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_closure_never_called(git: &GitCommandExecutor, path: &Path) -> Result<()> {",
    "  let take = || lock_file(path.to_path_buf(), timeout, &|| {});",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_shadowed_lock(git: &GitCommandExecutor, path: &Path) -> Result<()> {",
    "  let lock = lock_file(path.to_path_buf(), timeout, &|| {})?; // hit",
    "  let lock = 5u8;",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
    "fn a_closure_that_answers_no_lock(git: &GitCommandExecutor, read: impl Fn() -> Result<String>) -> Result<()> {",
    "  let text = read()?;",
    "  execute_remote_command(git, &[\"fetch\"], path)?;",
    "  Ok(())",
    "}",
  ]);
  assert_eq!(locks_across_network(&source), hits);
}
