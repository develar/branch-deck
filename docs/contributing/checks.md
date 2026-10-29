# Checks

**Status:** Active  
**Updated:** 2026-10-29  
**Tags:** [tests, lint, nextest, e2e]

## Summary

What to run for a change and why: nextest, the cost of a test, the Studio, the table of checks by what changed,
the scripts of `package.json` and how to debug a failed E2E run. [AGENTS.md](../../AGENTS.md)
("Verification") keeps the short table and links here.

## Rust Tests

⚠️ **NEVER `cargo test`, ALWAYS `cargo nextest run`.** `cargo test` runs the tests of a binary as threads of one
process, and a test that sets environment variables (`test_utils::env_vars::EnvVars`, sound only with a process per
test) changes them under the others. `nextest` (`.config/nextest.toml`) runs a process per test, all in parallel
(the suite starts about 70,000 git processes, 94,000 at 2e503b03; on the 20 cores of the Studio 208–220 s against
232–279 s for 2e503b03 in turns, several times that on a loaded Mac); kills a test still running after 5 minutes (a hung git) and fails one whose
git process outlives it; in CI (`NEXTEST_PROFILE=ci`) runs a failed test twice more, and one that passes on a retry is
FLAKY and still fails the run, because a flaky test is a race to fix (locally no retries). With recording on in the user
config (`[experimental] record = true` and `[record] enabled = true` in `~/.config/nextest/config.toml`),
`cargo nextest run -R latest` runs only what failed; finish with a full run.

**A test's time is its git processes** ([Fast Rust Tests](rust-tests.md)): a fixture repository is
`TestRepo::new()` (one `git init`, its config appended by `append_config`), a commit `create_commit*` or `commit_with`
(`add` and `commit`, the id from the summary, no `rev-parse HEAD`), a history of many commits `with_history` or
`commit_history` (one `fast-import`); never a `git config` process per key, a sleep, or a wall-clock limit an
assertion depends on. `pnpm perf:test-processes measure|summarize|compare` counts the git
processes of every test, which the load of the machine does not change.

**Heavy runs go to the Mac Studio**: the full `cargo nextest run --workspace`, a full `pnpm e2e` and
`pnpm lint:backend` run as `bd-host run studio -- <command>` from the lease, on its working tree, committed or not
([Studio Runs](studio-runs.md), `[host]` of `.branchdeck.toml`). This Mac runs several agents at once,
and under that load the suites take far longer and E2E fails on timeouts. What a command edits (`cargo fmt`,
`pnpm e2e-up`, `UPDATE_BINDINGS=1`) comes back with `--apply`. Exit code 75: the Studio did not run it, run it here.

If Homebrew `cargo`/`rustc` come before rustup in `PATH`, prefix cargo commands with `PATH=$HOME/.cargo/bin:$PATH`:
the nightly toolchain pinned in `rust-toolchain.toml` is required to parse the workspace manifest.

## By Change

