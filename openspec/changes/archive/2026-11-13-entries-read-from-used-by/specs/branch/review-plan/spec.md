## ADDED Requirements

### Requirement: Modules: What enters a new module is read from the files that resolve it

Whether a declaration of a new module is entered MUST be read from its registrations and from accio's `used_by`, each
file that resolves a use of it with whether it is test code, never from its `uses`: `--limit` cuts them, and they list
the uses by the name and those of tests before a later resolved one.

#### Scenario: Uses a limit cut
- **WHEN** the uses accio lists of a declaration hold only one by the name alone, and its `used_by` a file of other code that is no test
- **THEN** it is an entry point, used by that file
  [@test] crates/review-plan/src/groups_test.rs (`a_new_module_names_what_enters_it_and_its_group_starts_with_their_files`)

#### Scenario: Entered from test code alone
- **WHEN** every file of the `used_by` of a declaration is test code
- **THEN** it enters nothing
  [@test] crates/review-plan/src/groups_test.rs (`a_new_module_names_what_enters_it_and_its_group_starts_with_their_files`)

## MODIFIED Requirements

### Requirement: Modules: A module the commits add knows what enters it

A module the commits add MUST know what enters it (`entries`, `"schema": 34`): its declarations something outside it
uses, a class a plugin descriptor registers first (`registered`: the extension point and what it is registered for,
the interface of a service), then those the code of other modules uses (`usedBy`, each file at its first use, a
descriptor's own file aside); a use by the name alone or from test code MUST enter nothing.

#### Scenario: A new module used from outside
- **GIVEN** the commits add a module whose declarations a plugin descriptor registers and the code of other modules uses
- **WHEN** the plan is read
- **THEN** its `entries` list first the classes the descriptor registers (`registered`), then the declarations other modules use (`usedBy`, each file at its first use)
  [@test] crates/review-plan/src/groups_test.rs (`a_new_module_names_what_enters_it_and_its_group_starts_with_their_files`)

#### Scenario: A use by the name alone or from test code
- **WHEN** a declaration of the new module is used only by its name, or only from test code
- **THEN** it enters nothing
  [@test] crates/review-plan/src/groups_test.rs (`a_new_module_names_what_enters_it_and_its_group_starts_with_their_files`)

### Requirement: Modules: Only a module the commits add and accio answers has entries

A module the commits change or delete MUST have no `entries`, and so MUST each module without an answer (no accio, an
accio without `surface` or whose `accio versions` names another revision of its answer than this reader's,
`answers.surface` 2, JetBrains/accio#748), which is no reason of the `index_note`.

#### Scenario: A changed module, or no answer of accio
- **WHEN** a module is changed or deleted, or accio is missing, has no `surface` or gives another revision of its answer
- **THEN** the module has no `entries`, and the `index_note` gives no reason for it
  [@test] crates/file-diffs/src/accio_test.rs (`an_accio_that_answers_no_surface_as_it_is_read_is_not_asked`)
