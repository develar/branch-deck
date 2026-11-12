## MODIFIED Requirements

### Requirement: File diffs: The diffs stream, the files in view first

The diffs MUST stream: the files, then the content of each file as it is known with the marks of its pair, the files
in view (`priorityPaths`) first, the scopes of a text the caches lacked after it. Then the marks of each file the
others of its subject say something of ([Quiet Changes](../quiet-changes/spec.md)), in the content itself when the
caches had every pair. The stream MUST ask no index of the repository and end with the last of its files: where one
is there to be asked of the names the subject renamed, their proof says so (`asked`). A late event MUST take no row
of a file, and leave its quiet runs and its stops as its content had them.

#### Scenario: The order of the stream
- **WHEN** `get_file_diffs` streams the diffs of a commit with files in view
- **THEN** the files come first, then the content of each file as it is known, the files in view (`priorityPaths`) first
- **AND** the scopes of a text the caches did not have come after its blocks
  [@test] crates/file-diffs/src/service_test.rs (`the_stream_lists_the_files_then_the_files_in_view_first_and_their_scopes_after_their_blocks`)
  [@test] crates/test-server/src/commit_files_test.rs (`commit_files_routes_read_the_fixture_repository`)

#### Scenario: The files after the ones in view
- **WHEN** the files in view are done
- **THEN** the others follow in the order of the tree
  [@test] crates/file-diffs/src/service_test.rs (`the_work_takes_the_files_in_view_then_the_tree_order_with_directories_first_and_numbers_as_numbers`)

#### Scenario: The marks of a text
- **WHEN** a text the caches did not have is compared
- **THEN** its content goes out with its marks, and the event of its scopes brings none
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_a_text_come_with_its_content`)

#### Scenario: The marks of the subject after the contents
- **WHEN** the diffs of a commit whose files were not all in the caches are streamed
- **THEN** each content has the marks of its pair, and after every content and every scope one `marks` event brings the marks of each file the subject says something of, with the same starts
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_the_subject_follow_the_contents_or_come_with_them_from_the_caches`)
  [@test] crates/test-server/src/commit_files_test.rs (`the_diffs_of_a_subject_say_what_its_files_say_of_each_other_and_the_index_is_asked_after_them`)

#### Scenario: The marks of the subject with the contents
- **WHEN** the caches have every pair of the subject
- **THEN** the content of each file comes with the marks of the subject, and no `marks` event follows
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_the_subject_follow_the_contents_or_come_with_them_from_the_caches`)

#### Scenario: The stream asks no index
- **WHEN** a subject renamed a name that is not local and an index of the repository is there
- **THEN** the marks of its files say that the index is to be asked (in a `marks` event, or in the content from the caches), and the stream ends with them: it asked nothing
- **AND** without an index, and for the working tree, the names are unavailable in the same events
  [@test] crates/file-diffs/src/service_test.rs (`the_index_is_asked_after_the_diffs_of_a_subject_in_one_call_and_its_answer_is_kept`)
  [@test] crates/test-server/src/commit_files_test.rs (`the_diffs_of_a_subject_say_what_its_files_say_of_each_other_and_the_index_is_asked_after_them`)

### Requirement: File diffs: The caches key a diff by its blob pair, policy and grammars

The caches MUST key a diff by its blob pair, the policy and how each side is read: by its grammar, else by whether
its indentation has a meaning; what the pair left to the other files of a subject is kept beside it under the same
key and version, and a view knows a text with it or compares it again. The store MUST NOT keep a diff that the time
cut short, and MUST NOT answer one made with other parsers or other rules of the names. The layout of what a store
keeps MUST be pinned with the version of the store: a value written under a layout is never read under another.

#### Scenario: Two commits with the same blob pair
- **WHEN** two commits change a file from the same blob to the same blob
- **THEN** they share one diff
  [@test] crates/file-diffs/src/service_test.rs (`two_commits_with_the_same_blob_pair_share_its_diff`)

#### Scenario: Another policy
- **WHEN** the same blob pair is read under another policy
- **THEN** that policy keys its own diff
  [@test] crates/file-diffs/src/service_test.rs (`the_policy_keys_its_own_diff`)
  [@test] crates/object-cache/src/cache_test.rs (`file_diffs_are_read_by_their_blob_pair_policy_grammars_and_version`)

#### Scenario: Another grammar
- **WHEN** the same blob pair is a `.ts` file, a `.tsx` file and a text file of one commit
- **THEN** each has its own diff, from the comparison, from memory and from the store of another app: the spaces of a type assertion are formatting in the `.ts` file alone
- **AND** a scan of the pair at each path finds the diff the view made of it there, without a process
  [@test] crates/file-diffs/src/service_test.rs (`the_same_pair_read_by_two_grammars_is_two_diffs`)
  [@test] crates/file-diffs/src/blocks_test.rs (`a_pair_the_view_showed_costs_no_process`)
  [@test] crates/object-cache/src/cache_test.rs (`file_diffs_are_read_by_their_blob_pair_policy_grammars_and_version`)

#### Scenario: Another meaning of the indentation
- **WHEN** the same blob pair is a Python file, a text file and a Starlark file of one commit, a rule moved in it with one line at another indent
- **THEN** each has its own diff, from the comparison, from memory and from the store of another app: the line is an edited line of the move in the Python file and the Starlark file alone
- **AND** a scan of the pair at each path finds the diff the view made of it there, without a process
  [@test] crates/file-diffs/src/service_test.rs (`the_moves_of_a_text_come_with_its_content_and_are_those_of_its_indentation`)

#### Scenario: A diff the time cut short
- **WHEN** the time cut a diff short
- **THEN** the store does not keep it
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_out_of_time_is_by_line_and_not_stored`)

