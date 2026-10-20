//! Git booleans as git reads them from its config, without a git process

/// A git boolean as `git config --type=bool` reads it: `true`/`yes`/`on`, `false`/`no`/`off` (any case, no white space
/// around), the empty text is false, a key without a value is true, else an integer of git's (0 is false); `None` for
/// what git refuses
pub fn parse_git_bool(value: Option<&str>) -> Option<bool> {
  let Some(value) = value else {
    return Some(true);
  };
  if ["true", "yes", "on"].iter().any(|word| value.eq_ignore_ascii_case(word)) {
    return Some(true);
  }
  if value.is_empty() || ["false", "no", "off"].iter().any(|word| value.eq_ignore_ascii_case(word)) {
    return Some(false);
  }
  parse_git_int(value).map(|number| number != 0)
}

/// An integer as git's config reads it (`git_parse_int`): `strtoimax` with base 0 (white space before, a sign, `0x`
/// for hex, `0` for octal), then an optional unit `k`, `m` or `g` (any case, 1024-based), within the range of a C `int`.
/// C17's base 0, not C23's: `0b` is not a binary prefix, as in the git of macOS and Windows; the git of glibc 2.38+
/// reads `0b1` as 1, but which libc the user's git has is unknown, so `0b1` is refused everywhere.
fn parse_git_int(value: &str) -> Option<i64> {
  // C's `isspace`, which has the vertical tab too
  let rest = value.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
  let (negative, rest) = match rest.strip_prefix('-') {
    Some(rest) => (true, rest),
    None => (false, rest.strip_prefix('+').unwrap_or(rest)),
  };
  let hex = rest
    .strip_prefix("0x")
    .or_else(|| rest.strip_prefix("0X"))
    .filter(|hex| hex.starts_with(|c: char| c.is_ascii_hexdigit()));
  let (radix, digits) = match hex {
    Some(hex) => (16, hex),
    None if rest.starts_with('0') => (8, rest),
    None => (10, rest),
  };
  let end = digits.find(|c: char| !c.is_digit(radix)).unwrap_or(digits.len());
  if end == 0 {
    return None;
  }
  let number = i128::from_str_radix(&digits[..end], radix).ok()?;
  let factor = match &digits[end..] {
    "" => 1,
    unit if unit.eq_ignore_ascii_case("k") => 1 << 10,
    unit if unit.eq_ignore_ascii_case("m") => 1 << 20,
    unit if unit.eq_ignore_ascii_case("g") => 1 << 30,
    _ => return None,
  };
  let number = (if negative { -number } else { number }).checked_mul(factor)?;
  i32::try_from(number).ok().map(i64::from)
}
