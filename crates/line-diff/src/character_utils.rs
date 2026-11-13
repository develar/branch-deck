// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/CharacterUtils.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). The Unicode data of
// ICU4X (`icu_properties`) replaces the range tables of the source and Kotlin's `CharCategory`.

//! Code points of UTF-16 text and the Unicode properties that split a text into words.

use icu_properties::props::{Alphabetic, GeneralCategory, Ideographic, Script};
use icu_properties::{CodePointMapData, CodePointSetData};

/// The minimum value of a supplementary code point, `U+10000`
const MIN_SUPPLEMENTARY_CODE_POINT: u32 = 0x10000;

/// The code point at `index`: a surrogate pair decoded, an unpaired surrogate as itself
pub fn code_point_at(text: &[u16], index: usize) -> u32 {
  let high = text[index];
  if is_high_surrogate(high)
    && let Some(&low) = text.get(index + 1)
    && is_low_surrogate(low)
  {
    return to_code_point(high, low);
  }
  u32::from(high)
}

/// The number of UTF-16 code units of a code point
pub const fn char_count(code_point: u32) -> usize {
  if code_point >= MIN_SUPPLEMENTARY_CODE_POINT { 2 } else { 1 }
}

/// A letter (`Lu`, `Ll`, `Lt`, `Lm`, `Lo`), a letter number (`Nl`) or `Other_Alphabetic`: the derived `Alphabetic`
/// property. The source checks the general category and the `Other_Alphabetic` table of `PropList.txt`; `Alphabetic`
/// also holds `Other_Lowercase` and `Other_Uppercase`, whose chars are letters or `Other_Alphabetic` too.
pub fn is_alphabetic(code_point: u32) -> bool {
  CodePointSetData::new::<Alphabetic>().contains32(code_point)
}

pub fn is_ideographic(code_point: u32) -> bool {
  CodePointSetData::new::<Ideographic>().contains32(code_point)
}

pub const fn is_bmp_code_point(code_point: u32) -> bool {
  code_point >> 16 == 0
}

pub fn is_hiragana_script(code_point: u32) -> bool {
  script(code_point) == Script::Hiragana
}

pub fn is_katakana_script(code_point: u32) -> bool {
  script(code_point) == Script::Katakana
}

pub fn is_thai_script(code_point: u32) -> bool {
  script(code_point) == Script::Thai
}

pub fn is_javanese_script(code_point: u32) -> bool {
  script(code_point) == Script::Javanese
}

/// A decimal digit of any script, the general category `Nd`
pub fn is_decimal_digit(code_point: u32) -> bool {
  CodePointMapData::<GeneralCategory>::new().get32(code_point) == GeneralCategory::DecimalNumber
}

/// The `Script` property of `Scripts.txt`, not `Script_Extensions`, as the tables of the source
fn script(code_point: u32) -> Script {
  CodePointMapData::<Script>::new().get32(code_point)
}

const fn is_high_surrogate(unit: u16) -> bool {
  matches!(unit, 0xD800..=0xDBFF)
}

const fn is_low_surrogate(unit: u16) -> bool {
  matches!(unit, 0xDC00..=0xDFFF)
}

/// Converts a surrogate pair to a code point
const fn to_code_point(high: u16, low: u16) -> u32 {
  (((high as u32 - 0xD800) << 10) | (low as u32 - 0xDC00)) + 0x10000
}
