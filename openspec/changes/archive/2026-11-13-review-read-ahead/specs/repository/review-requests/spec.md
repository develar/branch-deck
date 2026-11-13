## ADDED Requirements

### Requirement: Read ahead: A listing reads ahead the requests without their last reading

Each listing GitHub answered MUST read ahead the waiting requests the session has no last reading of at the head and
the base the listing names: never one being read; again one that could not be read and one whose renames wait for the
index. A repository that reads no review plans (`branchdeck.reviewExperimental`) reads none, and its rows show nothing.

#### Scenario: What a listing asks
- **WHEN** a listing names three requests, one being read, one read, one whose renames wait; then a new head of one
- **THEN** only the one that waits is asked, a failure is asked again, and the new head is asked and shows nothing of the old one
  [@test] crates/repository-model/src/review_readings_test.rs (`a_listing_asks_the_waiting_requests_without_their_last_reading`)
  [@test] crates/repository-model/src/session_test.rs (`a_listing_reads_ahead_its_waiting_requests_and_the_prunes_keep_their_refs`)

### Requirement: Read ahead: One read for every waiting request

The read ahead MUST be one read of every request a listing asks, a phase a process: one `cat-file` and one `rev-parse` of
the merge bases of those whose commits are here; one fetch of every missing head (two for a mirror) into
`refs/branch-deck/review/<n>/ahead/`, apart from the refs of a window; one `diff-tree --stdin` and one `cat-file --batch`
of their diffs. It asks no index, and one read of a repository runs at a time.

#### Scenario: Three requests the clone lacks
- **WHEN** three waiting requests are read ahead whose heads the clone lacks
- **THEN** one fetch brings the three heads, one `rev-parse` gives their merge bases and one `diff-tree` their diffs; warm, one `cat-file` and one `rev-parse`
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_read_ahead_of_n_waiting_requests_is_one_fetch_and_one_diff_tree`)
  [@test] crates/sync-core/src/process_budget_test.rs (`the_read_ahead_of_the_waiting_requests_is_one_fetch_and_one_rev_parse`)
  [@test] crates/sync-core/src/review_fetch_test.rs (`the_heads_of_the_waiting_requests_are_one_fetch_and_their_merge_bases_one_rev_parse`)

#### Scenario: A mirror
- **WHEN** origin is a mirror of GitHub, and a head has no common history with its base
- **THEN** the heads come from the branches of the mirror, and that request is not read
  [@test] crates/sync-core/src/review_fetch_test.rs (`the_waiting_requests_of_a_mirror_come_from_its_branches`)

### Requirement: Read ahead: Each row says what its request leaves to read

Each row of a waiting request MUST say what it leaves to read before any window opens: "Checking…" while it is read,
then its verdict as a chip before its age; the author's word its diff proves wrong as a warning chip after its labels;
a stack header the sum of its layers, each layer its own chip. No row moves by its verdict; the rows of Reviews, the
same row, show the same chips.

#### Scenario: Two requests read ahead
- **WHEN** a request of formatting alone and one that changes a body wait, the read ahead held, then released
- **THEN** both say "Checking…", then "Nothing to read · formatting" and "1 change to read in 1 file", in the same order, no window opened
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say what each request leaves to read before any window opens, checking first, its rows in place`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`the_waiting_requests_say_what_they_leave_to_read_and_a_stack_the_sum_of_its_layers`)
  [@test] src/presenters/range-reading.test.ts (`says what a row of Your Turn reads ahead: checking, then the verdict of its request or the sum of its stack`)

#### Scenario: A refactoring that changes a body
- **WHEN** a request labelled refactoring changes the body of a function
- **THEN** its row shows "Labelled refactoring · 1 change to read" after its label, its hover why
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should flag in its row a request labelled a refactoring whose diff changes a body`)

#### Scenario: A stack
- **WHEN** a stack of four layers waits, two of formatting alone
- **THEN** its header says "Checking…", then "2 with nothing to read · 2 to read · 2 changes", and each layer under it its own reading
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should sum a stack in its header and say what each layer under it leaves to read`)

### Requirement: Read ahead: A final reading shows at once after a restart

A reading that is final MUST show again after a restart of the app without a read of its diff: memory and the store of
the readings have it ([Range Reading](../../review/range-reading/spec.md#requirement-memory-a-final-reading-is-kept-on-disk)),
and the read ahead tells it with the first answer of the new session, never "Checking…" first.

#### Scenario: A restart
- **WHEN** a request was read, and the app starts again with the read ahead held
- **THEN** its row shows its reading, never "Checking…"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show a final reading after a restart without reading it again`)

### Requirement: Read ahead: The review refs of a waiting request stay

The session MUST prune the review refs of every pull request but those the review windows show and those that wait,
each time that set changes: a change of the windows and a listing alike.

#### Scenario: A listing and a window
- **WHEN** a listing names #7 and #8, then a window shows #9
- **THEN** the first prune keeps #7 and #8, the next #7, #8 and #9
  [@test] crates/repository-model/src/session_test.rs (`a_listing_reads_ahead_its_waiting_requests_and_the_prunes_keep_their_refs`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should keep the review when the request is withdrawn meanwhile, and its refs`)

## MODIFIED Requirements

### Requirement: Review window: The close of the window prunes its refs and its stores

The close of the window MUST stop its fetch, and the session MUST prune the refs of the request it showed (the demand
no longer names it) unless the request still waits; the stores of the files and the diffs of a range are the window's
and go with it.

#### Scenario: The stores of a closed review
- **WHEN** the window closes
- **THEN** the files of its range, loaded by its id `base..head`, and its diffs are forgotten
  [@test] src/stores/commit-files.test.ts (`loads the range of a review by its id, base..head, until its review drops it`)
  [@test] src/stores/file-diffs.test.ts (`keeps the last subjects only, and forgets the subjects of a closed review under every policy`)
