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
  from the branch menu; Move to… takes `auth/login`. The push button of a branch pushes its parents first while they
  have commits for the remote (Push Stack), Push with Children a parent with the branches stacked on it, and a push
  retargets the branch's pull request onto its base. Safe Push leads root first; a Safe Push of a child lands its
  parents with it and closes their pull requests. A branch whose parent has a conflict is blocked; one whose parent has
  no commits is built on the nearest ancestor with commits or the baseline.
- **Issue groups**: one branch for the commits of several issues, without a rewrite. `ABC-1 Parse lists` and
  `ABC-2 Parse maps` go into `parser`, and both subjects stay as they are. The map of issue key to branch
  name lives in the ref `refs/branch-deck/issue-groups`. `bd suggest` proposes the groups, `bd group` writes them, and
  the app writes them when you move whole issues.
- **Move to a branch**: drag commits or whole branches onto a branch, or select them (Cmd/Shift-click a branch row) and
  choose Move to…, to ship them together. A whole issue moves by the issue groups without a rewrite, so its next
  commits follow; only a part of an issue gets the `(branch)` prefix. The form says what the move does before it runs,
  and one Undo takes it back.
- **Unassigned commits**: commits without a prefix are listed separately. Create a branch from a selection or move commits
  to an existing branch.
- **Branch name suggestions**: two names for the selected commits from your Claude Code (Haiku), after you agree to send
  their messages and changed file names; no diffs, nothing on your machine to download.
- **Conflict viewer**: when grouping changes the order of dependent commits, Branch Deck shows the conflicting files in a
  3-way merge view and lists the missing commits.
- **Integration detection**: merged, rebased and squashed branches are detected and archived automatically.
- **Push and remote status** per branch, with push or create remote in one click. The header sums up what is waiting
  to be pushed, per branch, and marks the branches that need a force push.
- **Update Project** (`⌘T`): fetches the baseline and rebases your local commits onto it. The header tells you
  beforehand whether the rebase is clean. On a conflict nothing is changed, and you can copy a ready prompt for a
  coding agent, which resolves the conflicts commit by commit with `bd update`, outside the working tree, so other
  sessions in the same tree keep their files. Uncommitted changes survive as long as upstream didn't touch the same
  files.
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
- **At home next to IntelliJ**: the colours and the density of the IntelliJ Islands themes, light and dark, and the
  toolbar in the title bar, beside the macOS traffic lights or with its own window buttons on Windows.
- **Zoom**: View → Zoom In (⌘+, also ⌘=), Zoom Out (⌘−) and Actual Size (⌘0), from 67% to 150%, in every window and kept
  after a restart; the toolbar keeps the size of a title bar.
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
  the pull request texts of its `descriptions` (the app shows their titles after the branch names) and rebuilds the
  branches; `bd check` tries an edited plan first, without writing refs. `bd push` pushes the branches with a lease,
  parents first, a stacked branch with the branches it is built on (`--with-children` adds the branches stacked on them, `--alone` pushes only the named ones); `bd update` updates the branch onto its baseline as Update Project does, with an undo,
  and resolves a conflict in a resolve session (`bd help update`), and `bd fold` folds the fixup commits;
  `bd help agent` prints the workflow.
  Help → Install Agent CLI… (or `bd install`) puts `bd` on `PATH` and a skill into `~/.claude/skills` and
  `~/.agents/skills`, so coding agents know when to use it.
- Auto-sync when the window gets focus (off by default, in the sync settings next to Sync), auto-update, light and
  dark theme.

## How It Works

Two kinds of prefix are involved:

- **Commit message prefix**: text like `(feature-auth)` at the start of a commit subject on your main branch (HEAD). It
  only groups commits: the cherry-picked commits on the virtual branch are stored without it (`Add login`, not
  `(feature-auth) Add login`).
- **Branch prefix**: your namespace, typed in the field next to the repository selector or set with
  `git config --global branchdeck.branchPrefix username` (the field wins). It becomes the first path segment of every
  branch, e.g. `refs/heads/username/virtual/feature-auth`.