#### Scenario: Other parsers
- **WHEN** the store holds a diff of the pair that an app with other parsers, other rules of the tokens, or other rules of the names (a scope that was fixed since) made
- **THEN** a view and a scan do not answer it: they compare the pair, and the view keeps its diff under the version of its own tokens and rules, which drops the other one
- **AND** the same diff stored under the version of this app is answered to both
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_stored_with_the_tokens_of_other_parsers_is_not_read`)

#### Scenario: A diff without what it left
- **WHEN** the store holds the diff of a pair and not what the pair left
- **THEN** a scan answers the stored diff and writes nothing beside it, and a view compares the pair again and writes both; the next view compares nothing
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_stored_without_what_it_left_is_compared_again_for_a_view`)
  [@test] crates/object-cache/src/cache_test.rs (`what_a_diff_left_is_read_by_the_key_of_its_diff_from_a_store_of_its_own`)

#### Scenario: The layout of a stored value
- **WHEN** a field of a stored diff, of what a pair left, or of the diff a stored merge of one side holds is added, taken away, moved or renamed
- **THEN** the sample of the value that is pinned with the version of its store (`MODEL_VERSION`, `MERGE_MODEL_VERSION`) reads otherwise, and the version is raised with the new sample
  [@test] crates/file-diffs/src/stored_layout_test.rs (`the_layout_of_a_stored_value_is_pinned_with_the_version_of_its_store`)

### Requirement: Reading the diffs: The diffs stream, the files in view first

The diffs stream ([File diffs](#file-diffs)): a file MUST show once its content arrives and "Loading…" before, the
files in view MUST be asked for first (`priorityPaths`, the file a review opens at), and the late scopes and the
late marks of a text MUST take their place in its content without losing the lines it expanded or the notes it
opened.

#### Scenario: The files in view are asked for first
- **WHEN** a view reads the diffs of a subject
- **THEN** it asks for the files in view first (`priorityPaths`, the file a review opens at)
  [@test] src/stores/file-diffs.test.ts (`reads a subject once per policy, the files in view first`)

#### Scenario: A file shows as its content arrives
- **WHEN** the content of a file arrives, and later the scopes of its text
- **THEN** the file shows once its content arrives, and "Loading…" before
- **AND** the late scopes name its folds without losing the lines it expanded
  [@test] src/stores/file-diffs.test.ts (`shows each file as its content arrives, and the late scopes of a text keep the lines it expanded`)

#### Scenario: The late marks of a file
- **WHEN** the marks of a file arrive after its content, with what the other files of its subject say
- **THEN** the file has the marks of the event in place of those of its content, with the folds and the notes a view opened of it
- **AND** the marks of a file without a text, or of one whose content never came, change nothing
- **AND** a subject none of whose names waits for the index asks nothing of it
  [@test] src/stores/file-diffs.test.ts (`takes the marks of a late marks event in place of those of the content, and keeps what a view opened of the file`)

## ADDED Requirements

### Requirement: Reading the diffs: The index is asked of the renamed names of a subject a view shows

The read of a subject's diffs MUST end with its diffs. What the index says of the names the subject renamed
([Quiet Changes](../quiet-changes/spec.md)) MUST be asked after it (`get_rename_proofs`), only of a subject a view
shows, one question at a time, the subject shown last first; a question whose subject no view shows any more gives
way. The answer MUST take the place of the marks of the files it names and keep what the views opened.

#### Scenario: The read ends before the index is asked
- **WHEN** the diffs of a subject with a name to ask of the index are read, and no view shows the subject
- **THEN** the read is complete, what waits for its end runs, and the index is asked nothing
- **AND** once a view shows the subject the index is asked of it, once however many views hold it
- **AND** its answer takes the place of the marks of the files it names: the entry stays complete, its folds stay, and a view that shows it again asks nothing
  [@test] src/stores/file-diffs.test.ts (`ends the read of a subject with its diffs, and asks the index of its renamed names once a view shows it`)

#### Scenario: One question at a time
- **WHEN** two views show two subjects, the first view goes and another shows a third subject
- **THEN** the question of the first runs alone, gives way to the third, shown last, once no view shows the first, and the second is asked when the third answered
- **AND** a subject whose question answered nothing or failed is not asked again until a view shows it anew
  [@test] src/stores/file-diffs.test.ts (`asks the index of one subject at a time, the one shown last first, and lets the question of a subject no view shows give way`)
