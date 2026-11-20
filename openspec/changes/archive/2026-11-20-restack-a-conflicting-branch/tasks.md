## 1. The rule

- [x] 1.1 `Reads` and `failed_sync` of `crates/domain-policy/src/actions.rs`: the rewrites of the originals run on a
  merge conflict, Unapply, the push and the review wait for a build; the table tests

## 2. The fix

- [x] 2.1 `Change::merged` of `crates/trial-check/src/conflict_fix.rs` adds no step whose effect the change has; the
  test of a chain of four branches

## 3. The app

- [x] 3.1 The E2E of Stack On… and of the drop on the chip of a conflicting branch
