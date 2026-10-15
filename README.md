# Branch Deck

Work on one branch, get a real Git branch per feature. Branch Deck groups your commits by a `(name)` prefix or an issue
key in the subject and cherry-picks each group into its own branch, ready to push and review.

Branch Deck started as a workaround for [performance](https://github.com/gitbutlerapp/gitbutler/issues/3235)
[issues](https://github.com/gitbutlerapp/gitbutler/issues/9018) with [GitButler](https://gitbutler.com) on large
repositories: all operations run through the Git CLI.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/branch-deck-dark.png">
  <img alt="Branch Deck: unassigned commits and virtual branches grouped by prefix and issue key" src="docs/screenshots/branch-deck-light.png">
</picture>

## Features

- **Branches from commit messages**: `(feature-auth) Add login` and `IJPL-163558: Fix observability` become the branches
  `username/virtual/feature-auth` and `username/virtual/IJPL-163558`. `fixup!`, `squash!` and `amend!` commits follow
  their target.
- **Stacked branches** (opt-in, `git config branchdeck.stacks true`): `(auth/login) Add form` builds `login` on top of
  `auth`, and the branch table shows the stack as a tree. Stack a branch on another one, unstack it or rename a parent
  from the branch menu; Move to… takes `auth/login`. A branch whose parent has a conflict or is missing is blocked
  instead of moved.
- **Issue groups**: one branch for the commits of several issues, without a rewrite. `ABC-1 Parse lists` and
  `ABC-2 Parse maps` go into `parser`, and both subjects stay as they are. The map of issue key to branch
  name lives in the ref `refs/branch-deck/issue-groups`. `bd suggest` proposes the groups, `bd group` writes them, and
  the app shows and renames them.
- **Unassigned commits**: commits without a prefix are listed separately. Create a branch from a selection, with branch
  name suggestions from an on-device AI model, or move commits to an existing branch.
- **Conflict viewer**: when grouping changes the order of dependent commits, Branch Deck shows the conflicting files in a
  3-way merge view and lists the missing commits.
- **Integration detection**: merged, rebased and squashed branches are detected and archived automatically.
- **Push and remote status** per branch, with push or create remote in one click. The header sums up what is waiting
  to be pushed, per branch, and marks the branches that need a force push.
- **Update Project** (`⌘T`): fetches the baseline and rebases your local commits onto it. The header tells you
  beforehand whether the rebase is clean. On a conflict nothing is changed, and you can copy a ready prompt for a
  coding agent. Uncommitted changes survive as long as upstream didn't touch the same files.
- **Landed commits**: when a robot or a pull request brought your commits into the baseline, the header and the branch
  rows say so before the update, the update toast leads with it, and an island lists what landed (with the baseline
  commits, the robot or pull request) until you have seen it, also after an update made in the IDE.
- **Activity**: what Branch Deck did in this repository, with Undo while it lasts: pushes (and the remote commits a
  force push dropped), updates, what landed, Patronus robots, squashes and moves, errors.
- **Squash**: squash selected commits into the oldest one, even when other branches' commits lie between them. Fold
  `fixup!`, `squash!` and `amend!` commits into their targets in one click, as `git rebase --autosquash` does. Both
  can be undone.
- **GitHub pull requests**: the PR number, review state and checks of each branch, read with the
  [`gh`](https://cli.github.com) CLI. After a push, "Create Pull Request" opens GitHub's form with the base already set
  (the parent branch of a stacked one). Branches pushed to Space work too when its committed settings
  (`.space/repository-settings.json`) mirror the repository to GitHub; otherwise `git config branchdeck.githubRepository
  <owner>/<name>` names it.
- **Patronus** (JetBrains Space repositories): the Safe Push, Safe Merge or Dry Run of each branch, running, failed or
  merged, and whether the branch changed since it started (a sync that only rebuilt the branch on a newer baseline did
  not). Start a Safe Push (a button in the row) or a Dry Run with a merge profile, restart (pushing first) or cancel it
  from the branch; a push that would make a running Safe Push fail asks first. Before a Safe Push, the code-owner
  approvals it needs, with the merge request to create (also from the branch menu) or to open. A branch reviewed in a
  GitHub pull request starts from it: Branch Deck comments `/patronus merge`, so the approvals of the pull request
  count and Patronus closes it once merged. When a Safe Push merged,
  a toast (or an OS notification in the background) says so and fetches the baseline on request. Uses the token of the
  Safe Push CLI.
- **Commit editing**: preview a commit or a whole branch with Space, reword a message, amend uncommitted changes into a
  branch, split or unapply a branch.
- **Commit files**: every commit row says how many files and lines it changes ("3 files · +42 −7"). The commit
  preview lists the files after the message, each expanding to its diff (`F` hides them), and the context menu copies
  the changed paths.
- **Issue links**: issue keys in commit subjects link to your tracker, configured with IntelliJ IDEA issue navigation
  (`.idea/vcs.xml`).
- **Agent CLI** (`branch-deck cli`, or the binary symlinked as `bd`; `bd.exe` next to the app on Windows): a coding
  agent reads the branches, the unassigned commits and every commit ahead of the baseline as JSON, with the files, the
  dependencies between commits and per branch statistics, to cluster commits into branches; `--brief` views with the
  areas of each commit and the units that must stay together fit in one read. `bd suggest` proposes a grouping,
  already checked by a trial sync, with whole issue branches as issue groups that rewrite no subject. One
  `bd group --sync` call applies the plan (one validated rewrite of the commit messages that can be undone), writes
  the pull request texts of its `descriptions` and rebuilds the branches; `bd check` tries an edited plan first,
  without writing refs. `bd push` pushes the branches with a lease, parents first; `bd update` updates the branch
  onto its baseline as Update Project does, with an undo, and `bd fold` folds the fixup commits; `bd help agent` prints
  the workflow.
  Help → Install Agent CLI… (or `bd install`) puts `bd` on `PATH` and a skill into `~/.claude/skills` and
  `~/.agents/skills`, so coding agents know when to use it.
- Auto-sync when the window gets focus, auto-update, light and dark theme.

## How It Works

Two kinds of prefix are involved:

- **Commit message prefix**: text like `(feature-auth)` at the start of a commit subject on your main branch (HEAD). It
  only groups commits: the cherry-picked commits on the virtual branch are stored without it (`Add login`, not
  `(feature-auth) Add login`).
- **Branch prefix**: the namespace set with `git config branchdeck.branchPrefix username`. It becomes the first path
  segment of every branch, e.g. `refs/heads/username/virtual/feature-auth`.

Grouping rules, checked on the first line of the commit message:

1. An explicit `(name)` prefix wins.
2. Otherwise an issue key (`JIRA-123`, `ABC-456`) groups the commit: into its issue group, if it has one, else into a
   branch of the key; a leading `[subsystem]` tag is ignored, so `[threading] IJPL-163558: Fix observability` goes to
   `IJPL-163558`.
3. Commits with neither stay on the main branch as unassigned.

See [Virtual Branches](docs/design/virtual-branches.md) for the branch layout and storage.

## Installation

[Download the latest release](https://github.com/develar/branch-deck/releases/latest) for macOS (Apple silicon), Windows or Linux.

Requires Git 2.54 or newer (`brew install git` on macOS). Pull request status needs `gh`, logged in with
`gh auth login`.

History rewrites (reword, move, squash, amend, unapply) never touch your working tree or your staged changes. Update
Project is the one operation that writes files, and only files that upstream changed and you didn't. See
[Git Operations](docs/design/git-operations.md).

## Documentation

- [Virtual Branches](docs/design/virtual-branches.md) — how commits are grouped and cherry-picked
- [Integration Detection](docs/design/integration-detection.md) — how merged branches are detected
- [Git Operations](docs/design/git-operations.md) — working-tree safety, history rewrites, Update Project and the Git
  version requirement
- [Remote Status](docs/design/remote-status.md) — how ahead/behind and push states are computed and coloured
- [All design docs](docs/design/)
- [Feature specs](spec/) — the exact behaviour of each feature (commit row, squash, fixups, branch preview, update,
  pull requests)

## Contributing

See [DEVELOPMENT.md](DEVELOPMENT.md) for development setup and contribution guidelines.
