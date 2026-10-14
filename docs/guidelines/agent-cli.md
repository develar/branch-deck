# Agent CLI (`bd`)

**Status:** Active (read-only commands)  
**Updated:** 2026-10-14  
**Tags:** [cli, agents, json]

## Summary

`bd` lets an agent read what Branch Deck sees: the commits ahead of the baseline, their groups, and what ties them
together. Today it has the read-only commands `status`, `commits`, `schema` and `help agent`. The commands that
change things (`check`, `group`, `sync`, `describe`, `push`) come later.

It is the `cli` subcommand of the app binary: `branch-deck cli status`. If the binary is symlinked as `bd`, the CLI
runs without the subcommand. `src-tauri/src/main.rs` dispatches before Tauri starts, so no window or plugin is created.
The code is in `crates/bd-cli`. Windows needs a console sidecar and is not supported yet.

## Invocation

```bash
branch-deck cli [-C <repo>] [--prefix <p>] [--full-ids] [-v] <command> [options]
cargo run -p branch-deck -- cli status -C .   # from a checkout
```

- `-C <repo>` sets the repository. It defaults to the current directory, and any subdirectory resolves to the
  top level of the working tree.
- `--prefix <p>` sets the branch prefix. The default is the app setting `globalUserBranchPrefix`, read from
  `settings.json` in the app data directory (`~/Library/Application Support/org.develar.branch-deck/` on macOS).
  Without it, `git config branchdeck.branchPrefix` is used. An empty prefix is the error `no_branch_prefix`. Every
  output echoes the prefix.
- `--full-ids` prints full commit ids. By default an id is cut to 8 characters, as in Branch Deck's messages
  (`git_ops::rewrite_plan::short_id`). When two ids of one output share that prefix, all ids in it get longer.
- `-v` logs debug output to stderr and `-vv` logs trace output. Without either, stderr stays empty.
- Git comes from `BD_GIT`, else `git` on `PATH`, else Homebrew's git on macOS. It must be 2.54 or newer. Unlike the
  app, the CLI never asks a login shell for git.

## The JSON contract

- stdout gets exactly one JSON object per run, on one line. Only `bd help` prints text.
- Every object starts with `"schema": 1`. The version goes up only when a field changes meaning or disappears. New
  optional fields don't change it.
- Optional fields are left out when they are empty. `group` is the exception: it is always present and is `null` for
  an unassigned commit.
- `bd schema` prints the JSON Schema of each output (`status`, `commits`, `error`). The schemas are generated from the
  Rust types with `schemars`, and `schema_test.rs` checks the real outputs against them.

An error looks like this:

```json
{"schema":1,"error":{"code":"no_branch_prefix","message":"No branch prefix: ...","commits":[],"retryable":false}}
```

| Exit | Meaning | Codes |
|---|---|---|
| 0 | ok | |
| 1 | conflict | `conflict` |
| 2 | invalid input | `invalid_arguments`, `not_a_repository`, `no_branch_prefix`, `unknown_branch`, `invalid_plan`, `not_ahead_of_baseline` |
| 3 | stale, read again and retry | `head_moved`, `lease_rejected`, `locked` |
| 4 | refused | `no_baseline`, `operation_in_progress`, `detached_head`, `staged_overlap` |
| 5 | git or network error | `git_not_found`, `git_error`, `network_error` |

Agents branch on the code and the exit code, never on the message. `commits` names the commits an error is about.
`retryable` is true for `head_moved`, `lease_rejected`, `locked` and `network_error`. The codes for the future write
commands are already reserved.

## Commands

### `bd status [--branch <b>] [--unassigned] [--limit N] [--offset M]`

Prints HEAD, the baseline (`detect_baseline_branch`), the prefix, the branches and the unassigned commits. It groups
the commits the same way a sync does (`sync_core::grouping`, the first phase of a sync without its events). It does no
integration detection, no update check and no network access. It runs about ten short git processes, with one `log`
for all the commits.

```json
{"schema":1,"repository":"/repo","prefix":"develar","head":{"id":"ff366ae0","branch":"master"},
 "baseline":"origin/master","branch_count":2,"unassigned_count":1,
 "branches":[{"name":"auth","ref":"refs/heads/develar/virtual/auth","latest_time":1700000400,"commit_count":2,
   "tip":"2c1d0e93","commits":[{"id":"9f3a2b1c","subject":"Add logout"},{"id":"41c7d00e","subject":"Add login"}]},
  {"name":"ABC-12","ref":"refs/heads/develar/virtual/ABC-12","issue_key":"ABC-12", "...": "..."}],
 "unassigned":[{"id":"77e01aa4","subject":"Tidy up"}]}
```

- Branches are listed newest first, the same order as the UI. The commits of a branch are newest first too, and
  their subjects have the `(prefix)` stripped.
- `tip` is the tip of the existing virtual branch, or `null` if the branch was never synced.
- `--branch` lists that one branch and `--unassigned` lists only the unassigned commits. `--limit`/`--offset` page
  the commits in both of these modes, and page the branches otherwise.
