## MODIFIED Requirements

### Requirement: Modules: A module counts its files by role and its lines

A module MUST count its changed `files` by role: `build` (the build model, a manifest, a `moduleList`), `prose`
(Markdown, reStructuredText, AsciiDoc, text), `tests` (the paths of [Files](#files)), `code` (a language the summaries
parse), the rest `resources`; its lines `additions` and `deletions` over the range as git counts them, of a file
several commits change by its pair, of one the range adds or deletes the lines it leaves or had.

#### Scenario: The files by role
- **WHEN** the commits change files of a module
- **THEN** it counts them as `build`, `prose`, `tests`, `code` and `resources`
  [@test] crates/review-plan/src/modules_test.rs (`a_module_the_commits_add_comes_first_with_its_files_by_role_and_what_it_newly_uses`)

#### Scenario: A file the range adds
- **WHEN** the range adds or deletes a file
- **THEN** the `additions` and `deletions` of its module count the lines it leaves or had, as the diff of the range counts them
  [@test] crates/review-plan/src/modules_test.rs (`a_file_the_range_adds_counts_the_lines_its_commits_leave`)

#### Scenario: The lines of one commit and of several
- **WHEN** one commit of the range changes a file, and several commits change another
- **THEN** the lines of the first are git's counts of its commit, those of the other the blocks of its pair as the
  engine counts them
  [@test] crates/review-plan/src/modules_test.rs (`the_lines_of_a_module_are_what_git_counts_where_the_engine_aligns_otherwise`, `the_lines_of_a_file_several_commits_change_are_the_engines_count_of_its_pair`)
