//! How a list too long to give whole is cut, in `bd`'s output and in what the app shows and copies for an agent: the
//! first items and the count of the rest, never a count of one where that one item would stand (`cutList` and
//! `joinNames` in `src/lib/text.ts`, the same rule; docs/contributing/ux-writing.md).

/// How many of `len` items a list cut at `limit` keeps: all of them up to one more than the limit, so the count of the
/// rest is never 1; from two more on, `limit`
#[must_use]
pub const fn kept(len: usize, limit: usize) -> usize {
  if len <= limit.saturating_add(1) { len } else { limit }
}
