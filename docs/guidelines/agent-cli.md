# Agent CLI (`bd`)

**Status:** Active  
**Updated:** 2026-10-15  
**Tags:** [cli, agents, json]

## Summary

`bd` lets an agent read what Branch Deck sees (the commits ahead of the baseline, their groups, and what ties them
together), regroup the commits and sync them. It reads with `status`, `commits`, `schema` and `version`, suggests a
checked grouping with `suggest`, rewrites commit messages with `group`, `rename`, `restack`, `unstack` and `undo`,
rebuilds the virtual branches with `sync` and keeps their pull request text with `describe`, tries a plan without
writing refs with `check` and pushes with `push`. `group --sync` applies a plan, its pull request texts and a sync in
one call. `update` brings the checked out branch onto its baseline as the app's Update Project does, and resolves a
conflict in a resolve session outside the working tree; `fold` folds the fixup commits.

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
  app, the CLI never asks a login shell for git. Without one, the error is `git_not_found`: a line "Cannot find Git:
  <reason>" or "Git 2.39 is too old: Branch Deck requires Git 2.54 or newer", then how to install Git.

## The JSON contract

- stdout gets exactly one JSON object per run, on one line. Only `bd help` prints text.
- The JSON is ASCII: every other character is a `\uXXXX` escape, so a shell that decodes stdout with a legacy code
  page (Windows PowerShell 5.1) still reads every path and subject. A plan or a body that bd reads (`--plan`, stdin,
  `--body-file`; `-` is stdin) is UTF-8, with or without a byte order mark, or UTF-16 with one. A pipe of Windows
  PowerShell 5.1 turns other characters into `?` before bd reads them, so pass a file there.
- Every object starts with `"schema": 1`. The version goes up only when a field changes meaning or disappears. New
  optional fields don't change it.
- Optional fields are left out when they are empty. `group` is the exception: it is always present and is `null` for
  an unassigned commit. In a `--brief` view a field is also left out when another field says it: `issue_key` when it
  is the `group` (`bd commits --brief`) or the `name` (`bd status --brief`) of an issue branch, and the `(prefix)` of a
  subject when it is exactly the `group`.
- `bd schema` prints the JSON Schema of each output (see `bd schema` below). The schemas are generated from the Rust
  types with `schemars`, and `schema_test.rs` checks the real outputs against them.
- Read only: `status`, `commits`, `suggest`, `check`, `schema`, `version`, `update --dry-run`, `update --status` and
  `fold --dry-run` write no ref, config, index or working tree. They write at most git objects (the trial sync of `check` and
  `suggest`, the replay of `update --dry-run`) and the object cache (none with `--no-cache`). `bd --help` and
  `bd help agent` print the same line (`READ_ONLY` in `agent_help.rs`), and `agent_help_test.rs` checks the refs, the
  config, the index and the working tree around each of them.
- The working tree: only `bd update` and its undo write the working tree, through the guarded update of the app's
  Update Project: only the files the update changes, and in a resolve session only at the finish.

An error looks like this:

```json
{"schema":1,"error":{"code":"no_branch_prefix","message":"No branch prefix: ...","commits":[],"retryable":false}}
```

`uncommitted_overlap` also has `paths`, the local files that block an update or its undo; a refused resolution of
`bd update --continue` has `paths` too, the conflicting files:

```json
{"schema":1,"error":{"code":"uncommitted_overlap","message":"…","commits":[],"paths":["src/app.txt"],"retryable":false}}
```

`approvals_missing` of `bd safe-push` has `review`, the code-owner review the Safe Push lacks (see `bd safe-push`).

| Exit | Meaning | Codes |
|---|---|---|
| 0 | ok | |
| 1 | conflict | `conflict` |
| 2 | invalid input | `invalid_arguments`, `not_a_repository`, `no_branch_prefix`, `unknown_branch`, `invalid_plan`, `not_ahead_of_baseline`, `no_session`, `unresolved_markers`, `resolution_outside_conflict`, `missing_resolution`, `empty_resolution`, `round_moved` |
| 3 | stale, read again and retry | `head_moved`, `groups_moved`, `stacks_moved`, `lease_rejected`, `locked` |
| 4 | refused | `no_baseline`, `operation_in_progress`, `detached_head`, `staged_overlap`, `uncommitted_overlap`, `undo_stale`, `session_stale`, `unsupported_conflict`, `finish_pending`, `push_refused`, `no_remote`, `install_refused`, `approvals_missing`, `robot_running`, `stack_not_landed`, `not_on_space`, `not_logged_in`, `patronus_refused` |
| 5 | git or network error | `git_not_found`, `git_error`, `network_error`, `patronus_error` |

`no_baseline` is also a repository without commits (`git init`): every command that reads HEAD or the baseline says
"The repository has no commits yet: commit to the main branch, then sync".

Agents branch on the code and the exit code, never on the message. `commits` names the commits an error is about.
`retryable` is true for `head_moved`, `groups_moved`, `stacks_moved`, `lease_rejected`, `locked` and `network_error`.

One output carries an `error` next to its other fields: a rewrite (`bd group` and the others) whose step after the
rewrite failed. The rewrite stands, and its `undo_token` reverts it (see `bd group`).

## Commands

### `bd status [--branch <b>] [--unassigned] [--limit N] [--offset M] [--stats [--include-generated]] [--brief]`

Prints HEAD, the baseline (`detect_baseline_branch`), the prefix, the branches and the unassigned commits. Without a
baseline (no upstream and no `origin/master` or `origin/main`, or no remote and no local `master` or `main`) it is
`no_baseline`. It groups
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
- `origin` says what groups the commits: `prefix` (a commit has the `(prefix)`), `issue_group` (the issue groups put
  commits of other keys into it) or `issue`. `issue_keys` lists the keys whose issue group the branch is.
- `idle_issue_groups` lists the keys of the issue groups that group no commit ahead of the baseline on this branch, and
  `issue_group_problems` (`{line, message}`) the lines of the store that are not used; both are left out when empty.
  The map is shared by every branch and worktree, so an idle key may group the commits of another one: leave it,
  unless you know that no branch needs it (then `ungroup_issues`).
- With stacked branches on, `parent_source` says where `parent` comes from: `prefix` (a parent level of a prefix) or
  `store` (the stack parents, `refs/branch-deck/stack-parents`, see [Stack Parents](../../spec/branch/stack-parents.spec.md)).
  `idle_stack_parents` lists the children of the stack parents that stack no branch ahead of the baseline here, and
  `stack_parent_problems` the lines of that store that are not used. An idle child may stack a branch of another
  worktree; a sync drops its line once its branch is integrated, `bd group` with `unstack` drops it at once.
- `--branch` lists that one branch and `--unassigned` lists only the unassigned commits. `--limit`/`--offset` page
  the commits in both of these modes, and page the branches otherwise.
