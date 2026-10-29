# Working in a Lease

**Status:** Active  
**Updated:** 2026-10-29  
**Tags:** [agents, worktrees, lease, git, history]

## Summary

How an agent works in this repository: the session in a worktree of the `bd lease` pool, what a lease shares with
the others, how its commits reach `master`, and the git and shell mistakes that cost sessions here.
[AGENTS.md](../../AGENTS.md) ("Session Workflow", "Worktrees", "Commits and History") keeps the short rules
and links here; the commands are [bd lease](../../spec/cli/lease.spec.md).

## Session Workflow

1. **Lease** a worktree before the first edit: `bd lease get --holder <agent-name> -b <branch>` in the main checkout,
   then work in the `path` it prints; its `home` names where the commits land, its `next` what to run
   ([Worktrees](#worktrees)).
2. **Read** the spec and the design doc of what you change ([Feature Index](../../AGENTS.md#feature-index)): the spec is the
   contract, the doc the why. Before changing a git operation, [Adding a git operation](../../AGENTS.md#adding-a-git-operation).
3. **Change** the code together with its spec, its test links and the docs it contradicts.
4. **Verify** by the table in [Verification](../../AGENTS.md#verification).
5. **Commit** in the lease, a conventional subject as in `git log` (`feat(bd): …`, `fix(e2e): …`, `docs(agents): …`),
   then land it on `master` of the main checkout with `bd -C <path> lease pick` (a bd without it: `bd pick --from
   <path> --follow -C <main checkout>`): the commit goes onto `master` as it is then, the lease follows; a conflict
   lands nothing and comes back to you (`conflict.next`). Never `git rebase` or `git merge` for it, never `bd update`
   in the lease.
6. **Release**: stop the processes you started in the lease (the e2e mock server, `cargo watch`), then
   `bd lease return --workspace <path>`; a run on the Studio keeps nothing there.

## Worktrees

Parallel work runs in a worktree of the `bd lease` pool (`~/.branch-deck/pool/<repo>-<hash>/`,
[bd lease](../../spec/cli/lease.spec.md)); never `git worktree add`, `EnterWorktree` or `isolation: "worktree"`.
- **Every session edits in its own lease**, the main session and a background job too, not only a spawned agent: other
  sessions work in this repository at the same time. A harness default such as "edit files directly in your working
  directory" does not override this rule. Nothing is edited, built or committed in the main checkout: the commits of a
  lease reach its `master` only by `bd lease pick` ("commit right away" means: commit in the lease, then
  `bd lease pick`).
- **Acquire**: `bd lease get --holder <agent-name> -b <branch>` creates `<branch>` at the HEAD of the checkout that runs
  it (local `master` in the main checkout, which may be ahead of `origin/master`; `-C <worktree>` cuts from another
  branch; without `-b` the worktree is detached) and prints the lease as JSON (`path`, `lease_id`, `home`, `next`).
  It does not fetch.
  A leased worktree is never handed out again.
- **Inside**: the rules of the main checkout apply (`.githooks/` via the shared `core.hooksPath`, `cargo nextest`).
  `node_modules` and `target/` survive a reuse; on macOS a new slot is an APFS clone of a free slot or of the main
  checkout, so it starts with them too. Run `pnpm install` when `pnpm-lock.yaml` changed.
- **Rust cache**: every checkout keeps its own `target/`; never a shared `CARGO_TARGET_DIR` or `build.build-dir`: the
  unit hash of a workspace crate does not depend on the checkout's path, so worktrees would overwrite each other's
  units. Cargo never deletes a stale unit, so the return of a lease applies the clean rules of `.branchdeck.toml` (the
  stale units of `target/`, the e2e artifacts); `bd lease clean --free` applies them to the free slots now
  (`--dry-run` only reports). Dependencies are built with workspace-wide features (`.cargo/config.toml`) and line
  tables only.
- **Disk**: a get refuses as `low_disk_space` when the volume of the pool has less free space than
  `branchdeck.leaseMinFree` (git config, the machine's: `200G` on the user's Mac); tell the user, never pass
  `--min-free 0` on your own.
- **E2E**: the mock server listens on 3030 in every checkout; when another worktree's server holds it, run
  `E2E_PORT=3031 pnpm e2e` (any free port). Global setup refuses a server that serves another checkout, because
  Playwright reuses whatever listens on the port.
- **Release**: `bd lease return --workspace <path>` refuses local changes, a commit that no branch holds and that
  `master` lacks by content, or a process inside. `--confirm-preserved` discards the changes and commits it reports, so
  pass it only when they are preserved elsewhere. The branch survives; the next get moves the worktree.
  `bd lease status` lists the pool. A worktree leased from the old Treehouse pool (`~/.treehouse/<repo>-<hash>/`)
  goes back with `treehouse return <path>`.

## Commits and History

How agents commit to this repository; "Git Operations" below is about the git the app runs.
- **Linear history, no merge commits**: not of master, not of an integration branch, not of an agent worktree branch.
- An agent lands the commits of its lease with `bd lease pick` (Session Workflow, step 5), never with
  `git cherry-pick`, which leaves `CHERRY_PICK_HEAD`. By hand, bring a branch up to date with `git rebase <target>`
  and integrate it with `git merge --ff-only <branch>`. Rebasing your own branch in your worktree is fine: the
  "never write to the working tree" rule is about the app's code.
- `.githooks/` enforces it (`pre-merge-commit`, and `pre-commit` while `MERGE_HEAD` exists). Enable it once per clone,
  every worktree shares the setting: `git config core.hooksPath .githooks && git config merge.ff only`; with it
  `git config rerere.enabled false`, which keeps rerere off even where an `rr-cache` directory exists
  ([Pitfalls](#pitfalls)).
- Never `--no-ff`, never `--no-verify`, never `commit-tree` with two `-p`.

## Pitfalls

- **Same-second commits are the same commit**: a `git replay` or cherry-pick of a commit onto its own parent within one
  second writes the identical object, so "the copy" is the original. A test that needs a new copy makes the target
  diverge first (one more commit) or uses `cherry-pick -x`; a test that passes on a rerun is often this.
- **zsh does not word-split `$args`**: `args="--no-default-features --features=x"; cargo build $args` passes one
  argument. Write the arguments out, use `${=args}`, or run the loop under `bash -c '…'`.
- **An "unresolved import" of a module that clearly exists**, after another session moved branches under the same
  `target/`, is a stale cargo fingerprint: `touch` the crate's files and rebuild.
- **One stash and one rerere cache for every lease**: `refs/stash` and `.git/rr-cache` belong to the common git dir, so
  every worktree of the lease pool shares them, and a `git stash pop` in one lease can apply the entry another lease
  pushed meanwhile. Rerere replays a resolution recorded in one lease into the same conflict of a `git rebase`,
  `cherry-pick`, `merge` or `stash pop` in another ("Resolved '<path>' using previous resolution.", no markers left),
  matched by the text of the conflict alone, whatever that lease meant; git turns it on without any config once the
  `rr-cache` directory exists, and agents had filled it with 132 resolutions by 2026-10-29. So the clone sets
  `rerere.enabled false` ([Commits and History](#commits-and-history)), which no directory overrides; a conflict of
  `bd lease pick` opens its resolve session, which rerere never touched. Never `git stash` in a lease: keep work in
  progress as a WIP commit of the lease or a patch file in `notes.local/`.
