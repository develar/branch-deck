## MODIFIED Requirements

### Requirement: The environment of a run and `[host]`: heavy names the commands of the heavy runs

`heavy` of `[host]` MUST be a list of patterns, none empty. A pattern MUST match the words of a command joined by one
space, whole, after a leading `env` and the `NAME=value` words; `*` matches any characters, the rest is literal. A shell
of the command MUST count by what it runs: its `-c` script, a shell in it, a script file of the working tree, also one
run by its path or sourced. A shell's script file the client cannot read MUST count as heavy.

#### Scenario: The patterns of heavy
- **WHEN** `heavy` is `["cargo nextest run*--workspace*", "pnpm e2e*", "pnpm test"]`
- **THEN** `cargo nextest run --no-fail-fast --workspace`, `pnpm e2e-up` and `env DEBUG_E2E=1 pnpm e2e x.spec.ts`
  are heavy runs, and `cargo nextest run -p host-link`, `pnpm test:unit` and `env -i pnpm e2e` are not
  [@test] crates/host-link/src/config_test.rs (`a_heavy_pattern_matches_the_whole_command_line_after_env_and_its_assignments`, `a_pattern_matches_its_literal_ends_and_never_lets_them_overlap`)

#### Scenario: A shell that wraps a heavy command
- **WHEN** the command is `zsh -c 'pnpm install; pnpm e2e > e2e.log 2>&1'`, `bash -lc` with a script whose line runs
  `pnpm e2e`, a shell in such a script, or `zsh probe/run.sh` whose file runs `pnpm e2e`
- **THEN** the run is heavy by `pnpm e2e*`, and its line says `by `pnpm e2e*` of [host] in its script`
- **AND** so is `env FOO=1 zsh -c 'pnpm e2e'`, `./probe/run.sh` whose first line is `#!/bin/zsh`, `. ./lib.sh` of such a
  file, and `bash -c 'out="$(pnpm e2e)"'`
- **AND** `zsh -c "echo 'pnpm e2e'"`, `zsh -c 'cargo nextest run -p host-link'`, `zsh -l`, `bash -s -- x`, a binary of
  the tree run by its path and `source ~/.cargo/env` are no heavy runs
  [@test] crates/host-link/src/heavy_test.rs (`a_heavy_command_in_the_script_of_a_shell_makes_the_run_heavy`, `a_script_without_a_heavy_command_and_a_table_without_heavy_make_no_heavy_run`)
  [@test] crates/host-link/src/heavy_test.rs (`a_script_splits_into_its_commands_and_their_words_as_a_shell_splits_it`, `a_shell_stands_for_the_commands_it_runs`)

#### Scenario: A script file the client cannot read
- **WHEN** a shell of the command runs a script file outside the shipped working tree, absent from it, or over 256 KiB,
  or a shell is nested deeper than four shells
- **THEN** the run is heavy, and its line says `by its script `<path>`, which brd-host cannot read for `heavy` of [host]`
  [@test] crates/host-link/src/heavy_test.rs (`a_script_file_the_checkout_cannot_give_makes_the_run_heavy`, `a_script_file_is_read_from_the_working_tree_inside_it_and_within_its_size`)
