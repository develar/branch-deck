//! Parse IntelliJ IDEA issue navigation configuration from .idea/vcs.xml, and the link it gives an issue key

use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::Event;
use std::fmt::Write as _;
use std::path::Path;
use sync_types::issue_navigation::{IssueNavigationConfig, IssueNavigationLink};
use tracing::{debug, warn};

/// Load issue navigation configuration from .idea/vcs.xml if it exists
pub fn load_issue_navigation_config(repository_path: &str) -> Option<IssueNavigationConfig> {
  let vcs_xml_path = Path::new(repository_path).join(".idea").join("vcs.xml");

  if !vcs_xml_path.exists() {
    debug!(path = ?vcs_xml_path, "No .idea/vcs.xml found");
    return None;
  }

  let xml_content = match std::fs::read_to_string(&vcs_xml_path) {
    Ok(content) => content,
    Err(e) => {
      warn!(error = %e, "Failed to read .idea/vcs.xml");
      return None;
    }
  };

  parse_issue_navigation_xml(&xml_content)
}

fn parse_issue_navigation_xml(xml: &str) -> Option<IssueNavigationConfig> {
  let mut reader = Reader::from_str(xml);
  reader.config_mut().trim_text(true);

  let mut links = Vec::new();
  let mut buf = Vec::new();

  let mut in_issue_navigation = false;
  let mut in_links_list = false;
  let mut in_issue_link = false;
  let mut current_issue_regexp: Option<String> = None;
  let mut current_link_regexp: Option<String> = None;

  loop {
    match reader.read_event_into(&mut buf) {
      Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
        match e.name().as_ref() {
          "component" => {
            // Check if this is the IssueNavigationConfiguration component
            for attr in e.attributes().flatten() {
              if attr.key.as_ref() == "name" && attr.value == "IssueNavigationConfiguration" {
                in_issue_navigation = true;
              }
            }
          }
          "list" if in_issue_navigation => {
            in_links_list = true;
          }
          "IssueNavigationLink" if in_links_list => {
            in_issue_link = true;
            current_issue_regexp = None;
            current_link_regexp = None;
          }
          "option" if in_issue_link => {
            // Parse option attributes
            let mut option_name: Option<String> = None;
            let mut option_value: Option<String> = None;

            for attr in e.attributes().flatten() {
              match attr.key.as_ref() {
                "name" => {
                  option_name = Some(attr.value.into_owned());
                }
                "value" => {
                  // Unescape entities (e.g. `&amp;` in URLs) and normalize whitespace as per XML spec
                  option_value = attr.normalized_value(XmlVersion::Implicit1_0).ok().map(std::borrow::Cow::into_owned);
                }
                _ => {}
              }
            }

            // Store the values based on option name
            if let (Some(name), Some(value)) = (option_name, option_value) {
              match name.as_str() {
                "issueRegexp" => {
                  current_issue_regexp = Some(value);
                }
                "linkRegexp" => {
                  current_link_regexp = Some(value);
                }
                _ => {}
              }
            }
          }
          _ => {}
        }
      }
      Ok(Event::End(ref e)) => {
        match e.name().as_ref() {
          "IssueNavigationLink" if in_issue_link => {
            // Add the link if we have both regexps
            if let (Some(issue_regexp), Some(link_regexp)) = (current_issue_regexp.clone(), current_link_regexp.clone()) {
              links.push(IssueNavigationLink { issue_regexp, link_regexp });
            }
            in_issue_link = false;
          }
          "list" if in_links_list => {
            in_links_list = false;
          }
          "component" if in_issue_navigation => {
            // We can break here as we've found what we need
            break;
          }
          _ => {}
        }
      }
      Ok(Event::Eof) => break,
      Err(e) => {
        warn!(error = %e, "Error parsing .idea/vcs.xml");
        break;
      }
      _ => {}
    }
    buf.clear();
  }

  if links.is_empty() {
    debug!("No issue navigation links found in .idea/vcs.xml");
    None
  } else {
    debug!(links = links.len(), "Found issue navigation links");
    Some(IssueNavigationConfig { links })
  }
}

