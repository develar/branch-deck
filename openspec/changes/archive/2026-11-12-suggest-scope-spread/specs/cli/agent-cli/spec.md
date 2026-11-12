## MODIFIED Requirements

### Requirement: `brd suggest`: Soft merges go by a deterministic score

Soft merges MUST go by a deterministic integer score of issue key mentions, conventional scopes (`type(scope):` and
`type scope:`) weighted by their spread over the range, shared paths weighted by IDF, the disagreement of issue
projects and scopes, and log adjacency: the best pair first, a tie to the oldest, under `--max-commits` (40) and down
to the threshold 0.40.

#### Scenario: Adjacency alone
- **WHEN** only log adjacency lifts a pair over the threshold
- **THEN** the merge is `weak`
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_merge_that_only_adjacency_lifts_over_the_threshold_is_weak`)

#### Scenario: Scopes
- **WHEN** commits without keys have the same conventional scope, `type(scope):` or `type scope:`
- **THEN** they merge, and another scope halves the weight of their shared paths
  [@test] crates/commit-clustering/src/clustering_test.rs (`the_same_scope_merges_commits_without_keys_and_another_scope_halves_paths`)
  [@test] crates/sync-utils/src/issue_pattern.rs (`semantic_prefixes_with_and_without_parentheses`)

#### Scenario: A scope over much of the range
- **WHEN** a conventional scope covers a share s of the commits of the range
- **THEN** its link weighs √(1 − s): a scope of most of the range names an area several branches share, and its
  commits don't merge by it alone, even next to each other
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_scope_over_much_of_the_range_weighs_less`)

#### Scenario: Mentions of issue keys
- **WHEN** a commit mentions an issue key anywhere in its text
- **THEN** the mention links keys of one project, and hardly across projects
  [@test] crates/commit-clustering/src/clustering_test.rs (`a_mention_links_keys_of_one_project_but_hardly_across_projects`)
  [@test] crates/sync-utils/src/issue_pattern.rs (`issue_keys_anywhere_in_a_text`)

#### Scenario: Shared paths
- **WHEN** commits share paths
- **THEN** they merge by their IDF weight, and a feature of many units or a peripheral file weighs little
  [@test] crates/commit-clustering/src/clustering_test.rs (`shared_paths_merge_but_a_feature_of_many_units_or_a_peripheral_file_weighs_little`)

#### Scenario: The cap and a tie
- **WHEN** a merge would pass `--max-commits`, or two pairs score the same
- **THEN** the cap stops the merge, and the tie goes to the oldest pair
  [@test] crates/commit-clustering/src/clustering_test.rs (`the_cap_stops_soft_merges_and_a_tie_goes_to_the_oldest_pair`)
