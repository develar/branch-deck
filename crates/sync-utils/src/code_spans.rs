//! The code in a line of prose (`code_spans`), one grammar for every place the page sets a line of a commit body, the
//! story of a review and the prose of a spec (`CommitBody::code`, `review_plan::brief`): the spans in backticks, which
//! the author marked (`explicit`), and the tokens that read as code, which the grammar finds:
//! - a path (`./x`, `../x`, `~/x`, `//plugins/air/...`, a path of two slashes or more, a file name of a known extension);
//! - a dotted name of three parts or more that starts lowercase (`intellij.air.backend`), an `Owner.member` whose owner
//!   is capitalised (`Frame.paint`, `SplashScreen.hide`), a call (`run()`, `Foo.bar()`), `Foo::bar`, `Foo#bar`;
//! - a flag (`--lane`, `-p`, `--level=debug`);
//! - `CONSTANT_CASE` and `snake_case` (an underscore needed: `HTTP` stays prose), `camelCase` from five characters,
//!   `PascalCase` of two humps or more, each hump a capital and lowercase letters or digits (`SessionStore`; `GenUI`,
//!   `AssertJ` stay prose);
//! - never a name of a product the patterns would take (`NOT_CODE`: `macOS`, `JavaScript`, `GitHub`, `TeamCity`), nor a
//!   capitalised word before `.js` (`Vue.js`), nor a token in double quotes; the punctuation around a token stays prose,
//!   a possessive (`SplashScreen's`) and the dashes of a sentence (`Frame.paint—`) too, and tokens of code one space
//!   apart are one span (`bt.cmd --lane`).
//!
//! Offsets are UTF-16 code units of the line, the end excluded, the backticks of an explicit span included.

use domain_types::commit_body::CodeSpan;

/// The extensions of a file name that is code
const CODE_EXTENSIONS: [&str; 31] = [
  "kt",
  "kts",
  "java",
  "cs",
  "ts",
  "tsx",
  "js",
  "mjs",
  "cjs",
  "rs",
  "go",
  "py",
  "json",
  "toml",
  "yaml",
  "yml",
  "md",
  "cmd",
  "sh",
  "bazel",
  "bzl",
  "gradle",
  "xml",
  "css",
  "html",
  "iml",
  "properties",
  "swift",
  "c",
  "h",
  "cpp",
];

/// Camel-cased names of products and companies the patterns would take for identifiers
const NOT_CODE: &[&str] = &[
  "macOS",
  "iOS",
  "iPadOS",
  "watchOS",
  "tvOS",
  "visionOS",
  "iPhone",
  "iPad",
  "iCloud",
  "iMac",
  "eBay",
  "JavaScript",
  "TypeScript",
  "GitHub",
  "GitLab",
  "IntelliJ",
  "JetBrains",
  "PyCharm",
  "WebStorm",
  "PhpStorm",
  "GoLand",
  "RubyMine",
  "DataGrip",
  "DataSpell",
  "RustRover",
  "ReSharper",
  "AppCode",
  "TeamCity",
  "YouTrack",
  "dotTrace",
  "dotMemory",
  "dotCover",
  "dotPeek",
  "YouTube",
  "PowerPoint",
  "PowerShell",
  "SharePoint",
  "LinkedIn",
  "OAuth",
  "GraphQL",
  "PostgreSQL",
  "MySQL",
  "MongoDB",
  "SonarQube",
  "WebKit",
  "WebView",
  "DevTools",
  "OpenType",
  "PayPal",
  "FedEx",
  "MacBook",
  "NuGet",
  "CocoaPods",
  "WordPress",
  "LastPass",
  "OneDrive",
  "VirtualBox",
];

/// The spans of code of `line`, in its order (the module's header)
pub fn code_spans(line: &str) -> Vec<CodeSpan> {
  let mut spans = Vec::new();
  let mut at = 0;
  while at < line.len() {
    let rest = &line[at..];
    let Some(open) = rest.find('`') else {
      detect(line, at, line.len(), &mut spans);
      break;
    };
    let close = rest[open + 1..].find('`').filter(|&close| close > 0);
    let Some(close) = close else {
      detect(line, at, line.len(), &mut spans);
      break;
    };
    detect(line, at, at + open, &mut spans);
    let end = at + open + close + 2;
    spans.push(span(line, at + open, end, true, false));
    at = end;
  }
  spans
}

