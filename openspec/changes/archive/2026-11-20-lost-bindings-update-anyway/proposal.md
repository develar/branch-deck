# Proposal

## Why

The update refuses a clean replay that loses bindings (`lost-bindings-update`), but two gaps were left. The header
said "Rebases cleanly" until a click found the lost bindings. And a false finding of the rule left an app user stuck:
the dialog has no terminal path and no override.

## What Changes

- The check inside every sync and `brd update --dry-run` read the lost bindings of their replay too. The rule is asked
  only when a file the update changes is one a local commit changes; the `log` of the local commits that the pairing
  reads already names their paths, so most syncs pay no process. A preview keeps what the rule found in its cache.
- "Update Anyway" in the dialog of a conflict of lost bindings: the update without a fetch, its lost bindings kept
  unbound on the user's word (`Intent::UpdateProject::keep`); a lost binding of another name refuses it again. The
  toast and Activity name the kept names.
- `brd update --dry-run` prints the lost bindings of its conflict; `replayed` of a finish counts no `fixup!` commit.

Capabilities: `cli/update`, `repository/update-project`.
