//! What must hold of the tokens of any file (`check`), for the example `tokens` over a corpus and the test of the
//! fixtures: the tokens are in the order of the text with the lines and columns of the view, each in every line it
//! names, with only blanks between them; the same text formatted again (`reformatted`) has the same tokens, the same
//! content and the same text without its blanks, and a name renamed has not.

use code_symbols::highlight::Grammar;
use code_symbols::tokens::{Equality, MAX_BYTES, Token, TokenClass, Tokens, compare, content, indentation_matters, is_blank, same_without_blanks, tokens};

/// What the checks of one file found
pub(crate) struct Checked {
  pub(crate) tokens: usize,
  /// The lines `reformatted` changed
  pub(crate) lines: usize,
}

/// The lines of a text as the view splits them (`\n`, `\r\n`, a lone `\r`): the bytes of each without its line end, and
/// the byte after its line end
fn view_lines(text: &str) -> Vec<(usize, usize, usize)> {
  let bytes = text.as_bytes();
  let mut lines = Vec::new();
  let (mut start, mut index) = (0, 0);
  while index < bytes.len() {
    if bytes[index] == b'\n' || bytes[index] == b'\r' {
      let next = index + if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') { 2 } else { 1 };
      lines.push((start, index, next));
      start = next;
      index = next;
    } else {
      index += 1;
    }
  }
  lines.push((start, bytes.len(), bytes.len()));
  lines
}

fn offset(byte: usize) -> u32 {
  u32::try_from(byte).unwrap_or(u32::MAX)
}

/// `text` with other whitespace where no token is: the indent of a line doubled (not where the indentation is the
/// syntax), two spaces after a line, a blank line after it; and the lines it changed. A line whose indent or whose end
/// is in a token (a string over lines, the text of a JSX element, a branch of `#if` not taken) stays, as does one that
/// ends with a `\` or with a token that runs to the end of its line, a directive, and the indent of a line with a
/// comment over several lines
pub(crate) fn reformatted(path: &str, text: &str, all: &Tokens) -> (String, usize) {
  let indented = indentation_matters(path);
  let mut out = String::with_capacity(text.len() * 2);
  let mut changed = 0;
  for (start, end, next) in view_lines(text) {
    let line = &text[start..end];
    let code = line.trim_start_matches([' ', '\t']);
    let indent = &line[..line.len() - code.len()];
    let free = |from: usize, to: usize| all.in_bytes(offset(from)..offset(to)).is_empty();
    let before = out.len();
    // a comment over several lines keeps the indent of its other lines: moved alone, its first line is another comment
    let comment = all
      .in_bytes(offset(start)..offset(end.max(start + 1)))
      .iter()
      .any(|token| token.is_comment() && token.end_line != token.line);
    if !indented && !comment && !indent.is_empty() && free(start, start) && free(start, start + indent.len()) {
      out.push_str(indent);
    }
    out.push_str(line);
    let last = all
      .in_bytes(offset(start)..offset(end.max(start + 1)))
      .last()
      .filter(|token| token.byte_end as usize == end);
    // a literal, or a token with whitespace inside it (a message, a conflict marker), takes what follows it on the line
    let to_the_end = |token: &Token| token.class == TokenClass::Literal || token.text_in(text).contains([' ', '\t']);
    let open_end = code.ends_with('\\') || code.starts_with('#') || last.is_some_and(to_the_end);
    if next > end && !open_end && free(end, end) && free(end, next) && free(next, next) {
      out.push_str("  ");
      out.push_str(&text[end..next]);
    }
    out.push_str(&text[end..next]);
    changed += usize::from(out.len() - before != next - start);
  }
  (out, changed)
}

