//! The pieces of a leaf that the grammar does not split: the escapes of a string's content, the tags of a doc comment

use crate::highlight::Role;

/// How to split a leaf
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lex {
  /// Backslash escapes (`\n`, `A`, `\$`) in the base style's text
  Escapes,
  /// The escapes of a JavaScript string: `\n`, `\x1f`, `\u00e9`, `\u{1F600}`, an octal `\012`
  JsEscapes,
  /// A doc comment: `@tag` and the name after `@param`-like tags, a `KDoc` `[link]`, a Javadoc `{@link …}`
  Doc,
}

/// The spans of `text` (a leaf that starts at byte `offset`): calls `emit(end, style)` in order, the base style between
pub(crate) fn lex(lex: Lex, text: &str, offset: usize, base: u32, emit: &mut impl FnMut(usize, u32)) {
  match lex {
    Lex::Escapes => escapes(text, offset, base, emit),
    Lex::JsEscapes => js_escapes(text, offset, base, emit),
    Lex::Doc => doc(text, offset, base, emit),
  }
}

fn escapes(text: &str, offset: usize, base: u32, emit: &mut impl FnMut(usize, u32)) {
  escapes_by(text, offset, base, emit, escape_length);
}

/// The length of the escape at the start of `rest`, its `\\` included: a unicode `\\u0041`, else one character
fn escape_length(rest: &[u8]) -> usize {
  if rest.get(1) == Some(&b'u') && rest.len() >= 6 && rest[2..6].iter().all(u8::is_ascii_hexdigit) {
    6
  } else {
    2
  }
}

/// The length of the JavaScript escape at the start of `rest`, its `\\` included, as tree-sitter-javascript's
/// `escape_sequence` reads it
fn js_escape_length(rest: &[u8]) -> usize {
  let hex = |from: usize, count: usize| rest.len() >= from + count && rest[from..from + count].iter().all(u8::is_ascii_hexdigit);
  match rest.get(1) {
    Some(b'x') if hex(2, 2) => 4,
    Some(b'u') if hex(2, 4) => 6,
    Some(b'u') if rest.get(2) == Some(&b'{') => rest.iter().position(|it| *it == b'}').map_or(2, |close| close + 1),
    Some(b'0'..=b'7') => 1 + rest[1..].iter().take(3).take_while(|it| (b'0'..=b'7').contains(*it)).count(),
    Some(_) => 2,
    None => 1,
  }
}

fn js_escapes(text: &str, offset: usize, base: u32, emit: &mut impl FnMut(usize, u32)) {
  escapes_by(text, offset, base, emit, js_escape_length);
}

/// The escapes of `text`, each `length` long from its `\\`, in their colour, the rest in `base`
fn escapes_by(text: &str, offset: usize, base: u32, emit: &mut impl FnMut(usize, u32), length: fn(&[u8]) -> usize) {
  let bytes = text.as_bytes();
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] == b'\\' && index + 1 < bytes.len() {
      emit(offset + index, base);
      let end = (index + length(&bytes[index..])).min(bytes.len());
      // the escaped character may be a multi-byte one
      let end = (end..=bytes.len()).find(|&it| text.is_char_boundary(it)).unwrap_or(bytes.len());
      emit(offset + end, Role::Escape.style());
      index = end;
    } else {
      index += 1;
    }
  }
  emit(offset + bytes.len(), base);
}

/// The tags whose next word is the name they document
const NAMED_TAGS: [&str; 6] = ["param", "throws", "exception", "property", "see", "sample"];

fn doc(text: &str, offset: usize, base: u32, emit: &mut impl FnMut(usize, u32)) {
  let bytes = text.as_bytes();
  let word_end = |from: usize| {
    (from..bytes.len())
      .find(|&it| !(bytes[it].is_ascii_alphanumeric() || bytes[it] == b'_' || bytes[it] == b'.' || bytes[it] == b'#'))
      .unwrap_or(bytes.len())
  };
  let mut index = 0;
  while index < bytes.len() {
    let byte = bytes[index];
    let at_word_start = index == 0 || !(bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_');
    if byte == b'@' && at_word_start && bytes.get(index + 1).is_some_and(u8::is_ascii_alphabetic) {
      emit(offset + index, base);
      let end = word_end(index + 1);
      emit(offset + end, Role::DocTag.style());
      let tag = &text[index + 1..end];
      index = end;
      if NAMED_TAGS.contains(&tag) {
        let start = (index..bytes.len()).find(|&it| bytes[it] != b' ' && bytes[it] != b'\t').unwrap_or(bytes.len());
        let end = if bytes.get(start) == Some(&b'<') {
          (start..bytes.len()).find(|&it| bytes[it] == b'>').map_or(start, |it| it + 1)
        } else {
          word_end(start)
        };
        if end > start {
          emit(offset + start, base);
          emit(offset + end, Role::DocTagValue.style());
          index = end;
        }
      }
    } else if byte == b'[' && bytes.get(index + 1).is_some_and(u8::is_ascii_alphabetic) {
      // a KDoc link: `[Name]`, `[Name.member]`
      if let Some(close) = (index + 1..bytes.len()).find(|&it| bytes[it] == b']' || bytes[it] == b'\n')
        && bytes[close] == b']'
      {
        emit(offset + index + 1, base);
        emit(offset + close, Role::DocTagValue.style());
        index = close;
      } else {
        index += 1;
      }
    } else {
      index += 1;
    }
  }
  emit(offset + bytes.len(), base);
}
