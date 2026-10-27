//! The id of one operation of the app or `bd`: a ULID (48 bits of Unix milliseconds, then 80 random bits), written as
//! 26 characters of Crockford base32.
//!
//! The time first makes the ids of one log sort by when they started; the random part keeps the ids of the app and of
//! every `bd` process apart without a shared counter. The root span of a `bd` command and the span of a Tauri command's
//! body, both built by [`OperationId::span`], carry it as `operation_id`, and an `ApiError` names it, so the Activity
//! entry of an error leads to its log lines.

use std::fmt::{self, Write};
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// Crockford's base32: no I, L, O, U
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 26 characters of 5 bits hold the 128 bits (the first character takes the top 3)
const ENCODED_LENGTH: usize = 26;
const RANDOM_BITS: u32 = 80;

/// The id of one operation, unique across processes; `Display` writes the ULID text
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationId(u128);

impl OperationId {
  /// A new id at the current time
  #[must_use]
  pub fn new() -> Self {
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
    Self::from_parts(u64::try_from(now_ms).unwrap_or(u64::MAX), random_bits())
  }

  /// The id of `timestamp_ms` (its low 48 bits) and `random` (its low 80 bits)
  #[must_use]
  pub const fn from_parts(timestamp_ms: u64, random: u128) -> Self {
    let timestamp = (timestamp_ms as u128) & ((1 << 48) - 1);
    Self((timestamp << RANDOM_BITS) | (random & ((1 << RANDOM_BITS) - 1)))
  }

  /// When the operation started, in milliseconds since the Unix epoch
  #[must_use]
  pub const fn timestamp_ms(self) -> u64 {
    (self.0 >> RANDOM_BITS) as u64
  }

  /// The span of the operation, `operation{operation_id=<id>}`, with `op` (the words of a `bd` command, `lease get`)
  /// before the id when there is one: `operation{op=<op> operation_id=<id>}`. The one place that names the span and its
  /// fields, which a reader greps in a log and which the root span of `bd` and the body of a Tauri command open
  /// (`src-tauri/src/tracing_log_test.rs`, `crates/bd-cli/src/operation_test.rs` quote the text)
  #[must_use]
  pub fn span(self, op: Option<&str>) -> tracing::Span {
    if let Some(op) = op {
      tracing::info_span!("operation", %op, operation_id = %self)
    } else {
      tracing::info_span!("operation", operation_id = %self)
    }
  }
}

impl OperationId {
  /// The root span of an operation of the remote that a session of the app runs (`operations::engine::run`):
  /// `operation{op=<what it does> operation_id=<id> tool=<app> subject=<subject>}`, the subject only for one that has
  /// it (`safe_push` of `auth`); the lines of its admission, its effects and its end, on every thread, carry it
  #[must_use]
  pub fn operation_span(self, op: &str, tool: &str, subject: Option<&str>) -> tracing::Span {
    let span = tracing::info_span!("operation", %op, operation_id = %self, %tool, subject = tracing::field::Empty);
    if let Some(subject) = subject {
      span.record("subject", tracing::field::display(subject));
    }
    span
  }
}

impl Default for OperationId {
  fn default() -> Self {
    Self::new()
  }
}

/// 80 bits from the OS; should the OS refuse, from the randomly keyed hasher of std, which keeps the ids apart too
fn random_bits() -> u128 {
  let mut bytes = [0u8; 16];
  if getrandom::fill(&mut bytes[..10]).is_err() {
    let state = std::collections::hash_map::RandomState::new();
    for (index, chunk) in bytes[..10].chunks_mut(5).enumerate() {
      let mut hasher = state.build_hasher();
      hasher.write_usize(index);
      chunk.copy_from_slice(&hasher.finish().to_le_bytes()[..5]);
    }
  }
  u128::from_le_bytes(bytes)
}

impl fmt::Display for OperationId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for index in 0..ENCODED_LENGTH {
      let shift = 5 * (ENCODED_LENGTH - 1 - index);
      f.write_char(char::from(ALPHABET[((self.0 >> shift) & 0x1f) as usize]))?;
    }
    Ok(())
  }
}

impl fmt::Debug for OperationId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(self, f)
  }
}

impl serde::Serialize for OperationId {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(self)
  }
}