- With stacked branches on (`git config branchdeck.stacks true`), each branch is placed as a sync places it:
  - `parent`, the parent its commits declare, and `parent_state`: `active` (it has commits), `integrated` (its branch
    is integrated), `unapplied` or `missing`. A parent without commits, in any of these states, doesn't block: the
    branch goes on the nearest active ancestor or the baseline and has `pr_base_changed: true` (its pull request base
    is not the declared parent). A parent whose commits left HEAD since the last sync is detected now, as the next
    sync detects it, without writing the detection note;
  - `effective_base`, the branch it is built on (omitted for the baseline), or `blocked_by` `{parent, reason}` when the
    parent can't be built: `error` (a stack problem or a failed build), `conflict` or `blocked`;
  - `problems` (`{message, commits}`): conflicting parents, a cycle, a self parent, a case collision, an empty level.

  The output has `forest`: the branches with commits, each after its parent.
- `description` is `{title}` of the branch's `bd describe` text, read for all branches with one
  `git config --get-regexp`.
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
    of the newest commit, and `issue_keys` of an issue group. With stacked branches on, it keeps `parent`,
    `parent_source`, `blocked_by`, `pr_base_changed`, `problems` and `forest`. It keeps `idle_issue_groups`,
    `issue_group_problems`, `idle_stack_parents` and `stack_parent_problems`.
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

This is in `crates/sync-core/src/dependencies.rs`.

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

### `bd group [--dry-run | --sync] [--expect-head <id> [--allow-append]] [--map] [--plan <file>] < plan.json`

Moves commits between branches by their subject prefixes, in one message-only history rewrite
(`sync_core::regroup`, see "Regroup, rename, restack" in [Git Operations](../design/git-operations.md)), and changes
the issue groups and the stack parents in the same ref transaction. Then it sets the pull request texts of the plan and, with `--sync`,
rebuilds the virtual branches, under the same repository lock.

```json
{"branches": {"auth": ["41c7d00e", "5e2f9c3b"], "auth/login": ["9f3a2b1c"]}, "unassign": ["77e01aa4"],
 "issue_groups": {"parser": ["ABC-1", "ABC-2"]}, "ungroup_issues": ["XYZ-7"], "unstack": ["docs"],
 "descriptions": {"auth": {"title": "Add login", "body": "The login form."}}}
```

- A path is a branch name, or `parent/leaf` with stacked branches on (`git config branchdeck.stacks true`). Ids may
  be abbreviated. `unassign` removes the prefix: the commit falls back to its issue branch or becomes unassigned.
- A path `parent/leaf` stacks `leaf` on `parent` in the stack parents (`refs/branch-deck/stack-parents`), not in the
  subjects: a commit that needs a prefix gets the bare `(leaf)`, and a commit already in `leaf` by its issue key or
  issue group keeps its subject. So stacking whole issue branches (`{"IJAI-3992/IJAI-4047": [ids]}`, or an empty list)
  rewrites nothing. A commit whose prefix declares another parent for a branch the plan stacks loses that level.
  `unstack` makes branches roots: their line goes, and so do the parent levels of their commits; a bare path never
  unstacks.
- A commit without a prefix that the plan puts into the branch of its own issue key keeps its subject: it is in that
  branch already. So a plan that names the current branch of each commit (its stack path with stacked branches on, not
  the leaf) rewrites no commit that has no prefix or a canonical one. A prefix the grouper sanitizes, such as
  `(Foo Bar)` in `Foo-Bar`, is rewritten to the name.
- `issue_groups` maps an issue group name (a branch name without `/`) to issue keys: the commits without a prefix that
  carry one of the keys go into that branch, and no subject changes. `ungroup_issues` removes the issue group of a
  key, so its commits go back to the issue branch of the key. The map lives in `refs/branch-deck/issue-groups`; see
  [Issue Groups](../../spec/branch/issue-groups.spec.md). A plan of issue groups only rewrites nothing
  (`rewritten` 0) and moves only that ref, still with `--expect-head` and a check that HEAD did not move.
- Before anything is written, the plan is applied in memory to every subject ahead of the baseline and validated,
  with the issue groups as they are after the plan:
  - unknown ids and ids not ahead of the baseline are `not_ahead_of_baseline`;
  - duplicate ids, ids both in a branch and in `unassign`, invalid names, a fixup sent away from its target and a
    stack problem the plan introduces (a cycle, conflicting parents, a self parent, a case collision) are
    `invalid_plan`; problems that already existed are listed in `problems`;
  - a key in two issue groups, or in an issue group and in `ungroup_issues`, a key to ungroup that is in none (and in
    no line of the store), an invalid key or name, a key that no commit without a prefix carries, and a fixup the issue
    groups would separate from its target (`fixup! <id>` has no issue key) are `invalid_plan` too;
  - a path whose leaf is its parent, two paths that give a branch two parents, a branch both in a path and in
    `unstack`, and an `unstack` of a branch that is not stacked (or with stacked branches off) are `invalid_plan`.
- `descriptions` maps a branch (the group name, as for `bd describe`: the leaf of a stacked branch) to its pull
  request text `{title, body}`. bd
  validates it with the plan: a title of one line that is not empty, and a branch that has commits after the plan (a
  prefix branch, an issue branch or an issue group); else `invalid_plan`, and nothing is written. After the ref
  transaction bd writes the texts that differ from the current ones, as `bd describe` does: one `git config` process
  per branch, because git has no batch writer for config. A plan of descriptions only rewrites nothing, and
  `--expect-head` stays optional, as for any plan.
- Fixups not in the plan follow their targets.
- `--expect-head` refuses with `head_moved` (exit 3) unless HEAD is still that commit; the rewrite itself only moves
  the branch from the HEAD it read (compare-and-swap). With `--allow-append` a HEAD with commits on top of it is
  fine while that commit is still ahead of the baseline (a peer appended, nothing the plan was made for changed): the
  commits on top are regrouped with the rest and listed in `appended` (`{id, subject}`, new ids). `rename`, `restack`
  and `unstack` work by name, so they rename the prefix of an appended commit too.
- The repository lock serializes rewrites with the app and other `bd` calls; after 10 s of waiting it is `locked`
  (exit 3). A rebase, merge, cherry-pick or revert in progress is `operation_in_progress`, a detached HEAD
  `detached_head` (exit 4).

```json
{"schema":1,"repository":"/repo","prefix":"develar","operation":"group","dry_run":false,
 "previous_head":"77e01aa4","new_head":"c0ffee12","rewritten":3,"undo_token":"<full previous>..<full new>",
 "groups":{"auth":["1b2c3d4e","5f6a7b8c"],"login":["9d0e1f2a"]},"unassigned":["c0ffee12"]}
```

- `groups` and `unassigned` list every commit ahead of the baseline afterwards, with the new ids, oldest first.
- `issue_groups` (only when they change) lists every issue group afterwards with its keys. A write changes only the
  keys the plan names; the keys of other branches and the lines bd can't read stay.
- `stack_parents` (only when they change) lists every stack parent afterwards, child -> parent, those of other
  worktrees included.
