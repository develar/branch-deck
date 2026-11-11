## MODIFIED Requirements

### Requirement: File diffs: The caches key a diff by its blob pair, policy and grammars

The caches MUST key a diff by its blob pair, the policy and how each side is read: by its grammar, else by whether
its indentation has a meaning. So two commits with the same blob pair share one diff, and the same pair at a path
read another way is another diff. The store MUST NOT keep a diff that the time cut short, and MUST NOT answer a diff
made with the tokens of other parsers: it keeps the diffs under a version made of the model's and of the tokens'.

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
- **WHEN** the store holds a diff of the pair that an app with other parsers, or other rules of the tokens, made
- **THEN** a view and a scan do not answer it: they compare the pair, and the view keeps its diff under the version of its own tokens, which drops the other one
- **AND** the same diff stored under the version of these tokens is answered to both
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_stored_with_the_tokens_of_other_parsers_is_not_read`)