Grouping rules, checked on the first line of the commit message:

1. An explicit `(name)` prefix wins.
2. Otherwise an issue key (`JIRA-123`, `ABC-456`) groups the commit: into its issue group, if it has one, else into a
   branch of the key; a leading `[subsystem]` tag is ignored, so `[threading] IJPL-163558: Fix observability` goes to
   `IJPL-163558`.
3. Commits with neither stay on the main branch as unassigned.

See [Virtual Branches](docs/design/virtual-branches.md) for the branch layout and storage.

## Installation

One command, which is also the upgrade:

```sh
# macOS (Apple silicon), Linux (x86_64)
curl -fsSL https://raw.githubusercontent.com/develar/branch-deck/master/install.sh | sh
```

```powershell
# Windows
irm https://raw.githubusercontent.com/develar/branch-deck/master/install.ps1 | iex
```

The builds are not signed yet, and these scripts are the way around it: a browser marks a download as quarantined
(macOS) or as from the internet (Windows), and then Gatekeeper or SmartScreen blocks the unsigned app; a download by
`curl` or PowerShell has no such mark, the same as the app's own updates. The scripts take the release from its
`latest.json`, the file the updater reads, and install:

- **macOS**: `BranchDeck.app` into `/Applications` (`~/Applications` when that is not writable), quitting and
  restarting a running Branch Deck.
- **Linux**: the `.deb` with `apt-get`, the `.rpm` with `dnf` or `zypper` (both through `sudo`), else the AppImage into
  `~/.local/bin` with a menu entry; `BRANCH_DECK_FORMAT=appimage` picks the AppImage anyway.
- **Windows**: the per-user installer, silently, no administrator rights.

Add `--with-cli` (`-WithCli` on Windows) to also install the [Agent CLI](#features) for coding agents, `--version 0.11.0`
(`-Version`) for an older release:

```sh
curl -fsSL https://raw.githubusercontent.com/develar/branch-deck/master/install.sh | sh -s -- --with-cli
```

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/develar/branch-deck/master/install.ps1))) -WithCli
```

Read [install.sh](install.sh) and [install.ps1](install.ps1) before you pipe them into a shell.

Or [download the release](https://github.com/develar/branch-deck/releases/latest) yourself (`.dmg`, `.exe`, `.msi`,
AppImage, `.deb`, `.rpm`):

- **macOS**: move `BranchDeck.app` to Applications, then run `xattr -dr com.apple.quarantine /Applications/BranchDeck.app`
  once, or open it and allow it in System Settings → Privacy & Security → Open Anyway.
- **Windows**: SmartScreen warns about an unknown publisher; choose More info → Run anyway.

Requirements:

- **Git 2.54 or newer**: `brew install git` on macOS; on Ubuntu the distribution's Git is older, use the PPA
  (`sudo add-apt-repository ppa:git-core/ppa && sudo apt install git`); [Git for Windows](https://git-scm.com/downloads/win).
  The app says so, with the command, when Git is missing or too old.
- **A Git identity** (`git config --global user.name` and `user.email`): Branch Deck records its notes and rewritten
  commits with it.
- Optional: `gh`, logged in with `gh auth login`, for the pull request status, and
  [Claude Code](https://code.claude.com/docs/en/setup), logged in, for branch name suggestions.

## Getting Started

1. Open Branch Deck and select a repository (the selector in the header, or the folder button).
2. Choose your branch prefix: the welcome card suggests the name part of your Git email; Save it, or type another
   one.
3. Commit on your branch as usual, starting a subject with a prefix: `(auth) Add login form` or `ABC-123 Fix login bug`.
   Commits without one are listed as unassigned; select them and Move them to a branch.
4. Sync (⌘R, Ctrl+R): Branch Deck creates a local branch `<prefix>/virtual/<name>` for each group. Your working tree
   and the checked-out branch stay as they are.
5. Push a branch from its row, and open its pull request from the toast.

Help → Install Agent CLI… (or the installer's `--with-cli`) puts `bd` on `PATH` for coding agents (see Features).

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
