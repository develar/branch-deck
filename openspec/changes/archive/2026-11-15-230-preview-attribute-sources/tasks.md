# Tasks

- [x] `ConfigSnapshot::attributes_file`, `git_ops::update_branch::with_attribute_files` and
  `GitCommandExecutor::system_attributes`; the sync and `check_baseline_update` put the files on their executor
- [x] `AttributeSources` stamps a `.gitattributes` of the working tree only as a regular file; the probe reads
  `:<dir>/.gitattributes` in its `cat-file` and blocks a file under a tracked one the working tree lacks
- [x] Tests of each source, of the racy margin of the sources, of the kept probe of a sync and of a CRLF across the
  chunks of `holds_crlf`
- [x] Docs: git-operations ("Update Project", the preview and its cache)
