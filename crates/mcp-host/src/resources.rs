//! The page of each view and each file of the Air build as resources of `brd mcp`
//! (openspec/specs/repository/ide-embedding/spec.md, "The page of each view is a resource built into the binary"):
//! `ui://branch-deck/<view>` is the page (`index.html` of the build, `text/html;profile=mcp-app`), which names its view
//! by the `hostContext` of the IDE, and `ui://branch-deck/<path>` each file of the build, so the lazy chunks and the
//! worker load as files of the origin of the app. Any other URI is `badParams`. A build of brd without the files of the
//! page (`dist-air/` absent) serves a stub page that says so.

use base64::Engine;
use serde_json::{Value, json};

include!(concat!(env!("OUT_DIR"), "/resources.rs"));

/// The scheme and the host of a resource of Branch Deck
pub const PREFIX: &str = "ui://branch-deck/";

/// The views of the app file (`docs/design/ide-embedding/protocol/examples/branch-deck.yaml`), each the page
pub const VIEWS: &[&str] = &["main", "activity", "review", "settings", "conflicting-files", "missing-commits"];

/// The type of the page of a view (SEP-1865)
const PAGE_TYPE: &str = "text/html;profile=mcp-app";

/// The page that a build without the files of the page serves
const STUB_PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>Branch Deck</title></head><body><p>This build of Branch Deck has no page for the IDE: build it with <code>vite build --mode air</code>.</p></body></html>";

/// The contents of `uri`: its text, or its bytes in base64; none for a URI that names no page and no file
pub fn read(uri: &str) -> Option<Value> {
  let path = uri.strip_prefix(PREFIX)?;
  if VIEWS.contains(&path) {
    let page = file("index.html").map_or_else(|| STUB_PAGE.to_owned(), |bytes| String::from_utf8_lossy(bytes).into_owned());
    return Some(json!({ "uri": uri, "mimeType": PAGE_TYPE, "text": page }));
  }
  let bytes = file(path)?;
  let mime = mime_of(path);
  Some(if is_text(mime) {
    json!({ "uri": uri, "mimeType": mime, "text": String::from_utf8_lossy(bytes) })
  } else {
    json!({ "uri": uri, "mimeType": mime, "blob": base64::engine::general_purpose::STANDARD.encode(bytes) })
  })
}

fn file(path: &str) -> Option<&'static [u8]> {
  FILES.iter().find(|(name, _)| *name == path).map(|(_, bytes)| *bytes)
}

/// The media type of a file of the build by its extension
fn mime_of(path: &str) -> &'static str {
  match path.rsplit_once('.').map(|(_, extension)| extension) {
    Some("html") => "text/html",
    Some("js" | "mjs") => "text/javascript",
    Some("css") => "text/css",
    Some("json" | "map") => "application/json",
    Some("svg") => "image/svg+xml",
    Some("wasm") => "application/wasm",
    Some("png") => "image/png",
    Some("woff2") => "font/woff2",
    Some("woff") => "font/woff",
    Some("ttf") => "font/ttf",
    _ => "application/octet-stream",
  }
}

fn is_text(mime: &str) -> bool {
  mime.starts_with("text/") || mime == "application/json" || mime == "image/svg+xml"
}

#[cfg(test)]
#[path = "resources_test.rs"]
mod resources_test;
