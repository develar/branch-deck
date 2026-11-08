// The headers a summary writes, the text `trial-check` compares and the review shows (`as_written`)
use std::borrow::Cow;
use std::fmt::Debug;

pub fn borrowed<'a>(text: &'a str, fallback: &'static str) -> &'a str {
  if text.is_empty() { fallback } else { text }
}

pub fn generic<T: Debug + Clone, const N: usize>(items: [T; N]) -> Vec<T>
where
  T: Default,
{
  items.to_vec()
}

pub async unsafe fn modifiers(raw: *const u8, slice: &mut [u8]) -> Option<Box<dyn Fn(u8) -> u8 + Send>> {
  let _ = (raw, slice);
  None
}

pub(crate) fn closure(apply: impl Fn(&str) -> Cow<'_, str>, tuple: (u8, String)) -> Result<(), String> {
  let _ = (apply, tuple);
  Ok(())
}

#[must_use = "a \"quoted\" reason"]
pub const fn literal(byte: u8, default: char) -> u8 {
  let _ = default;
  byte
}

pub struct Holder<'a, T: ?Sized> {
  pub value: &'a T,
  pub(super) raw: r#"raw"#,
}

impl<'a, T: ?Sized + Debug> Holder<'a, T> {
  pub fn get(&self) -> &'a T {
    self.value
  }

  pub fn map<U, F>(self, transform: F) -> U
  where
    F: FnOnce(&'a T) -> U,
  {
    transform(self.value)
  }
}

pub trait Visit {
  type Output: Default;
  const LIMIT: usize = 8;

  fn visit(&mut self, depth: usize) -> Self::Output;
}

pub static NAMES: &[&str] = &["a", "b"];
pub type Callback<'a> = Box<dyn FnMut(&'a [u8]) -> bool + 'a>;
