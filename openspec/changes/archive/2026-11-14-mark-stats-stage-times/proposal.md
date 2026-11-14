# Proposal

## Why

`brd commits --mark-stats --index` printed one time for the moves and the uses that follow a move together
(`moves_ms`), so a measurement could not say what `accio moves` costs a view: the uses that follow a move are one more
`accio renames`, asked only of a subject with a file that may follow one. The question whether the moves should be
asked beside the renames, not after them, needs the time of the moves alone.

## What Changes

- `FileDiffs::measure` times each stage of the answer of the index: the renames (`MeasuredSubject::index`), the moves
  (`moves`, `None` where the subject asks none: it only adds lines, or only deletes them) and the uses that follow
  a move (`moved_uses`, `None` where the subject asks none). Each stage of `FileDiffs::index_marks` says whether it
  asked the index (`IndexMarks::asked`, never sent).
- `brd commits --mark-stats` prints `moved_uses_ms` beside `index_ms` and `moves_ms`, each of the subjects that ask
  it, and the count of those subjects (`count`).
- Capability `commit/quiet-changes`: the two requirements of the measurement say so, with a scenario of the stages.
- `docs/design/diff-view.md`: the times of the stages measured on the quiet Mac Studio, before and after the moves of
  accio compared two trees where their segments differ (accio 565a37275), in place of the numbers measured under the
  load of a laptop.
