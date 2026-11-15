# Tasks

- [x] The checkout of a new slot with the pointers of Git LFS under the clones lock, `git lfs pull` after its release,
  the index of a slot whose pull failed deleted; the tests with a fake smudge filter and a fake `git lfs pull` that log
  the locks of the pool held as they ran, with mutations (the content under the lock, the pull before the release, no
  delete of the index), the parse of the config, the process budget
- [x] Git Operations, "Lease"
