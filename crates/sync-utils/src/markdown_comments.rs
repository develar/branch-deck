//! The HTML comments of a body (`without_comments`): `<!-- … -->`, also over several lines, the hints a template of a
//! pull request leaves in its description, which GitHub never shows. Outside a fence, a line indented as code and a code
//! span; an opening without a `-->` after it anywhere in the body is no comment, so a sentence about `<!--` keeps its
//! words. The words around a comment on its line are joined by a space; a line a comment holds alone, or covers, is
//! blank, so the blocks around it stay apart.

use crate::commit_body::{fence_of, is_code_indented};
use crate::markdown_inline::code_span_len;
use std::borrow::Cow;

/// `body` without its HTML comments (the module's header)
pub(crate) fn without_comments(body: &str) -> Cow<'_, str> {
  let Some(last_close) = body.rfind("-->") else {
    return Cow::Borrowed(body);
  };
  if !body.contains("<!--") {
    return Cow::Borrowed(body);
  }
  let mut lines: Vec<Cow<'_, str>> = Vec::new();
  let mut fence: Option<&'static str> = None;
  let mut in_comment = false;
  let mut line_start = 0;
  for line in body.split('\n') {
    let offset = line_start;
    line_start += line.len() + 1;
    if let Some(mark) = fence {
      if line.trim_start().starts_with(mark) {
        fence = None;
      }
      lines.push(Cow::Borrowed(line));
      continue;
    }
    if !in_comment && (is_code_indented(line) || !line.contains("<!--")) {
      fence = fence_of(line);
      lines.push(Cow::Borrowed(line));
      continue;
    }
    let starts_outside = !in_comment;
    // the pieces of the line outside its comments
    let mut pieces: Vec<&str> = Vec::new();
    let mut at = 0;
    loop {
      if in_comment {
        let Some(end) = line[at..].find("-->") else { break };
        at += end + 3;
        in_comment = false;
      }
      let Some(start) = comment_start(&line[at..]).filter(|start| last_close >= offset + at + start + 4) else {
        pieces.push(&line[at..]);
        break;
      };
      pieces.push(&line[at..at + start]);
      at += start + 4;
      in_comment = true;
    }
    let words: Vec<&str> = pieces.iter().map(|piece| piece.trim()).filter(|piece| !piece.is_empty()).collect();
    let indent = if starts_outside { &line[..line.len() - line.trim_start().len()] } else { "" };
    lines.push(if words.is_empty() {
      Cow::Borrowed("")
    } else {
      Cow::Owned(format!("{indent}{}", words.join(" ")))
    });
  }
  Cow::Owned(lines.join("\n"))
}

/// The place of the `<!--` of `text` outside its code spans
fn comment_start(text: &str) -> Option<usize> {
  let mut at = 0;
  while let Some(next) = text[at..].chars().next() {
    let rest = &text[at..];
    if rest.starts_with("<!--") {
      return Some(at);
    }
    at += if next == '`' { code_span_len(rest) } else { next.len_utf8() };
  }
  None
}
