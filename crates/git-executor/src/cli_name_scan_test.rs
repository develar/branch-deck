//! Agent-facing source text names the CLI with its current name.

use crate::source_scan_test::{Exemption, Source, all_crate_sources, check_sources};

const OLD_CLI_NAME: &str = concat!("b", "d");
const EXEMPTIONS: &[Exemption] = &[Exemption {
  path: "crates/app-api/src/api_error_test.rs",
  hits: 1,
  reason: "identifies a process holding the previous CLI name to distinguish it from the application",
}];

#[test]
fn printed_commands_use_brd() {
  check_sources(
    "crate source commands use the current CLI name",
    "use `brd` for agent commands",
    &all_crate_sources(),
    EXEMPTIONS,
    command_lines,
  );
}

fn command_lines(source: &Source) -> Vec<usize> {
  source
    .text
    .lines()
    .enumerate()
    .filter_map(|(index, line)| has_old_command(line).then_some(index + 1))
    .collect()
}

fn has_old_command(line: &str) -> bool {
  line.match_indices(OLD_CLI_NAME).any(|(index, _)| {
    let before = line[..index].chars().next_back();
    if before.is_some_and(|character| character.is_ascii_alphanumeric() || character == '_') {
      return false;
    }
    let Some(rest) = line[index + OLD_CLI_NAME.len()..].strip_prefix(' ') else {
      return false;
    };
    if let Some(after_option) = rest.strip_prefix("-C") {
      return after_option.chars().next().is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
    }
    rest.chars().next().is_some_and(|character| character.is_ascii_alphabetic())
  })
}
