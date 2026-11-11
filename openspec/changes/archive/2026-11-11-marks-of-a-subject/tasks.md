# Tasks

- [x] 1.1 What a pair leaves: the rows of its changed lines no mark took (`text_diff::moves_across::left_rows`), the
  runs of the blocks of a name it declares none of (`Renames::pending`), the places of its declarations
  (`Leftovers`); the `file_leftovers` store under the key of its diff.
- [x] 1.2 The moves between the files of a subject (`find_moves_across`) and the pass over them (`Subject::of`).
- [x] 1.3 A name another file declares, with the runs its pair kept; a run of the subject takes its lines from the
  moves between files.
- [x] 1.4 The index asked once per subject (`SymbolIndex::renames`, `accio renames`, gated by `accio versions`), its
  answer read into a proof of each rename and the runs it confirms (`Subject::marks`, `as_confirmed`).
- [x] 1.5 The table of the pass, the fake accio of the client, one run against the accio of this machine.
