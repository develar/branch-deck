## 1. The stat data of a clone

- [x] 1.1 `git_ops::cloned_index::patch_cloned_index`: index versions 2, 3 and 4, `TREE` and `REUC` kept, `UNTR`, `FSMN`, `EOIE` and `IEOT` dropped, `index.lock` created exclusively
- [x] 1.2 `refresh_cloned_index` with the patch and the default check; the mtime of the index of the template read before its copy
- [x] 1.3 Tests: the patch for versions 2, 3 and 4, a racy and a changed entry, the extensions; a clone keeps the birth time of an unchanged file
