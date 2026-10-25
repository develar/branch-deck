//! Sizes as a setting writes them and a message names them: a number and a binary unit (`200G`, `50MiB`), as `df -h`
//! counts (`branchdeck.leaseMinFree`, `branchdeck.pushSizeLimit`, `bd lease get --min-free`).

/// A size: a number and a binary unit, `200G` or `200GiB` (K, M, G, T, P; `B` or none for bytes), as `df -h` counts;
/// the error says why the text is none
pub fn parse_size(text: &str) -> Result<u64, String> {
  let text = text.trim();
  let split = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
  let (number, unit) = text.split_at(split);
  let number: u64 = number
    .parse()
    .map_err(|error| format!("the size {text:?} is not a number and a unit, such as \"200G\" ({error})"))?;
  let unit = unit.trim();
  let unit = unit.strip_suffix("iB").or_else(|| unit.strip_suffix('B').filter(|rest| !rest.is_empty())).unwrap_or(unit);
  let shift = match unit {
    "" | "B" => 0,
    "K" | "k" => 10,
    "M" => 20,
    "G" => 30,
    "T" => 40,
    "P" => 50,
    _ => return Err(format!("the size {text:?} has no unit K, M, G, T or P")),
  };
  number.checked_mul(1 << shift).ok_or_else(|| format!("the size {text:?} is too large"))
}

/// `bytes` in the largest binary unit it fills, with one decimal from KiB up, as a message names a size: `512 B`,
/// `12.6 MiB`, `200.0 GiB`
#[expect(clippy::cast_precision_loss, reason = "one decimal of a size in a message")]
pub fn format_size(bytes: u64) -> String {
  const UNITS: [&str; 5] = ["KiB", "MiB", "GiB", "TiB", "PiB"];
  if bytes < 1 << 10 {
    return format!("{bytes} B");
  }
  let mut value = bytes as f64 / 1024.0;
  let mut unit = 0;
  while value >= 1024.0 && unit + 1 < UNITS.len() {
    value /= 1024.0;
    unit += 1;
  }
  format!("{value:.1} {}", UNITS[unit])
}
