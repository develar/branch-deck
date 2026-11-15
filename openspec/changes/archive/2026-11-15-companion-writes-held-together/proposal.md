# A write of a companion is one write of both configs, and a half link shows and heals

## Why

An add of a companion ran in three steps, each in the writer of one config: the check of the primary, the companion's
`branchdeck.primary`, the key of the primary. An add and a removal of one pair at once interleaved across the two
writers: the add found the companion named and left it, the removal unset it, the add wrote the key of the primary. The
primary listed a companion that named no primary, a half link visible only through a later refusal. Across processes
(the app and `brd`) nothing ordered the read of a config and the write after it at all.

## What Changes

- A write of a companion (`companion_write::write_companion`) asks which companion it touches, takes the companion
  locks of both repositories (`branch-deck/companions.lock` in each common git directory, in the order of the
  directories), asks again under them (another companion now: it writes nothing and asks once more), then reads and
  writes both configs under them. The app also holds the writers of both configs for the whole write
  (`ConfigWriters::write_together`, taken in the order of their files: no two such writes wait for each other).
- A half link is the problem of the side that has it: a companion that names another primary, or none, on its row in
  the primary (`CompanionSetting::link_problem`), a primary that is gone or does not list the companion in the companion
  (`RepositoryConfig::primary_problem`); Settings → Landing and `brd companion list` show both. The next write of the
  pair repairs it: an add links both sides again and takes over a companion whose primary is gone or does not list it;
  a removal unsets the companion's side only while it names this primary.
- A name of another companion, and a companion listed under another name, are refused; `brd companion` answers
  `locked` when another process holds a companion lock past the wait.
- Capability `branch/declared-needs`.