/// Checks the tokens of `text`, a file at `path` of `grammar`: `None` for a text without tokens, the first thing that
/// does not hold as the error
pub(crate) fn check(grammar: Grammar, path: &str, text: &str) -> Result<Option<Checked>, String> {
  let Some(all) = tokens(grammar, text) else { return Ok(None) };
  let lines = view_lines(text);
  if all.line_count() as usize != lines.len() {
    return Err(format!("{} lines, the view has {}", all.line_count(), lines.len()));
  }
  let mut end = 0;
  let blank = |from: usize, to: usize| text.get(from..to).is_some_and(|between| between.chars().all(is_blank));
  for token in all.all() {
    let place = format!("{}:{}", token.line + 1, token.start);
    if token.byte_start < end || token.byte_end <= token.byte_start || token.byte_end as usize > text.len() {
      return Err(format!("{place}: the token {}..{} after {end}", token.byte_start, token.byte_end));
    }
    if !blank(end as usize, token.byte_start as usize) {
      return Err(format!("{place}: what stands before the token is not blank"));
    }
    end = token.byte_end;
    // its lines and columns are those of its bytes; a line break that ends its bytes is of the line it ends
    let bytes = token.text_in(text);
    let shown = bytes.strip_suffix("\r\n").or_else(|| bytes.strip_suffix(['\n', '\r'])).unwrap_or(bytes);
    let shown_end = token.byte_start as usize + shown.len();
    for (line, column, byte) in [(token.line, token.start, token.byte_start as usize), (token.end_line, token.end, shown_end)] {
      let Some(&(start, _, _)) = lines.get(line as usize) else {
        return Err(format!("{place}: no line {line}"));
      };
      let Some(before) = text.get(start..byte) else {
        return Err(format!("{place}: the byte {byte} is before its line"));
      };
      // a place inside a `\r\n` is the end of its line
      let before = before.strip_suffix('\r').unwrap_or(before);
      if before.encode_utf16().count() != column as usize || before.contains('\n') {
        return Err(format!("{place}: the column {column} of the byte {byte}"));
      }
      // the token is in the line it names
      if !all.in_lines(line..line + 1).iter().any(|it| it.byte_start == token.byte_start) {
        return Err(format!("{place}: the token is not in its line {line}"));
      }
    }
  }
  if !blank(end as usize, text.len()) {
    return Err("what stands after the last token is not blank".to_string());
  }
  if compare(all.all(), all.all()) != Equality::Same {
    return Err("the tokens are not the same as themselves".to_string());
  }

  let (formatted, changed) = reformatted(path, text, &all);
  let Some(again) = tokens(grammar, &formatted) else {
    // the indents doubled may take it over the limit
    return if formatted.len() > MAX_BYTES {
      Ok(Some(Checked {
        tokens: all.all().len(),
        lines: 0,
      }))
    } else {
      Err("formatted again, the text has no tokens".to_string())
    };
  };
  if compare(all.all(), again.all()) != Equality::Same {
    let first = all.all().iter().zip(again.all()).find(|(old, new)| old.key() != new.key());
    let place = first.map_or_else(
      || format!("{} tokens, then {}", all.all().len(), again.all().len()),
      |(old, new)| format!("{}:{} {:?}, then {:?}", old.line + 1, old.start, old.text_in(text), new.text_in(&formatted)),
    );
    return Err(format!("formatted again, the tokens differ: {place}"));
  }
  if content(all.all()) != content(again.all()) {
    return Err("formatted again, the content differs".to_string());
  }
  if !same_without_blanks([text], [formatted.as_str()]) {
    return Err("formatted again, the text differs without its blanks".to_string());
  }

  // a renamed name is another token
  if let Some(name) = all.all().iter().find(|token| token.class == TokenClass::Name) {
    let renamed = format!("{}{}_renamed{}", &text[..name.byte_start as usize], name.text_in(text), &text[name.byte_end as usize..]);
    if tokens(grammar, &renamed).is_some_and(|renamed| compare(all.all(), renamed.all()) != Equality::Different) {
      return Err(format!("{}:{}: {:?} renamed is the same token", name.line + 1, name.start, name.text_in(text)));
    }
  }
  Ok(Some(Checked {
    tokens: all.all().len(),
    lines: changed,
  }))
}
