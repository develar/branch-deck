## MODIFIED Requirements

### Requirement: The environment of a run and `[host]`: heavy names the commands of the heavy runs

`heavy` of `[host]` MUST be a list of patterns, none empty. A pattern MUST match a command's words joined by one space,
whole, `*` any characters, the rest literal: as written after a leading `env` and `NAME=value` words, or by what it
runs (`pnpm` without its options, `playwright test` by any runner). A shell MUST count as itself and by what it runs:
its `-c` script, a shell in it, a script of the working tree read once, 64 at most. One it cannot read is heavy.

#### Scenario: The patterns of heavy
- **WHEN** `heavy` is `["cargo nextest run*--workspace*", "pnpm e2e*", "pnpm test"]`
- **THEN** `cargo nextest run --no-fail-fast --workspace`, `pnpm e2e-up` and `env DEBUG_E2E=1 pnpm e2e x.spec.ts`
  are heavy runs, and `cargo nextest run -p host-link`, `pnpm test:unit` and `env -i pnpm e2e` are not
  [@test] crates/host-link/src/config_test.rs (`a_heavy_pattern_matches_the_whole_command_line_after_env_and_its_assignments`, `a_pattern_matches_its_literal_ends_and_never_lets_them_overlap`)

#### Scenario: A command by what it runs
- **WHEN** `heavy` is `["playwright test*", "pnpm e2e*"]`
- **THEN** `npx playwright test x.spec.ts`, `npx -y playwright test`, `pnpm exec playwright test`, `pnpm playwright
  test`, a path of the binary, `pnpm -C web e2e`, `pnpm --dir=web e2e`, `pnpm --filter app -r e2e`,
  `pnpm --loglevel error e2e` and `/opt/homebrew/bin/pnpm -C . e2e` are heavy runs
- **AND** `npx playwright install webkit` and `pnpm -C web test:unit` are not, and `pnpm exec playwright test*`, written
  with its runner, still matches `pnpm exec playwright test`
  [@test] crates/host-link/src/config_test.rs (`a_command_is_matched_by_what_it_runs_without_the_options_of_pnpm_and_the_runner_of_playwright`, `the_host_table_of_this_repository_parses`)

#### Scenario: A shell that wraps a heavy command
- **WHEN** the command is `zsh -c 'pnpm install; pnpm e2e > e2e.log 2>&1'`, `bash -lc` with a script whose line runs
  `pnpm e2e`, a shell in such a script, or `zsh probe/run.sh` whose file runs `pnpm e2e`
- **THEN** the run is heavy by `pnpm e2e*`, and its line says `by `pnpm e2e*` of [host] in its script`
- **AND** so is `env FOO=1 zsh -c 'pnpm e2e'`, `./probe/run.sh` whose first line is `#!/bin/zsh` or that has no first
  line `#!` (a shell runs such a text itself), `. ./lib.sh` of such a file, `bash ./tool.py` after `./tool.py` with
  `#!/usr/bin/python3`, and `bash -c 'out="$(pnpm e2e)"'`
- **AND** `zsh -c "echo 'pnpm e2e'"`, `zsh -c 'cargo nextest run -p host-link'`, `zsh -l`, `bash -s -- x`, a binary of
  the tree run by its path and `source ~/.cargo/env` are no heavy runs
  [@test] crates/host-link/src/heavy_test.rs (`a_heavy_command_in_the_script_of_a_shell_makes_the_run_heavy`, `a_script_without_a_heavy_command_and_a_table_without_heavy_make_no_heavy_run`)
  [@test] crates/host-link/src/heavy_test.rs (`a_script_splits_into_its_commands_and_their_words_as_a_shell_splits_it`, `a_shell_comes_before_the_commands_it_runs`)

#### Scenario: A script file the client cannot read
- **WHEN** a shell of the command runs a script file outside the shipped working tree, absent from it, or over 256 KiB,
  or a shell is nested deeper than four shells
- **THEN** the run is heavy, and its line says `by its script `<path>`, which brd-host cannot read for `heavy` of [host]`
  [@test] crates/host-link/src/heavy_test.rs (`a_script_file_the_checkout_cannot_give_makes_the_run_heavy`, `a_script_file_is_read_from_the_working_tree_inside_it_and_within_its_size`)

#### Scenario: A pattern that names the wrapper
- **WHEN** `heavy` names the wrapper, `./probe/run.sh*`, `zsh -c *` or `./node_modules/.bin/playwright test*`, and what
  the wrapper runs is light (`echo hi`, the shim of the binary)
- **THEN** the run of the wrapper is heavy by that pattern, in the command of the run and in a script
  [@test] crates/host-link/src/heavy_test.rs (`a_pattern_that_names_the_wrapper_catches_it`, `a_shell_comes_before_the_commands_it_runs`)

#### Scenario: A script that reads itself, and the depth
- **WHEN** a script sources itself 40 times or calls itself 20 times, a tree of scripts has more files than 64, or a
  chain of scripts run by their paths goes past four
- **THEN** the self-reading script is read once, the tree counts as heavy after 64 reads, and the script past the
  depth counts as heavy
  [@test] crates/host-link/src/heavy_test.rs (`a_script_is_read_once_and_the_reads_are_bounded`, `a_script_past_the_depth_counts_as_heavy`)
