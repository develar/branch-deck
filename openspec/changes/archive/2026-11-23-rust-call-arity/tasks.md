# Tasks

- [x] psi-syntax: the signature of a Rust function without `self`, the `Shape::Call` of a Rust call with `method`,
      `RULES_VERSION` 30, the goldens and the tests of `rust/summary_test.rs`
- [x] `accepts_call` judges a Rust call by the count of its arguments (`rust_arguments`)
- [x] A new Rust arity is a step (`new_step`, `reach_changes`, `follow_entries`)
- [x] `method_on_value` and `Evidence::MethodArity`; the steps of a method call on a value are only new forms
- [x] The review keeps the Rust header as the form (`form_signature`)
- [x] Tests: the call of the incident, a call both forms accept, a path call, `brd check` of the incident
- [x] Decision 31 of docs/design/symbol-dependencies.md, the doc of `MissingDeclaration::evidence`
