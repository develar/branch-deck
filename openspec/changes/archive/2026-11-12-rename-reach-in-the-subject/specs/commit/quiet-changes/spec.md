## ADDED Requirements

### Requirement: Subject: The file that declares a renamed name says how far the rename reaches

For each name a file declares that is not local, the pass MUST say how many places of the files of the subject hold
the two names, its own and those of every file that names the rename (`subject.used`), and in how many files
(`subject.declared`: `sites`, `files`). A file that uses the name and names no rename MUST NOT be counted.

#### Scenario: The store and its callers
- **WHEN** a store renames its function and a report calls it at two places; the store is the one file of its subject; and two files more call it, one of which also adds a call
- **THEN** the store says three places in two files; alone, its one place in one file; with the callers four places in three files
- **AND** the file that added a call names no rename, and its places are not counted
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_declaration_says_how_many_places_of_the_subject_hold_its_name_and_in_how_many_files`)

#### Scenario: The fields
- **WHEN** the marks of a file with what its subject says of a name it declares are written as JSON
- **THEN** the check of the name has `sites` and `files` beside its `proof`
  [@test] crates/text-diff/src/model_test.rs (`the_marks_are_camel_case_json_with_every_field`)

### Requirement: Views: The note at the declaration of a name counts the places of the subject

Where a file declares a renamed name that other files of the subject hold too, its note MUST count the places of
all of them and their files ("Renamed openStore → connectStore · 3 sites in 2 files"): what a proof beside the count
speaks of. A name no other file holds MUST count the places of the file, as any rename.

#### Scenario: The words
- **WHEN** a note words a name the file declares at one place, which the subject says three places of two files hold; the same name with no other file; and that rename as a counted one of a note of three
- **THEN** it says "3 sites in 2 files", as one sentence "Renamed openStore to connectStore, 3 sites in 2 files, all uses in this change"; with no other file the places of the file, "4 sites"
- **AND** the counted rename says the same in its line of the hover and to a screen reader
  [@test] src/presenters/diff-marks.test.ts (`counts the places of a name the file declares in every file of the subject that holds it, and those of the file alone where no other does`)

#### Scenario: What the note carries
- **WHEN** the notes of a file are made that declares a function the subject says three places of two files hold, beside a local
- **THEN** the note of the function carries those places and files with what the index proved; the note of the local nothing
  [@test] src/lib/diff/block-parts.test.ts (`says of a name the file declares what the index proved of it and how far it reaches in the subject, by its number among the renames of the file, and nothing of a local`)

#### Scenario: The declaration of the store, in a browser
- **WHEN** the preview shows Store.kt of the `commit_marks` template, which declares the function Report.kt calls twice, and the review of a commit of eight files shows Accounts.kt, which declares a function Names.kt calls once
- **THEN** the notes say "Renamed openStore → connectStore · 3 sites in 2 files" and "Renamed openAccounts → loadAccounts · 2 sites in 2 files"
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a name that is not local is named at its declaration, which is read, and once in a file that only uses it, each use in the mark of a renamed name`)
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note names the file that declares a name in plain words while the review does not show that file: no button, nothing to go to; shown again, it is a button, in a note of two renames beside the name it speaks of`)