- `undo_token` holds full ids, the parts of the refs that moved joined by `+` in this order: `<previous head>..<new
  head>` for a rewrite, `groups:<previous>..<new>` (commits of the issue groups) and `stacks:<previous>..<new>`
  (commits of the stack parents). A previous store commit is zeros when there was no store before. Pass the token to
  `bd undo` as printed.
- `--dry-run` writes nothing and adds `changes` (`{id, from, to}` subjects) and, with stacked branches on, `forest`
  (`{name, parent, parent_source, has_commits, problems}` per branch). `--map` adds `map`, old → new id of every commit.
- `described` lists the branches whose description the plan wrote (with `--dry-run`: would write), and
  `replaced_descriptions` the texts they replaced, by branch. `bd undo` doesn't restore a description; `bd describe`
  can, with the replaced text.
- `--sync` runs `bd sync` after the writes, without a gap in the lock, so no other rewrite comes between them, and
  adds its output as `sync`. The exit code is 1 when a branch conflicts, as for `bd sync`. `--sync` doesn't go with
  `--dry-run` (`invalid_arguments`): a dry run writes nothing, and `bd check` is the trial sync of a plan.
- `error` (`{code, message, commits, retryable}`, as in an error output) is set when a step after the rewrite fails:
  a description write, which also skips the sync and leaves the later descriptions unwritten, or the sync. The
  rewrite stands, `undo_token` reverts it, and the exit code is that error's.
- `bd undo` moves back only HEAD, the issue groups and the stack parents: the descriptions stay, and the virtual
  branches stay as the sync built them until the next `bd sync`.

### `bd rename <old> <new>`, `bd restack <branch> <parent>`, `bd unstack <branch>`

Plans that `bd` computes, validated and written like `bd group`, with the same options (`--sync` too) and output:

- `rename` replaces the level `old` in every prefix path, so a parent's children follow, renames the issue groups
  named `old` and names an issue branch by the issue groups, without a rewrite, and renames `old` in the stack parents
  (as a child, and as the parent of a branch with commits here). The new name must not contain `/`. A name that exists
  merges the branches.
- `restack` writes the stack parent of the branch, `unstack` removes it; both rewrite only the commits whose prefix
  declares another parent. An issue branch and an issue group are stacked like any branch, without a rewrite. Both
  need stacked branches on; `unstack` of a root is `invalid_plan`.

### `bd undo <undo_token>`

Moves the branch back from the token's new HEAD to its previous HEAD, and the issue groups and the stack parents back
from the token's new commit to its previous one (or away), in one transaction with CAS on each, under the lock:
`head_moved` once anything else changed the branch, `groups_moved` or `stacks_moved` (exit 3) once anything else
changed that store. Give the token as it was printed: a token with abbreviated ids, or whose store commit is not a
write on its previous one, is `invalid_arguments`. Prints
`{"schema":1,…,"head":"<restored>","undone":"<undone>","undone_issue_groups":"<undone>","undone_stack_parents":"<undone>"}`;
each `undone…` is left out for a token without that part.

An `update:<previous>..<new>` token of `bd update` moves the branch back and the files with it (see `bd update`).

### `bd update [--no-fetch] [--dry-run] [--dir <path>]`

The app's Update Project (`sync_core::update_project::update_project_with`, "Update project" in
[Git Operations](../design/git-operations.md)): it fetches the baseline's remote branch, then takes the repository
lock, checks again and applies a clean update. The fetch runs before the lock, so a slow fetch blocks no other writer.

```json
{"schema":1,…,"branch":"master","upstream":"origin/master","state":"rebase","dry_run":false,"fetched_at":1760500000,
 "previous_head":"77e01aa4","head":"c0ffee12","upstream_head":"5f6a7b8c","replayed":2,"dropped_already_upstream":1,
 "undo_token":"update:<full previous>..<full new>",
 "landed":[{"branch":"auth","commits":[{"commit":"1b2c3d4e","subject":"Add login","upstream":"9d0e1f2a"}],"remaining":0}]}
```

- `state` is `up_to_date`, `fast_forward` or `rebase`; `replayed`, `dropped_already_upstream` and `undo_token` come
  only with a change. `landed` lists the local commits the baseline has, which the update dropped.
- A conflict starts a resolve session (below): `state` `conflict`, exit 1. With `--dry-run`, nothing starts: `conflict`
  lists the likely conflicting `files`, the local `commits` and the `upstream_commits` that change them (`commit`,
  `subject`, `author`, `files`).
- `uncommitted_overlap` (exit 4): uncommitted, staged or untracked files sit on paths the update writes; `paths`
  lists them. bd never stashes, resets or commits them: the agent asks the user, then runs `bd update` again.
- `--no-fetch` updates onto the last fetched baseline. `--dry-run` implies it, takes no lock and prints what the
  update would do; it writes at most git objects (the replay).
- A failed fetch is `network_error` (exit 5), a held lock `locked` (exit 3), and `operation_in_progress`,
  `detached_head` and `no_baseline` (a local baseline) refuse (exit 4).
- `bd undo <undo_token>` takes the update back: the branch moves back, and the files follow through the same
  guarded update with the trees swapped, under the lock. Only the last update of the branch can be undone (its
  reflog entry proves it; the update writes it with `--create-reflog`), with full ids. Local changes on a path it
  writes are `uncommitted_overlap`; a branch moved, or a HEAD switched, since is `undo_stale` (exit 4: a retry can't
  succeed). The landed report is the one from before the update again (`landed_restored`). A repeated undo exits 0
  with `already_undone: true` and changes nothing.
- The move updates `HEAD` and commits only while git holds its lock, after a check that HEAD is still on the checked
  branch; a `git switch` meanwhile is `head_moved` with nothing changed (see "Ref moves" in
  [Git Operations](../design/git-operations.md)). A lock file of git that is in the way (`HEAD.lock`, or one that a killed
  git left) is `locked` with its path.
- A `bd update` or `bd undo` killed after the move and before the files followed leaves the mark
  `refs/branch-deck/update-pending/<worktree id>`. The next `bd update`, `bd undo` or Update Project lets the files
  follow first; `bd update --status` shows it as `pending_update`. When the branch moved since, `finish_pending`
  (exit 4) names the command that removes the mark.

### The resolve session of `bd update`

`bd help update` prints this workflow for an agent; the contract is [bd update](../../spec/cli/update.spec.md), the
design "Resolve session" in [Git Operations](../design/git-operations.md). The session replays the local commits
outside the working tree (`sync_core::resolve_session`, `git_ops::update_resolve`) and opens one round per
conflicting commit. It belongs to the worktree and the branch, keeps the baseline it started with, and lives in the
ref `refs/branch-deck/resolve/<worktree id>`. Peers' commits on top of HEAD join it as more rounds. `bd update` with a
session open prints the open round again, fetches nothing and writes the missing copies.

