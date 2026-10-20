//! The URL of a YouTrack as the user gives it, and the URLs built from it. A token goes only to the YouTrack it is for:
//! https (http only to a loopback host, the fake of the tests), the path from segments, an issue key checked first, so
//! a key that a repository's `.idea/vcs.xml` put into a link never names another path.

use reqwest::Url;

/// The first path segments of YouTrack's own pages: a pasted page URL keeps only what comes before them
const PAGE_SEGMENTS: [&str; 9] = ["issue", "issues", "dashboard", "agiles", "articles", "projects", "users", "admin", "api"];

/// `https://youtrack.example.com/youtrack/issue/ABC-1?x#y` → `https://youtrack.example.com/youtrack`. A URL without a
/// scheme is https. Refused: another scheme, http to a host that is not loopback, a user name or password.
pub fn normalize_base_url(input: &str) -> Result<String, String> {
  let input = input.trim();
  if input.is_empty() {
    return Err("Enter the URL of your YouTrack".to_string());
  }
  let with_scheme = if input.contains("://") { input.to_string() } else { format!("https://{input}") };
  let mut url = Url::parse(&with_scheme).map_err(|e| format!("Not a URL: {e}"))?;
  check_url(&url)?;
  if !url.username().is_empty() || url.password().is_some() {
    return Err("The URL must not hold a user name or password".to_string());
  }
  url.set_query(None);
  url.set_fragment(None);
  let segments: Vec<String> = url.path_segments().map_or_else(Vec::new, |segments| {
    segments
      .take_while(|segment| !PAGE_SEGMENTS.contains(segment))
      .filter(|segment| !segment.is_empty())
      .map(str::to_string)
      .collect()
  });
  url.set_path(&segments.join("/"));
  Ok(url.as_str().trim_end_matches('/').to_string())
}

/// https, or http to a loopback host
pub fn check_url(url: &Url) -> Result<(), String> {
  match url.scheme() {
    "https" => Ok(()),
    "http" if is_loopback(url) => Ok(()),
    "http" => Err("YouTrack must be reached over https".to_string()),
    scheme => Err(format!("Not a web URL: {scheme}")),
  }
}

fn is_loopback(url: &Url) -> bool {
  match url.host() {
    Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
    Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
    Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
    None => false,
  }
}

/// `ABC-1`, `IJPL_X-42`: a project short name, a dash, a number
pub fn is_issue_key(key: &str) -> bool {
  let Some((project, number)) = key.rsplit_once('-') else {
    return false;
  };
  let mut chars = project.chars();
  chars.next().is_some_and(|first| first.is_ascii_alphabetic())
    && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    && !number.is_empty()
    && number.chars().all(|c| c.is_ascii_digit())
}

/// `<base>/<segments…>`, each segment escaped
pub fn url_of(base: &str, segments: &[&str]) -> Result<Url, String> {
  let mut url = Url::parse(base).map_err(|e| format!("Not a URL: {e}"))?;
  check_url(&url)?;
  url
    .path_segments_mut()
    .map_err(|()| "The URL cannot have a path".to_string())?
    .pop_if_empty()
    .extend(segments);
  Ok(url)
}

/// `<base>/api/<segments…>?fields=<fields>`
pub fn api_url(base: &str, segments: &[&str], fields: &str) -> Result<Url, String> {
  let mut all = vec!["api"];
  all.extend_from_slice(segments);
  let mut url = url_of(base, &all)?;
  url.query_pairs_mut().append_pair("fields", fields);
  Ok(url)
}

/// The page of an issue: `<base>/issue/<key>`
pub fn issue_page(base: &str, key: &str) -> String {
  url_of(base, &["issue", key]).map_or_else(|_| format!("{base}/issue/{key}"), |url| url.to_string())
}
