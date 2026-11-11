# Tasks

- [x] 1.1 `whitespace::TextTokens` and `Sides`: the tokens of a blob once per plan, the answer for a hunk, a block and
  a declaration, the text a fingerprint reads.
- [x] 1.2 `files::mechanical_kind`, `units::SourceFile::owner` and `cut` ask it; `indentation_matters` comes from
  `code_symbols::tokens`.
- [x] 1.3 Tests: a file of each case of the rule, a block and a pair of one file, the fingerprint of the same change
  with other whitespace.
- [x] 1.4 `REVIEW_PLAN_SCHEMA_VERSION` 32 and its mentions.
- [x] 1.5 After the review: `whitespace_only` asks `equal_by_tokens` after `same_without_blanks`; the fingerprint
  reads `content`; tests of a byte order mark removed, a line break that is a syntax error, a rename between a file of
  a grammar and one of none, the fingerprint after a line break that ends a statement; the snapshot of the concerns
  overview, whose step id is made of fingerprints.
