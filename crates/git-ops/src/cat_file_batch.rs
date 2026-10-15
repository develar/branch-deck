//! The output of `git cat-file --batch -Z`, shared by the readers of many objects in one process.

use anyhow::{Result, anyhow};
use tracing::debug;

/// An object of the output: its type and its bytes as git stores them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BatchObject<'a> {
  pub kind: &'a str,
  pub content: &'a [u8],
}

/// Split the output of `cat-file --batch -Z` for `count` names: every entry is `<oid> <type> <size>\0<content>\0`, or
/// `<name> missing\0` (`ambiguous`) for a name git cannot resolve, which is `None`
pub(crate) fn parse_batch_output(bytes: &[u8], count: usize) -> Result<Vec<Option<BatchObject<'_>>>> {
  let mut objects = Vec::with_capacity(count);
  let mut i = 0;
  for _ in 0..count {
    let end = memchr::memchr(0, &bytes[i..]).map(|pos| i + pos).ok_or_else(|| anyhow!("truncated cat-file output"))?;
    let header = std::str::from_utf8(&bytes[i..end]).map_err(|_| anyhow!("cat-file header is not UTF-8"))?;
    i = end + 1;
    let mut fields = header.rsplitn(3, ' ');
    let (Some(size), Some(kind)) = (fields.next(), fields.next()) else {
      return Err(anyhow!("Failed to parse git cat-file header: {header:?}"));
    };
    let Ok(size) = size.parse::<usize>() else {
      // `missing` or `ambiguous`
      debug!(header, "object missing");
      objects.push(None);
      continue;
    };
    let content = bytes
      .get(i..i + size)
      .ok_or_else(|| anyhow!("Insufficient data for content: expected {size} bytes, but only {} remaining", bytes.len() - i))?;
    objects.push(Some(BatchObject { kind, content }));
    // skip the content and its NUL terminator
    i += size + 1;
  }
  Ok(objects)
}
