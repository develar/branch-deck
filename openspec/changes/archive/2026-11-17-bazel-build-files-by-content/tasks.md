# Tasks

- [x] `BUILD` and `BUILD.bazel` leave the names of the build model (`files::is_build_model`)
- [x] A change inside the regions a tool writes is `generated` (`GeneratedBy::Region`, `files::region_of`), and follows
  the `*.iml` of its section (`concerns::names_stem`)
- [x] `load(` starts an import statement of a Starlark file (`files::is_import`)
- [x] A `BUILD` file is a manifest of targets (`dependencies::Format::Bazel`, `DependencyKind::Target`)
- [x] The header of a folded region says its name (`formatGenerated`)
- [x] psi-syntax: the regions (`diff_marks::regions`), the quiet run of a region (`RunKind::Generated`), the reader of a
  build file