/// The link of the issue `key`: of the first link of `config` whose pattern matches the whole key, with `$0` the key and
/// `$1`, `$2`… its groups, percent-encoded, as the app links a key (`src/lib/issue-links.ts`); None when no pattern does
pub fn issue_link(config: &IssueNavigationConfig, key: &str) -> Option<String> {
  config.links.iter().find_map(|link| {
    // IntelliJ's patterns are Java regexes; one that this engine refuses links nothing, as in the app
    let regex = regex::Regex::new(&link.issue_regexp).ok()?;
    let captures = regex.captures_iter(key).find(|captures| captures.get(0).is_some_and(|whole| whole.as_str() == key))?;
    let mut href = link.link_regexp.replace("$0", &encode_uri_component(key));
    for group in 1..captures.len() {
      href = href.replace(&format!("${group}"), &encode_uri_component(captures.get(group).map_or("", |value| value.as_str())));
    }
    Some(href)
  })
}

/// JavaScript's `encodeURIComponent`
fn encode_uri_component(text: &str) -> String {
  let mut encoded = String::with_capacity(text.len());
  for byte in text.bytes() {
    if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
      encoded.push(char::from(byte));
    } else {
      let _ = write!(encoded, "%{byte:02X}");
    }
  }
  encoded
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_key_links_as_the_first_pattern_that_matches_all_of_it_names_it() {
    let link = |issue_regexp: &str, link_regexp: &str| IssueNavigationLink {
      issue_regexp: issue_regexp.to_string(),
      link_regexp: link_regexp.to_string(),
    };
    let config = IssueNavigationConfig {
      links: vec![
        link(r"EA\-(\d+)", "https://ea.example.com/#/issue/$1"),
        link(r"\b[A-Z]+\-\d+\b", "https://youtrack.example.com/issue/$0"),
      ],
    };
    assert_eq!(issue_link(&config, "IJAI-4390").as_deref(), Some("https://youtrack.example.com/issue/IJAI-4390"));
    assert_eq!(issue_link(&config, "EA-12").as_deref(), Some("https://ea.example.com/#/issue/12"));
    // a pattern that matches only a part of the key links nothing
    let partial = IssueNavigationConfig {
      links: vec![link(r"\d+", "https://example.com/$0")],
    };
    assert_eq!(issue_link(&partial, "IJAI-4390"), None);
    let spaced = IssueNavigationConfig {
      links: vec![link(r"[A-Z]+ \d+", "https://example.com/?q=$0")],
    };
    assert_eq!(issue_link(&spaced, "AB 1").as_deref(), Some("https://example.com/?q=AB%201"));
  }

  #[test]
  fn test_parse_issue_navigation_xml() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="4">
  <component name="IssueNavigationConfiguration">
    <option name="links">
      <list>
        <IssueNavigationLink>
          <option name="issueRegexp" value="\b[A-Z]+\-\d+\b" />
          <option name="linkRegexp" value="https://youtrack.jetbrains.com/issue/$0" />
        </IssueNavigationLink>
        <IssueNavigationLink>
          <option name="issueRegexp" value="EA\-(\d+)" />
          <option name="linkRegexp" value="https://web.ea.pages.jetbrains.team/#/issue/$1" />
        </IssueNavigationLink>
      </list>
    </option>
  </component>
</project>"#;

    let config = parse_issue_navigation_xml(xml).expect("Config should be parsed");
    assert_eq!(config.links.len(), 2);

    assert_eq!(config.links[0].issue_regexp, r"\b[A-Z]+\-\d+\b");
    assert_eq!(config.links[0].link_regexp, "https://youtrack.jetbrains.com/issue/$0");

    assert_eq!(config.links[1].issue_regexp, r"EA\-(\d+)");
    assert_eq!(config.links[1].link_regexp, "https://web.ea.pages.jetbrains.team/#/issue/$1");
  }

  #[test]
  fn test_parse_escaped_attribute_values() {
    let xml = r##"<project version="4">
  <component name="IssueNavigationConfiguration">
    <option name="links">
      <list>
        <IssueNavigationLink>
          <option name="issueRegexp" value="#(\d+)" />
          <option name="linkRegexp" value="https://tracker.example.com/?project=a&amp;id=$1" />
        </IssueNavigationLink>
      </list>
    </option>
  </component>
</project>"##;

    let config = parse_issue_navigation_xml(xml).expect("Config should be parsed");
    assert_eq!(config.links.len(), 1);
    assert_eq!(config.links[0].link_regexp, "https://tracker.example.com/?project=a&id=$1");
  }

  #[test]
  fn test_parse_empty_xml() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="4">
  <component name="VcsDirectoryMappings">
    <mapping directory="" vcs="Git" />
  </component>
</project>"#;

    let config = parse_issue_navigation_xml(xml);
    assert!(config.is_none());
  }

  #[test]
  fn test_parse_malformed_xml() {
    let xml = "not valid xml";
    let config = parse_issue_navigation_xml(xml);
    assert!(config.is_none());
  }
}
