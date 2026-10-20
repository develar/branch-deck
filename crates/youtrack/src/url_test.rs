use crate::url::{api_url, is_issue_key, issue_page, normalize_base_url};
use pretty_assertions::assert_eq;

#[test]
fn a_pasted_url_is_cut_to_the_youtrack_it_names() {
  for (input, expected) in [
    ("youtrack.example.com", "https://youtrack.example.com"),
    ("https://youtrack.example.com/", "https://youtrack.example.com"),
    ("https://YouTrack.Example.com/issue/ABC-1?tab=x#y", "https://youtrack.example.com"),
    ("https://example.com/youtrack/issues?q=for:me", "https://example.com/youtrack"),
    ("https://example.com/youtrack/dashboard", "https://example.com/youtrack"),
    ("http://127.0.0.1:8080/", "http://127.0.0.1:8080"),
    ("http://localhost:8080/youtrack", "http://localhost:8080/youtrack"),
  ] {
    assert_eq!(normalize_base_url(input).as_deref(), Ok(expected), "{input}");
  }
}

#[test]
fn a_token_goes_only_over_https_and_never_in_the_url() {
  for input in [
    "http://youtrack.example.com",
    "ftp://youtrack.example.com",
    "https://user:secret@youtrack.example.com",
    "  ",
  ] {
    assert!(normalize_base_url(input).is_err(), "{input}");
  }
}

#[test]
fn only_an_issue_key_is_a_path_segment_of_a_lookup() {
  for key in ["ABC-1", "IJPL_X-42", "a1-7"] {
    assert!(is_issue_key(key), "{key}");
  }
  for key in ["..%2Fadmin-1", "ABC-", "-1", "1ABC-2", "ABC-1/x", "ABC-1?x", "ABC 1", "../users-1"] {
    assert!(!is_issue_key(key), "{key}");
  }
}

#[test]
fn a_segment_is_escaped_and_never_leaves_the_base() {
  let url = api_url("https://example.com/youtrack", &["issues", "a/b?c"], "id").unwrap();
  assert_eq!(url.as_str(), "https://example.com/youtrack/api/issues/a%2Fb%3Fc?fields=id");
  assert_eq!(issue_page("https://example.com/youtrack", "ABC-1"), "https://example.com/youtrack/issue/ABC-1");
}