- `parent` (the parent of a stacked branch) and `pull_request` (from `sync_core::pull_requests`) are reserved, and
  left out until stacks and pull request status exist.

### `bd commits [--files] [--patch] [--branch <b>] [--include-generated]`

Prints every commit ahead of the baseline, oldest first, which is the order they are applied in.

```json
{"schema":1,"repository":"/repo","prefix":"develar","head":"77e01aa4","baseline":"origin/master",
 "commits":[{"id":"41c7d00e","subject":"(auth) Add login","body":"...","author":"Name <mail>","time":1700000100,
   "group":"auth","files":[{"path":"auth.rs","status":"A","additions":12,"deletions":0}]},
  {"id":"5e2f9c3b","subject":"fixup! (auth) Add login","group":"auth",
   "fixup":{"kind":"fixup","target":"41c7d00e","target_subject":"(auth) Add login"}, "...": "..."}],
 "dependencies":[{"from":"5e2f9c3b","to":"41c7d00e","files":["auth.rs"]}],
 "cluster_overlap":[{"branches":["auth","login"],"files":["auth.rs"]}]}
```

- `group` is the branch the commit is grouped into, or `null`. `issue_key` is the issue key of the subject.
- `fixup` is set on a `fixup!`, `squash!` or `amend!` commit. Its `target` is resolved with `git rebase --autosquash`
  rules (`plan_autosquash`), and is `null` when no older commit matches.
- `--files` lists the changed files of each commit. `status` is `A`, `M`, `D`, `R` (`old_path` is set) or `T`.
  `additions` and `deletions` match `--numstat`. A binary file has `"binary": true` and no counts.
- `--patch` adds the patch of each listed commit, with three lines of context.
- `--branch` keeps only the commits of that branch, and the edges and overlaps that involve it. The dependencies are
  still computed over every commit.
- `dependencies` are edges where `from` depends on `to`, an older commit, through `files`.
- `cluster_overlap` lists, for each pair of current branches, the files both change.
- Lock files and generated files don't count in `dependencies` and `cluster_overlap` (see below);
  `--include-generated` keeps them. `--files` still lists them, with `"generated": true`.

All changes, the file lists and the dependencies come from one `git log --no-walk=unsorted --stdin -z --raw -p -U0`
process. `--patch` adds one more process, with `-U3`. Both pass explicit diff options (`-M`, `--no-ext-diff`,
`--no-textconv`, `a/`/`b/` prefixes), so the user's diff config doesn't change the output.

#### Lock files and generated files

Nobody edits them by hand, and they link every commit that touches them, so by default they are left out. A file is
generated when:

- its name is a lock file: `Cargo.lock`, `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `go.sum`,
  `poetry.lock`, `Gemfile.lock` and the others in `LOCK_FILES` (`crates/bd-cli/src/generated.rs`);
- or the attributes of HEAD mark it `linguist-generated` or `-diff` (which `binary` implies). One
  `git check-attr --source=HEAD --stdin -z` process checks all paths.

Other generated files, such as bindings or snapshots, count until `.gitattributes` marks them, e.g.
`src/lib/bindings.ts linguist-generated`.

#### How dependencies are approximated

This is in `crates/bd-cli/src/dependencies.rs`.

- Every line a commit adds is owned by that commit. The owned line ranges of each file are carried forward through
  the later commits.
- A commit depends on an earlier one when its hunk replaces or deletes an owned line, or is adjacent to one. Git's
  three-way merge conflicts on adjacent changes too.
  - `-a,b` with `b > 0` touches lines `a-1` to `a+b`.
  - A pure insertion after line `a` touches lines `a` and `a+1`.
- A change to a file created in the range depends on the commit that created it.
- Re-adding a deleted file depends on the commit that deleted it.
- A binary file or a type change depends on the previous commit that changed the file.
- Renames are followed. Copies are not.

The approximation sees only text. It misses semantic dependencies, such as a call to a function that another commit
adds in another file. It can also report independent edits that happen to be next to each other. Treat an edge as
"these can't be cherry-picked apart without a conflict", not as a proof.

### `bd schema`

Prints `{"schema":1,"outputs":{"commits":{...},"error":{...},"status":{...}}}`.

### `bd help agent`

Prints the workflow below as plain text.

## Agent workflow

1. Run `bd status`, then `bd commits --files`. For the clusters that are unclear, run
   `bd commits --patch --branch <b>`.
2. Cluster the commits by intent. Use `dependencies` and `cluster_overlap` to keep together, or stack, the commits that
   can't be separated.
3. Run `bd check < plan.json` until it is clean. *Not yet available.*
4. Run `bd group --expect-head <id> < plan.json`, then `bd sync`. *Not yet available.*
5. Run `bd describe` for each branch. *Not yet available.*
6. Show the result to the user, and run `bd push` only after the user approves. *Not yet available.*

The agent never reorders, squashes or drops commits. These are the same rules as `src/lib/agent-prompt.ts`. Until
`bd group` exists, the user applies the grouping in the app.