```json
{"schema":1,…,"state":"conflict","session":{"worktree":"main","status":"resolving","round":1,"replayed":0,"total":1,
 "dir":"/repo/.git/branch-deck/resolve",
 "commit":{"commit":"d88f5726","subject":"(auth) Change line 2","author":"Me","patch":"/repo/.git/branch-deck/resolve/r1/commit.patch"},
 "files":[{"path":"shared.txt","kind":"content","edit":"/repo/.git/branch-deck/resolve/r1/edit/1-shared.txt",
   "before":"…/r1/before/1-shared.txt","onto":"…/r1/onto/1-shared.txt","commit":"…/r1/commit/1-shared.txt",
   "markers":1,"hint":"…","upstream_commits":[{"commit":"c9631487","subject":"Change line 2 upstream","author":"Dana Lee"}]}],
 "blocked_paths":[],"marker_labels":{"onto":"c9631487…","before":"d88f5726…^","commit":"d88f5726…"},
 "next":"/Users/me/.local/bin/bd -C /repo update --continue --round 1"}}
```

- The sides are named by content: `edit` is the merge result with zdiff3 markers, the file to edit; `before` the
  commit's parent; `onto` the new parent (the baseline and the commits replayed so far); `commit` the commit's version;
  `patch` the whole commit. Edit only the files at `edit`, never the same path in the working tree. Write a copy as
  UTF-8 without a byte order mark: in Windows PowerShell with `[IO.File]::WriteAllText(<edit>, $text)`, because
  `Set-Content` and `Out-File` add one or write UTF-16. On Windows the paths have backslashes, and the `\\?\` form
  from 260 characters on. A change that the
  commit needs outside the blocks, or in another file, goes into a `fixup! <subject>` commit after the update, folded
  with `bd fold`.
- bd finds a block by the labels it wrote after the markers (`marker_labels`, the full ids, followed by `:<path>` for a
  renamed file), at any marker length (`conflict-marker-size`); a `=======` outside the blocks, e.g. under a Markdown
  heading, is text.
- `blocked_paths` lists local changes on paths the update likely writes: they block the finish, so ask the user about
  them before you resolve. `next` is the command that continues, with this binary and `-C <repository>`.
- `kind` is `content`, `modify_delete` or `rename_delete` (with `renamed_from`; `--take` and `--delete` take either
  path); a `hint` says how to resolve it. `commit.hint` "likely landed" says the baseline likely has the commit
  already: take `onto`. `landed_dropped` lists the local commits left out because the baseline has the same patch.
- A refusal names the round and its commit and changes nothing: the round stays open, and the copies keep your edits;
  `edited` per file says whether a copy differs from bd's merge result.
- `bd update --continue [--round N] [--take onto:<path>|commit:<path>]… [--delete <path>]… [--allow-empty |
  --drop-empty <commit>] [--dry-run]` commits the round: every file needs an edited `edit`, a `--take` or a
  `--delete`. Exit 1 is the next round; exit 0 is the finish (`state` `rebase`, the `undo_token`,
  `dropped_by_resolution`, and `resolutions`: per round the `commit`, the `resolved` commit, `empty` (`kept` or
  `dropped`) and per file `how` with `dropped_upstream_lines` and `dropped_commit_lines`, the lines of a side's change
  that the result lost, to report to the user). `--dry-run` checks and reports the resolution, `resolved` id included,
  and moves no ref.
- Exit 2 changes nothing: `missing_resolution`, `resolution_outside_conflict` (a line outside the blocks changed, or
  only the line ends, or a byte order mark or UTF-16), `unresolved_markers`, with `paths` and the copies in the message (and the paths changed in the
  working tree, where bd never reads); `no_session` without a session. `round_moved` (exit 2): another round is open,
  `bd update` prints it. `empty_resolution` (exit 2): the resolution changes nothing; ask the user, then
  `--allow-empty` keeps an empty commit and `--drop-empty <commit>` drops it (never counted as landed).
  `session_stale` (exit 4): the branch was rewritten or HEAD left it. `unsupported_conflict` (exit 4): binary,
  file/directory, rename, mode, symlink or submodule. `uncommitted_overlap` at the finish keeps the session `ready`
  (`state` `pending`), and a later `--continue` only finishes. `finish_pending` (exit 4): the branch moved but its
  files did not follow, and bd can't finish; the message says what is left.
- `bd update --abort [--worktree <id> [--force]]` removes the session and its copies; `--worktree` names another
  worktree's session, e.g. an orphaned one, and a live one needs `--force`. `bd update --status [--materialize [--force]]` prints this worktree's session and
  every session of the repository (`orphaned`, `own`), and changes nothing but the copies it is asked to write.
- The copies live in `branch-deck/resolve/r<n>/{edit,before,onto,commit}/<key>-<basename>` of the worktree's git dir,
  or under `--dir <path>` (absolute, outside every worktree, empty or bd's for this worktree, used by no other
  session), for a sandbox that keeps `.git` read only.

### `bd fold [--branch <b>] [--expect-head <id>] [--dry-run]`

Folds the `fixup!`, `squash!` and `amend!` commits into their targets, as the app's Squash Fixups does
(`sync_core::autosquash`), under the repository lock. One rewrite that keeps HEAD's tree, so the index and the working
tree stay as they are.

```json
{"schema":1,…,"dry_run":false,"previous_head":"77e01aa4","new_head":"c0ffee12",
 "folded":[{"target":"1b2c3d4e","subject":"(auth) Add login","new_id":"2c3d4e5f","fixups":["9d0e1f2a"]}],
 "orphans":["5f6a7b8c"],"undo_token":"<full previous>..<full new>"}
```

- `--branch` folds only the fixups whose target is in that branch; a name that is no branch is `unknown_branch`.
  `orphans` are fixups without an older target: they stay. Nothing to fold prints `folded: []` without a token.
- `--expect-head` refuses with `head_moved` unless HEAD is still that commit; the fold also checks that the planned
  commits and the history it rewrites end at the planned head, and moves the branch only from it (compare-and-swap).
- `bd undo <undo_token>` restores the previous head.

### `bd check [--plan FILE | < plan.json] [--build COMMAND [--build-paths PATHSPEC…]]`

A trial sync of a plan (`sync_core::check`), or of the current grouping without one. The plan is applied in memory
and validated as `bd group --dry-run` does, `descriptions` included (an invalid plan is exit 2), the branches are
placed as a stacked sync places them, and each one is built with the sync's `build_branch`, parents first. It writes
commit objects and the object cache only: no ref, note, config, HEAD or working tree changes, no remote status and no
repository lock.

```json
{"schema":1,…,"head":"77e01aa4","baseline":"origin/master","base_commit":"3c0ffee1","rewritten":1,"ok":false,
 "branches":[{"name":"a","built_on":null,"commit_count":1,"result":"ok"},
  {"name":"b","built_on":null,"commit_count":1,"result":"conflict",
   "conflict":{"commit":"5e2f9c3b","subject":"(b) Change shared","files":["shared.txt"],"missing_commits":["41c7d00e"]}}],
 "unassigned_count":0}
