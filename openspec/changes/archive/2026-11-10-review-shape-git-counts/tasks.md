# Tasks

- [x] 1.1 `read_commit_diffs` and `read_pair_diff` read `--numstat` with the default algorithm, parsed after the raw
  part into `FileChange::git_additions` and `git_deletions`; `commit_diff` version 5.
- [x] 1.2 `line_counts` of the modules takes git's counts, the blocks of the pair (`pair::counted`) for a file several
  commits change.
- [x] 1.3 Tests of the parse, of git's counts beside the engine's, of the pinned algorithm and of the module counts;
  the process budget of the plan unchanged.
