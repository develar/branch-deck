//! Text that bd reads from a file or stdin (a plan, a pull request body): UTF-8, with a byte order mark dropped, or
//! UTF-16 with its byte order mark, which Windows PowerShell 5.1 writes (`Set-Content -Encoding UTF8` adds a UTF-8
//! BOM, `Out-File` and `>` write UTF-16LE)

use crate::error::{CliError, ErrorCode};
use std::io::IsTerminal;
use std::path::Path;

/// The text of `bytes`: UTF-8 without its byte order mark, or UTF-16 (little or big endian) with one
///
/// # Errors
/// The reason when the bytes are neither
pub fn decode_text(bytes: &[u8]) -> Result<String, String> {
  if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
    return String::from_utf8(rest.to_vec()).map_err(|e| e.to_string());
  }
  let utf16 = |rest: &[u8], from: fn([u8; 2]) -> u16| {
    if !rest.len().is_multiple_of(2) {
      return Err("UTF-16 with an odd number of bytes".to_string());
    }
    let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|pair| from(*pair)).collect();
    String::from_utf16(&units).map_err(|e| e.to_string())
  };
  if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
    return utf16(rest, u16::from_le_bytes);
  }
  if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
    return utf16(rest, u16::from_be_bytes);
  }
  String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())
}

/// The text of the file `path`, of stdin for `-`; `what` names it in an error (`the plan`, `the body`)
///
/// # Errors
/// `invalid_arguments` when it can't be read or decoded
pub fn read_text(path: &Path, what: &str) -> Result<String, CliError> {
  let (bytes, source) = if path.as_os_str() == "-" {
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes).map_err(|e| invalid(format!("Cannot read {what} from stdin: {e}")))?;
    (bytes, "stdin".to_string())
  } else {
    let bytes = std::fs::read(path).map_err(|e| invalid(format!("Cannot read {}: {e}", path.display())))?;
    (bytes, path.display().to_string())
  };
  decode_text(&bytes).map_err(|e| invalid(format!("Cannot read {what} from {source}: {e}; write it as UTF-8")))
}

/// The text of stdin, or nothing when stdin is a terminal
///
/// # Errors
/// As `read_text`
pub fn read_stdin_unless_terminal(what: &str) -> Result<String, CliError> {
  if std::io::stdin().is_terminal() {
    return Ok(String::new());
  }
  read_text(Path::new("-"), what)
}

fn invalid(message: String) -> CliError {
  CliError::new(ErrorCode::InvalidArguments, message)
}
