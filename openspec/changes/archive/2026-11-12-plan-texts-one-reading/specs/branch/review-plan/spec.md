## ADDED Requirements

### Requirement: Files: A blob has the same text wherever it is read

A blob MUST have the same text from the memory of the diffs and from the `cat-file` of the summaries: up to the size
of the diffs (`MAX_BLOB_BYTES`, 8 MiB), an invalid UTF-8 sequence replaced, so that the plan of the same commits is the
same plan whatever the object cache holds.

#### Scenario: The same plan from git and from the object cache
- **GIVEN** a range with a source file not in UTF-8 and a text file larger than a parsed source file (1 MiB)
- **WHEN** it is planned once with its diffs read from git, then by a new process with them in the object cache
- **THEN** both plans are the same, the source file parsed both times
  [@test] crates/review-plan/src/plan_sources_test.rs (`a_plan_from_the_object_cache_is_the_plan_from_git`)