| You changed | Run |
|---|---|
| Rust | `pnpm lint:backend`, then `cargo nextest run --workspace` (`-p <crate>` or `-E 'test(/…/)'` while iterating); a new cross-crate edge fails the architecture test (`crates/bd-cli/src/architecture_test.rs`: the rules of [Crates](../design/rust-domain-authority.md#crates-и-направление-зависимостей) on `cargo metadata`, an allowlist that only shrinks); a lock, an async mutex or a network call changed: `cargo nextest run -p git-executor -E 'test(/scan/)'` (the concurrency scan beside the git scans) |
| a git operation | also its test in `process_budget_test.rs` and the scan tests (`worktree_safety_test.rs`, `git_usage_scan_test.rs`, `concurrency_scan_test.rs`: `cargo nextest run -p git-executor`); a core that takes the repository lock: its row of [Lock spans](../design/git-operations.md#lock-spans) and the test that pins its span (`test_utils::lock_span::LockSpan`, `bd_cli::fixture_test::LockSpanGit` for a `bd` command) |
| a command of app-api (`crates/app-api/src/commands.rs`) or a type it passes | `src/lib/bindings.ts`: `UPDATE_BINDINGS=1 cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'` writes it, headless, the commands in the order of the registry; `crates/app-api/commands.json`, the table of the page mock: `UPDATE_COMMANDS=1 cargo nextest run -p app-api -E 'test(/commands_json/)'`; without the variables the tests fail on a stale file; a new command raises `COMMAND_COUNT` (`registry_test.rs`) and needs no route or wrapper of its own |
| frontend | `pnpm lint:frontend`, `pnpm test:unit`, `pnpm e2e`; `pnpm e2e-up` when the HTML changes, its snapshot diff reviewed like code; an E2E spec or helper changed: also `pnpm lint:e2e-pins` (`command_registry_test.rs`: no page answer of a write, the pinned page answers only go down; `pnpm e2e` doesn't run it) |
| code anywhere | `pnpm lint:cpd`: fix a clone with a helper, constant or fixture; `// jscpd:ignore-start` / `// jscpd:ignore-end` only for an intentional duplication, with the reason in the comment |
| code anywhere | `pnpm lint:arch` (`scripts/lint-arch.mjs`): no branch on the words of an error, no `kind` of an error, a code in every `routeCommandError`, no polling timer in `src/stores` or `src/hooks` (`poll-timer`: a fact is the session's), no rule of the domain in the page (`rule-module`: a deleted rule module, an export of `src/lib` or `src/hooks` named `can…`, `needs…`, `allows…`, `…Blocker`, `…Decision`, `is…Blocked`; the rules are `domain-policy`'s, the page presents them), no read of the start (`startup-read`: a command of `src/main.tsx`, `src/router.ts`, `src/stores/repository.ts` but Browse, `resolveBranchPrefix` outside Settings, `syncBranches("open")`), the engine only through the intents (`intent-call`: `commands.execute` and `commands.cancelOperation` only in `src/model/intents.ts`, each `app(…)` command of `crates/app-api/src/commands.rs` from its one module of `APP_COMMAND_CALLERS`), no store read in a presenter (`presenter-state`: no `.getState()` in `src/presenters`, nor a value import of `@/stores/…` or `@/model/store`, whose getters read the store; the caller passes the part it selected), the key of the selected project before a run waits (`late-project-read`: no `getSelectedProject`, `getSelectedRepositoryPath`, `getFullBranchName` and the like after the first `await` of an async function); an exception is an `ALLOWLIST` entry with its reason, only of a timer of the UI (`poll-timer`) |
| a fact of the session (a family, `auth` of the sign-ins among them, its refresh rule, the scheduler, the demand) | `cargo nextest run -p domain-policy -p remote-facts -E 'test(/refresh|scheduler/)'` and `cargo nextest run -p repository-model -E 'test(/facts|session/)'`; `auth` also `cargo nextest run -p remote-facts -E 'test(/sign_in/)'` (the sign-ins its host reads); the e2e of the family moves the session's clock with `repo.advanceBackendClock(ms)` and asserts the counters of the fakes ([Session facts](../design/git-operations.md#session-facts)) |
| the frame of a session (what `warm_start.rs` keeps, the seed, the catch-up, a type a frame holds, the view's types of `domain-types` among them) or the start of the main window | bump `FRAME_SCHEMA` when a type a frame holds changes (no frame of the old schema is read, the first start is cold); `cargo nextest run -p repository-model -p operations` (`warm_start_test.rs`, the seeded sessions of `session_test.rs`, `rewrite_test.rs`), `pnpm e2e tests/e2e/startup/` (`warm-start.spec.ts`: B7, one `subscribe_view` and its channel before the rows) and `pnpm lint:arch`; a change of what a first sync patches: `pnpm perf:table --project=chromium` on the Studio (`warmFirstSync*`, B4) ([Warm start](../design/git-operations.md#warm-start)) |
| the next step, the Safe Push decision or `plan()` of `bd safe-push`; the sync, `patronus_view`, the robot list, the review scan or what the session derives (`repository_model::derive`) | `cargo nextest run -p bd-remote -E 'test(/scenario/)'`: one expectation per case that the app's session and `bd` both answer; the cases in `crates/domain-policy/cases/` updated in the same commit, the diff reviewed like code ([Scenario table](../design/rust-domain-authority.md#таблица-сценариев)) |
| a rule of `domain-policy` (a file of the [rule index](../../AGENTS.md#rule-index)) or what the view derives of it | `cargo nextest run -p domain-policy` (the table test of each rule beside it, `<rule>_test.rs`) and `cargo nextest run -p repository-model -E 'test(/derive|action_view|state/)'` (the parts of the view, `derive_budget_test.rs` its time, `patch_bytes_test.rs` the bytes of its patches); then the scenario table (the row above), and the page's presenter of what changed (`src/presenters/*.test.ts`) |
| a `Reason` code (`domain_types::reason`), an action or a blocker of an action (`domain_types::actions`) | `UPDATE_REASON_CATALOG=1 cargo nextest run -p domain-policy -E 'test(/reason/)'` writes `crates/domain-policy/reasons.json`, `UPDATE_ACTION_CATALOG=1 cargo nextest run -p domain-policy -E 'test(/action_catalog/)'` `crates/domain-policy/actions.json`; `src/presenters/reasons.test.ts` and `actions.test.ts` word them code by code, so a code without words fails `pnpm test:unit` |
| a spec, a doc, a test title, a rule of `domain-policy` or a presenter | `pnpm lint:specs` (`scripts/check-spec-links.mjs`): every test link a line of its own, naming tests that exist; the `targets:` of every spec, each once, `../../` paths of files that exist; every rule of `crates/domain-policy/src/` a target of each spec its `//!` header names (a header that names none fails); every presenter of `src/presenters/` a target of a spec |
| `#[cfg]` or `cfg!` code | Linux and Windows too: [Platform Checks](platform-checks.md) (a check on the Mac compiles only its side) |
| what the release app shows (a window that opens late, a "Loading…" frame) | [Real App Checks](real-app-checks.md) (`pnpm install-app`, Peekaboo) |

## Commands

See `package.json` for the full list of scripts.

```bash
pnpm install              # Install dependencies
pnpm run tauri dev        # The full app (frontend + backend)
UPDATE_BINDINGS=1 cargo nextest run -p branch-deck -E 'test(/the_bindings_are_what_the_export_writes/)'  # Write src/lib/bindings.ts
UPDATE_COMMANDS=1 cargo nextest run -p app-api -E 'test(/commands_json/)'  # Write crates/app-api/commands.json (the page mock's table)
pnpm dev                  # Only the Vite frontend (port 1420)
pnpm build                # The frontend into dist/ (E2E global setup runs it when its inputs changed)
pnpm install-app          # Unsigned release build of the .app, copied to /Applications (macOS)

pnpm test                 # test:backend + test:unit (Rust and Vitest; not E2E)
pnpm test:backend         # cargo nextest run --workspace
bd-host run studio -- cargo nextest run --workspace  # The same on the Mac Studio, on this working tree (75: run it here)
bd-host run studio -- pnpm e2e  # The E2E suite there; --apply brings back what a command edits (pnpm e2e-up, cargo fmt)
cargo nextest run -R latest  # Rerun what failed or did not run in the last run (record on in the user config)
pnpm test:unit            # Vitest (`vp test run`; src/**/*.test.ts)

pnpm e2e                  # Playwright E2E tests (installs Chromium if needed; `pnpm setup:e2e` for every browser)
pnpm e2e-up               # E2E tests that rewrite the snapshots
pnpm e2e:webkit           # The E2E tests tagged @webkit in WebKit (playwright.webkit.config.ts)
DEBUG_E2E=1 pnpm e2e      # With the mock server's debug log and every page console line
E2E_PORT=3031 pnpm e2e    # On another port when another checkout's mock server holds 3030
pnpm test-server:dev      # Optional: the E2E mock server with auto-restart (needs watchexec); `pnpm e2e` reuses it
pnpm perf:diff            # The diff review performance test (playwright.perf.config.ts)
pnpm perf:table           # The branch view at 141 rows: layout, sync, IPC (ui-theme.md#branch-columns); --project=webkit
pnpm screenshots:readme   # The README images in docs/screenshots (tests/screenshots, the readme_showcase template)

pnpm lint                 # Every lint:* script
pnpm exec vp check        # Formatting, lint and types without fixing (what CI checks first)
pnpm lint:backend         # cargo fmt, clippy --fix, then clippy -D warnings
pnpm lint:frontend        # `vp check --fix` (Oxfmt, type-aware Oxlint) + lint:typecheck
pnpm lint:typecheck       # tsc of tsconfig.app.json, tsconfig.node.json and tsconfig.e2e.json
pnpm lint:cpd             # jscpd copy/paste detection (config in .jscpd.json); fails on any clone
pnpm lint:specs           # the test links of spec/ and docs/ name tests that exist; the targets: of the specs name files, of every rule and presenter
pnpm lint:arch            # the page acts on the code of an error, never on its words, decides no rule, reads nothing before its subscription, calls the engine through src/model/intents.ts (scripts/lint-arch.mjs)
pnpm lint:e2e-pins        # the commands E2E tests answer in the page, pinned (crates/test-server/src/command_registry_test.rs)
bd lease clean --free     # the clean rules of .branchdeck.toml on the free lease slots (`--dry-run` only reports)
pnpm perf:monorepo pin|measure|summarize|compare  # bd's git processes and times on a pinned large clone (git-operations.md)
pnpm perf:bd-rebuild measure|summarize|compare  # the rebuild of the bd crates after a one-line edit (scripts/perf/bd-rebuild.mjs)
pnpm perf:test-processes measure|summarize|compare  # the git processes of every Rust test (docs/contributing/rust-tests.md)
cargo run -p branch-deck -- cli status -C <repo>  # agent CLI, JSON on stdout (docs/contributing/agent-cli.md)
```

## Debugging

- **E2E**: a run ends with its timing (start, slowest tests, stalled actions; `tests/results/timing.json`), and a
  failed test carries the console errors of its pages as the `page errors` attachment. Run with
  `DEBUG_E2E=1` for the debug log of the mock server and every console line live; without it the server logs warnings
  and errors only (`TEST_SERVER_LOG`, not `RUST_LOG`, which `.cargo/config.toml` forces to debug), and a green run
  prints none. Global setup installs Chromium when the version of Playwright has none; "Executable doesn't exist" of
  another browser: `pnpm setup:e2e`.
- **Types**: `pnpm lint:typecheck` isolates TypeScript errors.