/// Whether a bare word reads as code by the grammar (the module's header)
fn is_code(token: &str) -> bool {
  !NOT_CODE.contains(&token)
    && !is_framework(token)
    && (is_path(token) || is_dotted(token) || is_flag(token) || is_call(token) || is_member_reference(token) || is_case_name(token))
}

/// A capitalised word before `.js` names a library (`Vue.js`, `Next.js`, `Node.js`): a file of it takes its directory or
/// backticks
fn is_framework(token: &str) -> bool {
  token
    .strip_suffix(".js")
    .is_some_and(|name| name.starts_with(|c: char| c.is_ascii_uppercase()) && all(&name[1..], |c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

fn span(line: &str, start: usize, end: usize, explicit: bool, joined: bool) -> CodeSpan {
  CodeSpan {
    start: utf16(&line[..start]),
    end: utf16(&line[..end]),
    explicit,
    joined,
  }
}

fn utf16(text: &str) -> u32 {
  u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// The punctuation before a token: prose
const LEADING: &[char] = &['(', '"', '\'', '[', '‘', '“', '—', '–'];
/// The punctuation after a token: prose, the dashes of a sentence too (`Frame.paint—`)
const TRAILING: &[char] = &[')', ']', '"', '\'', ',', '.', ';', ':', '!', '?', '’', '”', '—', '–'];

/// The tokens of code of `line[from..to]`, tokens one space apart joined into one span
fn detect(line: &str, from: usize, to: usize, spans: &mut Vec<CodeSpan>) {
  let text = &line[from..to];
  // the open span: its start and end in `line`, and whether it joined tokens
  let mut open: Option<(usize, usize, bool)> = None;
  let flush = |open: &mut Option<(usize, usize, bool)>, spans: &mut Vec<CodeSpan>| {
    if let Some((start, end, joined)) = open.take() {
      spans.push(span(line, start, end, false, joined));
    }
  };
  let mut offset = 0;
  for token in text.split(' ') {
    let start = from + offset;
    offset += token.len() + 1;
    if token.is_empty() {
      flush(&mut open, spans);
      continue;
    }
    let leading = token.len() - token.trim_start_matches(LEADING).len();
    let mut core = token[leading..].trim_end_matches(TRAILING);
    // a possessive is prose after its name (`SplashScreen's`), and so is what follows it
    if let Some(name) = core.strip_suffix("'s").or_else(|| core.strip_suffix("’s")) {
      core = name.trim_end_matches(TRAILING);
    }
    let after = &token[leading + core.len()..];
    // Bazel's recursive pattern ends with dots, a call with its parentheses
    if core.ends_with('/') && after.starts_with("...") {
      core = &token[leading..leading + core.len() + 3];
    } else if core.ends_with('(') && after.starts_with(')') {
      core = &token[leading..=leading + core.len()];
    }
    let quoted = token.starts_with('"');
    if core.is_empty() || quoted || !is_code(core) {
      flush(&mut open, spans);
      continue;
    }
    let core_start = start + leading;
    let core_end = core_start + core.len();
    match open.as_mut() {
      // one space after the open span and no punctuation before the token: the same span
      Some((_, end, joined)) if leading == 0 && *end + 1 == core_start => {
        *end = core_end;
        *joined = true;
      }
      _ => {
        flush(&mut open, spans);
        open = Some((core_start, core_end, false));
      }
    }
    // punctuation after the token ends the span
    if leading + core.len() < token.len() {
      flush(&mut open, spans);
    }
  }
  flush(&mut open, spans);
}

/// `[A-Za-z0-9_]`
const fn is_word(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

fn all(text: &str, test: impl Fn(char) -> bool) -> bool {
  !text.is_empty() && text.chars().all(test)
}

/// A path: `./x`, `../x`, `~/x`, `//x`, two slashes or more, or a file name of a known extension
fn is_path(token: &str) -> bool {
  if ["./", "../", "~/", "//"]
    .iter()
    .any(|lead| token.strip_prefix(lead).is_some_and(|rest| !rest.is_empty() && !rest.contains(char::is_whitespace)))
  {
    return true;
  }
  let segment = |part: &str| all(part, |c| is_word(c) || c == '.' || c == '-');
  let trimmed = token.strip_suffix('/').unwrap_or(token);
  let parts: Vec<&str> = trimmed.split('/').collect();
  if parts.len() >= 3 && parts[0].starts_with(is_word) && parts.iter().all(|part| segment(part)) {
    return true;
  }
  // a file name, maybe under directories: its stem and the extension of code
  let (path, name) = token.rsplit_once('/').unwrap_or(("", token));
  let directories_ok = path.is_empty() || path.split('/').all(segment) && path.starts_with(|c: char| is_word(c) || c == '-');
  let Some((stem, extension)) = name.rsplit_once('.') else { return false };
  directories_ok && CODE_EXTENSIONS.contains(&extension) && stem.split('.').all(|part| all(part, |c| is_word(c) || c == '-'))
}

/// A dotted name: three parts or more led by a lowercase one (`intellij.air.backend`), or an `Owner.member` with a
/// capitalised owner, each part of two characters or more (`Frame.paint`; `U.S` stays prose)
fn is_dotted(token: &str) -> bool {
  let parts: Vec<&str> = token.split('.').collect();
  if parts.len() < 2 {
    return false;
  }
  let identifier = |part: &&str| part.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && all(part, |c| is_word(c) || c == '-');
  if parts.len() >= 3 && parts[0].starts_with(|c: char| c.is_ascii_lowercase()) && parts.iter().all(identifier) {
    return true;
  }
  parts[0].starts_with(|c: char| c.is_ascii_uppercase()) && parts.iter().all(|part| identifier(part) && part.len() >= 2 && all(part, is_word))
}

/// `--lane`, `-p`, `--level=debug`
fn is_flag(token: &str) -> bool {
  let rest = token.strip_prefix("--").or_else(|| token.strip_prefix('-'));
  let Some(rest) = rest else { return false };
  let (name, value) = rest.split_once('=').map_or((rest, None), |(name, value)| (name, Some(value)));
  name.starts_with(|c: char| c.is_ascii_lowercase()) && all(name, |c| is_word(c) || c == '-') && value.is_none_or(|value| !value.is_empty() && !value.contains(char::is_whitespace))
}

/// `run()`, `Foo.bar()`
fn is_call(token: &str) -> bool {
  token.strip_suffix("()").is_some_and(|name| name.split('.').all(|part| all(part, is_word)))
}

/// `Foo::bar`, `Foo#bar`
fn is_member_reference(token: &str) -> bool {
  if let Some((owner, member)) = token.split_once("::") {
    return all(owner, is_word) && all(member, is_word);
  }
  token
    .split_once('#')
    .is_some_and(|(owner, member)| owner.starts_with(|c: char| c.is_ascii_alphabetic()) && all(owner, is_word) && all(member, is_word))
}

/// `CONSTANT_CASE`, `snake_case`, `camelCase` from five characters, `PascalCase` of two humps or more
fn is_case_name(token: &str) -> bool {
  let parts: Vec<&str> = token.split('_').collect();
  if parts.len() >= 2 {
    let constant = token.starts_with(|c: char| c.is_ascii_uppercase()) && parts.iter().all(|part| all(part, |c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    let snake = token.starts_with(|c: char| c.is_ascii_lowercase()) && parts.iter().all(|part| all(part, |c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    return constant || snake;
  }
  if !all(token, |c| c.is_ascii_alphanumeric()) {
    return false;
  }
  let first = token.chars().next().unwrap_or(' ');
  if first.is_ascii_lowercase() {
    return token.len() >= 5 && token.chars().any(|c| c.is_ascii_uppercase());
  }
  if !first.is_ascii_uppercase() {
    return false;
  }
  // humps: a capital and one lowercase letter or digit at least
  let mut humps = 0;
  let mut chars = token.chars().peekable();
  while let Some(c) = chars.next() {
    if !c.is_ascii_uppercase() {
      return false;
    }
    let mut tail = 0;
    while chars.peek().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
      chars.next();
      tail += 1;
    }
    if tail == 0 {
      return false;
    }
    humps += 1;
  }
  humps >= 2
}
