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
- **THEN** `npx playwright test x.spec.ts`, `npx -y playwright test`, `npx -p @playwright/test playwright test`,
  `/usr/local/bin/npx playwright test`, `pnpm exec playwright test`, `pnpm playwright test`, a path of the binary,
  `pnpm -C web e2e`, `pnpm --dir=web e2e`, `pnpm --filter app -r e2e`, `pnpm --loglevel error e2e`,
  `pnpm --registry https://registry.example e2e` and `/opt/homebrew/bin/pnpm -C . e2e` are heavy runs
- **AND** `npx playwright install webkit`, `pnpm -C web test:unit` and `pnpm --silent test:unit e2e-pins`, whose flag
  takes no value, are not, and `pnpm exec playwright test*`, written with its runner, still matches
  `pnpm exec playwright test`
  [@test] crates/host-link/src/config_test.rs (`a_command_is_matched_by_what_it_runs_without_the_options_of_pnpm_and_the_runner_of_playwright`, `the_host_table_of_this_repository_parses`)
