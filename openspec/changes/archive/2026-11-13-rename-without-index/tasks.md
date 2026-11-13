# Tasks

- [x] 1.1 Ask whether the repository has an index (`SymbolIndex::has_index`, `accio stats --no-build`), kept per repository.
- [x] 1.2 The read ahead waits for renames only where it has one; the window's reading without one is final.
- [x] 1.3 Keep such a reading in memory and on disk, dropped once the repository has an index.
- [x] 1.4 An E2E test of a private member renamed in a repository without an index, and the test of the kept readings.
