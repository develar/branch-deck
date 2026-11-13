# Proposal

## Why

The pure crates of the marks of a diff (`line-diff`, `code-tokens`, `diff-marks`) move to psi-syntax, and Branch Deck
links them by a revision of its public mirror, develar/psi-syntax. Their tests move with them, but the specs of Branch
Deck keep the rules those tests prove. `pnpm lint:specs` checks a test link and a target only in the files of this
repository, so a spec could not name a test of psi-syntax, and a renamed or moved test there would fail no check.

## What Changes

- A `[@test]` link and a `targets:` item may name a file of a crate of psi-syntax as
  `psi-syntax:crates/<crate>/<path>`. spec-links reads that file in the checkout cargo has fetched of the crate, at the
  revision Cargo.lock pins: `cargo metadata --offline --locked`, asked once and only when a spec or a doc names such a
  path. It checks the test names of such a link as it checks those of a local link.
- A crate that the workspace does not depend on, a crate at two revisions, a path that is no file of a crate, and
  sources that cargo has not fetched are problems of the link, with cargo's reason and `cargo fetch` as the remedy.
  spec-links never fetches and never writes Cargo.lock.
- CI fetches the dependencies (`cargo fetch --locked`) before `pnpm lint:specs`, because only a build fetches them.
- The checks of `pnpm lint:specs` get their spec, a new capability: `cli/specs-lint`.
