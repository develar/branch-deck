// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/TrimUtilTest.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2015 JetBrains s.r.o. and contributors).

use crate::test_support::{CH_MAN, CH_SMILE};
use crate::trim_util::{is_alpha, is_continuous_script, is_punctuation};

const PUNCTUATION: &str = "(){}[],./?`~!@#$%^&*-=+|\\;:'\"<>";

#[test]
fn punctuation() {
  for c in 0..=u32::from(u16::MAX) {
    let expected = char::from_u32(c).is_some_and(|c| PUNCTUATION.contains(c));
    assert_eq!(expected, is_punctuation(c), "{c}");
  }
}

#[test]
fn continuous_script() {
  assert_is_alpha("!? \t\n+~", false);

  assert_is_alpha("AB12", true);
  assert_is_alpha("АБВ汉语日ひรไ", true);
  assert_is_alpha("óèäñĀ", true);
  assert_is_alpha("\r_\u{0001}", true);
  assert_is_alpha(&format!("{CH_SMILE}{CH_MAN}"), true);

  assert_is_continuous("12_ABZ", false);
  assert_is_continuous("АБВ", false);
  assert_is_continuous("ʁit", false);
  assert_is_continuous("음훈", false);
  assert_is_continuous("óèäñĀ", false);
  assert_is_continuous("\r_\u{0001}", false);

  assert_is_continuous("象形文字", true);
  assert_is_continuous("อักษรไ", true);
  assert_is_continuous("ひらがなカタカナ日本語", true);
  assert_is_continuous("汉语漢語", true);
  assert_is_continuous("☺♥", true);
  assert_is_continuous(&format!("{CH_SMILE}{CH_MAN}"), true);
  assert_is_continuous("\u{200e}\u{200f}\u{061c}", true);
}

fn assert_is_alpha(text: &str, expected: bool) {
  for c in text.chars() {
    assert_eq!(expected, is_alpha(u32::from(c)), "{} - {c}", u32::from(c));
  }
}

fn assert_is_continuous(text: &str, expected: bool) {
  for c in text.chars() {
    assert_eq!(expected, is_continuous_script(u32::from(c)), "{} - {c}", u32::from(c));
  }
}
