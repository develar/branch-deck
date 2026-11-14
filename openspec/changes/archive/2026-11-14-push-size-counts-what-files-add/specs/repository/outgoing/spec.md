## RENAMED Requirements

- FROM: `### Requirement: Push: The files of a push are the blobs the remote lacks`
- TO: `### Requirement: Push: A file of a push counts once, by what it adds to the remote's version`

## MODIFIED Requirements

### Requirement: Push: A file of a push counts once, by what it adds to the remote's version

The files MUST be the paths the commits of the push add or change, the commits no remote-tracking branch of the remote
reaches, each once, by the bytes it adds: its largest version in the push, less the remote's version of the path,
never below nothing; a path that adds nothing is no file of the push. A blob MUST count once, and not at all when the
remote has it. One `log --raw` and one `cat-file --batch-check`
([Git Operations](../../../../docs/design/git-operations.md#push-size)).

#### Scenario: The files a push sends
- **WHEN** the size of a push is measured
- **THEN** each path its commits add or change counts once, by its largest version less the remote's version: a new
  file whole, a changed file of the remote its growth, a file added and then deleted in the push its version
- **AND** a deleted file, a file moved to another path and the commits the remote has add nothing
  [@test] crates/git-ops/src/push_contents_test.rs (`a_push_sends_the_files_its_commits_add_that_the_remote_lacks`)

#### Scenario: A file changed by many commits counts once
- **GIVEN** a push whose ten commits each change one file
- **WHEN** the size of the push is measured
- **THEN** the file counts once, by its largest version less the remote's, and a refusal names it once
  [@test] crates/sync-core/src/push_size_test.rs (`a_file_changed_by_many_commits_counts_once`)

#### Scenario: The sizes in one batch
- **WHEN** the sizes of those blobs are read
- **THEN** it is one `cat-file --batch-check`
  [@test] crates/git-ops/src/push_contents_test.rs (`the_sizes_of_objects_are_one_batch_check`)
