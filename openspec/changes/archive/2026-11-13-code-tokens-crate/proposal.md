# Proposal

## Why

The marks of a diff are to be one implementation that Branch Deck and accio share, moved into psi-syntax later: the
tokens of a file and the rule "equal by tokens" are a pure function of a text, but they lived inside `code-symbols`
beside the summaries, the highlighting and the navigation, and their version was keyed by the version of the
summaries (`extractor_version`), a crate they do not need.

## What Changes

- `crates/code-tokens`: the tokens (`code_symbols::tokens` and the `tokens.rs` of each language), `Grammar`,
  `grammar_of`, `Grammar::is_enabled`, `indentation_matters`, and the parse of each language by one parser per
  thread, which `code-symbols` reads its trees with (`code_tokens::kotlin::parse`, `code_tokens::js::with_program`):
  a thread keeps one parser of a language, not two. No git, no IO; Apache-2.0, ready to move as it is.
- `tokens_version(parsers)` is keyed by the rules of the tokens (`RULES_VERSION`), the grammars of the build and the
  revision of the parsers its consumer names (`code_symbols::extract::PSI_SYNTAX_REVISION`), no longer by the rules of
  the summaries. Its value changes once: the `file_diff` store reads no stored diff of before and makes it again.
- The oracle of the tokens (`examples/shared/token_checks.rs`) is `code_tokens::checks`; the test of the fixtures of
  `code-symbols` stays with them (`tokens_fixture_test.rs`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/equal-by-tokens`: the version of the tokens does not depend on the rules of the summaries.

## Impact

- `crates/code-tokens/` (new), `crates/code-symbols/src/{lib,extract}.rs`, the `mod.rs` of each language, the walks
  that parse (`highlight`, `navigate`, `summary` of each language), `crates/file-diffs/src/service.rs`
  (`model_version`), the imports of `Grammar`, `grammar_of` and the tokens in `file-diffs`, `review-plan`, `bd-rewrite`.
- `docs/contributing/code-map.md`.
