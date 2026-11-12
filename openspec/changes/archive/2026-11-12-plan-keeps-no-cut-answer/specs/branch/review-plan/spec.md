## MODIFIED Requirements

### Requirement: In the App: A plan whose every query answered is kept in memory

A plan whose every query answered (`index_note` null) and none cut short (`ReviewPlan::is_final`) MUST be kept in the
memory of the app by what it is of (the repository, the prefix, the branch, `base`, `head`, the reviewed pair and
`sinceReviewed`), at most 64 MiB of its JSON and 10 minutes unread.

#### Scenario: The same plan asked again
- **GIVEN** a kept plan, read less than 10 minutes ago
- **WHEN** it is asked again
- **THEN** it comes whole in one event, with no git past the resolve of its subject and no query of the index
  [@test] crates/test-server/src/review_plan_test.rs (`the_plan_of_a_review_since_mark_reviewed_reads_the_new_commit_with_its_usages`)

### Requirement: Modules: The entries of the new modules are read in one accio process

The entries MUST come from accio at the head in one process for every new module (`accio surface --no-build --budget
15`), at once with the usages, the tests and the descriptors; `complete` MUST hold only when accio asked every
declaration within its budget and tied every use. An answer not complete that says why (the budget ran out, the index
could not answer) is cut (`ModuleEntries::cut`): asked again it may say more, so its plan is not final.

#### Scenario: The entry points of the new modules of a side
- **WHEN** the commits add several modules
- **THEN** one `accio surface --no-build --budget 15` at the head reads the entries of all of them, at once with the usages, the tests and the descriptors
  [@test] crates/file-diffs/src/accio_test.rs (`the_entry_points_of_the_new_modules_of_a_side_are_read_in_one_process`)

#### Scenario: Entries the budget of accio cut short
- **WHEN** accio answers a new module not complete, with the note that its budget ran out
- **THEN** its entries are cut and the plan is not final, so the app does not keep it; an answer not complete only for a use left unresolved is not cut
  [@test] crates/review-plan/src/groups_test.rs (`a_plan_whose_entries_the_budget_of_accio_cut_short_is_not_final`)
