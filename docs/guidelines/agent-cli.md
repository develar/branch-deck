# Agent CLI (`bd`)

**Status:** Active  
**Updated:** 2026-10-14  
**Tags:** [cli, agents, json]

## Summary

`bd` lets an agent read what Branch Deck sees (the commits ahead of the baseline, their groups, and what ties them
together), regroup the commits and sync them. It reads with `status`, `commits`, `schema` and `version`, suggests a
checked grouping with `suggest`, rewrites commit messages with `group`, `rename`, `restack`, `unstack` and `undo`,
rebuilds the virtual branches with `sync` and keeps their pull request text with `describe`, tries a plan without
writing refs with `check` and pushes with `push`.

On macOS and Linux it is the `cli` subcommand of the app binary: `branch-deck cli status`. If the binary is symlinked
as `bd`, the CLI runs without the subcommand. `src-tauri/src/main.rs` dispatches before Tauri starts, so no window or
plugin is created. On Windows the app binary is a GUI-subsystem program without a console, so the installer puts a
console `bd.exe` next to `branch-deck.exe`: the `bd` binary of `crates/bd-cli`, which `scripts/build-bd.mjs` builds
for the Tauri `externalBin` of `src-tauri/tauri.windows-bd.conf.json` (passed with `--config` by the Windows CI jobs
only, so other builds don't need the file). The code is in `crates/bd-cli`; the requirements are in the
[Agent CLI spec](../../spec/cli/agent-cli.spec.md).

## Invocation

```bash
branch-deck cli [-C <repo>] [--prefix <p>] [--full-ids] [--no-cache] [-v] <command> [options]
cargo run -p branch-deck -- cli status -C .   # from a checkout
```

- `-C <repo>` sets the repository. It defaults to the current directory, and any subdirectory resolves to the
  top level of the working tree.
- `--prefix <p>` sets the branch prefix. The default is the app setting `globalUserBranchPrefix`, read from
  `settings.json` in the app data directory (`~/Library/Application Support/org.develar.branch-deck/` on macOS,
  `%APPDATA%\org.develar.branch-deck\` on Windows).
  Without it, `git config branchdeck.branchPrefix` is used. An empty prefix is the error `no_branch_prefix`. Every
  output echoes the prefix.
- `--full-ids` prints full commit ids. By default an id is cut to 8 characters, as in Branch Deck's messages
  (`git_ops::rewrite_plan::short_id`). When two ids of one output share that prefix, all ids in it get longer.
- `--no-cache` neither reads nor writes the object cache (see below).
- `-v` logs debug output to stderr and `-vv` logs trace output. Without either, stderr stays empty.
- Git comes from `BD_GIT`, else `git` on `PATH`, else Homebrew's git on macOS. It must be 2.54 or newer. Unlike the
  app, the CLI never asks a login shell for git.

## The JSON contract

- stdout gets exactly one JSON object per run, on one line. Only `bd help` prints text.
- Every object starts with `"schema": 1`. The version goes up only when a field changes meaning or disappears. New
  optional fields don't change it.
- Optional fields are left out when they are empty. `group` is the exception: it is always present and is `null` for
  an unassigned commit. In a `--brief` view a field is also left out when another field says it: `issue_key` when it
  is the `group` (`bd commits --brief`) or the `name` (`bd status --brief`) of an issue branch, and the `(prefix)` of a
  subject when it is exactly the `group`.
- `bd schema` prints the JSON Schema of each output (see `bd schema` below). The schemas are generated from the Rust
  types with `schemars`, and `schema_test.rs` checks the real outputs against them.

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
| 4 | refused | `no_baseline`, `operation_in_progress`, `detached_head`, `staged_overlap`, `push_refused`, `no_remote`, `install_refused` |
| 5 | git or network error | `git_not_found`, `git_error`, `network_error` |

Agents branch on the code and the exit code, never on the message. `commits` names the commits an error is about.
`retryable` is true for `head_moved`, `lease_rejected`, `locked` and `network_error`. The codes for the future write
commands are already reserved.

## Commands

### `bd status [--branch <b>] [--unassigned] [--limit N] [--offset M] [--stats [--include-generated]] [--brief]`

Prints HEAD, the baseline (`detect_baseline_branch`), the prefix, the branches and the unassigned commits. It groups
the commits the same way a sync does (`sync_core::grouping`, the first phase of a sync without its events). It does no
integration detection (except for a stacked parent whose commits left HEAD, below), no update check and no network
access. It runs about ten short git processes, with one `log` for all the commits.

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
- With stacked branches on (`git config branchdeck.stacks true`), each branch is placed as a sync places it:
  - `parent`, the parent its commits declare, and `parent_state`: `active` (it has commits), `integrated` (its branch
    is integrated: the branch goes on the nearest active ancestor), `unapplied` or `missing`. A parent whose commits
    left HEAD since the last sync is detected now, as the next sync detects it, without writing the detection note;
  - `effective_base`, the branch it is built on (omitted for the baseline), or `blocked_by` `{parent, reason}`;
  - `problems` (`{message, commits}`): conflicting parents, a cycle, a self parent, a case collision, an empty level.

  The output has `forest`: the branches with commits, each after its parent.
- `description` is `{title}` of the branch's `bd describe` text, read for all branches with one
  `git config --get-regexp`.
- `pull_request` (from `sync_core::pull_requests`) is reserved, and left out until pull request status is wired.
- `--stats` adds `stats` to each listed branch:

  ```json
  "stats":{"files":4,"generated_files":1,"additions":7,"deletions":1,
    "top_dirs":[{"dir":"src/api","lines":7},{"dir":"docs","lines":1}],
    "authors":[{"author":"Name <mail>","commits":3}],"first_time":1700000100,"last_time":1700000300,
    "issue_keys":["PROJ-7"],"diffstat":{"base":"3c0ffee1","files":4,"additions":6,"deletions":0}}
  ```

  - `files` counts the distinct files the commits change, and `generated_files` how many of them are lock or
    generated files (see `bd commits`).
  - `additions`, `deletions` and `top_dirs` (at most three directories, by added plus deleted lines) are summed over
    the commits. Generated files don't count without `--include-generated`.
  - `first_time` and `last_time` are author times. `issue_keys` are the distinct keys of the subjects.
  - `diffstat` is the net change of the synced virtual branch: its tip against `base`, the parent of the oldest
    commit ahead of the baseline, which a sync rebuilds every branch on. It is exact while the tip is synced with
    the current commits, and `null` for a branch that was never synced.
  - It costs at most three more git processes for all branches together: the commit diffs the cache misses (the same
    `-U0` pass and cache as `bd commits`), one `diff-tree --stdin` with a `<tip> <base>` line per synced branch, and
    one `check-attr`.
- `--brief` prints `StatusBrief` (`status_brief` in `bd schema`), small enough to read whole: about 2 KB for the 12
  branches and 74 commits of a large monorepo branch, where `bd status` prints 11.5 KB.
  - Per branch it lists `name`, `commit_count`, `issue_key` (left out when it is the `name`) and `newest`, the subject
    of the newest commit. With stacked branches on, it keeps `parent`, `blocked_by`, `problems` and `forest`.
  - `unassigned` lists the ids and subjects.
  - It reads what `bd status` reads, except the virtual branch tips and the descriptions.
  - It works with `--unassigned`, `--limit` and `--offset`, but not with `--branch` or `--stats`.

  ```json
  {"schema":1,…,"head":{"id":"ff366ae0","branch":"master"},"baseline":"origin/master","branch_count":2,
   "unassigned_count":1,"branches":[{"name":"auth","commit_count":2,"newest":"Add logout"},
    {"name":"ABC-12","commit_count":1,"newest":"ABC-12 Fix crash"}],
   "unassigned":[{"id":"77e01aa4","subject":"Tidy up"}]}
  ```

### `bd commits [<id>…] [--files] [--patch] [--branch <b> | --unassigned] [--include-generated] [--brief [--depth N]]`

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
- `--branch` keeps only the commits of that branch, and the edges and overlaps that involve it. `--unassigned` keeps
  the unassigned commits, and `<id>…` the commits with these ids: full, or abbreviated to at least 4 characters, as in
  a plan. An id that matches no commit ahead of the baseline is `not_ahead_of_baseline`, one that matches several is
  `invalid_arguments`. The three filters exclude each other. The dependencies are still computed over every commit;
  the overlaps kept are those of the branches the listed commits are in.
- `dependencies` are edges where `from` depends on `to`, an older commit, through `files`.
- `cluster_overlap` lists, for each pair of current branches, the files both change.
- Lock files and generated files don't count in `dependencies` and `cluster_overlap` (see below);
  `--include-generated` keeps them. `--files` still lists them, with `"generated": true`.
- `--brief` prints `CommitsBrief` (`commits_brief` in `bd schema`): what an agent needs to cluster, small enough to
  read whole. Measured on the 74 commits of a large monorepo branch, it is 19.5 KB, where `--files` is 278 KB.
  `--files` and `--patch` don't go with it; the filters do.

  ```json
  {"schema":1,…,"head":"77e01aa4","baseline":"origin/master",
   "commits":[{"id":"41c7d00e","subject":"Add login","group":"auth","areas":[{"dir":"src/auth","lines":12}]},
    {"id":"5e2f9c3b","subject":"fixup! Add login","group":"auth","fixup":{"kind":"fixup","target":"41c7d00e"},
     "areas":[{"dir":"src/auth","lines":2}]},
    {"id":"9f3a2b1c","subject":"ABC-1 Add parser","group":"ABC-1","areas":[{"dir":"src/parser","lines":30}]}],
   "dependencies":[{"from":"5e2f9c3b","to":"41c7d00e"}],
   "units":[["41c7d00e","5e2f9c3b"]]}
  ```

  - Per commit it lists `id`, `subject`, `group`, `issue_key`, `fixup` and `areas`, without the body, the author and
    the files. It leaves out what another field says: the subject drops a `(prefix)` that is exactly the `group`, and
    `issue_key` is left out when it is the `group`. A stack path, whose leaf is the `group`, and a prefix the grouper
    sanitizes (`(Foo Bar)` is in `Foo-Bar`) stay in the subject, so a plan made from the brief view keeps them.
  - `dependencies` are `{from, to}`, without the files.
  - `units` are the commits that must stay together: the connected components of the dependencies and of the fixups
    with their targets, over every commit. Only components of two or more commits are listed, ordered by their oldest
    commit, each oldest first. With a filter, the units that hold a listed commit are kept whole.

#### Areas

`areas` (`crates/bd-cli/src/areas.rs`) say where a commit belongs without its file list:

- The area of a path is its first `--depth` directories (3 by default), or all of them for a shorter path, and `.`
  for a file at the top level: `plugins/air/shared/src/Foo.kt` is in `plugins/air/shared`.
- Per commit, at most three areas with the most added plus deleted lines. An area under 5 % of the commit's lines is
  left out, unless it is the first. An area of renames or binary files only has 0 lines, so it shows only when the
  commit has no other lines.
- Lock files and generated files don't count without `--include-generated`. Hub files never count: changes in many
  unrelated areas touch them, so they say nothing about where a commit belongs. `HUB_FILES` in
  `crates/bd-cli/src/generated.rs` lists them: module and build registries, file lists and top-level docs. It matches
  by file name, and a pattern with a `/` (`.idea/modules.xml`) by the end of the path.
- `top_dirs` of `bd status --stats` keeps its meaning: the full directories of the files.

All changes, the file lists and the dependencies come from one `git log --no-walk=unsorted --stdin -z --raw -p -U0`
process, for the commits the object cache misses. `--patch` adds one more process, with `-U3`. Both pass explicit diff options (`-M`, `--no-ext-diff`,
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

### The object cache

The changes of a commit never change, so `bd commits` and `bd status --stats` keep them in a disk cache keyed by the
commit id (`crates/object-cache`, LMDB through heed):

- It lives in `$GIT_COMMON_DIR/branch-deck/cache/`, so all worktrees share it. The app and parallel `bd` calls can use
  it at once: LMDB serializes writers through its lock file, and readers never wait.
- git runs only for the ids the cache misses, in one batch process, and the results are written back in one
  transaction.
- The cap is the LMDB map size, 1 GiB by default. A write that finds the map full clears the cache and is retried.
- The schema version is part of each database name (`commit_diff.v1`). Opening drops the databases of older versions.
- Any cache failure is a miss, logged with `-v`; it never fails a command. `--no-cache` bypasses it.
- `bd check` memoizes its merges in `merge_result` ((base, commit, target) trees → result tree or conflict files) and
  commit trees in `commit_tree`. Sync doesn't use them yet.

### `bd group [--dry-run] [--expect-head <id>] [--map] [--plan <file>] < plan.json`

Moves commits between branches by their subject prefixes, in one message-only history rewrite
(`sync_core::regroup`, see "Regroup, rename, restack" in [Git Operations](../design/git-operations.md)).

```json
{"branches": {"auth": ["41c7d00e", "5e2f9c3b"], "auth/login": ["9f3a2b1c"]}, "unassign": ["77e01aa4"]}
```

- A path is a branch name, or `parent/leaf` with stacked branches on (`git config branchdeck.stacks true`). Ids may
  be abbreviated. `unassign` removes the prefix: the commit falls back to its issue branch or becomes unassigned.
- A commit without a prefix that the plan puts into the branch of its own issue key keeps its subject: it is in that
  branch already. So a plan that names the current branch of each commit (its stack path with stacked branches on, not
  the leaf) rewrites no commit that has no prefix or a canonical one. A prefix the grouper sanitizes, such as
  `(Foo Bar)` in `Foo-Bar`, is rewritten to the name.
- Before anything is written, the plan is applied in memory to every subject ahead of the baseline and validated:
  - unknown ids and ids not ahead of the baseline are `not_ahead_of_baseline`;
  - duplicate ids, ids both in a branch and in `unassign`, invalid names, a fixup sent away from its target and a
    stack problem the plan introduces (a cycle, conflicting parents, a self parent, a case collision) are
    `invalid_plan`; problems that already existed are listed in `problems`.
- Fixups not in the plan follow their targets.
- `--expect-head` refuses with `head_moved` (exit 3) unless HEAD is still that commit; the rewrite itself only moves
  the branch from the HEAD it read (compare-and-swap).
- The repository lock serializes rewrites with the app and other `bd` calls; after 10 s of waiting it is `locked`
  (exit 3). A rebase, merge, cherry-pick or revert in progress is `operation_in_progress`, a detached HEAD
  `detached_head` (exit 4).

```json
{"schema":1,"repository":"/repo","prefix":"develar","operation":"group","dry_run":false,
 "previous_head":"77e01aa4","new_head":"c0ffee12","rewritten":3,"undo_token":"<full previous>..<full new>",
 "groups":{"auth":["1b2c3d4e","5f6a7b8c"],"login":["9d0e1f2a"]},"unassigned":["c0ffee12"]}
```

- `groups` and `unassigned` list every commit ahead of the baseline afterwards, with the new ids, oldest first.
- `--dry-run` writes nothing and adds `changes` (`{id, from, to}` subjects) and, with stacked branches on, `forest`
  (`{name, parent, has_commits, problems}` per branch). `--map` adds `map`, old → new id of every commit.

### `bd rename <old> <new>`, `bd restack <branch> <parent>`, `bd unstack <branch>`

Plans that `bd` computes, validated and written like `bd group`, with the same options and output:

- `rename` replaces the level `old` in every prefix path, so a parent's children follow; an issue branch's commits
  get the prefix `(new)`. The new name must not contain `/`. A name that exists merges the branches.
- `restack` sets the prefix of every commit of the branch to `(parent/branch)`, `unstack` to `(branch)`. Both need
  stacked branches on.

### `bd undo <undo_token>`

Moves the branch back from the token's new HEAD to its previous HEAD, with CAS and the lock: `head_moved` once
anything else changed the branch. Prints `{"schema":1,…,"head":"<restored>","undone":"<undone>"}`.

### `bd check [--plan FILE | < plan.json]`

A trial sync of a plan (`sync_core::check`), or of the current grouping without one. The plan is applied in memory
and validated as `bd group --dry-run` does (an invalid plan is exit 2), the branches are placed as a stacked sync
places them, and each one is built with the sync's `build_branch`, parents first. It writes commit objects only: no
ref, note, HEAD or working tree changes, no remote status and no repository lock.

```json
{"schema":1,…,"head":"77e01aa4","baseline":"origin/master","base_commit":"3c0ffee1","rewritten":1,"ok":false,
 "branches":[{"name":"a","built_on":null,"commit_count":1,"result":"ok"},
  {"name":"b","built_on":null,"commit_count":1,"result":"conflict",
   "conflict":{"commit":"5e2f9c3b","subject":"(b) Change shared","files":["shared.txt"],"missing_commits":["41c7d00e"]}}],
 "unassigned_count":0}
```

- `result` is `ok`, `conflict`, `blocked` (with `blocked_by` `{parent, reason}`: the parent conflicts, is blocked,
  missing or unapplied), `problems` or `error`. `built_on` is the branch it would be built on, `null` for
  `base_commit`.
- `missing_commits` are the older commits outside the branch and the branches it is built on that change the
  conflicting files, computed from the cached commit diffs: move them into the branch, or stack the branch on the
  one that has them.
- Conflicts are light: the files only, without the contents and the analysis a conflict view needs.
- Merges are memoized in the object cache by `(parent tree, commit tree, target tree)` (`merge_result`), and commit
  trees by commit (`commit_tree`): trying variants of a plan repeats no `merge-tree`. On the grouped 169-commit clone,
  a check takes 3.4 s cold and 0.2 s warm.
- The exit code is 1 when any branch is not `ok`.

### `bd suggest [--fresh] [--max-commits N] [--rewrite-issues]`

Suggests a grouping of the commits ahead of the baseline into branches, as a plan for `bd group`, and checks the plan
with the trial sync of `bd check`. It writes git objects and the cache only: no ref, config, index or working tree
changes, and no repository lock.

It reads the commits once with a `CheckSession` (`sync_core::check`: the snapshot of a regroup, the baseline and the
branch tips), their changes from the object cache without lock files, generated files and hub files
(`read_linked_diffs`), the dependencies of `bd commits` and the fixup targets. The clustering
(`crates/bd-cli/src/clustering.rs`) runs no git:

1. Hard units, joined with union-find: the dependency edges, a fixup and its target, the commits with the same
   `(prefix)` (an anchor), and the commits without a prefix that have the same issue key. `--fresh` ignores the
   prefixes. A dependency between two anchors joins nothing: it is an `anchor_dependency` warning.
2. A score between two clusters, `S = max(R, P × Kp) × K + adj`, in integer units of 1/10 000 so that ties are exact:
   - R, the links: a body names an issue key of the other side (0.9); both sides name a third key (0.7), unless more
     than max(10, units / 3) units name it; the same conventional scope (0.8 without issue keys, 0.5 with them). A
     scope is read from `type(scope):` and from `type scope:`, as in `setup vm:` (`parse_semantic_prefix`).
   - P, the paths: the IDF-weighted Jaccard similarity of the areas (the first 3 directories, weight 0.5), the modules
     (the directories before the first `src`, `testSrc`, `test`, `tests`, `resources` or `gen`), the packages (the
     directories after it) and the files. A feature of more than max(4, units / 4) units weighs 0. A file under 5 % of
     the commit's lines and outside its main area weighs 0.2.
   - K: 0.3 when the issue keys of the two sides are of different projects (`ABC-1` and `XYZ-2`), 0.5 when both sides
     have scopes and none in common. Kp: 0.7 when the keys differ in the same project.
   - adj: 0.05 when the two sides have neighbouring commits in the log and R or P is not 0.
3. Merges, the best pair first and on a tie the oldest, until the best score is under 0.40 (`MERGE_THRESHOLD`). Two
   anchors never merge, and no merge makes a cluster of more than `--max-commits` commits (default 40).

Each cluster then gets a name, the first of: its prefix; its single issue key; a scope of at least 60 % of its commits;
`<area leaf>-<word>`, for the subject word of at least half its subjects with the highest TF-IDF; its most frequent key;
`cluster-N`. When another cluster, a group now or an existing virtual branch has the name (in any case), and the
cluster's commits don't come from it, the name gets `-2`, `-3`, and so on.

The trial sync builds the plan. A conflict joins the conflicting commit to its `missing_commits` as hard edges, and the
clustering runs again, at most 5 times. A repair never joins two anchors, so a conflict between two prefixes stays.

```json
{"schema":1,…,"head":"77e01aa4","baseline":"origin/master",
 "plan":{"branches":{"vm":["41c7d00e","5e2f9c3b"]},"unassign":[]},
 "clusters":[{"name":"vm","commit_count":2,"change":"new","from":[],"why":"2 units; scope vm in 2 subjects; area vm",
   "weak":false,"issue_keys":[],"areas":[{"dir":"vm","lines":2}]},
  {"name":"parser-parse","commit_count":2,"change":"merged","from":["ABC-1","ABC-2"],
   "why":"2 issue units; ABC-1 named in 1 body; area parser","weak":false,"issue_keys":["ABC-1","ABC-2"],
   "areas":[{"dir":"parser","lines":2}],"via":"issue_group"}],
 "rewritten":2,"check":{"ok":true,"repair_rounds":0,"branches":[…]}}
```

- `plan` lists only the commits whose branch changes. A fixup that follows its target is left out, and a commit with a
  prefix that goes to the branch of its own issue key is in `unassign`. The commit ids appear only in `plan`. Give the
  plan to `bd group` as it is, or edit it and run `bd check`.
- Per cluster: `change` against the groups now in `from` (`kept`, `merged`, `split` or `new`), `why` (one line to
  quote to the user, with the repair rounds), `weak` (a merge by paths near the threshold, or one that adjacency
  decided: review these first), `issue_keys` and `areas` (as in `bd commits --brief`).
- A cluster of two or more whole issue branches stays out of the plan with `"via":"issue_group"`, because the plan
  would give each of its commits a prefix. `--rewrite-issues` puts it into the plan.
- `rewritten` counts the subjects the plan changes. `check` has the `bd check` result of each branch and the
  `repair_rounds`. `warnings` lists `anchor_dependency` and `unresolved_conflict`, with the branches.
- The exit code is 1 when a branch of the plan still conflicts, as for `bd check`.
- On the 73 commits and 11 branches of a large monorepo branch it keeps the 11 branches with an empty plan, in 0.6 s
  warm (a debug build). With `--fresh` it finds the grouping a human made there with a pairwise precision of 1.0 and
  a recall of 0.945; the golden test of `clustering_test.rs` checks its anonymized input (`--dump-input`, hidden, with
  every name hashed with a salt that is not kept).

### `bd sync`

Rebuilds the virtual branches with `sync_core::sync::sync_branches`, the app's sync: it includes the integration
detection of archived branches and the update check of the checked out branch. It holds the repository lock.

```json
{"schema":1,"repository":"/repo","prefix":"develar","baseline":"origin/master","unassigned_count":3,
 "branches":[{"name":"auth","ref":"refs/heads/develar/virtual/auth","status":"created","commit_count":2,
   "tip":"1b2c3d4e","remote":{"exists":false,"unpushed":2,"behind":0,"head":null}},
  {"name":"login","ref":"…","status":"merge_conflict","commit_count":1,"tip":null,
   "conflict":{"commit":"9d0e1f2a","subject":"(login) Use the session","files":["src/session.rs"],
     "missing_commits":["5f6a7b8c"]}}],
 "archived":[{"name":"old-fix","status":"integrated","integrated_at":1700000000}],
 "update":{"upstream":"origin/master","state":"up_to_date"}}
```

- `status` is `created`, `updated`, `unchanged`, `blocked` (a stacked branch whose parent can't be built on, with the
  reason in `error`), `error` (with `error`) or `merge_conflict` (with `conflict`: the
  commit whose cherry-pick conflicts, the files, and the commits of other branches the change likely needs).
  `blocked` lists the commits not synced after an earlier failure.
- `remote` compares with the remote-tracking branch of `origin` by content: `unpushed` commits it lacks, `behind`
  commits it has that the branch doesn't (a push drops them), and `head`, the remote commit the counts compare with
  (`null` without a remote branch). `dropped_commits` lists the newest ten of the `behind` commits, newest first:
  `{"commit","subject","author","author_time"}`.
- `update.state` is `up_to_date`, `fast_forward`, `rebase`, `conflict` or `blocked` (with `files`), or `error`.
- `update.landing` lists the local commits the fetched baseline already has (the update drops them), and `landed` what
  landed with an update and the app hasn't dismissed: per branch (omitted for unassigned commits) the commits with
  `commit`, `subject` and the baseline commit `upstream` (omitted when squashed upstream), and `remaining`, the commits
  that stay local.
- The exit code is 1 when any branch has a conflict; the summary is printed either way.

### `bd push <branch>… | --all --yes [--force] [--expect-remote COMMIT] [--remote R] [--dry-run]`

Syncs, then pushes the branches in one atomic `git push --porcelain` with a lease per ref, parents first, under the
repository lock (see "Agent push" in [Git Operations](../design/git-operations.md)).

```json
{"schema":1,…,"remote":"origin","dry_run":false,
 "branches":[{"name":"auth","remote_branch":"develar/virtual/auth","commit":"1b2c3d4e","status":"new",
   "lease":null,"lease_source":"absent",
   "pull_request":{"base":"master","head":"develar/virtual/auth","title":"Add login","body":"The login form.",
     "command":"gh pr create --base master --head develar/virtual/auth --title 'Add login' --body-file -"}}]}
```

- Refused with `push_refused` (exit 4), nothing pushed: a branch that didn't sync, one whose remote branch has
  commits it lacks (`behind`) without `--force`, and a stacked branch whose parent isn't on the remote or pushed with
  it. `--all` needs `--yes`. A branch refused as behind is in `error.branches` with its remote status, as below.
- `status` is `new`, `updated`, `forced` or `up_to_date`. The lease is the remote head the sync of the same run
  compared with (`remote.head`, `lease_source: expected`), or none without a remote branch (`absent`: it must not
  exist). The push replaces exactly that remote state: a remote commit the branch has by content goes through,
  `--force` drops the `behind` commits of that sync, and nothing pushed since is lost. `--expect-remote COMMIT` leases
  one branch on the given remote commit, a `remote.head` bd printed: the remote state the user saw.
- With `--remote` other than `origin`, which the sync compares with, bd computes the same remote status of each
  pushed branch against that remote's remote-tracking branch, and the refusal, the lease and the errors use it. A
  `--remote` that is not a configured remote is `no_remote`.
- A remote branch that is not where the lease expects it is `lease_rejected` (exit 3), nothing pushed. bd
  fetches the rejected branches and lists them in `error.branches` with their remote status as fetched:
  `{"name":"auth","remote":{"exists":true,"unpushed":1,"behind":1,"head":"7c8d9e0f","dropped_commits":[…]}}`.
  Show the `dropped_commits` to the user; only after the user
  approves, `bd push auth --force --expect-remote 7c8d9e0f`.
- `pull_request.base` is the parent's remote branch, or the baseline without the remote (`master`). With a
  `bd describe` text, `command` takes the title and reads the body from stdin (`--body-file -`); without one it uses
  `--fill`.

### `bd describe <branch> [--title T] [--body B | --body-file F] [--clear]`

Prints or sets the pull request text of a branch in git's own `branch.<prefix>/virtual/<branch>.description`, stored
as `title`, a blank line and the body, as `git branch --edit-description` writes it. Without options it prints
`{…,"branch","ref","description":{"title","body"}|null,"changed":false}`. `--title` or `--body` replace only that part;
`--body-file -` reads stdin; `--clear` removes it. The branch must be a current group.

### `bd schema`

Prints `{"schema":1,"outputs":{"check":…,"commits":…,"commits_brief":…,"describe":…,"error":…,"install":…,"push":…,"rewrite":…,"status":…,"status_brief":…,"sync":…,"undo":…,"version":…}}`;
`rewrite` is the output of `group`, `rename`, `restack` and `unstack`, and `commits_brief` and `status_brief` the
outputs of `--brief`.

### `bd version`, `bd --version`

Both print `{"schema":1,"version":"0.11.0","git":"2.56.0"}` and need no repository. `version` is the version of
Branch Deck, the one the app shows: `build.rs` of `crates/bd-cli` reads it from `src-tauri/tauri.conf.json`, which
release-please updates. The workspace version of the crates stays 0.1.0, so `CARGO_PKG_VERSION` is not it. `git` is
the version of the git bd finds (see Invocation), also of one older than 2.54, and `null` without one: never an
error. The first line of `bd help agent` names the version too.

### `bd help agent`

Prints the workflow below as plain text.

### `bd install [--uninstall]`, `bd help skill`

How an agent learns that bd exists. `bd install`, or Help → Install Agent CLI… in the app
(`src-tauri/src/commands/agent_cli.rs`, the same code), writes:

- on macOS and Linux a `bd` symlink in `~/.local/bin` to the app binary, which runs the CLI under that name. A symlink
  there is replaced; a file that is not a symlink is `install_refused` (exit 4). On Windows `bd.exe` is next to the app
  already, and a symlink would need developer mode;
- the skill `branch-deck` (`SKILL.md`, the Agent Skills format) into `~/.claude/skills/branch-deck/` (Claude Code) and
  `~/.agents/skills/branch-deck/` (Codex and others). Its description tells an agent when to use bd: grouping or
  splitting commits into branches or pull requests, stacked branches, Branch Deck. Its body names bd by its full path,
  so it works without `PATH`, and sends the agent to `bd help agent` first. The workflow stays in the binary, so the
  skill can't fall out of date with it.

It prints `{"schema":1,"bd":"/Users/me/.local/bin/bd","on_path":true,"skills":[…],"uninstalled":false}`. `on_path`
says whether `bd` on `PATH` is this one; the app asks the user's login shell for `PATH`, since an app started from the
Finder has a minimal one. `--uninstall` removes the skills and the symlink if it points at this app (or at a removed
`branch-deck`). `bd help skill` prints the skill as text, for an agent that reads skills from elsewhere.

## Agent workflow

1. Run `bd suggest` for a checked plan to start from (`--fresh` ignores the prefixes). Run `bd status --brief`, then
   `bd commits --brief`: its `units` are the commits that must stay together. For more,
   run `bd status`, `bd commits --files`, and `bd commits --patch <id>…` (or `--branch <b>`, `--unassigned`) for the
   commits that are unclear.
2. Cluster the commits by intent. Use `dependencies` and `cluster_overlap` to keep together, or stack, the commits that
   can't be separated.
3. Write the plan and run `bd group --dry-run < plan.json` until it is accepted, then `bd check < plan.json` until
   every branch is `ok`: move the `missing_commits` of a conflict into the branch, or stack the branch on the one
   that has them.
4. Run `bd group --expect-head <head> < plan.json` with the HEAD `bd status` printed. On `head_moved` or `locked`,
   read again and retry. Then run `bd sync`; on a conflict (exit 1) regroup the commits the change needs into the same
   branch or its parent, and sync again.
5. Run `bd describe <branch> --title … --body …` for each branch.
6. Show the result to the user, and run `bd push <branches>` only after the user approves; then the `gh pr create`
   lines it prints, piping each `body` into its command.

The agent never reorders, squashes or drops commits: a plan only moves commits between branches. These are the same
rules as `src/lib/agent-prompt.ts`. `bd undo` with the printed token reverts a rewrite.
