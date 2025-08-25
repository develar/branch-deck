//! Parse IntelliJ IDEA issue navigation configuration from .idea/vcs.xml

use quick_xml::Reader;
use quick_xml::events::Event;
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
      Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
        match e.name().as_ref() {
          b"component" => {
            // Check if this is the IssueNavigationConfiguration component
            for attr in e.attributes().flatten() {
              if attr.key.as_ref() == b"name" {
                let value = String::from_utf8_lossy(&attr.value);
                if value == "IssueNavigationConfiguration" {
                  in_issue_navigation = true;
                }
              }
            }
          }
          b"list" if in_issue_navigation => {
            in_links_list = true;
          }
          b"IssueNavigationLink" if in_links_list => {
            in_issue_link = true;
            current_issue_regexp = None;
            current_link_regexp = None;
          }
          b"option" if in_issue_link => {
            // Parse option attributes
            let mut option_name: Option<String> = None;
            let mut option_value: Option<String> = None;

            for attr in e.attributes().flatten() {
              match attr.key.as_ref() {
                b"name" => {
                  option_name = Some(String::from_utf8_lossy(&attr.value).to_string());
                }
                b"value" => {
                  option_value = Some(String::from_utf8_lossy(&attr.value).to_string());
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
          b"IssueNavigationLink" if in_issue_link => {
            // Add the link if we have both regexps
            if let (Some(issue_regexp), Some(link_regexp)) = (current_issue_regexp.clone(), current_link_regexp.clone()) {
              links.push(IssueNavigationLink { issue_regexp, link_regexp });
            }
            in_issue_link = false;
          }
          b"list" if in_links_list => {
            in_links_list = false;
          }
          b"component" if in_issue_navigation => {
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

#[cfg(test)]
mod tests {
  use super::*;

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
