/// What the reading commands write, for `bd --help` and `bd help agent`; a macro, so that `concat!` takes it
macro_rules! read_only {
  () => {
    "Read only: `bd status`, `bd commits`, `bd suggest`, `bd check`, `bd robot`, `bd schema`, `bd version`,
`bd describe <branch>` without a setter, `bd request-review --check`, `bd update --status` without --materialize,
`bd lease status`, `bd workspace`, and
`bd group --dry-run`, `bd rename --dry-run`, `bd restack --dry-run`, `bd unstack --dry-run`, `bd update --dry-run`
(also with --continue), `bd pick --dry-run`, `bd fold --dry-run`, `bd move-hunk --dry-run` and `bd reword --dry-run`
write no ref, config, index or working tree: at most git objects (a trial sync, a replay, the trees of a move) and the
object cache (none with --no-cache); `bd check --build` runs its command in a temporary copy of each tree.
`bd push --dry-run` and `bd safe-push --check` sync first."
  };
}

/// The read-only line of `bd --help`
pub const READ_ONLY: &str = read_only!();

/// `bd help agent`: the workflow of an agent that groups commits into virtual branches (docs/guidelines/agent-cli.md);
/// the first line names the version (`version::VERSION`). `agent_help_test.rs` checks it against the commands and
/// options
pub const AGENT_HELP: &str = concat!(
  "bd ",
  env!("BD_VERSION"),
  ": Branch Deck for agents. JSON on stdout, one object per run (every object has \"schema\": 1); logs on stderr (-v for
debug).

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into branches, one per name: the `(name)`
prefix of a subject, else the issue group of its issue key, else the issue key (`ABC-123 Fix ...`). A sync rebuilds
each branch as the virtual branch `{prefix}/virtual/{name}` on the baseline; the working tree is never touched. With
stacked branches (`git config branchdeck.stacks true`) a branch is built on a parent: a subject declares it with
`(parent/leaf)`, or the stack parents do (`refs/branch-deck/stack-parents`) and the subject keeps the bare `(leaf)`; a
stacked branch is named by its leaf. `upstream` in the output of `bd update` is the baseline.

Rules
- Ask the user first, and run only what the user approved: a push (`bd push`, also --force and --restart-running), a
  Safe Push or a Patronus Dry Run (`bd safe-push`, also --cancel-running, --without-approvals, --allow-dangling and
  --create-pull-request), `bd request-review`, a `gh pr create` that bd prints, and any change of a commit that
  another session made, a reword (`bd reword`) of its message too, and `bd park` or `bd unpark`. Show what bd printed
  (`lands`, `review`, `dropped_commits`, the plan, the `changes` of a `bd reword --dry-run`) when you ask.
- A branch `parked` in `bd status` stays local: the user keeps its commits, waiting for a proper fix elsewhere. Never
  push it, start a Safe Push of it or unpark it on your own; a branch built on it is `parked` too (`with` names the
  parked one), and so is the whole stack that holds it. `bd park <branch> --note '<why>'` and `bd unpark <branch>`
  only when the user asks: no rewrite, the store `refs/branch-deck/parked`; `idle_parked` lists the lines of branches
  without commits here.
- Never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`; move a
  change between commits only with `bd move-hunk`, never with plumbing. Never stash, reset, check out or commit the
  user's files: on `uncommitted_overlap` (exit 4) show `paths` to the user and ask.
- Only `bd update`, `bd pick` and their undo write the working tree, through the guarded update.
- Branch on `error.code` and the exit code, never on the message. 0 ok. 1 a conflict, never an error object: the
  output says what conflicts (`state: \"conflict\"`, a branch not built, `ok: false`). 2 invalid input: fix the
  arguments or the plan. 3 stale: read again and retry. 4 refused: do what this help says for the code, else stop and
  show `message` to the user. 5 git or network error: show it. Retry when `retryable` is true, whatever the exit code,
  and never when it is false. A `git_error` \"not updating refs/branch-deck/tmp/chain (… does not contain …)\" is the
  ref a Branch Deck 0.21 or older left behind: `git update-ref -d refs/branch-deck/tmp/chain`, then retry; a newer sync
  deletes it itself.
