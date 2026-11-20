# Proposal

## Why

On 2026-11-20 the Safe Push of `claude-fork-open` (IJAI-6702) failed to compile (robot 1137287182):
`Unresolved reference 'backend'` and `'rethrowControlFlowException'`. The commit had removed the only use of a private
constructor property and of an import, and the property and the import with it; an update replayed it onto a baseline
commit that used both again, in other hunks, which git merged without a conflict. No check of Branch Deck sees a
removed import, and the dangling references compare the tree that lands with HEAD, which was broken the same way.

One file can't tell such a lost binding from a name that resolves another way: of 11741 commits of the monorepo's
master, each built where it was authored, 68 had one (an extension called in a lambda with a receiver, a default
import, a helper of the package in another file). What a move of a commit onto another parent brings is precise:
the copy loses a binding its original keeps.

## What Changes

- `trial_check::lost_bindings`: a commit that removes the import or the `private` declaration of a name that its file
  still uses bare after it (Kotlin and Java, by the summaries of `code_symbols`), and of the copies of commits the
  findings that their originals lack.
- The landing gate refuses the start of a branch whose built copy of a commit has a lost binding its original on HEAD
  lacks (`LandingRefusal::LostBinding`, `lost_binding`, exit 4 in `brd`): the branch is built without a commit that
  changed the use. `--allow-dangling` and Start Anyway lift it. A dangling reference of the same name and file says it
  already, with its fix.
- The row says "Needs fix of <hash>"; the reason names the commit, the name and the use.

The update, where the robot of 2026-11-20 got its lost binding, checks its replay by the same rule in a change of its
own.

Capabilities: `branch/patronus`, `cli/agent-cli`.
