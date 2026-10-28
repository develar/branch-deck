//! Shared utilities for detecting issue number patterns in commit messages

use domain_types::commit::IssueLead;

use memchr::memchr;

/// Skip [category] prefix if present, returns new position or None if newline encountered
fn skip_bracket_prefix(bytes: &[u8], mut i: usize) -> Option<usize> {
  let len = bytes.len();

  if i < len && bytes[i] == b'[' {
    i += 1;
    // Find the closing bracket or newline using memchr
    let rest = &bytes[i..];
    let close_pos = memchr(b']', rest);
    let nl_pos = memchr(b'\n', rest);
    match (close_pos, nl_pos) {
      (_, Some(nl)) if close_pos.unwrap_or(usize::MAX) > nl => {
        return None; // newline before closing bracket
      }
      (Some(p), _) => {
        i += p + 1; // skip to after ']'
        // Skip any whitespace after bracket; stop at newline
        while i < len && bytes[i].is_ascii_whitespace() {
          if bytes[i] == b'\n' {
            return None;
          }
          i += 1;
        }
        return Some(i);
      }
      (None, None | Some(_)) => {
        return None; // no closing bracket found before newline or end
      }
    }
  }

  Some(i) // No bracket prefix, return unchanged
}

/// Skip semantic commit prefix (e.g., "fix:", "feat(scope):"), returns new position or None
fn skip_semantic_prefix(bytes: &[u8], mut i: usize) -> Option<usize> {
  let len = bytes.len();

  if i < len && bytes[i].is_ascii_lowercase() {
    let type_start = i;
    // Collect the type (fix, feat, chore, etc.)
    while i < len && bytes[i].is_ascii_lowercase() {
      i += 1;
    }

    // Check newline/end
    if i >= len || bytes[i] == b'\n' {
      return Some(type_start);
    }

    // Optional (scope)
    if bytes[i] == b'(' {
      i += 1;
      let rest = &bytes[i..];
      let close_pos = memchr(b')', rest);
      let nl_pos = memchr(b'\n', rest);
      match (close_pos, nl_pos) {
        (_, Some(nl)) if close_pos.unwrap_or(usize::MAX) > nl => {
          return Some(type_start); // newline before ')': not a semantic prefix
        }
        (Some(p), _) => {
          i += p + 1;
        }
        (None, None | Some(_)) => {
          return Some(type_start); // no closing ')'
        }
      }
    }

    if i >= len || bytes[i] == b'\n' {
      return Some(type_start);
    }

    // Colon
    if bytes[i] == b':' {
      i += 1;
      while i < len && bytes[i].is_ascii_whitespace() {
        if bytes[i] == b'\n' {
          return None;
        }
        i += 1;
      }
      return Some(i);
    }

    return Some(type_start);
  }

  Some(i)
}

/// Try to extract issue at current position, returns (start, end) positions
const fn extract_issue_at(bytes: &[u8], mut i: usize) -> Option<(usize, usize)> {
  let len = bytes.len();

  if i >= len || bytes[i] == b'\n' || !bytes[i].is_ascii_uppercase() {
    return None;
  }

  let start = i;

  // Collect uppercase letters
  while i < len && bytes[i].is_ascii_uppercase() {
    i += 1;
  }

  // Check for hyphen
  if i < len && bytes[i] == b'-' {
    i += 1; // Skip hyphen

    // Must have at least one digit
    if i < len && bytes[i].is_ascii_digit() {
      // Collect all digits
      while i < len && bytes[i].is_ascii_digit() {
        i += 1;
      }

      // Check word boundary at end
      if i >= len || bytes[i] == b'\n' || !bytes[i].is_ascii_alphanumeric() {
        return Some((start, i));
      }
    }
  }

  None
}

/// Find issue number pattern and return its position range
/// Returns (start, end) positions of the issue, or None if not found
/// Only searches up to the first newline (subject line only)
pub fn find_issue_range(text: &str) -> Option<(usize, usize)> {
  let bytes = text.as_bytes();

  // Early exit for strings too short to contain a valid pattern
  if bytes.len() < 3 {
    return None; // Minimum: "A-1"
  }

  // Try to extract issue pattern at current position
  extract_issue_at(bytes, issue_key_position(text)?)
}

