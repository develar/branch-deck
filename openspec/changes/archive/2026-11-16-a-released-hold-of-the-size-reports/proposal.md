# A released hold of the size reports lets them through

## Why

The review of e4acc6530 (bd-flakes-review): `holdSizeReports` delivered what it held at its release and went on
holding every later report of the element to the end of the test, where its docs say "until `release`" and the other
holds (`holdRewrites`, `holdIntent`) let go once released: a test that resized the element after the release waited for
a report that never came.

## What Changes

- The hold keeps a flag in the page: once released, its wrapper passes every report through.
- Capability `commit/commit-review`: the scenario of the height read as the view mounts says a later resize reaches
  the view.
