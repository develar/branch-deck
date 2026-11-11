## MODIFIED Requirements

### Requirement: The index is never built or waited for

Nothing MUST run when accio is missing, and without a usable index of the repository accio MUST answer at once
(`--no-build`): never an index build, a wait for one or a refresh.

#### Scenario: Accio is missing
- **WHEN** accio is not installed
- **THEN** nothing of accio runs

#### Scenario: Accio has no usable index of the repository
- **WHEN** accio cannot use an index of the repository
- **THEN** it answers at once (`--no-build`) that it has no index, with its reason
- **AND** no index is built, waited for or refreshed
  [@test] crates/file-diffs/src/accio_test.rs (`an_index_accio_cannot_use_answers_no_index_with_its_reason`)

#### Scenario: The renames of a subject
- **WHEN** the diffs of a subject ask accio of the names it renamed and accio cannot use an index of the repository
- **THEN** the query runs with `--no-build` and answers for each pair that it has no index, which the diffs read as no answer, never as "no use"
  [@test] crates/file-diffs/src/accio_test.rs (`the_renames_of_a_subject_are_asked_in_one_process`)
  [@test] crates/file-diffs/src/accio_test.rs (`renames_without_an_index_answer_no_index_and_a_canceled_query_is_killed`)
