# Proposal

## Why

`brd suggest` joined two clusters by a shared conventional scope alone (R 0.8 without issue keys, 0.5 with them), so a
scope that names an area of the code joined the branches of every session that worked there. Measured on the
labelled suggest corpora of the review-concerns lab (the commits of 2–4 real branches interleaved into one local
history): on 100 cases of this repository's lease picks, where a scope names an area (`bd`, `e2e`, `review`), only
30 % of the pairs of keyless commits with one scope were of one branch, and `--fresh` scored pair precision 31.7 %,
ARI 0.20. In the IntelliJ monorepo (73 cases of its virtual branches) and in the golden branch a keyless scope names
the branch (`docs ai:`, `setup build:`), and the same pairs were 90 % and 100 % of one branch.

## What Changes

- A shared scope weighs √(1 − s), s the share of the commits of the range that have it, IDF-style: a scope of few
  commits keeps nearly all its weight, a scope of most of the range none. No setting, no switch.
- Of several shared scopes the heaviest links, on a tie the first by name.
- Measured: the lease picks, pair P 31.7 → 76.0 %, R 82.0 → 82.0 %, ARI 0.20 → 0.78, exact 4 → 62 %; the monorepo,
  pair P 88.7 → 90.0 %, R 83.4 → 83.4 %, ARI 0.78 → 0.79; the golden branch, precision 1.000, recall 0.945, as before.
  A scope's spread over issue keys did not change any case; the input has no commit dates.
