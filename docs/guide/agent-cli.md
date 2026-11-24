# Coding Agents and the Agent CLI (`brd`)

**Updated:** 2026-11-21  
**Tags:** [brd, agents, cli, worktrees, setup]

`brd` is the command line of Branch Deck for coding agents such as Claude Code and Codex. It gives an agent what the
app sees, as JSON, and the same operations the app runs: group commits into branches, check that every branch still
builds, push, update onto the baseline and land the commits of one worktree on another. An agent that knows `brd`
works beside you on your branch without touching your checkout, and you review and push its work in the app as your
own.

This guide installs `brd`, says how agents work with it, and walks through the commands an agent runs. The exact
contract of every command is in the [Agent CLI spec](../../openspec/specs/cli/agent-cli/spec.md); how it is built, in
[Agent CLI](../contributing/agent-cli.md) for contributors.

## Install

In the app: Help → Install Agent CLI…. From a terminal, with the app installed: `brd install`, or the installer with
`--with-cli` (`-WithCli` on Windows):

```sh
curl -fsSL https://raw.githubusercontent.com/develar/branch-deck/master/install.sh | sh -s -- --with-cli
```

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/develar/branch-deck/master/install.ps1))) -WithCli
```

It writes two things:

- **the command**: on macOS and Linux `~/.local/bin/brd`, a link to the app binary (`branch-deck cli`); on Windows
  `brd.exe` next to the app, which the installer puts there. `brd` is always the version of the app, so an update of
  the app updates it.
- **the skill** `branch-deck` into `~/.claude/skills/` (Claude Code) and `~/.agents/skills/` (Codex and others), so an
  agent knows when to use `brd`: before its first edit or commit, to group or stack commits, to update or rebase, to
  land the commits of a worktree. `brd help skill` prints it for an agent that reads skills from elsewhere.

`brd install --uninstall` removes both. Settings → Tools shows the skill as installed.

## Leaving agents alone: Unattended Mode

An agent team left running for the night stops at the first permission prompt of any of its agents: the prompt waits
for you until morning. Before you leave, turn on Unattended Mode: Turn On Unattended Mode in the command palette,
Unattended Mode in the menu bar (after Settings…), Settings → General, or `brd unattended on`. It adds its hook to the settings of Claude Code, `~/.claude/settings.json`
(every other setting and hook there stays as you wrote it), and the sessions that run pick it up within seconds. While
it is on, the title bar of the main window reads "Unattended", and every permission request of Claude Code, of a
session or any of its subagents, is denied with a message that nobody can approve it: the agent does the step without
new access, or skips it, names it in its report and goes on. When the settings file is no JSON Branch Deck can add the
hook to, or Claude Code is older than 2.1.295, which would run the hook without its arguments, the mode stays off and
says why. A quit or a crash of the app leaves the mode on: the night goes on; `brd install --uninstall` turns it off.

When you are back, click that mark to turn it off, which takes the hook out of the settings of Claude Code again. Its
toast says how many requests were denied, and Show Denied Requests… lists each:
the tool, what it asked for (a command, a path, a URL), when, and which agent asked where. Allow in the settings of
Claude Code what the agents need next time. `brd unattended status` prints the same list; it stays until the mode is
turned on again ([Unattended Mode](../../openspec/specs/repository/unattended-mode/spec.md)).

## How agents work beside you

You keep working in your checkout. Each agent session, before its first edit, runs `brd workspace`, which tells it the
rules of this repository:

1. **A worktree of its own.** The agent leases a worktree from a pool (`brd lease get`) and edits, builds and commits
   only there. Your files, your index and your checked-out branch stay as they are, and two agents never share a
   working tree ([brd lease](../../openspec/specs/cli/lease/spec.md)).
2. **Each commit lands on your branch at once.** Once a commit builds and passes its tests, the agent runs
   `brd lease pick`: the commit goes onto your branch as it is now, with no cherry-pick in your checkout and no
   rebase of your work. A conflict lands nothing and goes back to the agent
   ([brd pick](../../openspec/specs/cli/pick/spec.md)).
3. **The branch in the subject.** The agent starts each subject with `(branch) `, so the next sync puts the commit into
   its virtual branch and the copy drops the prefix.
4. **Push and pull request when you say so.** By default the agent stops after the pick: you see its branch in the app
   and push it from its row. Turned on, the agent pushes the branch and opens its pull request itself.

Each rule is yours to switch in Settings → Agents, for every repository or for one, or with `git config`
(`branchdeck.agentLease`, `branchdeck.agentSubjectBranch`, `branchdeck.agentPullRequest`); `brd workspace` says where
each value comes from ([brd workspace](../../openspec/specs/cli/workspace/spec.md)). The app shows what `brd` does on
the rows it changes ("brd: Unstacking beta…"), and Activity lists it with Undo.

How an agent writes the body of a commit, so that your review reads its story first:
[The Commit Body a Review Reads](commit-bodies.md).

## What an agent runs

```sh
brd status                                               # the branches, their commits and what each has to push
brd commits --files                                      # the commits ahead of the baseline and their files
brd suggest | jq .plan > plan.json                       # a grouping, already checked by a trial sync
brd check --plan plan.json                               # after an edit: does every branch still build?
brd group --expect-head <head> --sync --plan plan.json   # rewrite the prefixes and build the branches
brd push auth ABC-123                                    # push, parents first
brd update                                               # onto the baseline; a conflict opens a resolve session
brd pick --from ../lane --follow                         # land another worktree's commits here, no cherry-pick
brd pick --remote auth                                   # take in the commits someone else pushed to auth
brd lease get --holder lane-1                            # a worktree of its own for an agent session, from a pool
```

- **Reading.** `status` and `commits` answer what the app shows, as JSON with a schema (`brd schema <command>`).
  `commits` also says what ties the commits together: the lines they share, and the names they declare and use in
  Kotlin, Java, C#, Rust and TypeScript, so an agent knows which commits must stay in one branch
  ([symbol dependencies](../../openspec/specs/commit/symbol-dependencies/spec.md)).
- **Grouping.** `suggest` proposes branches and checks them with a trial sync, `check` tells whether a plan still
  builds every branch, and `group` applies it as one rewrite of the commit messages; `brd undo` takes it back.
  `rename`, `restack`, `unstack` and `stack-together` change the branches and their stacks without rewriting the code.
- **History.** `fold` folds fixup commits, `squash` squashes commits, `reword` changes a message, `move-hunk` moves a
  change of a file from one commit into another, `drop` drops a commit that must not land. Each prints its Undo.
- **Staying up to date.** `update` rebases your branch onto the baseline as Update Project does; on a conflict it
  opens a resolve session outside the working tree, which the agent resolves commit by commit
  ([brd update](../../openspec/specs/cli/update/spec.md)).
- **Landing.** `push` pushes branches, parents first; `request-review` asks the code owners of a pull request to
  review it.

The workflow an agent follows, with what needs your approval and what each exit code asks for, is `brd help agent`;
`brd help update` and `brd help worktrees` hold the resolve session and the work of several sessions on one branch,
`brd <command> --help` the fields and refusals of each command. They come with the binary, so they never fall out of
date with it.

## Running it yourself

`brd` works for a person too: `brd -C ~/projects/notes status | jq` in any terminal. Without the link on `PATH`, run
the app binary with `cli` (`branch-deck cli status`). A rewrite prints the `undo_token` that `brd undo` takes, and
`--dry-run` shows what a rewrite, a push or an update would do without doing it.
