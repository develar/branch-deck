# Proposal

## Why

The modules of a review (which module each changed file is in, which the commits add or delete, their names) came
from a walk of Branch Deck over the trees of the head (`trial_check::module_roots::ModuleRoots`, `root_of`): the
nearest directory with an `*.iml`, a `*.csproj`, a Bazel, Gradle or Maven build file, a `Cargo.toml` or a
`package.json`, a module added or deleted by the build files the commits touch. accio reads the same build files for
every tool, and more of them, with their readers (`accio modules`, answer revision 1, `--paths`: the root of the
nearest module of each path at a revision, and what each root declares: JPS, Bazel, Cargo, npm, Go, Python, Maven,
Gradle, MSBuild), so the knowledge of where a module is lives once, in the index.

## What Changes

- The module of each changed file is the root `accio modules --ref <head> --paths-from -` answers for its path, or the
  deeper root the same query answers at the base (a module the commits delete); a file outside every module, and every
  file of a plan without both answers (no accio, no index, an accio of another revision of the answer), is of the root
  of the repository.
- A module is `deleted` when its root is of the base alone, `added` when the head declares it and no file of it has
  that root at the base, else `changed`: a build file the commits rename leaves its module `changed`.
- Its name is the one its build files give it (its `*.iml` by the file's stem, its crate, its package, its Maven,
  Gradle, MSBuild, Go or Python project), else that of a dependency change of a build file at its root, else its
  directory. A crate is named as its `Cargo.toml` writes it (`review-plan`), so a dependency entry that names it names
  the module of the review.
- The two queries, one process a side (the base of Since Your Review the start of its scope, the reviewed tip where
  its carry conflicts, so a module the reviewer read is not added again), run at once with the git reads of the plan
  (the diffs of the reading, the summaries, the pairs). The plan of git does not wait for them: a cold answer takes
  seconds where git takes one, so the plan is sent first with no module, then again with them while another query of
  the index is left (`PlanWait::Index` both times); the units are read once, the concerns, the brief and the steps
  told again with the modules (measured in decision 30 of `docs/design/change-review.md`).
- The role of a file (`build`) and the markers of a module list read the names of the build files accio reads a module
  from (its `kind_of`): `go.mod`, `pyproject.toml` and `setup.py` too.
- `cross_module` of a link compares the modules of its two files by the same roots.
- `ModuleRoots` leaves the review plan: no `root_of`, `iml_of` or `is_module_file` in trial-check, no read of the trees
  above every changed file (`read_symbol_input_with` reads them above the source files alone, for the summaries); a plan
  of files of no language runs one `cat-file` fewer.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: the modules of a review and the links across them come from accio's `modules` at the head and
  the base; the plan of git is sent before they answer.

## Impact

- `crates/review-plan/src/{modules,links,lib,files}.rs` (`DeclaredRoots`, the stages of `review_plan_of`), `crates/file-diffs/src/accio.rs`
  (`SymbolIndex::modules`, `ModulesAnswer`, `MODULES_ANSWER`, the fixture's `modules`), `crates/test-server/src/state.rs`,
  `crates/trial-check/src/{module_roots,symbol_scan}.rs`; the budgets of `crates/bd-cli/src/process_budget_test.rs`.
- The e2e helpers of the modules of the `review_modules` template, which answer accio's modules in the fixture index.
- `docs/design/change-review.md`, `docs/design/git-operations.md`, `docs/design/symbol-dependencies.md` (why the sync
  keeps its own walk: the cost of `accio modules` at a new tree), `docs/contributing/code-map.md`.
