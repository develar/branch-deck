//! The SHA-256 of a binary as 64 lower-case hex digits: the name of a build in the hello and in the upgrade. The
//! version of Branch Deck can be the same for two builds, so only the SHA-256 tells them apart

use sha2::{Digest as _, Sha256 as Sha2};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// A SHA-256 that takes its bytes in parts
#[derive(Default)]
pub struct Sha256(Sha2);

impl Sha256 {
  pub fn update(&mut self, bytes: &[u8]) {
    self.0.update(bytes);
  }

  /// The digest as 32 bytes
  pub fn bytes(self) -> [u8; 32] {
    self.0.finalize().into()
  }

  /// The digest as 64 lower-case hex digits
  pub fn hex(self) -> String {
    self.bytes().iter().fold(String::with_capacity(64), |mut text, byte| {
      text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
      text.push(char::from_digit(u32::from(byte & 0xf), 16).unwrap_or('0'));
      text
    })
  }
}

/// The SHA-256 of the file `path`, read in parts of 64 KiB
pub fn sha256_file(path: &Path) -> io::Result<String> {
  let mut file = File::open(path)?;
  let mut digest = Sha256::default();
  let mut buffer = vec![0u8; 64 * 1024];
  loop {
    match file.read(&mut buffer) {
      Ok(0) => return Ok(digest.hex()),
      Ok(read) => digest.update(&buffer[..read]),
      Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
      Err(error) => return Err(error),
    }
  }
}

/// Whether `text` is 64 lower-case hex digits
pub fn is_sha256(text: &str) -> bool {
  text.len() == 64 && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}
