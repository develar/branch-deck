## Why

The brief read the specs with a grammar of its own: it cut every leading `../` of a test link, so each link of an
IntelliJ spec (`[@test] ../../backend/x.kt` of `plugins/air/spec/…`) named a file that does not exist
(`backend/x.kt` for `plugins/air/backend/x.kt`); it missed a link of a bullet and one in brackets, and it read no
"Summary". accio reads both formats with psi-syntax's `spec-syntax` now, a port of OpenSpec's own readers and of
`AirSpecReferencesTest`'s rules (accio's decision 2026-11-10-specs-parsed-by-spec-syntax).

## What Changes

- The brief reads a new spec and the test links of a change with `spec-syntax`: the count of the requirements as
  OpenSpec reads them, the description from the front matter, else "Purpose", else "Summary".
- A test link's path is from the root by the rule of its file's format: of an IntelliJ spec from its directory, of
  any other file from the root unless it starts `./` or `../`.

## Capabilities

### Modified Capabilities

- `branch/review-tour`: Brief: A new file is told by its description, a deleted one is deleted; Brief: An entry names
  its section, its tests and its paragraphs.
