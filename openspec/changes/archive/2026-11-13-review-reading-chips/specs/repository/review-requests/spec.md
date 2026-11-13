## MODIFIED Requirements

### Requirement: Card: A row shows the unread dot, the title and the meta

A row, 36 px, MUST show the unread dot, the title (`font-medium`, the issue key it starts with muted and regular, where
Rust split the title, `titleLead`), the labels of its pull request as muted chips (`Badge variant="secondary"`, the
first two, then "+2"), and the meta "#101 · carol · 2 files · +12 −3" (muted line counts, as in every row); the labels
and the meta are hidden below `sm`.

#### Scenario: Someone else than the author asked
- **WHEN** someone else than the author asked for the review, as for a pull request a bot opened
- **THEN** the meta says "#830 · agent-bot · requested by dave · …": that one waits for the user
  [@test] src/lib/review-requests.test.ts (`names who asked for the review when it is not the author`)

#### Scenario: Another base than the default branch
- **WHEN** the base of the pull request is not the default branch
- **THEN** the meta ends with "· → release/253"
- **AND** a layer of a stack leaves it out: its base is the layer below it, and the header of the stack names the base of its bottom
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`, `should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)

#### Scenario: The labels of a pull request
- **WHEN** the pull request has labels on GitHub
- **THEN** its row shows the first two as muted chips after the title, then "+2" for the rest
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)

### Requirement: Review window: A stack opens on its first layer to read and folds the others

Review Stack MUST read what every layer leaves to read in one read, open on the first layer with something to read once
the layers below it are read, unless the user chose an item first, and let ↑/↓ pass a layer with nothing to read. The
overview of All Changes MUST sum the layers and fold each layer with nothing to read, muted, with its proofs and its
Approve on its head.

#### Scenario: A stack of four, two with nothing to read
- **WHEN** the window reviews a stack whose bottom and third layer change formatting alone
- **THEN** it opens on the second layer, ↓ shows the top, ↑ the second again and no further
- **AND** All Changes says "2 of 4 with nothing to read · 2 changes to read", the two folded with "Nothing to read · formatting" and Approve, which approves that layer on its head
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open a stack on its first layer to read, fold the layers with nothing to read and pass them by ↑/↓`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`a_stack_counts_its_layers_and_opens_on_the_first_one_to_read_once_those_below_are_known`)
  [@test] src/presenters/range-reading.test.ts (`sums the layers of a stack, each count of its own unit`)

### Requirement: Read ahead: Each row says what its request leaves to read

Each row of a waiting request MUST say what it leaves to read before any window opens: "Checking…" while it is read,
then its verdict as a chip before its age; a stack header the sum of its layers, each layer its own chip. No row moves
by its verdict; the rows of Reviews, the same row, show the same chips.

#### Scenario: Two requests read ahead
- **WHEN** a request of formatting alone and one that changes a body wait, the read ahead held, then released
- **THEN** both say "Checking…", then "Nothing to read · formatting" and "1 change to read in 1 file", in the same order, no window opened
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say what each request leaves to read before any window opens, checking first, its rows in place`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`the_waiting_requests_say_what_they_leave_to_read_and_a_stack_the_sum_of_its_layers`)
  [@test] src/presenters/range-reading.test.ts (`says what a row of Your Turn reads ahead: checking, then the verdict of its request or the sum of its stack`)

#### Scenario: A stack
- **WHEN** a stack of four layers waits, two of formatting alone
- **THEN** its header says "Checking…", then "2 of 4 with nothing to read · 2 changes to read", and each layer under it its own reading
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should sum a stack in its header and say what each layer under it leaves to read`)

## ADDED Requirements

### Requirement: Review window: The column of a stack says what each layer leaves to read

The scope of each layer in the column of Review Stack MUST say what it leaves to read once read, a compact chip after
its title, its hover the whole verdict; the title of a layer with nothing to read is muted, so the column shows why
↑/↓ pass it.

#### Scenario: A stack of four, two with nothing to read
- **WHEN** the window reviews a stack whose bottom and third layer change formatting alone
- **THEN** the column says "Nothing to read", "1 to read", "Nothing to read", "1 to read" of the four layers, the two with nothing to read in the success tone
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open a stack on its first layer to read, fold the layers with nothing to read and pass them by ↑/↓`)
  [@test] src/lib/range-review.test.ts (`says in the scope of each layer what it leaves to read once read, a compact chip with the whole verdict in its hover`)
  [@test] src/presenters/range-reading.test.ts (`says a verdict compactly for a narrow list`)

### Requirement: Read ahead: A row says the author's word its diff proves wrong once

The author's word the diff of a request proves wrong MUST be said once in its row, without the count its verdict
says: a label is its own chip in the warning tone, first of the labels, its hover why and how much is left to read; a
word of the title is one compact warning chip after the labels, "refactor: in the title", with the same hover.

#### Scenario: A refactoring that changes a body
- **WHEN** a request labelled refactoring changes the body of a function
- **THEN** its label chip "refactoring" is in the warning tone, its hover why, and no other chip flags it
- **AND** the label of a request of formatting alone stays muted
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should flag in its row a request labelled a refactoring whose diff changes a body`)
  [@test] src/presenters/range-reading.test.ts (`words the author's word the diff proves wrong as the author's`)

### Requirement: Card: A narrow row gives way in one order

A row too narrow for all it says MUST give way in one order: the title first, down to its first words, then the meta,
then the title below them, then the labels, each cut with an ellipsis, never without one; what it leaves to read and
its age never shrink.

#### Scenario: A long title and two labels at 1000 pt
- **WHEN** a request with a long title and two labels waits, and the window is 1000 pt wide
- **THEN** its title is cut with an ellipsis, its labels, what it leaves to read and its age show whole
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should give way in a narrow row with the title first, its labels, its reading and its age whole`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should mark the user's moves alone, keep the other rows behind Show N More, and give the title the room before the meta`)
