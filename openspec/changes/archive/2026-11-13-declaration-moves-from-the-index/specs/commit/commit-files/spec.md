## MODIFIED Requirements

### Requirement: Reading the diffs: The index is asked of the renamed names of a subject a view shows

The read of a subject's diffs MUST end with its diffs. What the index says of the names the subject renamed
([Quiet Changes](../quiet-changes/spec.md)) MUST be asked after it (`get_index_marks`), only of a subject a view
shows, one question at a time, the subject shown last first; a question whose subject no view shows any more gives
way. Each stage of the answer (the renames first, then the moves) MUST take the place of the marks of the files it
names as it comes and keep what the views opened.

#### Scenario: The read ends before the index is asked
- **WHEN** the diffs of a subject with a name to ask of the index are read, and no view shows the subject
- **THEN** the read is complete, what waits for its end runs, and the index is asked nothing
- **AND** once a view shows the subject the index is asked of it, once however many views hold it
- **AND** its answer takes the place of the marks of the files it names: the entry stays complete, its folds stay, and a view that shows it again asks nothing
  [@test] src/stores/file-diffs.test.ts (`ends the read of a subject with its diffs, and asks the index of its renamed names once a view shows it`)

#### Scenario: The stages of the answer as they come
- **WHEN** the question of a subject a view shows tells the renames, then the moves, then ends
- **THEN** the proofs of the renames are in the entry before the moves come, the marks of the moves take their place, and a view that shows the subject again asks nothing
  [@test] src/stores/file-diffs.test.ts (`takes each stage of the answer of the index as it comes, the proofs of the renames before the moves`)

#### Scenario: One question at a time
- **WHEN** two views show two subjects, the first view goes and another shows a third subject
- **THEN** the question of the first runs alone, gives way to the third, shown last, once no view shows the first, and the second is asked when the third answered
- **AND** a subject whose question answered nothing or failed is not asked again until a view shows it anew
  [@test] src/stores/file-diffs.test.ts (`asks the index of one subject at a time, the one shown last first, and lets the question of a subject no view shows give way`)

#### Scenario: A subject shown again before its first question ended
- **GIVEN** a view showed a subject, then another, then the first again, before the question of the first, which gave way, ended
- **WHEN** the questions that gave way end without an answer
- **THEN** the question of the subject shown again still runs: a subject another view shows waits for it, and its answer is taken
  [@test] src/stores/file-diffs.test.ts (`keeps the question of a subject shown again running when the question it gave way for ends after it started`)
