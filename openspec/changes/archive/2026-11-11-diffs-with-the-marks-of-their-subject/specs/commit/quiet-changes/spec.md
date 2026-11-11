## MODIFIED Requirements

### Requirement: Renames: A name that is not local is a label, and its declaration is read

A name no body binds, declared in the file, MUST be a rename that is not `local` when every name of its spelling
changed: its uses are runs that are labels, its declaration is in no run. A name that changed at every place of the
file and is declared in none of them MUST mark nothing, and the pair MUST hand it out for the pass over the files
of its subject, with the runs its blocks have were it a rename.

#### Scenario: A member, a function, a class
- **WHEN** a property, a field, a function or a class is renamed with its uses in the file, in Kotlin, Java, TypeScript and Rust
- **THEN** the rename is not `local`, `declared` names its declaration, each use is a run that is a label, and the block of the declaration has its start
- **AND** a parameter of a TypeScript constructor that declares a property (`public total`) is such a rename, with its uses in the constructor and through `this`
- **AND** a property and a local of its spelling renamed alike are two renames, the local one proven
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_name_that_is_not_local_is_a_label_and_its_declaration_is_read`)

#### Scenario: A name the file does not declare
- **WHEN** the two calls of a function declared in another file get another name
- **THEN** the diff has no rename and no run, and the pair names the two names with their places, without a declaration
- **AND** it keeps the run each call would be were the name a rename, which the pass over its subject takes once another file declares it
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_name_the_file_does_not_declare_is_left_to_the_other_files`)
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_pair_keeps_the_runs_a_name_it_declares_none_of_would_have`)
