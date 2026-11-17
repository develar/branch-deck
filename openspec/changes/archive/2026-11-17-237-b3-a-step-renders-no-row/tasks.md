# Tasks

- [x] `useRowPart(subject, part, select, equal)` and `useBranchSyncEntry(…, equal)` with one kept answer per
  subscriber (`readEntry`, `keptAnswer`, their test in src/stores/branch-sync.test.ts); `sameJson` of src/lib/record.ts
  with its test; every field read of a part selected by it, an object kept by `sameJson`, the node of a row, Push with
  Children of a root and the fixes of a stack problem too
- [x] `part-field-read` of `pnpm lint:arch` with its test, arguments one call deep included
- [x] `step:<name>` of the render counter, `status:<name>` counted in `RowStatusCell`; `oneBranchSyncStatuses` of
  `perf:table`; `waitForSettledSession` of `tests/e2e/helpers/sync-helpers.ts`, shared with `perf:table`
- [x] `row-renders.spec.ts`: a sync of one more commit on one branch with Patronus answering renders the steps of the
  rows for their landing needs, of the rows and their status cells only those of the branch, neither the table nor an
  island; tagged `@webkit` and run by `pnpm e2e:webkit`
- [x] docs/design/rendering-performance.md (Key Decision 3, the renders of a sync), docs/contributing/frontend.md
  (Pitfalls), docs/design/e2e-testing.md (the render counter), docs/design/rust-domain-authority.md (B3)