- An error is {\"schema\":2,\"error\":{\"code\",\"message\",\"commits\",\"paths\",\"branches\",\"review\",\"blocks\",\"retryable\"}};
  `paths`, `branches`, `review` and `blocks` only where they apply.
- `bd fold --expect-head <id>`, as every write that takes it, refuses unless HEAD is still the `head` (or `new_head`)
  you read last: `head_moved` (exit 3). Every write prints `previous_head`, `new_head` and an `undo_token` (full ids:
  pass it as printed). A write whose step after it failed (a sync) keeps its output with `after_error`: retry that
  step, never the write. `bd undo <undo_token>` takes a write back, `undo_stale` (exit 4) once anything moved on.
- Ids are abbreviated per output: compare them by prefix, or pass --full-ids.

Global options: -C <repo>, --prefix <p> (else `branchdeck.branchPrefix` of this repository's git config, else the app
setting, else the global `branchdeck.branchPrefix`), --full-ids, --no-cache, -v.
",
  read_only!(),
  "
`bd schema` prints the JSON Schema of every output (every `state` with its values), `bd version` the version of bd
and git.

Workflow: the commits into pull requests
1. Look. `bd suggest` prints a plan, clustered and checked by a trial sync (exit 1: `check.ok` is false, edit the
   plan). Per cluster: `change` (kept, renamed, merged, split, new) against its `current_branches`; its commits in
   `kept`, `moved` ({commit, leaves}: the branch it leaves, null if unassigned) and `new_path` (the same branch,
   another stacked path in the subject): with `moved`, what the plan changes; `why`, one line to quote to the user;
   `weak`, review these first; `issue_keys` and `areas`; with stacked branches `parent`, `restacked` (it stacks each
   branch on the branches it needs) and `stacked_without_need` (a stack parent the branch needs nothing of: the plan
   keeps it, ask the user before an unstack). Whole issue branches that belong together go into `issue_groups`: no
   subject changes. `bd suggest --fresh` ignores the current prefixes and issue groups. To look closer:
   `bd status --brief`, `bd status --stats` (the size of each branch), `bd commits --brief` (per commit its group,
   issue key, fixup target and `areas`; `units`, the commits that must stay together; `dependencies` {from, to, via}:
   `from` touches lines `to` added, or with `via` `symbols` uses a name `to` adds, which a stack satisfies too), and
   `bd commits <id>... --patch` (the blocks of each file, with their ids) or `bd commits --unassigned` for unclear
   commits.
2. Plan and check. A plan is {\"branches\": {\"<path>\": [ids]}, \"unassign\": [ids], \"issue_groups\": {\"<name>\":
   [keys]}, \"ungroup_issues\": [keys], \"unstack\": [branches], \"descriptions\": {\"<branch>\": {\"title\", \"body\"}}};
   a path is a branch, or `parent/leaf` for a stacked one. Fixups follow their targets; a commit goes with, or stacks
   on, the commits it depends on. In Windows PowerShell write the plan to a file and pass the file (a PowerShell 5.1
   pipe turns non-ASCII into `?`). After any change of `branches`, `unassign`, `issue_groups` or `ungroup_issues`, run
   `bd check --plan plan.json`: exit 1 until the `result` of every branch is `ok`.
   - `conflict` {commit, subject, files, missing_commits}: move the missing commits into the branch, or stack it on
     the branch that has them. A conflict of git whose every file the words resolve is no conflict: with `git config
     branchdeck.resolveByWords` on (the default) the sync commits the result as IntelliJ resolves it, all or nothing
     per commit, and the branch is `ok`. `bd sync` lists such commits in `resolved` of the branch, `bd status` and
     `bd commits` in `resolved_by_words` of the commit; nothing to do.
   - `dangling_reference` (`dangling_references` {kind, identifier, file, line, removed_by, fixed_by, mutual,
     declared_in}): git merges it, the build fails. The branch removes (`removed_declaration`) or changes
     (`changed_declaration`, e.g. made `suspend`) a declaration (`declared_in` {file, line}) its tree still uses, or
     declares one its tree has in another file (`duplicate_declaration`), and `fixed_by` of another branch changes that
     use or deletes that file. Move `fixed_by` into the branch, or stack it on that branch; `mutual`: that change needs
     the branch too, so `fix` moves it into the branch, as `move_commit` (its `branches` into the plan) or `move_hunk`
     (a ready `command`, once the user approved), each proven by a trial build; no `fix`: none builds. When the user
     says the use means another declaration, `bd check --allow-dangling` keeps the `result` and exits 0.
   - `blocked` (`blocked_by`: the parent it can't be built on), `problems`, `error`; `build_failed` (below).
   `bd check --fix` searches, by trial builds, the regroup that fixes the conflicts and dangling references: each
   `fix.components[].plan` is a whole plan for the next step, and its `steps` say what moves where (show them before
   applying one). A branch that conflicts as a whole is restacked on the branch (or a chain of the branches) that
   holds the commits it needs, in one trial; dangling references are searched after the conflicts. Each trial is a
   line on stderr (`trial 3/12: restack <branch> on <parent> … verified (41 s)`); --max-trials N (12) and
   --time-budget SECONDS (180, the whole command) bound it, and `fix.search: budget` is the best plan so far: run it
   again, the cache makes it faster. `inverted_stack` {branch, child,
   fixes, reason}: the branch needs a commit of a branch stacked on it; each fix (`move_to_child`, `swap`) is
   `branches`/`unstack` for the plan, checked by a trial build; `reason` when none builds. `warnings`: a
   `possible_dangling_reference` (or `possible_changed_declaration`, `possible_duplicate_declaration`) that no commit
   proves: tell the user. `missing_declarations` {branch, identifier, file, used_by, declared_by, declared_in}, not
   counted in `ok` (none when `git config branchdeck.symbols` is `report` or `off`): a use whose declaration is
   outside the branch's tree: stack the branch on `declared_in`, or move one of the two. `bd check --build '<command>'`
   (--build-paths <path>...) also builds a temporary copy of each tree (`build_failed`): slow on a large repository,
   but a build that passed on the same tree is not run again (`cached`; --rebuild runs it after a change outside the
   tree).
3. Apply. Add `descriptions` (pull request title and body) for the branches that have none (`description` in
   `bd status`); replace one only when the user asks. Then `bd group --expect-head <head of bd suggest> --sync --plan
   plan.json`: one message-only rewrite, the issue groups, the descriptions and a sync, under one repository lock. It
   prints the new `groups`, `replaced_descriptions` and `sync` (per branch `state`; a `conflict` names the commit, the
   files and the commits it needs). Exit 1, a branch not built: `bd check --fix`, or regroup so that a change and what
   it needs are in one branch or its parent, then `bd sync`. `head_moved`, `groups_moved`, `stacks_moved` or `locked`
   (exit 3): read again and retry; a peer that only added commits on top: `bd group --expect-head <head>
   --allow-append` takes them along (`appended`). `bd group --dry-run` prints the changed subjects. `bd rename <old>
   <new>`, `bd restack <branch> <parent>` and `bd unstack <branch>` take the same options. `bd describe <branch>
   --title ... --body ...` sets the text of one branch. The undo token of a regroup moves HEAD, the issue groups and
   the stack parents back (`<head>..<head>`, `groups:<id>..<id>`, `stacks:<id>..<id>`, joined by `+`); the
   descriptions stay, and the virtual branches change with the next sync.
4. Push. Show the result to the user; once approved, `bd push <branch>...` (or --all --yes; --with-children adds the
   branches stacked on them) syncs, takes the parents along that have commits for the remote (--alone does not: the
   branch carries their commits), and pushes parents first in one atomic push with a lease per ref; name every branch
   of `branches` to the user. Every pull request goes into the baseline: each branch has a ready `gh pr create`; pipe
   its `body` into it. An open pull request into a virtual branch (a parent's) is retargeted to the baseline: name
   `retargeted` with `approved_by` (GitHub may dismiss those approvals); `retarget_warnings` says what GitHub refused.
   --dry-run syncs, then asks the remote. Refusals, nothing pushed:
   - `push_refused` (exit 4): a branch didn't sync, or is behind its remote branch (`remote.behind`, only what the
     baseline lacks too). Older versions push without --force: of commits amended or restacked here
     (`remote.amended`, `remote.restacked`), of commits the baseline has (`remote.landed`: after its whole stack
     landed, a member's remote branch has its commits as they were; `replaced_landed` names them, nothing is lost),
     and the remote copies of commits reworded here (`remote.reworded`); a stacked branch whose remote branch lacks
     new commits of the branches it is built on (`remote.ancestors_ahead`) is pushed too: its Safe Push lands them.
   - `lease_rejected` (exit 4): the remote changed; bd fetched it. For a branch behind or rejected,
     `error.branches[].remote.dropped_commits` lists what a push drops: show it; once approved, --force
     --expect-remote <remote.head> for that one branch.
   - `robot_running` (exit 4): a Safe Push runs on the branch (the push would fail it at the end), or on a branch
     built on it whose commit has commits of it (the message names them; a parent restacked below it after its start
     is pushed). Wait until it merges, or, once approved, --restart-running cancels the branch's own robot, pushes and
     restarts it on the pushed commit (`restarted`).
   - `parked` (exit 4): a named branch is parked, or built on a parked one; with --all or --with-children a parked
     branch is only left out, named in `skipped` with `code: parked`. Tell the user, never unpark it.
   - `landed` (exit 4): merged Safe Pushes landed all of every branch asked for (`landed_by` in `bd status`):
     nothing of them is left to push until they are archived. With others to push, `bd push` only leaves a landed
     one out (with `--with-children` its children too), named in `skipped` with who landed it; tell the user. A
     branch it is built on that landed is not pushed along.
   `bd sync` prints the push each branch needs as `push`, ready to run once approved. With stacked branches, `stacks`
   in `bd sync` and `bd status` is the stack branch of each stack that forks, `{prefix}/stack/<root>`: all its commits
   in one branch, for a Safe Push of the whole stack (`state`: `built`, `blocked` by `member`, `conflict`, `stale`);
   push none that isn't `built`. `bd push <root> --with-children` pushes it in the same atomic push once it is on the
   remote or its pull request is open, when this sync built it (`stack_warnings` names one left out).
5. Safe Push (Patronus; the repository on Space, the Safe Push CLI logged in once, else `not_logged_in`). First, always,
   `bd safe-push <branch> --check`: it syncs and starts nothing, and prints
   - `profile` (default: the one of the last robot of the branch) and the `profiles` to choose from (--profile
     <id|name|none>);
   - `review`: `readiness` (`not_required`, `approved`, `missing`, `unknown`), `groups` with their `reviewers` and
     `paths`, `waiting_for` (asked, not approved yet), `not_counted` (a bot's approval counts for nothing),
     `merge_request`; Patronus judges the approvals of the GitHub pull request;
   - `push_needed` and the `refusals` a start would meet; `remotes`, what the push replaces and drops of each remote
     branch (a drop refuses with `push_refused`); a reworded branch needs a push, the robot lands the remote's messages;
   - `target_check`: bd fetches the target and replays what lands onto its tip, as Patronus rebases it before its
     checks: `result` `clean`, `conflict` or `not_run`; `fetched: false` means the check used the last fetched tip;
   - `lands`, what the one robot lands into the target, root first: a stacked branch lands the branches it is built
     on with it, and the branches built on it stay. `alone` (a stacked branch): `builds` (it could land alone once
     unstacked, unless `uses` names a name of its stack) or `needs` (the commits of its parents it needs, `movable`
     into it). Landing what it is built on is right: name `lands` to the user, and offer the branch alone only as a
     second way.
   Once approved, `bd safe-push <branch>` pushes first when `push_needed` (the parents with commits for the remote,
   by content), then starts the robot: `robot.url` is its page; a branch with an open pull request into the target
   starts from a `/patronus` comment there, where its approvals count (`started_via`); one into a virtual branch is
   retargeted first (`retargeted`). --patronus-dry-run starts a Dry Run instead: the checks without the push into the
   target and without the review, but it pushes and starts a robot like a Safe Push. The whole stack, its side
   branches included, lands in one robot with --whole-stack (from any branch of it; --check first): the robot starts
   from the stack branch of its top-level root (`whole_stack.source`; a straight chain starts from its last branch),
   `lands` names every branch, and its pull request is the stack's own, where the approvals of the whole stack count.
   Refusals, nothing pushed or started:
   - `approvals_missing` (exit 4): `error.review` names who has to approve which files: show it. --create-pull-request
     pushes and opens the pull request where they approve, assigned to the user (--no-assign leaves it unassigned;
     --reviewer <login>, repeatable, asks a `github_login` of `review.groups[].reviewers` that the user chose;
     `pull_request.warnings` says whom GitHub refused). Once they approved (`review.approved_on_github`), run it
     again. --without-approvals starts anyway, only when the user says so: Patronus fails it.
   - `robot_running` (exit 4): a robot runs on the last commit, lands the branch with a branch of its stack or with the
     whole stack, or lands a branch it is built on; --cancel-running cancels a Safe Push of an older commit.
   - `dangling_reference` (exit 4, also in `refusals`): what the check of step 2 reports for the branch or a branch it
     lands: fix it there, or, when the user says it is a false alarm, --allow-dangling.
   - `target_conflict` (exit 4, `error.commits`, `error.paths`, also in `refusals`): Patronus can't rebase what lands
     onto the newest target; the message names the commits of the target that change the files. `bd update` resolves
     it locally (after the user approves), then run the Safe Push again.
   - `stack_conflict` (exit 4, `error.commits`, `error.paths`): the stack branch conflicts though each branch builds;
     `stack_not_built` (exit 4): a branch of it is blocked or failed, or the stack branch is stale. Land the stack in
     pieces.
   - `landed` (exit 4): a merged Safe Push landed all of the branch, or every branch of the whole stack: nothing is
     left to land. `lands` leaves out what merged Safe Pushes landed already.
   After a merge, until `bd update` archives them, `bd status` names the branches a merged Safe Push landed
   (`landed_by`: the `branch` whose Safe Push it was, or the root with `whole_stack`, the `robot`, the `commits` it
   landed, and `after`: commits added since, decided by content, the only work left, which a push and a Safe Push
   send). bd closes no pull request after the merge: the app closes those of the parents that landed when it sees the
   merge; when the app isn't running, tell the user that they stay open.
6. Review. A pull request that asks no code owner of a missing group: `bd request-review <branch> --check` names whom
   it would ask (`reviewers`: one suggested code owner of each such group); once approved, `bd request-review
   <branch>` asks them on GitHub (--reviewer <login>, repeatable, asks these instead): `requested`, `refused` with
   GitHub's reason, `waiting_for`. `no_pull_request` (exit 4): none is open, or a draft; `no_reviewer`: Patronus
   names no GitHub login, ask the user whom; `review_refused`: GitHub asked nobody.
7. Robot. `bd robot <branch>` prints the robot of the branch (`robot.state`; `running`: ask again later; --robot <id>
   for another one; a newer robot of the whole stack names `stack_root`), and --whole-stack the robot of the whole
   stack. After a failed Safe Push, --full says why (`failure`: `kind`, the failed `checks` with their build ids,
   `problems`, the failed `builds` with their failed tests and log tails) and which commits it tested (`commits`, by
   the ids of the checked-out branch: `tested`, `rebased`, `changed`, `added`, `removed`, `landed`).
   `kind: infrastructure`: nothing to fix, the user restarts it; else fix and check locally, the user pushes and
   restarts it.

Other tasks
- Before the first edit of a session: `bd workspace` says whether to work in a lease of your own
  (`branchdeck.agentLease`, on by default) and whether to start each subject with `(branch)`
  (`branchdeck.agentSubjectBranch`, off by default), and `next` what to run; the user's words win over it.
- Update the branch onto its baseline: `bd update` (--no-fetch, --dry-run), the app's Update Project; a conflict opens
  a resolve session (exit 1), which `bd help update` explains.
- Several sessions or worktrees on one branch: `bd lease get` leases a worktree of your own, `bd lease return` gives
  it back; `bd pick --from <worktree> --follow` lands the commits of another worktree here (or `bd pick <rev>`,
  `<a>..<b>`; --expect-head, --dry-run), never `git cherry-pick`; `dependencies` names a picked commit whose branch is
  not built on what it needs, with the `plan` for `bd group` or the `bd restack`; `reparented` a branch that a
  `(parent/leaf)` prefix stacked; `bd help worktrees` explains it.
- Fixups: `bd fold` (--branch, --expect-head, --dry-run) folds the fixup!, squash! and amend! commits into their
  targets and keeps HEAD's tree.
- A message to change (a subject too long, a typo): `bd reword <commit> --subject '<subject>'` (the body stays),
  --message '<message>' or --message-file <file> (`-` for stdin), or several commits in one rewrite with --plan <file>
  (`-` for stdin) of {\"rewrites\": [{\"commit\": \"<id>\", \"subject\": \"...\"}]} (or `message` for the whole
  message); --expect-head, --allow-append, --dry-run, --sync, --map as for `bd group`. The subject is the whole line:
  keep its `(prefix)`, unless the commit should move to another branch (`changes[].moved` says it, check with
  --dry-run first). Every tree, the index and the files stay; the commits after it get new ids. Never an empty
  `amend!` commit, an amend with `git commit`, `commit-tree` or `update-ref` for it. `invalid_plan` (exit 2) when a
  fixup would fold elsewhere: fold it first, or reword it in the same plan.
- A change in the wrong commit: `bd move-hunk <from> <to> <path>` (--block <id>, repeatable; --expect-head, --dry-run;
  --sync rebuilds the virtual branches at once, else the next sync does) moves the changes of the file `path`, or
  only its blocks by the ids that `bd commits <from> --patch` prints, into the commit `to`, older or newer; HEAD's
  tree, the index and the working tree stay, and the commits between keep their diff. `hunk_conflict` (exit 4): a
  commit between changes the same lines, or lacks the file, also when the words would resolve it: bd can't move it; `block_not_found` (exit 2): the file
  changed since the id was read, `error.blocks` are its ids now.
- `bd install` puts `bd` into ~/.local/bin (macOS, Linux; on Windows `bd.exe` is next to the app) and the skill into
  ~/.claude/skills and ~/.agents/skills; `bd help skill` prints the skill.
"
);

/// `bd help update`: the update and its resolve session (spec/cli/update.spec.md); `agent_help_test.rs` checks it with
/// `AGENT_HELP` against the commands and options
pub const UPDATE_HELP: &str = "\
bd update: bring the checked out branch onto its baseline (e.g. origin/master), as the app's Update Project, and
resolve its conflicts outside the working tree. Other sessions may work in the same tree: bd never stashes, resets or
checks out, and only the finish writes the working tree, through the guarded update (only the files the update
changes; local changes on them refuse it, `uncommitted_overlap` with `paths`).

`bd update` fetches the baseline (--no-fetch: the last fetched state; --dry-run: the check only, no fetch, nothing
written but git objects). Exit 0: `state` is up_to_date, fast_forward or rebase, the last two with an `undo_token`.
Exit 1: a commit conflicts, and a resolve session opens, one round per conflicting commit. The session belongs to this worktree
and branch, and keeps the baseline it started with; peers' commits on top of HEAD join it as more rounds. Running
`bd update` again prints the open round (and writes its missing copies); `state: pending` (exit 1) means every
commit is resolved and `session.next` finishes. Local commits whose patch the baseline has already (`git cherry`) are left out
as `git rebase` does: `landed_dropped`. A conflict of git that its words resolve (`branchdeck.resolveByWords`, on by
default) needs no round: the update commits the result and lists the commit in `resolved` (`commit`, `new_id`,
`conflicts`, `files`); tell the user. When every conflict resolves so, the update applies with exit 0.

A round (`state: conflict`, `session`):
- `session.commit`: the local commit being replayed, its `subject`, `body` and `patch` (the commit as a patch: the
  intent to reapply). `hint` \"likely landed\": the baseline probably has it already; check `onto`, and if it holds the
  change, take `onto` for each file (`--take onto:<path>`).
- `session.files[]`: `path`, `kind`, and absolute paths: `edit` (the merge result with zdiff3 markers, the file to
  edit), `before` (the commit's parent), `onto` (the new parent: the baseline and the commits replayed so far),
  `commit` (the commit's version), `markers` (blocks left), `upstream_commits` (who changed it upstream), `hint`.
  `session.marker_labels` names the ids after the markers. `session.blocked_paths`: local changes that will block the
  finish (the first 100, all 101 rather than one left out; `blocked_paths_omitted` counts the rest); ask the user
  about them first. `session.next` is the command that continues. A round prints only its own files;
  `bd update --status` adds the rounds resolved so far. `session.resolved` lists the rounds that resolved
  themselves by words: they count in `round`, so round 3 can be your first.
- Edit only the files at `edit`, never the same path in the working tree. Keep the change of `onto` and reapply the
  intent of the commit; change only the lines between the markers, keep the line ends, and remove every marker. A
  change that the commit needs outside the blocks or in another file goes into a `fixup! <subject>` commit after the
  update, folded with bd fold. Write a copy as UTF-8 without a byte order mark: in Windows PowerShell with
  `[IO.File]::WriteAllText(<edit>, $text)`, because `Set-Content` and `Out-File` add a BOM or write UTF-16.
- `kind` content: edit `edit`. `kind` modify_delete or rename_delete (one side deleted the file; `renamed_from`
  names its path before a rename): `--take onto:<path>`,
  `--take commit:<path>` or `--delete <path>` on the --continue command, or edit `edit`. Every file needs a
  resolution: a missing file is never a delete.

`bd update --continue --round <n>` commits the round's resolution, replays the rest and opens the next round (exit 1)
or applies the update (exit 0). --take and --delete resolve a file without an edit (repeatable).
`bd update --continue --dry-run` checks the resolution; it moves no ref and writes nothing but git objects. Exit 2:
`unresolved_markers`, `resolution_outside_conflict` (a line outside the blocks changed) or `missing_resolution`, with
`paths` and the copies in the message: fix those copies and run it again. A refusal changes nothing: the round stays
open and the copies keep your edits (`edited` in the round). `round_moved` (exit 3):
another round is open; `bd update` prints it. `empty_resolution` (exit 2): the resolution leaves the commit without
changes; ask the user, then add --allow-empty (keep an empty commit) or --drop-empty <commit> (drop it).
At exit 0, `resolutions[]` lists per round the `commit`, its `new_id` (`empty`: kept or dropped) and per file
`how` (edited, onto, commit, deleted; words for a round that resolved itself, with `words` on the round) with
`dropped_upstream_lines` and `dropped_commit_lines`: the lines of a side's
change that the result lost. Report each resolution to the user, these lines first. Then build and test: on a failure
commit the fix as `fixup! <subject>` and run `bd fold`, or take the update back with `bd undo <undo_token>`.

`uncommitted_overlap` at the finish (exit 4) keeps the session: once the user moved those files, run
`bd update --continue` again. `finish_pending` (exit 4): the branch moved, but its files did not follow; the message
says what is left.
`session_stale` (exit 4): the branch was rewritten or HEAD left it; `unsupported_conflict` (exit 4): a binary,
file/directory, rename, mode, symlink or submodule conflict, or an invalid path. Both: stop and show the user. `bd update --abort` removes the session
and its copies (nothing else changed); --worktree <id> removes an orphaned session of another worktree, a live one
only with --force.
`bd update --status` prints this worktree's session and every session of the repository (`orphaned` ones too), and
changes nothing; --materialize writes the missing copies, with --force all of them. --dir <path> puts the copies of a
new session into an empty directory outside every worktree, for a sandbox that keeps .git read only.
";

/// `bd help worktrees`: how agent sessions that share a checkout work beside each other, each in a worktree of its own
/// (spec/cli/pick.spec.md); `agent_help_test.rs` checks it with `AGENT_HELP` against the commands and options
pub const WORKTREES_HELP: &str = "\
bd help worktrees: several sessions on one branch. Each session commits in a worktree of its own (a lease: a linked
worktree, often a detached HEAD, sharing the git dir); its work counts once it is on the shared branch of the shared
checkout. One owner rewrites that branch: `bd update`, `bd fold`, `bd group`, `bd rename`, `bd reword`. Every other
session only adds commits to it, with `bd pick`, never with `git cherry-pick`, a rebase or a reset in the shared
checkout; the one rewrite it may run is a `bd reword` of its own landed commit (step 4).

0. Lease your worktree when `bd workspace` says `lease.on` (`branchdeck.agentLease`, on by default; its `next` is the
   command): `bd lease get --holder <your name>` in the shared checkout (or -C <shared checkout>). It
   prints `path`, a worktree of the pool at the HEAD of the shared checkout, detached, and `lease_id`; work and commit
   only there, never `git worktree add`. A free slot is moved to that HEAD (`reused`: only the diff is written); with
   none free the pool adds one (`created_by`: `clone`, an APFS clone on macOS, or `checkout`). --branch <name> (-b) creates a branch there. `skipped` names free slots with
   local changes (`dirty`) or processes in them (`busy`). `seeded`: ignored local files that `.worktreeinclude` names,
   copied from the shared checkout.

1. When `bd workspace` says `subject_branch.on` (`branchdeck.agentSubjectBranch`, off by default), write the prefix
   when you commit, never later; off, write the subject as the repository does, and the owner groups the commits
   (`bd suggest`, issue keys). A subject is `(branch) <one sentence>`. With stacked branches
   (`branchdeck.stacks`) a new stack is declared once, `(parent/leaf) <one sentence>`; once the stack parents know the
   leaf, `(leaf) <one sentence>` is enough, as a plan of `bd suggest` writes it. `(parent/leaf)` on a commit of a
   branch that is a root stacks the whole branch on `parent` (`reparented` in the pick); two parents for one branch
   are conflicting parents, `bd restack <leaf> <parent>` keeps one. The branch is a readable name of a few words, never an issue
   id as the name; an issue id stays in the subject after the prefix, or in the body. A series of commits is a stack
   root with one leaf per logical group; a change on its own gets a root of its own. One deliberate commit per change:
   no `fixup!` subject on the shared branch, `bd fold` would fold it into its target. Rename a branch with
   `bd rename <old> <new>`, which rewrites every prefix, children included. `bd suggest` is for commits that came
   without a prefix.
2. Land your commits, with -C <shared checkout>: `bd pick --from <your worktree> --follow`. bd replays them onto the
   branch in memory, under the repository lock that every worktree shares, so no fold or update runs halfway through
   it; the files follow, and --follow moves your worktree onto the new tip (its local changes on other paths stay).
   - `picked[]`: the copies, each with the `group` it lands in; `grouped_by: none` is a commit without a prefix: fix
     the subject before the owner groups it.
   - `dependencies[]`: a commit whose prefix names a branch that is not built on the branches it needs (`depends_on`:
     its hunks change what their commits wrote), so that branch can't sync. Fix the prefix in your worktree before
     the pick (--dry-run says it too), or after it run `bd group` with its `plan` (the commit moves into the branch
     that holds them) or its `restack`.
   - `already_present[]`: commits whose change the branch has already (`on`: its commit with the same patch; `by`:
     `patch_id`, left out before the replay, or `empty`, the change in another form); never an empty commit.
     `state: up_to_date`: nothing was left to pick.
   - `state: conflict` (exit 1): nothing changed. Do what `conflict.next` says: move your worktree onto the branch,
     make the commit again there, regenerate a generated file, never keep one side of a code hunk; then pick again.
     Never `git cherry-pick` it into the shared checkout: it leaves `CHERRY_PICK_HEAD`, which blocks every rewrite.
   - `uncommitted_overlap` (exit 4): local files of the named worktree sit on paths the pick writes; ask the user.
     `range_unknown` or `source_on_newer_baseline` (exit 4): name the commits (`bd pick <first>^..<last>`), or let the
     owner update the branch first.
   - `followed.state` `head_moved` or `blocked`: the branch moved, your worktree did not; the same command again moves
     only the worktree. A build daemon or a file watcher of the worktree may need a restart after it moved.
   - `bd undo <undo_token>` moves the branch and your worktree back.
3. Check that the branch still holds your work after the owner rewrote it: `bd pick --from <your worktree> --dry-run`
   (`up_to_date`, every commit `already_present`). Never compare by date (a `git log` limited by time): a rewrite can
   change the dates, and a fold changes the ids; compare subjects without the `(prefix) ` only as a hint, the content
   decides.
4. The owner's rewrites take the same lock and refuse while a cherry-pick, rebase, merge or revert is in progress in
   the shared checkout (`operation_in_progress`). After a rewrite, the owner tells the other sessions its new HEAD
   (`new_head`); they pick again or check with --dry-run. Nobody squashes, amends or redates another session's
   commit. A rewrite gives the commits after it new ids: the next write of a session that read the old HEAD gets
   `head_moved` (exit 3), and `bd group --allow-append` does not help, the old HEAD is gone: read the status again.
   To fix the message of your own commit after it landed, run `bd reword <commit> --expect-head <head> --subject
   '...'` from your worktree with -C <shared checkout>, without --sync (the owner syncs); your worktree needs nothing,
   a later pick or return finds the commit landed by content. A reword of a commit that
   another session made is the user's to approve.
5. Return your worktree once your commits landed: `bd lease return` inside it (or --workspace <dir>; --lease-id <id>
   refuses another lease). It frees the slot, removes the `seeded` files and detaches a branch, which stays. Refusals
   change nothing: `unlanded_commits` (exit 4, `commits`): commits that the branch of the shared checkout lacks by
   content (a dry run of the pick decides, also after the owner rewrote the branch); land them with step 2 first; `dirty_worktree` (exit 4, `paths`): commit and land them, or ask the user; `slot_busy` (exit 4): a process
   works in it (a shell, a build); stop it or wait. The return itself waits up to 60 s for git's background maintenance
   (`git maintenance`, an automatic `git gc`, the repack they start). The Bazel servers of the worktree (`--workspace_directory` in it)
   serve only it: the return stops them with SIGTERM (`stopped_servers`; their output bases stay), --keep-servers
   refuses instead. --confirm-preserved discards the changes and gives up the
   commits, only when the user said so. `bd lease status` lists the slots and their leases from the pool state
   (--processes adds the processes in each). The return applies the clean rules of `.branchdeck.toml` (`cleaned`:
   stale cargo units, test artifacts; `clean_error` when the file does not parse, the return still succeeds):
   `bd lease return --no-clean` applies none, and `bd lease return --keep <n> --age <duration>` changes every cargo
   rule for this call. `bd lease clean` applies the rules now (--free: the free slots; --dry-run only reports;
   --keep, --age as for the return). The rules are the user's choice: pass `bd lease return --no-clean` only when
   the user asks. `bd lease adopt <dir>...` adds clean linked worktrees of the repository as
   free slots, where they are, or with --move in the pool directory (`moved_from`; the same volume only,
   `other_volume`).
6. The reflog of the branch tells who moved it: `branch-deck: pick`, `undo pick`, `update project`, `undo update`,
   `squash fixups` (`bd fold` or the app's Squash Fixups), `reword <n> commits` (`bd group`, `bd rename`,
   `bd restack`, `bd unstack`, `bd reword`), `undo`,
   `squash <n> commits`, `drop <n> commits`, `amend <id>`, `move hunk`, each also with ` rolled back` when its files
   could not follow. A line without the `branch-deck: ` prefix came from outside Branch Deck.
";

/// The skill `bd install` writes and `bd help skill` prints (`SKILL.md`, the Agent Skills format of Claude Code, Codex
/// and others), naming bd by `command`, its full path. It only says when to use bd and where the workflow is, so it
/// can't fall out of date with the binary: `bd help agent` is the workflow. `allowed-tools` (Claude Code) lets the agent
/// run every bd command without a permission prompt while the skill is active, also the ones that rewrite the branch or
/// push: a push still waits for the user, by the rule of the body
pub fn skill_text(command: &str) -> String {
  let allowed_tools = yaml_quoted(&format!("Bash({command}:*)"));
  format!(
    "\
---
name: branch-deck
description: Use before the first edit or commit in a Git repository: `bd workspace` of Branch Deck says whether to \
work in a leased worktree of your own and how to write the commit subject. Also use to group, split or stack the \
commits of a branch into virtual branches and pull requests, to sync or push them, to fold fixup commits, to change \
the message of a commit or move a hunk into another commit, to update or rebase a branch onto master or main and \
resolve its conflicts, to land the commits of a worktree onto a shared branch, to lease or return a worktree, to start \
a Patronus Safe Push or Dry Run or find out why one failed, to ask code owners to review a pull request, or when the \
user mentions Branch Deck.
allowed-tools: {allowed_tools}
---

# Branch Deck (`bd`)

bd is `{command}`; run it as written, it is quoted for the shell. Below, `bd` stands for it.

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the `(name)` prefix
of their subjects. bd reads those groups and the commits, regroups the commits by rewriting the prefixes, rebuilds the
virtual branches, pushes them and starts their Patronus Safe Push. Only `bd update` (the branch onto its baseline),
`bd pick` (the commits of another worktree onto the branch) and their undo write the working tree, through the
guarded update.

Before the first edit or commit in a repository, run `bd workspace` and follow it; what the user says in the session
wins over it:
- `lease.on`: work in a worktree of your own. Run the first command of `next` (`bd lease get --holder <your name>`, a
  name of this session) and edit, build and commit only in its `path` (and `subdir` there); in Claude Code,
  `EnterWorktree` with that `path` moves the session there. `bd workspace` inside it names the rest: land each commit
  that builds and passes its tests at once, with the `bd pick --from <lease> --follow -C <shared_checkout>` of `next`;
  once the task is done and every commit landed, `bd lease return --lease-id <lease_id>`. Return only your own lease,
  never one of `active_leases` that another session holds. A lease lacks the `local_changes` of the shared checkout:
  when the task needs them, ask the user.
- `lease.on` false (`reason` says why, when it is set): work where you are; stage only the paths you changed, never
  `git add -A` or `git add .`.
- `subject_branch.on`: start each commit subject with `(branch) `, a readable name of a few words, never an issue id as
  the name; reuse a branch of `bd status --brief` that fits; with `stacks`, a new stack is `(parent/leaf)`. Off: write
  the subject as the repository does, without a prefix.
- Both are the user's settings (Settings → Agents of the app, `branchdeck.agentLease`, `branchdeck.agentSubjectBranch`):
  never change them.

To group or push commits or to start a Safe Push, run `bd help agent` first and follow the workflow it prints: it is the
contract of the installed version. To update a branch or resolve its update conflicts, run `bd help update` instead.
When several sessions or worktrees work on one branch, run `bd help worktrees` first.

- Run bd in the repository, or pass `-C <repo>`.
- Land the commits of another worktree with `bd pick`, never with `git cherry-pick`, which leaves `CHERRY_PICK_HEAD`.
- Every command prints one JSON object on stdout. Branch on `error.code` and the exit code, never on the message;
  `bd schema` prints the JSON Schema of every output, `bd version` the version of bd and git.
- Never reorder, squash or drop commits, except folding fixup!, squash! and amend! commits with `bd fold`; move a
  change between commits only with `bd move-hunk`, change a message only with `bd reword`; never push, start a Safe Push or create pull requests before the
  user approved them.
- Before a Safe Push, run `bd safe-push <branch> --check` first and show the user what it lands: the Safe Push of a
  stacked branch lands the branches it is built on with it, in one robot, and `--whole-stack` lands the whole stack,
  its side branches included, in one robot.
- A branch a merged Safe Push landed says so in `bd status` (`landed_by`) until `bd update`: only its `after` commits
  are left to push or land, and with none `bd push` leaves it out (`skipped`) and `bd safe-push` refuses it (`landed`).
- A branch `parked` in `bd status` stays local until the user unparks it: `bd push` and `bd safe-push` refuse it
  (`parked`). Never push or unpark it on your own.
- `bd check --fix` prints a line per trial on stderr and stops at `--max-trials` (12) or `--time-budget` (180 s): a
  `search: budget` answer is the best plan so far. A commit prefixed `(parent/leaf)` stacks the existing group `leaf`
  on `parent` (`reparented` in `bd pick`); two parents of one group name the `bd restack` that keeps one.
- A `git_error` \"not updating refs/branch-deck/tmp/chain (… does not contain …)\" is a ref a Branch Deck 0.21 or older left:
  `git update-ref -d refs/branch-deck/tmp/chain`, then retry.
"
  )
}

/// A YAML double-quoted scalar: the shell quoting of a path can hold ` #` or `: `, which end a plain scalar
fn yaml_quoted(value: &str) -> String {
  format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