```

- `result` is `ok`, `conflict`, `dangling_reference`, `blocked` (with `blocked_by` `{parent, reason}`: the parent has a
  problem, conflicts or is blocked; a parent without commits is passed), `problems`, `error` or, with `--build`,
  `build_failed`. `built_on` is the branch it would be built on,
  `null` for `base_commit`.
- `missing_commits` are the older commits outside the branch and the branches it is built on that change the
  conflicting files, computed from the cached commit diffs: move them into the branch, or stack the branch on the
  one that has them.
- `dangling_reference`: git merges the branch, but its build fails. It removes a declaration (a top-level type or
  function, a static member) that a file of its virtual tree still uses, and a commit outside it, `fixed_by`, changes
  that use in HEAD. `dangling_references` lists `{identifier, file, line, removed_by, fixed_by, mutual}`: move
  `fixed_by` into the branch or stack the branch on the one that has it; `mutual`, when the change in `fixed_by` uses
  what the branch adds (as in a move of a constant into a new class), means no stack builds both: move that change of
  `file` into `removed_by`. A use that no commit here changes, or one that the baseline got after the base (Patronus
  merges onto its tip), is `warnings[]` `{kind: "possible_dangling_reference", branch, identifier, file, line,
  removed_by, on_baseline}` and doesn't count for the exit code. It is a heuristic by the lines (Java, Kotlin,
  Groovy, Scala, Rust, TypeScript, JavaScript, Python, Go): instance members and locals are not searched, and a use
  through an alias or a star import is not seen. It searches only the paths where the branch's tree and HEAD's differ,
  so it costs a `diff-tree` and a `cat-file`, plus one `log -p -U0` for the commits the cache lacks.
- `inverted_stack` `{branch, child, fixes}`: the branch conflicts, or has a dangling reference, because it needs a
  commit (`missing_commits`, `fixed_by`) of `child`, a branch stacked on it: no order of the stack builds both. Each
  fix is `{kind, branches, unstack}` to add to the plan, kept only when a trial build of the plan with it builds both
  branches: `move_to_child` moves the branch's commits that need the child into it, `swap` stacks the branch on the
  child (`unstack` the child, `"child/branch": []`); no `swap` when the child's change uses what the branch adds
  (`mutual`). The `result` stays.
- `--build COMMAND` runs a shell command (`sh -c`, `cmd /C` on Windows) in the tree of each `ok` branch, extracted with
  `git archive` into a temporary directory outside every working tree (`BD_BRANCH`, `BD_COMMIT` set), and removes it
  afterwards; `build` is `{exit_code, output}` (the last 50 lines of a failure), and a failure is `build_failed`.
  `--build-paths` extracts only those paths: the whole tree of a large repository is gigabytes per branch.
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
   `(prefix)` or of the same issue group (an anchor), and the commits without a prefix that have the same issue key.
   `--fresh` ignores the anchors. A dependency between two anchors joins nothing: it is an `anchor_dependency`
   warning. With stacked branches (`branchdeck.stacks`), the anchor of a prefix is the leaf of its path, and a
   dependency joins nothing either: it stays an edge between two units, for the stack (step 4). Only the edges of a
   cycle of units join them (Kosaraju), because no stack builds a cycle. The commits without a prefix of an issue key
   that a prefix names join that prefix: `(K-1) X` and `K-1 Y` are one branch.
2. A score between two clusters, `S = max(R, P × Kp) × K + adj`, in integer units of 1/10 000 so that ties are exact:
   - R, the links: with stacked branches, a dependency between the two sides when a side without an anchor has no
     issue key (0.95): a follow-up joins the branch it needs, and a commit with its own key stacks on it instead; a
     body names an issue key of the other side (0.9); both sides name a third key (0.7); the same conventional scope
     (0.8 without issue keys, 0.5 with them). A key that more than max(10, units / 3) units name links nothing: it is
     the key of a series. A scope is read from `type(scope):` and from `type scope:`, as in `setup vm:`
     (`parse_semantic_prefix`).
   - P, the paths: the IDF-weighted Jaccard similarity of the areas (the first 3 directories, weight 0.5), the modules
     (the directories before the first `src`, `testSrc`, `test`, `tests`, `resources` or `gen`), the packages (the
     directories after it) and the files. A feature of more than max(4, units / 4) units weighs 0. A file under 5 % of
     the commit's lines and outside its main area weighs 0.2.
   - K: 0.3 when the issue keys of the two sides are of different projects (`ABC-1` and `XYZ-2`), 0.5 when both sides
     have scopes and none in common. Kp: 0.7 when the keys differ in the same project.
   - adj: 0.05 when the two sides have neighbouring commits in the log and R or P is not 0.
3. Merges, the best pair first and on a tie the oldest, until the best score is under 0.40 (`MERGE_THRESHOLD`). Two
   anchors never merge, no merge makes a cluster of more than `--max-commits` commits (default 40), and no merge
   makes a cycle: a pair where one side reaches the other through a third, over the dependencies and the declared
   parents, waits. So a follow-up of a branch and its parent joins the branch, not the parent. Each row of
   the score matrix keeps its best column, so a merge rescores one row and one column. On synthetic input of commits
   that all merge, 300 commits take 0.3 s and 1000 take 6.9 s in a debug build (1.1 s in a release build); commits
   that don't merge take 16 ms and 0.14 s.

4. With stacked branches, `place_clusters` gives each cluster a parent, so that every cluster it depends on is an
   ancestor (`stack_clusters`). The clusters go in a topological order of their dependencies and declared parents,
   the oldest first on a tie. Each takes the parent its prefixes declare when that parent covers what it needs, else
   the deepest cluster it needs that covers the others. When the clusters it needs lie on two stacks, those stacks
   become one chain in that order, which only adds ancestors, so the clusters placed before keep theirs. In a cycle the
   declared parent gives way first, then the oldest cluster goes first. A declared parent that is no cluster goes,
   because the sync would build the branch on the baseline anyway, and `--fresh` declares no parent.

Each cluster then gets a name, the first of: its prefix; the branch its commits come from, when it holds the most
commits of that branch (so `--fresh` keeps the names of the branches it finds again); its single issue key; a scope of
at least 60 % of its commits; `<area leaf>-<word>`, for the subject word of at least half its subjects with the highest
TF-IDF (issue keys and the conventional prefix are not words); its most frequent key; `cluster-N`. When another cluster, a group now or an existing virtual branch has the name (in any case), and the
cluster's commits don't come from it, the name gets `-2`, `-3`, and so on.

The name of a stacked cluster is its leaf, and the plan names it `parent/leaf`, and a root by its leaf. A prefix path of
more levels, as `(a/b/c)`, also declares the parent of its parent, which the placement may have moved, so the plan
rewrites it to `b/c`. The keys of a stacked cluster leave their issue group (`ungroup_issues`), because an issue
group can't name a parent.

The trial sync builds the plan. A conflict joins the conflicting commit to its `missing_commits` as hard edges, or to
the newest older commit outside its branch that changes a conflicting file when there are none, and the clustering
runs again, at most 5 times. An edge joins two hard units, also two of one cluster that build apart (an issue group,
below): the joined unit then goes into the plan. A repair never joins two anchors, so a conflict between two prefixes
stays. With stacked branches, a conflict on a commit of another cluster first stacks the two clusters; only when the
same conflict comes again does the next round join their units.

```json
{"schema":1,…,"head":"77e01aa4","baseline":"origin/master",
 "plan":{"branches":{"vm":["41c7d00e","5e2f9c3b"]},"unassign":[],"issue_groups":{"parser-parse":["ABC-1","ABC-2"]}},
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
- Per cluster: `change` against the groups now in `from` (`kept`: one whole branch under its name; `renamed`: one
  whole branch under another name; `merged`: two or more whole branches, or one with unassigned commits; `split`: a
  part of one branch; `new`: anything else), `why` (one line to
  quote to the user, with the repair rounds and a new parent), `weak` (a merge by paths near the threshold, or one
  that adjacency decided: review these first), `issue_keys` and `areas` (as in `bd commits --brief`). With stacked
  branches also `parent` (the branch it builds on) and `restacked` (the parent changes). `stacked_without_need`: the
  stack parents stack it on `parent`, a line with no prefix behind it, and it depends on no commit of `parent` or of
  its ancestors. The plan keeps the parent: the stack parents don't say why a branch is stacked, and "Stack On…"
  writes the same line. `bd unstack` makes it a root.
- A stacked cluster keeps its issue group: the plan names its path with an empty list (`"a/grp": []`), which writes
  the stack parent without a rewrite. A cluster unstacked from its stack parent is in `unstack`.
- A cluster of two or more whole issue branches goes into `issue_groups` of the plan, marked `"via":"issue_group"`:
  its subjects keep their keys, and `rewritten` stays 0 for it. `--rewrite-issues` gives its commits a prefix instead.
  An issue group is an anchor, as a prefix is: it stays one branch unless `--fresh`.
- `rewritten` counts the subjects the plan changes. `check` has the `bd check` result of each branch and the
  `repair_rounds`. `warnings` lists `anchor_dependency` (with stacked branches only a cycle between two prefixes) and
  `unresolved_conflict`, with the branches.
- The exit code is 1 when a branch of the plan still conflicts, as for `bd check`.
- On the 74 commits and 12 branches of a large monorepo branch it keeps the 12 branches with an empty plan, in 0.27 s
  warm (a debug build). With `--fresh` it finds the grouping a human made there with a pairwise precision of 1.0 and
  a recall of 0.945, and its plan rewrites 6 subjects. The golden test of `clustering_test.rs` checks its anonymized
  input: `--dump-input` (hidden) prints the input with every name hashed by SipHash with a random key of the
  operating system, which is not kept.

### `bd sync`

Rebuilds the virtual branches with `sync_core::sync::sync_branches`, the app's sync: it includes the integration
detection of archived branches and the update check of the checked out branch. It holds the repository lock. While a
rebase, merge, cherry-pick or revert is in progress it is `operation_in_progress`, on a detached HEAD `detached_head`
(exit 4), and changes nothing: a half-done history would archive the branches it lacks; finish the operation first.

```json
{"schema":1,"repository":"/repo","prefix":"develar","baseline":"origin/master","unassigned_count":3,
 "branches":[{"name":"auth","ref":"refs/heads/develar/virtual/auth","status":"created","commit_count":2,
   "tip":"1b2c3d4e","remote":{"exists":false,"unpushed":2,"behind":0,"head":null},"push":"bd -C /repo push auth"},
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
  `{"commit","subject","author","author_time"}`. Older versions of commits the branch has are not `behind`: `amended`
  counts those of commits amended here, `restacked` those of commits of the branches it is built on (amended or
  rebuilt since it was pushed); same author, author time and subject without the prefix, a push replaces them. Both
  are left out at 0.
- `push` is the `bd push` that brings the remote branch up to date, quoted for the shell, to run only after the user
  approves: `bd push <branch>` (older versions of amended or restacked commits need no `--force`),
  `bd push <branch> --force --expect-remote <remote.head>` when the push drops `dropped_commits`, and
  `bd push <branch> --force` when a branch it is built on has commits for the remote too (`--expect-remote` would push
  the branch alone). It is left out for a branch whose remote branch is up to date, one that didn't sync, and without
  a remote.
- `update.state` is `up_to_date`, `fast_forward`, `rebase`, `conflict` or `blocked` (with `files`), or `error`.
- `update.landing` lists the local commits the fetched baseline already has (the update drops them), and `landed` what
  landed with an update and the app hasn't dismissed: per branch (omitted for unassigned commits) the commits with
  `commit`, `subject` and the baseline commit `upstream` (omitted when squashed upstream), and `remaining`, the commits
  that stay local.
- A virtual commit is reused only while its mapping note still fits the commit: the note names the trees it was copied
  from, and the copy must have the message and the author of the commit. A note that `git notes copy`, `git rebase` or
  `git commit --amend` (`notes.rewriteRef`) carried to a changed commit is stale; the sync copies the commit again and
  logs it (`-v`). With `--no-cache` every note is checked against the tree of its copy.
- The exit code is 1 when any branch has a conflict; the summary is printed either way.

### `bd push <branch>… [--with-children] [--alone] | --all --yes [--force] [--expect-remote COMMIT] [--remote R] [--dry-run]`

Syncs, then pushes the branches in one atomic `git push --porcelain` with a lease per ref, parents first, under the
repository lock (see "Agent push" in [Git Operations](../design/git-operations.md)). A stacked branch takes the
branches it is built on along when they have commits for the remote, as the app's push button does: its pull request
is right only once they are on the remote. `--alone` pushes only the named branches; `--expect-remote` does too.

```json
{"schema":1,…,"remote":"origin","dry_run":false,
 "branches":[{"name":"auth","remote_branch":"develar/virtual/auth","commit":"1b2c3d4e","status":"new",
   "lease":null,"lease_source":"absent",
   "pull_request":{"base":"master","head":"develar/virtual/auth","title":"Add login","body":"The login form.",
     "command":"gh pr create --base master --head develar/virtual/auth --title 'Add login' --body-file -"}}]}
```

- Refused with `push_refused` (exit 4), nothing pushed: a branch that didn't sync, one whose remote branch has commits
  it lacks (`behind`) without `--force` (also a parent taken along: `--alone` leaves it), and with `--alone` a stacked
  branch whose parent isn't on the remote or pushed with it. `amended` and `restacked` need no `--force`: the push
  replaces older versions of commits the branch has, and a parent with only them goes along. `--all` needs `--yes`. A
  branch refused as behind is in `error.branches` with its remote status, as below.
- `--with-children` pushes the branches stacked on the named ones too, directly or further down, in the same atomic
  push; a blocked or conflicting child refuses it, as a named branch would. A parent taken along brings no children.
  Not with `--expect-remote`.
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

### `bd safe-push <branch> [--dry-run] [--profile P] [--check] [--without-approvals] [--create-pull-request] [--cancel-running] [--land-stack] [--allow-dangling]`

Starts a Patronus robot on the virtual branch, as the Safe Push form of the app ([Patronus](../../spec/branch/patronus.spec.md)):
a Safe Push (Patronus rebases the branch onto the target, runs the checks and pushes it into the target when they
pass) or, with `--dry-run`, a Dry Run (the checks only). The repository must be on Space (`origin` on
`git.jetbrains.team`, else `not_on_space`), and the Space token of the Safe Push CLI must be there: run
`./safePush.cmd` once, or set `SPACE_TOKEN` (else `not_logged_in`). `SAFEPUSH_PATRONUS_URL` picks another Patronus
instance, as for the Safe Push CLI.

```json
{"schema":1,…,"branch":"auth","mode":"safe_push","target":"master",
 "profile":{"id":"lsc","name":"Large Scale Change","description":"All the tests"},
 "profiles":[{"id":"default","name":"Default"},{"id":"lsc","name":"Large Scale Change","description":"All the tests"}],
 "review":{"required":false,"skipped":false,"counts_in":"space_merge_request",
   "merge_request":{"key":"IJ-CR-7","url":"https://jetbrains.team/p/ij/reviews/7"},"groups":[]},
 "push_needed":true,"pushed":[{"name":"auth",…,"status":"new",…}],
 "started_via":"api","robot":{"id":"r1","name":"Add login","status":"running","mode":"safe_push","commit":"1b2c3d4e",
   "url":"https://patronus.labs.jb.gg/robot/r1"}}
```

- `--check` syncs and prints the same object without pushing or starting anything: the profiles, the review,
  `push_needed`, and `refusals`, each `{code, message}` a start would fail with. The agent shows it to the user and
  starts only after the user approves.
- The profile: `--profile` takes an id or a name (any case), `none` for the default checks of the target; without it,
  the profile of the last robot of the branch, as the form does. `profiles` lists those of the target that allow the
  mode, never the emergency ones; another name is `invalid_arguments`, listing them.
- The code-owner review (never for a dry run): the commits of the branch go to the pre-check of Patronus. Missing
  approvals refuse a Safe Push with `approvals_missing` (exit 4), nothing pushed or started; the form only warns, but
  an agent shall not send a robot that Patronus fails with "Changes require review approval". `error.review` lists the
  `groups` with their `reviewers` (`suggested`, `available`) and `paths` (`branch_wide` for all files).
- The approvals count in the GitHub pull request of the branch (Space merge requests are deprecated; one found open,
  `review.merge_request`, still counts for a start through the API). `--create-pull-request` pushes the branch and
  opens its pull request through the GitHub API, as the `gh pr create` that `bd push` prints would (into the parent's
  branch or the target, with the `bd describe` text, else filled as `--fill`): `pull_request` with `created: true`, or
  the open one, also one that already existed; no robot starts. The GitHub token is the first of `GH_TOKEN`
  (`GITHUB_TOKEN`), `gh auth token`, the app's GitHub sign-in (Settings) and the git credentials of github.com;
  without one GitHub accepts it is `not_logged_in`. The
  GitHub repository is the mirror of the Space settings or `branchdeck.githubRepository`; without one it is
  `invalid_arguments`, and `approvals_missing` says how to set it. The pre-check can't see the approvals of a pull
  request, the robot started from it counts them: a Safe Push from a pull request someone approved
  (`review.approved_on_github`) starts. `--without-approvals` starts anyway, only when the user says so. A branch of
  more than 500 commits is not checked (`review.skipped`).
- The branch is pushed first, as `bd push <branch>` pushes it (`pushed`; the branches it is built on that have
  commits for the remote go along, parents first, with the lease and the refusals of `bd push`), under the repository
  lock with the sync and the checks; the start runs after the lock. A start is never repeated.
- A start through the API (`started_via: api`) starts the robot on `refs/heads/<prefix>/virtual/<branch>`, kept
  after the merge. Branches reviewed on GitHub (the mirror of the Space settings, or `branchdeck.githubRepository`)
  with an open pull request into the target start from it (`started_via: pull_request`): bd posts
  `/patronus merge --keep-source-branch` (`/patronus dry-run`) through the GitHub API, waits up to a minute for the answer of
  Patronus, and finds the robot, so the approvals of the pull request count (`review.counts_in: pull_request`). A
  comment can't choose a profile: `--profile` is `invalid_arguments` there. `pull_request.reply` is `pending` when
  Patronus didn't answer within the minute (the robot is `null`); a rejected comment or a draft pull request is
  `patronus_refused`.
- `robot_running` (exit 4): a robot runs on the last commit of the branch ("A Safe Push of the last commit is
  running"), or a Safe Push lands it with a branch of its stack. A running Safe Push of an older commit fails once the
  push changes its branch: `--cancel-running` cancels it first (`canceled`); a dry run is not affected.
- `stack_not_landed` (exit 4): a Safe Push of a stacked branch whose parents aren't in the target yet; Safe Push the
  root first, or `--land-stack` lands the parents with it. A dry run needs no flag. The message ends with a trial build
  of the branch as a root (`stack_independence` in `crates/sync-core/src/check.rs`): "builds alone on the baseline:
  `bd unstack <branch>`", or "needs <ids> (<branch>): move it into the branch". The needed commits come from the line
  dependencies, so a compile dependency on the parent is not seen. The build is on the base the sync builds on, not
  on the newest target. "Other branches need them too" means that a move would make those branches need this one.
- `dangling_reference` (exit 4, nothing pushed): the branch or a branch it is built on removes a declaration that a
  commit of another branch still changes the use of, as `bd check` reports it (`dangling_references`); the message
  names the use, the commits and the fix. `--check` lists it in `refusals`; `--allow-dangling` starts anyway, when
  the user says it is a false alarm. A possible one (`bd check` `warnings`) is in `warnings` and refuses nothing.
- `patronus_refused`: the target is not protected by Patronus, the pull request is a draft, or Patronus rejected the
  comment; `patronus_error` (exit 5): a request to Patronus failed, `message` is its answer.

### `bd describe <branch> [--title T] [--body B | --body-file F] [--clear]`

Prints or sets the pull request text of a branch in git's own `branch.<prefix>/virtual/<branch>.description`, stored
as `title`, a blank line and the body, as `git branch --edit-description` writes it. Without options it prints
`{…,"branch","ref","description":{"title","body"}|null,"changed":false}`. `--title` or `--body` replace only that part;
`--body-file -` reads stdin; `--clear` removes it. The branch must be a current group. The `descriptions` of a
`bd group` plan set the texts of several branches in the same call as the rewrite. The app shows the title after the
branch name, in the branch table and the branch preview.

### `bd schema`

Prints `{"schema":1,"outputs":{"check":…,"commits":…,"commits_brief":…,"describe":…,"error":…,"fold":…,"install":…,"push":…,"rewrite":…,"safe_push":…,"status":…,"status_brief":…,"suggest":…,"sync":…,"undo":…,"update":…,"update_abort":…,"update_status":…,"version":…}}`;
`rewrite` is the output of `group`, `rename`, `restack` and `unstack`, `commits_brief` and `status_brief` the
outputs of `--brief`, and `update_status` and `update_abort` those of `bd update --status` and `--abort`.

### `bd version`, `bd --version`

Both print `{"schema":1,"version":"0.11.0","git":"2.56.0"}` and need no repository. `version` is the version of
Branch Deck, the one the app shows: `build.rs` of `crates/bd-cli` reads it from `src-tauri/tauri.conf.json`, which
release-please updates. The workspace version of the crates stays 0.1.0, so `CARGO_PKG_VERSION` is not it. `git` is
the version of the git bd finds (see Invocation), also of one older than 2.54, and `null` without one: never an
error. The first line of `bd help agent` names the version too.

### `bd help agent`

Prints the workflow below as plain text, about 5 KB; it only names the resolve session, which `bd help update` prints
(`UPDATE_HELP`). `agent_help_test.rs` keeps both true. Together they must name every command and, for each command,
every long option, unless the test lists it in `NOT_IN_AGENT_HELP` (as `status --limit`). An option counts for the
command named last before it, and it must exist for that command.

### `bd install [--uninstall]`, `bd help skill`

How an agent learns that bd exists. `bd install`, or Help → Install Agent CLI… in the app
(`src-tauri/src/commands/agent_cli.rs`, the same code), writes:

- on macOS and Linux a `bd` symlink in `~/.local/bin` to the app binary, which runs the CLI under that name. A symlink
  there is replaced; a file that is not a symlink is `install_refused` (exit 4). On Linux an app run from an AppImage
  (`APPIMAGE` is set) is linked by the AppImage file, not by the binary in its mount under `/tmp/.mount_*`, which goes
  when the app quits. On macOS an app that Gatekeeper translocated (started from Downloads or a disk image, its path
  contains `/AppTranslocation/`) is `install_refused`: "Branch Deck runs from a temporary location: move it to
  Applications, then install the Agent CLI". On Windows `bd.exe` is next to the app already, and a symlink would need
  developer mode;
- the skill `branch-deck` (`SKILL.md`, the Agent Skills format) into `~/.claude/skills/branch-deck/` (Claude Code) and
  `~/.agents/skills/branch-deck/` (Codex and others). Its description tells an agent when to use bd: grouping or
  splitting commits into branches or pull requests, stacked branches, an update onto the baseline and its conflicts,
  Branch Deck. Its body names bd by its full path, so it works without `PATH`, and sends the agent to `bd help agent`
  first, or to `bd help update` for an update. The workflow stays in the binary, so the
  skill can't fall out of date with it.

It prints `{"schema":1,"bd":"/Users/me/.local/bin/bd","on_path":true,"skills":[…],"uninstalled":false}`. `on_path`
says whether `bd` on `PATH` is this one; the app asks the user's login shell for `PATH`, since an app started from the
Finder has a minimal one. `--uninstall` removes the skills and the symlink if it points at this app (or at a removed
`branch-deck`). `bd help skill` prints the skill as text, for an agent that reads skills from elsewhere. Without the
symlink it names the app binary with `cli` (`'/Applications/Branch Deck.app/…/branch-deck' cli`), quoted for the shell
and to be run as written: the bare app binary starts the app.

The app's prompts name bd the same way without an install: the Tauri command `agent_cli_command(repository)`
(`bd_command` and `for_shell_in` in `install.rs`) returns `<bd> -C <repository>`, with the `bd` symlink when it points
at the running app, else the app binary with `cli` (on Windows `bd.exe` next to the app; a binary named `bd`, such as
a cargo build, needs no `cli`). It is quoted for a POSIX
shell, or for PowerShell on Windows (`& '<bd.exe>' -C '<repository>'`, single quotes, in which `$` does not expand, `powershell: true`). It writes nothing, so the
bd of a prompt is always the version of the app that copied it. When the app can't tell its binary (no `bd.exe`), the
blocked prompt names no command: the user updates in the app.

## Agent workflow

1. Run `bd suggest`. It is read only, and its plan is checked already. Read the clusters: `change` and `from` against
   the branches now, `why` to quote to the user, `weak` to review first, and the `issue_groups` of the plan. For a
   closer look, run `bd status --brief` and `bd commits --brief`: per commit the group, issue key, fixup target and
   `areas`, the `units` that must stay together and the `dependencies` `{from, to}`. Run `bd commits <id>… --patch`
   or `bd commits --unassigned` for the commits that are unclear, and `bd status --stats` for the size of each branch.
2. Edit the plan where the clusters are wrong. `bd suggest` exits with 1 when its own check fails, so edit it then
   too. After any change of `branches`, `unassign`, `issue_groups` or `ungroup_issues`, run
   `bd check --plan plan.json` until every branch is `ok`: move the `missing_commits` of a conflict into the branch,
   or stack the branch on the one that has them.
3. Add `descriptions` (the pull request title and body) for the branches that have none (`description` in
   `bd status`); replace one only when the user asks. A stacked branch is named by its leaf. Run
   `bd group --expect-head <head> --sync --plan plan.json` with the `head` of `bd suggest`. On `head_moved`,
   `groups_moved` or `locked` (exit 3), read again and retry. On a sync conflict (exit 1), regroup the commits the
   change needs into the same branch or its parent, and run `bd sync` again. On another exit with `error` and an
   `undo_token`, the rewrite stands: run `bd sync` again, or undo it. `bd undo` with the printed token reverts the
   rewrite and the issue groups, not the descriptions; run `bd sync` after it.
4. Show the result to the user, and run `bd push <branches>` only after the user approves; then the `gh pr create`
   lines it prints, piping each `body` into its command.
5. For a Patronus Safe Push, run `bd safe-push <branch> --check` and show the profile and the review to the user;
   start with `bd safe-push <branch>` only after the user approves. On `approvals_missing`, show `error.review` and
   open the pull request with `--create-pull-request`; `--without-approvals`, `--cancel-running` and `--land-stack`
   only when the user says so.

To update the branch onto its baseline, run `bd update`. On `uncommitted_overlap`, show the `paths` to the user and
ask what to do with each file; never stash, reset, check out or commit them. On a conflict, resolve each round in the
files at `edit` and run `bd update --continue` until it exits with 0, then report each resolution. After a failed build, commit the fix as
`fixup! <subject>` and run `bd fold`, or `bd undo` the update with its token.

The agent never reorders, squashes or drops commits: a plan only moves commits between branches. The Update Project
hand-off prompt (`src/lib/agent-prompt.ts`) states the same rule for an agent that rebases; it also forbids a push,
which here the agent runs only after the user approves. For an update blocked by uncommitted changes, the prompt
sends the agent to the user about each file and then to `bd update --no-fetch`, and never to a stash. For a conflict,
the prompt sends the agent to `bd update --no-fetch` and its resolve session; without a bd command, to the git steps,
which ask the user about uncommitted changes instead of stashing them.
