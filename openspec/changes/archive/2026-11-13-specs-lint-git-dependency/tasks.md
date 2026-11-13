# Tasks

- [x] 1.1 `spec_links::GitSources` resolves `psi-syntax:crates/<crate>/<path>` by the packages of
  `cargo metadata --offline --locked` (`spec_links::cargo_metadata`), asked once at the first such path; the test links
  and the targets read their files through it.
- [x] 1.2 The tests: a fake checkout of two revisions, the sources not fetched, the cargo metadata of this workspace and
  of a revision cargo has not fetched.
- [x] 1.3 CI fetches before `pnpm lint:specs`; the spec `cli/specs-lint`, docs/contributing/specs.md and checks.md.
