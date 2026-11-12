use crate::resources::{FILES, VIEWS, read};

/// Every view is the page, as `text/html;profile=mcp-app`; a file the build lacks and another scheme are none
#[test]
fn a_view_is_the_page_and_a_missing_file_is_none() {
  for view in VIEWS {
    let contents = read(&format!("ui://branch-deck/{view}")).unwrap();
    assert_eq!(contents["mimeType"], "text/html;profile=mcp-app");
    assert!(contents["text"].as_str().is_some_and(|page| page.contains("<html")), "{contents}");
  }
  assert!(read("ui://branch-deck/nothing.js").is_none());
  assert!(read("ui://other/main").is_none());
}

/// Each file of the build is a resource by its path, its text or its bytes
#[test]
fn each_file_of_the_build_is_a_resource() {
  for (path, bytes) in FILES {
    let contents = read(&format!("ui://branch-deck/{path}")).unwrap();
    assert!(contents.get("text").is_some() || contents.get("blob").is_some(), "{path}");
    assert!(!bytes.is_empty() || contents["text"] == "", "{path}");
  }
}