/// Where `find_issue_range` reads the issue key of a subject: after an optional `[category]` and an optional semantic
/// prefix (`fix:`, `feat(scope):`), with the whitespace after them; None when the line ends inside one of them
pub fn issue_key_position(text: &str) -> Option<usize> {
  let bytes = text.as_bytes();
  let i = skip_bracket_prefix(bytes, 0)?;
  skip_semantic_prefix(bytes, i)
}

/// Manually find issue number pattern (e.g., JIRA-123) without regex
/// Returns the first issue number found, or None
/// Only searches up to the first newline (subject line only)
pub fn find_issue_number(text: &str) -> Option<&str> {
  find_issue_range(text).map(|(start, end)| &text[start..end])
}

/// Check if a message already contains an issue reference pattern (like ABC-123)
/// More efficient than `find_issue_number` when you only need existence check
pub fn has_issue_reference(text: &str) -> bool {
  find_issue_range(text).is_some()
}

/// `text` split at the issue key it starts with (`IJPL-1: Fix`, `[IJPL-1] Fix`, `IJPL-1 - Fix`), for a row that shows the
/// key apart: the key and what follows it and its separator (`:`, `,`, `-`, `–`, `—`), empty for a text that is only the
/// key. None without a leading key and when the key is a part of the sentence (`TEST-1 and PROD-9: …`)
pub fn issue_lead(text: &str) -> Option<IssueLead> {
  let bracketed = text.starts_with('[');
  let start = usize::from(bracketed);
  let (_, end) = extract_issue_at(text.as_bytes(), start)?;
  let key = &text[start..end];
  let mut rest = &text[end..];
  if bracketed {
    rest = rest.strip_prefix(']')?;
  }
  rest = rest.trim_start();
  let separator = rest.chars().next().filter(|c| matches!(c, ':' | ',' | '-' | '–' | '—'));
  if let Some(separator) = separator {
    rest = rest[separator.len_utf8()..].trim_start();
  } else if ["and", "or", "&"]
    .iter()
    .any(|word| rest.strip_prefix(word).is_some_and(|after| after.starts_with(char::is_whitespace)))
  {
    return None;
  }
  Some(IssueLead {
    key: key.to_owned(),
    text: rest.to_owned(),
  })
}

/// A conventional commit prefix: `type(scope)!: rest`, or `type scope: rest` as in `setup vm:` or `docs ai:`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticPrefix<'a> {
  /// `feat`, `fix`, `setup`, …
  pub kind: &'a str,
  pub scope: Option<&'a str>,
  /// The subject after the colon and the whitespace after it
  pub rest: &'a str,
}

/// The conventional commit prefix of the first line of `subject`; `None` without one. The type is lowercase letters, a
/// scope in parentheses is letters, digits and `-_./`, a scope after a space is lowercase letters, digits and `-_./`
pub fn parse_semantic_prefix(subject: &str) -> Option<SemanticPrefix<'_>> {
  let line = subject.lines().next()?;
  let kind_end = line.find(|c: char| !c.is_ascii_lowercase()).unwrap_or(line.len());
  if kind_end == 0 {
    return None;
  }
  let (kind, mut rest) = line.split_at(kind_end);
  let mut scope = None;
  if let Some(after) = rest.strip_prefix('(') {
    let close = after.find(')')?;
    let text = &after[..close];
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_alphanumeric() || is_scope_punctuation(c)) {
      return None;
    }
    scope = Some(text);
    rest = &after[close + 1..];
  } else if let Some(after) = rest.strip_prefix(' ') {
    let end = after
      .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || is_scope_punctuation(c)))
      .unwrap_or(after.len());
    let text = &after[..end];
    if !text.starts_with(|c: char| c.is_ascii_alphanumeric()) {
      return None;
    }
    scope = Some(text);
    rest = &after[end..];
  }
  let rest = rest.strip_prefix('!').unwrap_or(rest).strip_prefix(':')?;
  if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
    return None;
  }
  Some(SemanticPrefix {
    kind,
    scope,
    rest: rest.trim_start(),
  })
}

