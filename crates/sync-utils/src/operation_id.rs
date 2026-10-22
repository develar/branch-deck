//! The id of one operation of the app or `bd`: a ULID (48 bits of Unix milliseconds, then 80 random bits), written as
//! 26 characters of Crockford base32.
//!
//! The time first makes the ids of one log sort by when they started; the random part keeps the ids of the app and of
//! every `bd` process apart without a shared counter. The root span of a `bd` command and the span of a Tauri command's
//! `run_blocking` carry it as `operation_id`, and a `CommandError` names it, so the Activity entry of an error leads to
//! its log lines.

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
