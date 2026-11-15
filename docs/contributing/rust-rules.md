# Rust Rules

**Status:** Active  
**Updated:** 2026-11-14  
**Tags:** [rust, clippy, tracing, windows, psi-syntax]

## Summary

How the Rust code is written beside its git ([Git Rules](git-rules.md)): logging and spans, Clippy, platform
branches, file system removals, processes and imports, and how a crate of psi-syntax changes
([The crates of psi-syntax](#the-crates-of-psi-syntax)). [AGENTS.md](../../AGENTS.md) ("Code Style") keeps one line
per rule and links here.

- **Rust**: `snake_case` for variables, functions and modules; `tracing` for structured logging, `#[instrument]` to
  trace a function's entry, exit and arguments; proper `Result` types, not strings.
- **Operations and spans** ([Operation spans](../design/async-patterns.md#operation-spans)): every command of the
  app (a body of the registry of app-api; the app has no other) has `#[instrument]`, with `skip_all` and a field
  or two when an argument holds a collection; a command that can fail runs its body as an operation (`operation`,
  `operation_now`, or its one blocking step `run_blocking`; `crates/app-api/src/operation.rs`), so its error names the
  `operation_id` of its log lines; the object cache, the git directories and the GitHub repository only in that step
  (`crates/app-api/src/command_scan_test.rs`). A `session` command is the exception: its frame opens the span and the
  operation, so its body has no `#[instrument]` and no `operation`
  (`the_body_of_a_session_command_opens_no_span_and_no_operation_of_its_own`), and its entry names the fields of its
  span after its kind, beside `repository_path` (`session(reason = ?params.reason)`, `session(window)` for the window
  of the call; `the_span_of_a_session_command_records_the_fields_its_entry_names` of the e2e server). A
  closure or future handed to another thread carries the caller's span: `sync_utils::span::in_current_span(…)` or
  `.in_current_span()` (`crates/git-executor/src/thread_span_scan_test.rs`).
- **Clippy**: pedantic, nursery and cargo plus the restriction lints that catch defects (`[workspace.lints]` in
  `Cargo.toml`), on all targets; any warning fails `pnpm lint:backend` and CI (`-D warnings`), Windows included. An
  exception is `#[expect(lint, reason = "…")]` where it holds, never `#[allow]`; a lint that is wrong for this code is
  allowed in `Cargo.toml` with its reason.
- **A platform branch in a test** asserts on every platform: an `if cfg!(target_os = "macos")` has an `else` that
  checks what the other platforms do (a `worktree add` where no APFS clone is), never a branch that asserts nothing.
  A helper or import that only `#[cfg(unix)]` tests use carries `#[cfg(unix)]` too, or Windows clippy fails on it.
- **Single flights**: a read that other callers wait for (a token source, a renewal, a read of Patronus two windows ask
  at once) is a single flight (`sync_utils::single_flight`), never the guard of an async mutex held across an await;
  `concurrency_scan_test.rs` of `git-executor` fails on such a guard ([Single flights](../design/async-patterns.md#single-flights-no-async-mutex)),
  and on the guard of a `std` mutex held across git: a memo is locked to look up and to keep, git runs without it.
- **Removals and renames** of the file system go through `sync_utils::fs_retry` (`remove_dir_all`, `remove_dir`,
  `remove_file`, `rename`), never `std::fs`: on Windows a git or a shell whose current directory is in the path, or a
  scanner with a file open, fails them at once (`fs_retry_scan_test.rs`). A walk that removes a tree removes each
  entry with `remove_entry` and the file type of its `symlink_metadata`: a junction or a directory symlink of Windows
  goes as the link (`remove_dir`), which `remove_file` refuses with the "Access is denied" of a holder. A move that may
  cross two volumes (a slot of the pool and the state of its repository) is `move_path`, which copies where no rename
  crosses them (`EXDEV`). A process that works in a test's directory is `test_utils::git_in_directory::GitInDirectory`.
- **A process** starts through `sync_utils::child_process::windowless_command`, git through `GitCommandExecutor`,
  never `Command::new`: the release app on Windows is a GUI program, and every console program it started got a
  console window of its own, a Windows Terminal window in front of the app for each git (`console_window_scan_test.rs`).
  A child of work done ahead of a need (the read ahead of a review plan) starts through `windowless_command_at(…,
  ChildPriority::Background)`, below the app: Windows takes its priority class and its console flag in one
  `creation_flags`, which a second call would replace.
- **Rust imports** ([Import Guidelines](imports.md)): never `pub use` re-exports (except for internal
  module organization); make modules public with `pub mod`; every import uses the full module path, so it shows where
  a type or function comes from:
  - ✅ `use git_executor::git_command_executor::GitCommandExecutor`
  - ✅ `use domain_types::issue_navigation::IssueNavigationConfig`
  - ❌ `pub use pull_request::PullRequestAvailability` (in lib.rs)

## The crates of psi-syntax

`code-symbols` (the summaries, the highlighting, the navigation and the bindings of the names the marks ask),
`code-tokens`, `line-diff`, `diff-marks`, `spec-syntax`, `js-syntax` (with `tsconfig`) and the parsers below them are
accio's `psi-syntax/crates/`, which accio's `accio diff` reads too. Branch Deck links them from develar/psi-syntax, the
public mirror of accio's `psi-syntax/`, at one revision: the `rev` of their lines of `[workspace.dependencies]` in the
root `Cargo.toml`, the same as `file_diffs::PSI_SYNTAX_REVISION` (`the_psi_syntax_revision_is_the_one_cargo_builds`
checks every one Cargo.lock holds against it; `lmdb-env` has a revision of its own). None of their code is in this
repository, so a change of one is never an edit here:

1. **Try it** against accio's checkout (or a lease of accio) before a commit there. Every crate of psi-syntax that
   Cargo.lock holds, but `lmdb-env`, is patched to it, one `--config` each; a patch of one alone would link a second
   copy of the crates it depends on:

   ```bash
   accio=~/projects/accio   # or the lease of accio that holds the change
   patches=()
   for crate in code-symbols code-tokens csharp-syntax diff-marks java-syntax js-syntax kotlin-syntax line-diff \
     psi-builder rust-syntax spec-syntax starlark-syntax; do
     patches+=(--config "patch.\"https://github.com/develar/psi-syntax\".$crate.path=\"$accio/psi-syntax/crates/$crate\"")
   done
   cargo "${patches[@]}" nextest run -p file-diffs -p trial-check   # bash and zsh alike
   ```

   The patched build keys the cache by the revision it does not build (`summaries_version`, `code_tokens::tokens_version`
   of `PSI_SYNTAX_REVISION`), so a summary or a diff it writes would be read by the released app as its own: run `brd`
   of a patched build with `--no-cache` (the cache is the repository's, `$GIT_COMMON_DIR/branch-deck/cache`, shared by
   every worktree), and the app only on a repository of its own. cargo rewrites Cargo.lock to the paths; it goes back
   before a commit (`git checkout Cargo.lock`), and a Cargo.lock with a path fails
   `the_psi_syntax_revision_is_the_one_cargo_builds` all the same.
2. **Commit in accio** (its rules, its tests: a rule of `code-symbols` is tested there, `marks_bindings*_test.rs` for
   the marks), push, then mirror it (`scripts/psi-syntax-mirror.sh` of accio, its build and tests in the mirror, the
   push of the mirror).
3. **One revision here**: the mirror's commit in every psi-syntax line of `[workspace.dependencies]` and in
   `PSI_SYNTAX_REVISION`, one commit. Every revision makes the stored diffs (`file_diff`) and summaries
   (`blob_summary`) again, as their versions hash it; the highlighting is kept in memory only. The commit says so, and
   the A/B of `brd --no-cache commits --mark-stats` and `--symbol-stats` on a real repository before and after shows
   what the revision changed.
