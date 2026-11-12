## MODIFIED Requirements

### Requirement: Modules: The entries of the new modules are read in one accio process

The entries MUST come from accio at the head in one process for every new module (`accio surface --no-build --budget
15`), at once with the usages, the tests and the descriptors; `complete` MUST hold only when accio asked every
declaration within its budget and tied every use. An answer whose `cut` is `"time"` (its `--budget` seconds ran out)
or a value Branch Deck does not know is cut (`ModuleEntries::cut`): asked again it may say more, so its plan is not
final.

#### Scenario: The entry points of the new modules of a side
- **WHEN** the commits add several modules
- **THEN** one `accio surface --no-build --budget 15` at the head reads the entries of all of them, at once with the usages, the tests and the descriptors
  [@test] crates/file-diffs/src/accio_test.rs (`the_entry_points_of_the_new_modules_of_a_side_are_read_in_one_process`)

#### Scenario: Entries the budget of accio cut short
- **WHEN** accio answers a new module with `cut: "time"`
- **THEN** its entries are cut and the plan is not final, so the app does not keep it
  [@test] crates/review-plan/src/groups_test.rs (`a_plan_whose_entries_the_budget_of_accio_cut_short_is_not_final`)

#### Scenario: Entries a bound on work cut, or nothing
- **WHEN** accio answers a new module not complete with `cut: "work"` (a bound on files read, the same on every run) or `cut: null` (a use tied by the name alone)
- **THEN** its entries are not cut and the plan is final
  [@test] crates/review-plan/src/groups_test.rs (`a_plan_whose_entries_the_budget_of_accio_cut_short_is_not_final`)

#### Scenario: Entries of an accio without the field
- **WHEN** an accio that answers no `cut` answers a new module not complete
- **THEN** its entries are cut when the answer says why in its note (the budget ran out, the index could not answer), and not cut without a note
  [@test] crates/review-plan/src/groups_test.rs (`a_plan_whose_entries_the_budget_of_accio_cut_short_is_not_final`)
