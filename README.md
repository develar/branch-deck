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
- **Squash**: squash selected commits into the oldest one, even when other branches' commits lie between them. Fold
  `fixup!`, `squash!` and `amend!` commits into their targets in one click, as `git rebase --autosquash` does. Both
  can be undone.
- **GitHub pull requests**: the PR number, review state and checks of each branch, read with the
  [`gh`](https://cli.github.com) CLI.
- **Commit editing**: preview a commit or a whole branch with Space, reword a message, amend uncommitted changes into a
  branch, split or unapply a branch.
- **Issue links**: issue keys in commit subjects link to your tracker, configured with IntelliJ IDEA issue navigation
  (`.idea/vcs.xml`).
- **Agent CLI** (`branch-deck cli`, or the binary symlinked as `bd`): a coding agent reads the branches, the unassigned
  commits and every commit ahead of the baseline as JSON, with the files, the dependencies between commits and per
  branch statistics, to cluster commits into branches. `bd help agent` prints the workflow.
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
2. Otherwise an issue key (`JIRA-123`, `ABC-456`) groups the commit; a leading `[subsystem]` tag is ignored, so
   `[threading] IJPL-163558: Fix observability` goes to `IJPL-163558`.
3. Commits with neither stay on the main branch as unassigned.

See [Virtual Branches](docs/design/virtual-branches.md) for the branch layout and storage.

### Current Limitations

- Nested branches (e.g. `(feature-auth/oauth)`) are not supported yet.

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
