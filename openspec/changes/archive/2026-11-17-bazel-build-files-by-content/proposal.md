# Proposal

## Why

A review folded every `BUILD` and `BUILD.bazel` as a "Build file" by its name alone, so it said "nothing to read" of the
Starlark a person writes. In the IntelliJ monorepo 1227 of 8489 `BUILD.bazel` are written by hand, and the others hold
targets of their own beside the sections a generator writes from the `*.iml`. Of 24421 changes of `BUILD.bazel` in 3000
commits, 87 % changed only such sections, 9 % only the code a person writes, 4 % both; of 2209 in the last 800 commits
that change one, 69 %, 20 % and 11 %. The code owner of these files is often a reviewer group the code-owner check of a
Safe Push asks for, which the review told there was nothing to read.

## What Changes

- A `BUILD` file is mechanical by its content, never by its name.
- A change whose every line lies in a region a tool writes (the lines between the markers of a section, as
  `### auto-generated section \`build a.b\` start` … `end`, `diff_marks::regions`) is `generated`, its region named in
  the header of its diff, and it follows the concern of the `*.iml` the section is generated from; the lines of such a
  region are a quiet run of its diff (`RunKind::Generated`), so a file that changes both shows only what a person wrote.
  A `BUILD` file the commits add or delete with only such regions and `load` lines is `generated` too.
- A `load(` line of a Starlark file starts an import statement: a file whose `load` lines alone changed is `importsOnly`.
- A `BUILD` file is a manifest: the labels each target outside the regions writes by attribute are its entries
  (`target`), read by psi-syntax's reader of a build file, which accio reads too; it is `buildModel` when only those
  lists changed, as a `Cargo.toml`. A computed label or a statement the reader cannot read leaves the file unparsed,
  real work.
