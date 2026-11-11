## RENAMED Requirements

- FROM: `### Requirement: File diffs: The caches key a diff by its blob pair, policy and languages`
- TO: `### Requirement: File diffs: The caches key a diff by its blob pair, policy and grammars`

## MODIFIED Requirements

### Requirement: File diffs: The caches key a diff by its blob pair, policy and grammars

The caches MUST key a diff by its blob pair, the policy and the grammar of each side, which its scopes and its marks
are read by, so two commits with the same blob pair share one diff and the same pair at a path of another grammar is
another diff. The store MUST NOT keep a diff that the time cut short, and MUST NOT answer a diff made with the tokens
of other parsers: it keeps the diffs under a version made of the model's and of the tokens'.

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

#### Scenario: A diff the time cut short
- **WHEN** the time cut a diff short
- **THEN** the store does not keep it
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_out_of_time_is_by_line_and_not_stored`)

#### Scenario: Other parsers
- **WHEN** the store holds a diff of the pair that an app with other parsers, or other rules of the tokens, made
- **THEN** a view and a scan do not answer it: they compare the pair, and the view keeps its diff under the version of its own tokens, which drops the other one
- **AND** the same diff stored under the version of these tokens is answered to both
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_stored_with_the_tokens_of_other_parsers_is_not_read`)

### Requirement: File diffs: The diffs stream, the files in view first

The diffs MUST stream: the files first, then the content of each file as it is known, the files in view
(`priorityPaths`) first and the others in the order of the tree, then the scopes of a text that the caches did not
have. The content of a text MUST hold its marks ([Quiet Changes](../quiet-changes/spec.md)): they change the rows a
view draws, the scopes do not.

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