const fn is_scope_punctuation(c: char) -> bool {
  matches!(c, '-' | '_' | '.' | '/')
}

/// The scope of the conventional commit prefix of `subject`: `vm` of `setup vm: x` and of `fix(vm): x`
pub fn conventional_scope(subject: &str) -> Option<&str> {
  parse_semantic_prefix(subject)?.scope
}

/// Every issue key (`ABC-123` at word boundaries, as `find_issue_number` reads one) in `text`, any line, in order and
/// with repeats. A key after a hyphen is part of a longer id (`IJ-CR-1234`), not a key
pub fn issue_keys_in(text: &str) -> impl Iterator<Item = &str> {
  let bytes = text.as_bytes();
  let mut i = 0;
  std::iter::from_fn(move || {
    while i < bytes.len() {
      let at_boundary = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-');
      if !(at_boundary && bytes[i].is_ascii_uppercase()) {
        i += 1;
        continue;
      }
      let start = i;
      while i < bytes.len() && bytes[i].is_ascii_uppercase() {
        i += 1;
      }
      if bytes.get(i) == Some(&b'-') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
        let mut end = i + 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
          end += 1;
        }
        if bytes.get(end).is_none_or(|b| !b.is_ascii_alphanumeric()) {
          i = end;
          return Some(&text[start..end]);
        }
        i = end;
      }
    }
    None
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_find_issue_range() {
    // Test finding issue ranges
    assert_eq!(find_issue_range("ABC-123 Fix the bug"), Some((0, 7)));
    assert_eq!(find_issue_range("ISSUE-1"), Some((0, 7)));
    assert_eq!(find_issue_range("A-1 minimal"), Some((0, 3)));

    // Test with prefixes
    assert_eq!(find_issue_range("[tag] XYZ-999: title"), Some((6, 13)));
    assert_eq!(find_issue_range("fix: ABC-123 resolve bug"), Some((5, 12)));
    assert_eq!(find_issue_range("feat(auth): DEF-456 add login"), Some((12, 19)));

    // Test cases that should NOT match
    assert_eq!(find_issue_range("Fix the bug"), None);
    assert_eq!(find_issue_range("abc-123 lowercase"), None);
    assert_eq!(find_issue_range("ABC- missing number"), None);
    assert_eq!(find_issue_range("-123 missing prefix"), None);
    assert_eq!(find_issue_range("ABC-"), None);
    assert_eq!(find_issue_range(""), None);

    // Test with newlines (should not find in body)
    assert_eq!(find_issue_range("No issue here\nBUT-123 in the body"), None);
    assert_eq!(find_issue_range("ABC-123 in subject\nDEF-456 in body"), Some((0, 7)));
  }

  #[test]
  fn the_key_position_is_after_the_category_and_the_semantic_prefix() {
    assert_eq!(issue_key_position("Fix the bug"), Some(0));
    assert_eq!(issue_key_position("docs plugins: audit"), Some(0));
    assert_eq!(issue_key_position("[plugins] audit"), Some(10));
    assert_eq!(issue_key_position("fix: audit"), Some(5));
    assert_eq!(issue_key_position("[plugins] feat(auth): add login"), Some(22));
    assert_eq!(issue_key_position("[plugins\n] audit"), None);
  }

  #[test]
  fn test_find_issue_number() {
    // Test finding issue numbers at start
    assert_eq!(find_issue_number("ABC-123 Fix the bug"), Some("ABC-123"));
    assert_eq!(find_issue_number("ISSUE-1"), Some("ISSUE-1"));
    assert_eq!(find_issue_number("A-1 minimal"), Some("A-1"));

    // Test that arbitrary text before issue is NOT found
    assert_eq!(find_issue_number("Fix JIRA-456 in code"), None);
    assert_eq!(find_issue_number("Update (ABC-789) docs"), None);

    // But [tag] prefix IS recognized
    assert_eq!(find_issue_number("[tag] XYZ-999: title"), Some("XYZ-999"));

    // Test cases that should NOT match
    assert_eq!(find_issue_number("Fix the bug"), None);
    assert_eq!(find_issue_number("abc-123 lowercase"), None);
    assert_eq!(find_issue_number("ABC- missing number"), None);
    assert_eq!(find_issue_number("-123 missing prefix"), None);
    assert_eq!(find_issue_number("ABC-"), None);
    assert_eq!(find_issue_number("ABC-abc not digits"), None);
    assert_eq!(find_issue_number(""), None);

    // Edge cases
    assert_eq!(find_issue_number("ABC-123-456"), Some("ABC-123")); // Stops at first valid pattern
    assert_eq!(find_issue_number("ABC-123ABC-456"), None); // No word boundary after first pattern
    assert_eq!(find_issue_number("prefixABC-123"), None); // Not at word boundary
    assert_eq!(find_issue_number("ABC--123"), None); // Double hyphen
    assert_eq!(find_issue_number("ABC123-456"), None); // No hyphen after letters

    // Multiple patterns - only finds if at start or after recognized prefix
    assert_eq!(find_issue_number("Fix ABC-123 and DEF-456"), None); // "Fix" is not a recognized prefix
    assert_eq!(find_issue_number("ABC-123ABC-456 should find DEF-789"), None); // No valid issue at start

    // Test that it stops at newline (only searches subject line)
    assert_eq!(find_issue_number("No issue here\nBUT-123 in the body"), None);
    assert_eq!(find_issue_number("ABC-123 in subject\nDEF-456 in body"), Some("ABC-123"));
    assert_eq!(find_issue_number("Subject line\n\nXYZ-789 in body"), None);

    // Test [category] prefix handling
    assert_eq!(find_issue_number("[threading] IJPL-163558: Fix observability"), Some("IJPL-163558"));
    assert_eq!(find_issue_number("[subsystem] ABC-456: Update documentation"), Some("ABC-456"));
    assert_eq!(find_issue_number("[auth] No issue here"), None);

    // Test semantic commit prefix handling
    assert_eq!(find_issue_number("fix: ABC-123 resolve bug"), Some("ABC-123"));
    assert_eq!(find_issue_number("feat(auth): DEF-456 add login"), Some("DEF-456"));
    assert_eq!(find_issue_number("chore: no issue here"), None);

    // Test combination of prefixes
    assert_eq!(find_issue_number("[category] fix: GHI-789 combined"), Some("GHI-789"));
    assert_eq!(find_issue_number("[test] feat(api): JKL-012 all prefixes"), Some("JKL-012"));
  }

  #[test]
  fn test_has_issue_reference() {
    // Test with issue reference at the start
    assert!(has_issue_reference("ABC-123 Fix the bug"));
    assert!(has_issue_reference("ISSUE-456 Add new feature"));
    assert!(has_issue_reference("JIRA-1 Update docs"));

    // Test cases that should NOT match
    assert!(!has_issue_reference("Fix the bug"));
    assert!(!has_issue_reference("abc-123 lowercase not valid"));
    assert!(!has_issue_reference("ABC- missing number"));
    assert!(!has_issue_reference("-123 missing prefix"));
    assert!(!has_issue_reference(""));
    assert!(!has_issue_reference("Some text ABC-123 issue in the middle"));

    // Test that issue reference in body (not subject) is ignored
    assert!(!has_issue_reference("Fix the bug\n\nThis fixes ABC-123"));
    assert!(!has_issue_reference("Update feature\n\nRelated to ISSUE-456"));

    // Test [category] prefix handling
    assert!(has_issue_reference("[threading] IJPL-163558: Fix observability"));
    assert!(has_issue_reference("[subsystem] ABC-456: Update"));

    // Test semantic commit prefix handling
    assert!(has_issue_reference("fix: ABC-123 resolve bug"));
    assert!(has_issue_reference("feat(auth): DEF-456 add login"));
  }

  #[test]
  fn semantic_prefixes_with_and_without_parentheses() {
    let parsed = |subject| parse_semantic_prefix(subject).map(|prefix| (prefix.kind, prefix.scope, prefix.rest));
    assert_eq!(parsed("feat(cli): bd suggest"), Some(("feat", Some("cli"), "bd suggest")));
    assert_eq!(parsed("feat(api)!: drop v1"), Some(("feat", Some("api"), "drop v1")));
    assert_eq!(parsed("fix: a bug"), Some(("fix", None, "a bug")));
    assert_eq!(parsed("setup vm: a lease skips a busy worker"), Some(("setup", Some("vm"), "a lease skips a busy worker")));
    assert_eq!(parsed("docs ai: sync guides\n\nbody"), Some(("docs", Some("ai"), "sync guides")));
    assert_eq!(parsed("chore:"), Some(("chore", None, "")));

    // not a prefix
    assert_eq!(parsed("IJAI-3828 the docs: a summary"), None);
    assert_eq!(parsed("the frontend gets a client state"), None);
    assert_eq!(parsed("fix:no space"), None);
    assert_eq!(parsed("fix(): empty scope"), None);
    assert_eq!(parsed("fix(scope x): spaces"), None);
    assert_eq!(parsed("setup Vm: uppercase scope"), None);
    assert_eq!(parsed("setup vm and more: words"), None);
    assert_eq!(parsed(""), None);

    assert_eq!(conventional_scope("setup build: Bazel starts"), Some("build"));
    assert_eq!(conventional_scope("refactor(vm): one reader"), Some("vm"));
    assert_eq!(conventional_scope("fix: x"), None);
  }

  #[test]
  fn issue_keys_anywhere_in_a_text() {
    let keys = |text| issue_keys_in(text).collect::<Vec<_>>();
    assert_eq!(keys("IJAI-3016 subject\n\nAlso IJPL-258458 and (IJAI-3016)."), ["IJAI-3016", "IJPL-258458", "IJAI-3016"]);
    assert_eq!(keys("ABC-123-456 and DEF-7"), ["ABC-123", "DEF-7"]);
    assert_eq!(keys("x-ABC-1 xABC-2 ABC-3x ABC- -4 abc-5 IJ-CR-1234"), Vec::<&str>::new());
    assert_eq!(keys("(ABC-1), [DEF-2]/GHI-3"), ["ABC-1", "DEF-2", "GHI-3"]);
    assert_eq!(keys("Ünïcode ABC-9"), ["ABC-9"]);
    assert_eq!(keys(""), Vec::<&str>::new());
    // the subject parser is unchanged: only a key at the start counts there
    assert_eq!(find_issue_number("Fix ABC-123"), None);
  }

  #[test]
  fn a_title_led_by_an_issue_key_splits_at_it_and_its_separator() {
    let lead = |text: &str| issue_lead(text).map(|lead| (lead.key, lead.text));
    for (text, rest) in [
      ("IJPL-1 Fix the cache", "Fix the cache"),
      ("IJPL-1: Fix the cache", "Fix the cache"),
      ("IJPL-1 - Fix the cache", "Fix the cache"),
      ("IJPL-1 – Fix the cache", "Fix the cache"),
      ("[IJPL-1] Fix the cache", "Fix the cache"),
      ("IJPL-1, IJPL-2: Fix the cache", "IJPL-2: Fix the cache"),
      ("IJPL-1", ""),
    ] {
      assert_eq!(lead(text), Some(("IJPL-1".to_string(), rest.to_string())), "{text}");
    }
    // a key in the middle, a longer id, a key of the sentence
    for text in [
      "Fix IJPL-1 in the cache",
      "IJPL-12x Fix",
      "TEST-1 and PROD-9: Database optimization",
      "[IJPL-1 Fix",
      "ijpl-1 Fix",
    ] {
      assert_eq!(lead(text), None, "{text}");
    }
  }
}
